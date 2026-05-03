#!/usr/bin/env bash
# fetch-ffmpeg.sh — download FFmpeg sidecars for all Khukri target platforms
# Run from the repo root: bash sidecar/fetch-ffmpeg.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SIDECAR="$REPO_ROOT/sidecar"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Fetching FFmpeg sidecars into $SIDECAR ..."

curl -fL https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip -o "$TMPDIR/windows.zip"
curl -fL https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz -o "$TMPDIR/linux.tar.xz"
curl -fL https://evermeet.cx/ffmpeg/getrelease/zip -o "$TMPDIR/macos.zip"

unzip -q "$TMPDIR/windows.zip" -d "$TMPDIR/windows"
tar -xJf "$TMPDIR/linux.tar.xz" -C "$TMPDIR"
unzip -q "$TMPDIR/macos.zip" -d "$TMPDIR/macos"

install -m 0755 "$TMPDIR"/windows/ffmpeg-*/bin/ffmpeg.exe "$SIDECAR/ffmpeg-x86_64-pc-windows-msvc.exe"
install -m 0755 "$TMPDIR"/ffmpeg-master-latest-linux64-gpl/bin/ffmpeg "$SIDECAR/ffmpeg-x86_64-unknown-linux-gnu"
install -m 0755 "$TMPDIR/macos/ffmpeg" "$SIDECAR/ffmpeg-x86_64-apple-darwin"
install -m 0755 "$TMPDIR/macos/ffmpeg" "$SIDECAR/ffmpeg-aarch64-apple-darwin"

echo ""
echo "Checksums:"
shasum -a 256 "$SIDECAR"/ffmpeg-*
echo ""
echo "Done. Verify against sidecar/ffmpeg.version before use."
