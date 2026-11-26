#!/usr/bin/env bash
# Cross-compile FFmpeg (with LAME and x264) for Android and emit the two
# executables as lib*.so files, the only way Android lets an app run its own
# binaries (they are extracted to nativeLibraryDir, which is executable).
#
# Usage: ANDROID_NDK_HOME=... scripts/build-ffmpeg-android.sh <out-dir> [abi...]
# ABIs: arm64-v8a (default) x86_64
set -euo pipefail
out="$(realpath -m "${1:?output dir}")"; shift || true
abis=("${@:-arm64-v8a}")
api="${ANDROID_API:-26}"
ffmpeg_ver="${FFMPEG_VERSION:-8.0}"
lame_ver="3.100"
ndk="${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME}"
tc="$ndk/toolchains/llvm/prebuilt/linux-x86_64"
src="${SRC_DIR:-$PWD/.android-ffmpeg-src}"
jobs="$(nproc)"
mkdir -p "$src" "$out"

fetch() { # url file
  [[ -s "$src/$2" ]] || curl -fsSL --retry 5 -o "$src/$2" "$1"
}
fetch "https://ffmpeg.org/releases/ffmpeg-${ffmpeg_ver}.tar.xz" "ffmpeg.tar.xz"
fetch "https://downloads.sourceforge.net/project/lame/lame/${lame_ver}/lame-${lame_ver}.tar.gz" "lame.tar.gz"
fetch "https://code.videolan.org/videolan/x264/-/archive/stable/x264-stable.tar.bz2" "x264.tar.bz2"

for abi in "${abis[@]}"; do
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android; arch=aarch64; cpu=armv8-a; asm=() ;;
    x86_64) triple=x86_64-linux-android; arch=x86_64; cpu=x86-64; asm=(--disable-asm) ;;
    *) echo "unsupported ABI $abi" >&2; exit 1 ;;
  esac
  prefix="$src/prefix-$abi"
  build="$src/build-$abi"
  rm -rf "$build"; mkdir -p "$build" "$prefix"
  export CC="$tc/bin/${triple}${api}-clang" CXX="$tc/bin/${triple}${api}-clang++"
  export AR="$tc/bin/llvm-ar" RANLIB="$tc/bin/llvm-ranlib" NM="$tc/bin/llvm-nm" STRIP="$tc/bin/llvm-strip"
  export CFLAGS="-O3 -fPIC" LDFLAGS="-Wl,-z,max-page-size=16384"

  echo "== LAME ($abi)"
  tar -xzf "$src/lame.tar.gz" -C "$build"
  (cd "$build/lame-${lame_ver}" &&
    sed -i '/lame_init_old/d' include/libmp3lame.sym &&
    ./configure --host="$triple" --prefix="$prefix" --enable-static --disable-shared \
      --disable-frontend --disable-decoder --with-pic >/dev/null &&
    make -j"$jobs" >/dev/null && make install >/dev/null)

  echo "== x264 ($abi)"
  tar -xjf "$src/x264.tar.bz2" -C "$build"
  (cd "$build"/x264-stable &&
    ./configure --host="$triple" --cross-prefix="$tc/bin/llvm-" --sysroot="$tc/sysroot" \
      --prefix="$prefix" --enable-static --enable-pic --disable-cli --disable-opencl \
      $( [[ "$abi" == x86_64 ]] && echo --disable-asm ) >/dev/null &&
    make -j"$jobs" >/dev/null && make install >/dev/null)

  echo "== FFmpeg $ffmpeg_ver ($abi)"
  tar -xJf "$src/ffmpeg.tar.xz" -C "$build"
  (cd "$build/ffmpeg-${ffmpeg_ver}" &&
    PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure \
      --prefix="$prefix" --target-os=android --arch="$arch" --cpu="$cpu" --enable-cross-compile \
      --cc="$CC" --cxx="$CXX" --ar="$AR" --ranlib="$RANLIB" --nm="$NM" --strip="$STRIP" \
      --sysroot="$tc/sysroot" --pkg-config=pkg-config --pkg-config-flags=--static \
      --enable-pic --enable-static --disable-shared --disable-debug --disable-doc --disable-ffplay \
      --disable-network --disable-autodetect --enable-zlib \
      --enable-gpl --enable-version3 --enable-libmp3lame --enable-libx264 \
      --extra-cflags="-I$prefix/include -O3" --extra-ldflags="-L$prefix/lib -Wl,-z,max-page-size=16384" \
      --extra-libs="-lm" "${asm[@]}" >/dev/null &&
    make -j"$jobs" >/dev/null && make install >/dev/null)

  mkdir -p "$out/$abi"
  cp "$prefix/bin/ffmpeg" "$out/$abi/libffmpeg.so"
  cp "$prefix/bin/ffprobe" "$out/$abi/libffprobe.so"
  "$STRIP" "$out/$abi/libffmpeg.so" "$out/$abi/libffprobe.so"
  ls -la "$out/$abi"
done
