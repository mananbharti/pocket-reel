#!/usr/bin/env python3
"""End-to-end test of the Android build on an emulator.

1. Install the APK and launch it.
2. Check the engine started (bundled FFmpeg executes on the device).
3. Pick a real MP4 with Android's document picker.
4. Convert it with the default "Classic player AVI" preset inside the app.
5. Pull the result and verify it with ffprobe on the host.

Screenshots of every step are written to the output folder.
Usage: android-smoke.py <apk> <screenshot-dir>
"""
import os
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

PKG = "app.pocketreel.converter"
apk, outdir = sys.argv[1], sys.argv[2]
os.makedirs(outdir, exist_ok=True)


def adb(*args, check=True):
    r = subprocess.run(["adb", *args], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise SystemExit(f"adb {' '.join(args)} failed: {r.stderr}")
    return r.stdout


def shot(name):
    png = subprocess.run(["adb", "exec-out", "screencap", "-p"], capture_output=True).stdout
    with open(os.path.join(outdir, f"{name}.png"), "wb") as f:
        f.write(png)
    print(f"  screenshot {name}")


def nodes():
    adb("shell", "uiautomator", "dump", "/sdcard/ui.xml", check=False)
    xml = adb("shell", "cat", "/sdcard/ui.xml", check=False)
    try:
        root = ET.fromstring(xml[xml.find("<"):])
    except ET.ParseError:
        return []
    out = []
    for n in root.iter("node"):
        m = re.match(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", n.get("bounds", ""))
        if not m:
            continue
        x1, y1, x2, y2 = map(int, m.groups())
        out.append({
            "text": (n.get("text") or "") + " " + (n.get("content-desc") or ""),
            "center": ((x1 + x2) // 2, (y1 + y2) // 2),
            "clickable": n.get("clickable") == "true",
        })
    return out


def find(pattern, timeout=40, fail=None):
    rx = re.compile(pattern, re.I)
    bad = re.compile(fail, re.I) if fail else None
    end = time.time() + timeout
    while time.time() < end:
        ns = nodes()
        for n in ns:
            if bad and bad.search(n["text"]):
                shot("failure")
                raise SystemExit(f"Found failure text: {n['text'].strip()}")
        for n in ns:
            if rx.search(n["text"]):
                return n
        time.sleep(1.5)
    shot("timeout")
    for name, extra in (("logcat-timeout.txt", []), ("logcat-crash.txt", ["-b", "crash"])):
        with open(os.path.join(outdir, name), "w") as f:
            f.write(adb("logcat", "-d", *extra, check=False))
    texts = [n["text"].strip() for n in nodes() if n["text"].strip()]
    raise SystemExit(f"Timed out waiting for /{pattern}/. Visible: {texts[:40]}")


def tap(node):
    x, y = node["center"]
    adb("shell", "input", "tap", str(x), str(y))


print("Preparing a sample video")
subprocess.run(
    ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25:duration=8",
     "-f", "lavfi", "-i", "sine=f=440:d=8", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac",
     "sample.mp4"], check=True)
adb("push", "sample.mp4", "/sdcard/Download/sample.mp4")
adb("shell", "am", "broadcast", "-a", "android.intent.action.MEDIA_SCANNER_SCAN_FILE",
    "-d", "file:///sdcard/Download/sample.mp4", check=False)
adb("shell", "content", "call", "--uri", "content://media", "--method", "scan_volume",
    "--arg", "external_primary", check=False)

print("Installing and launching")
adb("install", "-r", "-g", apk)
adb("shell", "monkey", "-p", PKG, "-c", "android.intent.category.LAUNCHER", "1")
btn = find(r"^\s*Choose videos\s*$", timeout=90, fail=r"can.t start")
shot("01-launched")

print("Picking the sample through the document picker")
tap(btn)
time.sleep(3)
shot("02-picker")
try:
    item = find(r"sample", timeout=12)
except SystemExit:
    roots = find(r"Show roots", timeout=10)
    tap(roots)
    time.sleep(1.5)
    tap(find(r"^\s*Downloads", timeout=10))
    time.sleep(2)
    item = find(r"sample", timeout=15)
tap(item)
time.sleep(2.5)
shot("03-after-pick")
with open(os.path.join(outdir, "logcat.txt"), "w") as f:
    f.write(adb("logcat", "-d", "-t", "800", check=False))
print("Visible after pick:", [n["text"].strip() for n in nodes() if n["text"].strip()][:30])

print("Waiting for the file to be read and estimated")
convert = find(r"^\s*Convert 1 video\s*$", timeout=60)
find(r"about \d", timeout=60)
shot("03-ready")

print("Converting")
tap(convert)
deadline = time.time() + 180
remote = None
bases = ("/sdcard/Movies/Pocket Reel/", "/sdcard/Android/data/app.pocketreel.converter/files/Movies/")
while time.time() < deadline and not remote:
    for base in bases:
        for line in adb("shell", "ls", "-1", f"'{base}'", check=False).splitlines():
            if line.strip().endswith(".avi"):
                remote = base + line.strip()
                break
        if remote:
            break
    time.sleep(2)
time.sleep(4)
shot("04-converted")
with open(os.path.join(outdir, "logcat-end.txt"), "w") as f:
    f.write(adb("logcat", "-d", "-t", "800", check=False))
if not remote:
    raise SystemExit("No converted file appeared")
print("output:", remote)
adb("pull", remote, "converted.avi")
probe = subprocess.run(
    ["ffprobe", "-v", "error", "-show_entries", "stream=codec_name,width,height,sample_rate,channels",
     "-of", "csv=p=0", "converted.avi"], capture_output=True, text=True, check=True).stdout
print(probe)
assert "mjpeg,160,120" in probe, probe
assert "adpcm_ima_wav,22050,1" in probe, probe
frames = subprocess.run(["ffmpeg", "-v", "error", "-i", "converted.avi", "-f", "null", "-"],
                        capture_output=True, text=True)
assert frames.returncode == 0 and not frames.stderr.strip(), frames.stderr
print(f"OK: {remote} is a valid classic-player AVI")
