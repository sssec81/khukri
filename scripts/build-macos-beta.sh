#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="$(rustc -vV | awk '/^host:/ { print $2 }')"
ARTIFACTS="$REPO_ROOT/artifacts/beta"

case "$TARGET" in
  aarch64-apple-darwin|x86_64-apple-darwin) ;;
  *)
    echo "error: macOS beta builds require an Apple Rust target, got $TARGET" >&2
    exit 1
    ;;
esac

mkdir -p "$ARTIFACTS"

if [[ ! -x "$REPO_ROOT/sidecar/ffmpeg-$TARGET" ]]; then
  echo "error: sidecar/ffmpeg-$TARGET is missing." >&2
  echo "Run: bash sidecar/fetch-ffmpeg.sh macos" >&2
  exit 1
fi

cd "$REPO_ROOT"
if cargo audit --version >/dev/null 2>&1; then
  cargo audit --ignore RUSTSEC-2023-0071
fi
cargo test --workspace
cargo build --release -p khukri-bridge
install -m 0755 \
  "$REPO_ROOT/target/release/khukri-bridge" \
  "$REPO_ROOT/sidecar/khukri-bridge-$TARGET"

cargo tauri build \
  --config src-tauri/tauri.beta.conf.json \
  --bundles dmg

rm -f "$ARTIFACTS/khukri-extension-beta.zip"
(
  cd "$REPO_ROOT/extension"
  /usr/bin/zip -qr "$ARTIFACTS/khukri-extension-beta.zip" . \
    -x '.DS_Store' 'dist/*' '.cache/*'
)

find "$REPO_ROOT/target/release/bundle/dmg" -maxdepth 1 -name '*.dmg' \
  -exec cp {} "$ARTIFACTS/" \;

(
  cd "$ARTIFACTS"
  shasum -a 256 ./*.dmg khukri-extension-beta.zip > SHA256SUMS
)

echo "Beta artifacts:"
find "$ARTIFACTS" -maxdepth 1 -type f -print
