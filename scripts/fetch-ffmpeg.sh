#!/usr/bin/env bash
# Download a static GPL FFmpeg build (BtbN/FFmpeg-Builds) and place ffmpeg and
# ffprobe where the Tauri bundler picks them up.
# Usage: scripts/fetch-ffmpeg.sh linux64|linuxarm64|win64
set -euo pipefail
target="${1:?target: linux64, linuxarm64 or win64}"
series="${FFMPEG_SERIES:-n8.1}"
version="${series#n}"
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/app/src-tauri/resources/ffmpeg"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [[ "$target" == win64 ]]; then
  name="ffmpeg-${series}-latest-win64-gpl-${version}"
  url="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/${name}.zip"
  curl -fsSL --retry 4 "$url" -o "$work/ff.zip"
  unzip -q "$work/ff.zip" -d "$work"
  exe=".exe"
else
  name="ffmpeg-${series}-latest-${target}-gpl-${version}"
  url="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/${name}.tar.xz"
  curl -fsSL --retry 4 "$url" -o "$work/ff.tar.xz"
  tar -xf "$work/ff.tar.xz" -C "$work"
  exe=""
fi

mkdir -p "$dest"
rm -f "$dest"/ffmpeg* "$dest"/ffprobe*
cp "$work/$name/bin/ffmpeg$exe" "$work/$name/bin/ffprobe$exe" "$dest/"
cp "$work/$name/LICENSE.txt" "$dest/FFMPEG-LICENSE.txt" 2>/dev/null || true
chmod +x "$dest"/ff* || true
"$dest/ffmpeg$exe" -hide_banner -version | head -1 || true
echo "FFmpeg ($series, $target) -> $dest"
