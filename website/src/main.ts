import "./style.css";
import type { SceneController } from "./scene";

const REPO = "mananbharti/pocket-reel";
const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

type OS = "windows" | "linux" | "android" | "mac" | "ios" | "unknown";

function detectOS(): OS {
  const ua = navigator.userAgent;
  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const plat = nav.userAgentData?.platform || navigator.platform || "";
  if (/android/i.test(ua)) return "android";
  if (/cros/i.test(ua)) return "android"; // Chromebooks run Android apps
  if (/iphone|ipad|ipod/i.test(ua) || (/mac/i.test(plat) && navigator.maxTouchPoints > 1)) return "ios";
  if (/win/i.test(plat) || /windows/i.test(ua)) return "windows";
  if (/mac/i.test(plat)) return "mac";
  if (/linux|x11/i.test(plat + ua)) return "linux";
  return "unknown";
}

const ASSETS: Record<string, { file: string; label: string; meta: string }> = {
  windows: { file: "PocketReel-windows-x64-setup.exe", label: "Download for Windows", meta: "Windows 10 and 11 · 64-bit" },
  linux: { file: "PocketReel-linux-x86_64.AppImage", label: "Download for Linux", meta: "AppImage · x86-64" },
  android: { file: "PocketReel-android.apk", label: "Download for Android", meta: "Android 8 and newer · APK" },
};

const latest = (file: string) => `https://github.com/${REPO}/releases/latest/download/${file}`;
const mb = (n: number) => `${(n / 1e6).toFixed(0)} MB`;

function setupDownloads() {
  const os = detectOS();
  const btn = document.getElementById("primary-download") as HTMLAnchorElement;
  const label = document.getElementById("primary-label")!;
  const meta = document.getElementById("primary-meta")!;
  const asset = ASSETS[os];
  if (asset) {
    btn.href = latest(asset.file);
    label.textContent = asset.label;
    meta.textContent = asset.meta;
    document.querySelector(`.platform[data-os="${os}"]`)?.classList.add("detected");
  } else {
    btn.href = "#download";
    label.textContent = "See downloads";
    meta.textContent =
      os === "mac" || os === "ios"
        ? "Pocket Reel runs on Windows, Linux and Android."
        : "Available for Windows, Linux and Android.";
  }

  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } })
    .then((r) => (r.ok ? r.json() : null))
    .then((rel) => {
      if (!rel?.assets) return;
      const sizes: Record<string, number> = {};
      for (const a of rel.assets) sizes[a.name] = a.size;
      document.querySelectorAll<HTMLElement>("[data-asset]").forEach((el) => {
        const s = sizes[el.dataset.asset!];
        if (s) el.textContent = `${rel.tag_name} · ${mb(s)}`;
      });
      if (asset && sizes[asset.file]) meta.textContent = `${asset.meta} · ${rel.tag_name} · ${mb(sizes[asset.file])}`;
    })
    .catch(() => {});
}

function setupTabs() {
  const tabs = Array.from(document.querySelectorAll<HTMLButtonElement>('.tabs [role="tab"]'));
  const img = document.querySelector<HTMLImageElement>("#shot img")!;
  const select = (t: HTMLButtonElement, focus = false) => {
    tabs.forEach((x) => {
      x.setAttribute("aria-selected", String(x === t));
      x.tabIndex = x === t ? 0 : -1;
    });
    if (focus) t.focus();
    if (img.getAttribute("src") === t.dataset.src) return;
    img.classList.add("swap");
    const next = new Image();
    next.src = t.dataset.src!;
    next.decode().finally(() => {
      img.src = next.src;
      img.alt = t.dataset.alt!;
      img.classList.remove("swap");
    });
  };
  tabs.forEach((t, i) => {
    t.tabIndex = i === 0 ? 0 : -1;
    t.addEventListener("click", () => select(t));
    t.addEventListener("keydown", (e) => {
      const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
      if (!d) return;
      e.preventDefault();
      select(tabs[(i + d + tabs.length) % tabs.length], true);
    });
  });
}

function setupReveals() {
  const io = new IntersectionObserver(
    (entries) => entries.forEach((e) => e.isIntersecting && e.target.classList.add("in")),
    { threshold: 0.35 },
  );
  document.querySelectorAll(".beat").forEach((el) => io.observe(el));
  document.querySelectorAll(".card, .spec, .window, .phones img, .platform, .checks li").forEach((el, i) => {
    el.classList.add("reveal");
    (el as HTMLElement).style.transitionDelay = `${(i % 3) * 70}ms`;
    io.observe(el);
  });
}

async function setupScene() {
  const canvas = document.getElementById("scene") as HTMLCanvasElement;
  const story = document.querySelector<HTMLElement>(".story")!;
  const hero = document.querySelector<HTMLElement>(".hero")!;
  let scene: SceneController | null = null;
  try {
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) throw new Error("no webgl2");
    const { startScene } = await import("./scene");
    scene = startScene(canvas, { videoSrc: `${import.meta.env.BASE_URL}media/screen.mp4`, reducedMotion: reduced });
  } catch {
    canvas.remove();
    document.querySelector<HTMLImageElement>(".hero-fallback")!.hidden = false;
    return;
  }

  const update = () => {
    const vh = window.innerHeight;
    const start = hero.offsetTop;
    const end = story.offsetTop + story.offsetHeight - vh;
    const p = (window.scrollY - start) / Math.max(1, end - start);
    scene!.setProgress(p);
    // Fade the scene away as the story ends.
    const fade = 1 - Math.min(1, Math.max(0, (window.scrollY - end) / (vh * 0.5)));
    canvas.style.setProperty("--scene-opacity", String(fade));
    scene!.setVisible(fade > 0.01);
    document.getElementById("nav")!.classList.toggle("scrolled", window.scrollY > 10);
  };
  window.addEventListener("scroll", update, { passive: true });
  window.addEventListener("resize", update);
  update();
  if (!reduced) {
    window.addEventListener(
      "pointermove",
      (e) => scene!.setPointer((e.clientX / window.innerWidth) * 2 - 1, (e.clientY / window.innerHeight) * 2 - 1),
      { passive: true },
    );
  }
}

setupDownloads();
setupTabs();
setupReveals();
setupScene();
