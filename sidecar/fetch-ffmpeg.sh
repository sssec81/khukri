#!/usr/bin/env bash
# fetch-ffmpeg.sh — download FFmpeg sidecars for Khukri target platforms
# Run from the repo root: bash sidecar/fetch-ffmpeg.sh [all|macos|windows|linux]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SIDECAR="$REPO_ROOT/sidecar"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
PLATFORM="${1:-all}"

case "$PLATFORM" in
  all|macos|windows|linux) ;;
  *)
    echo "usage: $0 [all|macos|windows|linux]" >&2
    exit 1
    ;;
esac

echo "Fetching FFmpeg sidecars into $SIDECAR ..."

if [[ "$PLATFORM" == "all" || "$PLATFORM" == "windows" ]]; then
  curl -fL https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip -o "$TMPDIR/windows.zip"
  unzip -q "$TMPDIR/windows.zip" -d "$TMPDIR/windows"
  install -m 0755 "$TMPDIR"/windows/ffmpeg-*/bin/ffmpeg.exe "$SIDECAR/ffmpeg-x86_64-pc-windows-msvc.exe"
fi

if [[ "$PLATFORM" == "all" || "$PLATFORM" == "linux" ]]; then
  curl -fL https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz -o "$TMPDIR/linux.tar.xz"
  tar -xJf "$TMPDIR/linux.tar.xz" -C "$TMPDIR"
  install -m 0755 "$TMPDIR"/ffmpeg-master-latest-linux64-gpl/bin/ffmpeg "$SIDECAR/ffmpeg-x86_64-unknown-linux-gnu"
fi

if [[ "$PLATFORM" == "all" || "$PLATFORM" == "macos" ]]; then
  curl -fL https://evermeet.cx/ffmpeg/ffmpeg-8.1.2.zip -o "$TMPDIR/macos.zip"
  unzip -q "$TMPDIR/macos.zip" -d "$TMPDIR/macos"
  install -m 0755 "$TMPDIR/macos/ffmpeg" "$SIDECAR/ffmpeg-x86_64-apple-darwin"
  install -m 0755 "$TMPDIR/macos/ffmpeg" "$SIDECAR/ffmpeg-aarch64-apple-darwin"
fi

echo ""
echo "Checksums:"
find "$SIDECAR" -maxdepth 1 -type f -name 'ffmpeg-*' -exec shasum -a 256 {} \;
echo ""
echo "Done. Verify against sidecar/ffmpeg.version before use."
