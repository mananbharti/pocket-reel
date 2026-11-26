#!/usr/bin/env bash
# Download the official yt-dlp build and the QuickJS runtime (used by yt-dlp
# for YouTube) next to the bundled FFmpeg.
# Usage: scripts/fetch-downloader.sh linux64|win64|macos-arm64
set -euo pipefail
target="${1:?target: linux64, win64 or macos-arm64}"
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/app/src-tauri/resources/ffmpeg"
mkdir -p "$dest"
ytdlp="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
qjs="https://github.com/quickjs-ng/quickjs/releases/latest/download"
get() { curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 "$1" -o "$2"; }

case "$target" in
  linux64)
    get "$ytdlp/yt-dlp_linux" "$dest/yt-dlp"
    get "$qjs/qjs-linux-x86_64" "$dest/qjs"
    ;;
  win64)
    get "$ytdlp/yt-dlp.exe" "$dest/yt-dlp.exe"
    get "$qjs/qjs-windows-x86_64.exe" "$dest/qjs.exe"
    ;;
  macos-arm64)
    get "$ytdlp/yt-dlp_macos" "$dest/yt-dlp"
    get "$qjs/qjs-darwin-arm64" "$dest/qjs"
    ;;
  *) echo "unknown target $target" >&2; exit 1 ;;
esac
chmod +x "$dest"/yt-dlp* "$dest"/qjs* 2>/dev/null || true
get "https://raw.githubusercontent.com/yt-dlp/yt-dlp/master/LICENSE" "$dest/YT-DLP-LICENSE.txt" || true
{ if [[ "$target" == win64 ]]; then "$dest/yt-dlp.exe" --version; else "$dest/yt-dlp" --version; fi; } 2>/dev/null | sed "s/^/yt-dlp /" || true
echo "yt-dlp and QuickJS ($target) -> $dest"
