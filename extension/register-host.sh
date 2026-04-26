#!/usr/bin/env bash
# Khukri Native Messaging Host Registration (Linux / macOS)
#
# Usage:
#   ./register-host.sh <extension-id> [--bridge /path/to/khukri-bridge] [--dry-run]
#
# <extension-id>  The 32-character Chrome/Chromium/Brave/Edge extension ID
#                 shown in chrome://extensions when developer mode is on,
#                 or the ID assigned by the Chrome Web Store after publishing.
#
# The script:
#   1. Validates the extension ID (must be exactly 32 lowercase a-p chars).
#   2. Sets KHUKRI_EXTENSION_ORIGIN and delegates to khukri-bridge --register
#      so the bridge's validate_extension_origin check is always exercised.
#   3. Copies the generated manifest to Chromium, Brave, and Edge paths if
#      those directories exist.
#
# Dry-run prints what would happen without writing any files.
#
# Exit codes:
#   0  success
#   1  bad usage / invalid extension ID
#   2  bridge binary not found or registration failed

set -euo pipefail

# ── Argument parsing ─────────────────────────────────────────────────────────

EXTENSION_ID=""
BRIDGE_BIN=""
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge)
      BRIDGE_BIN="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --help|-h)
      sed -n '2,20p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      exit 1
      ;;
    *)
      if [[ -z "$EXTENSION_ID" ]]; then
        EXTENSION_ID="$1"
      else
        echo "error: unexpected argument: $1" >&2
        exit 1
      fi
      shift
      ;;
  esac
done

# ── Validate extension ID ────────────────────────────────────────────────────

if [[ -z "$EXTENSION_ID" ]]; then
  echo "error: extension ID is required." >&2
  echo "  Usage: $0 <extension-id> [--bridge /path/to/khukri-bridge] [--dry-run]" >&2
  echo "  Find your extension ID in chrome://extensions with developer mode on." >&2
  exit 1
fi

if [[ ! "$EXTENSION_ID" =~ ^[a-p]{32}$ ]]; then
  echo "error: '$EXTENSION_ID' is not a valid Chrome extension ID." >&2
  echo "  Chrome extension IDs are exactly 32 lowercase letters a-p." >&2
  exit 1
fi

ORIGIN="chrome-extension://${EXTENSION_ID}/"

# ── Locate bridge binary ─────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ -z "$BRIDGE_BIN" ]]; then
  # Look in release build, then debug build, then PATH.
  for candidate in \
    "$REPO_ROOT/target/release/khukri-bridge" \
    "$REPO_ROOT/target/debug/khukri-bridge" \
    "$(command -v khukri-bridge 2>/dev/null || true)"
  do
    if [[ -x "$candidate" ]]; then
      BRIDGE_BIN="$candidate"
      break
    fi
  done
fi

if [[ -z "$BRIDGE_BIN" || ! -x "$BRIDGE_BIN" ]]; then
  echo "error: khukri-bridge binary not found." >&2
  echo "  Build it first:  cargo build -p khukri-bridge --release" >&2
  echo "  Or pass:         --bridge /path/to/khukri-bridge" >&2
  exit 2
fi

# ── Detect OS and primary Chrome manifest path ───────────────────────────────

OS="$(uname -s)"

if [[ "$OS" == "Darwin" ]]; then
  CHROME_NMH_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
else
  CHROME_NMH_DIR="$HOME/.config/google-chrome/NativeMessagingHosts"
fi

HOST_ID="com.khukri.host"
PRIMARY_MANIFEST="$CHROME_NMH_DIR/$HOST_ID.json"

# ── Secondary browser manifest paths ─────────────────────────────────────────

declare -a SECONDARY_DIRS=()

if [[ "$OS" == "Darwin" ]]; then
  SECONDARY_DIRS=(
    "$HOME/Library/Application Support/Chromium/NativeMessagingHosts"
    "$HOME/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts"
    "$HOME/Library/Application Support/Microsoft Edge/NativeMessagingHosts"
  )
else
  SECONDARY_DIRS=(
    "$HOME/.config/chromium/NativeMessagingHosts"
    "$HOME/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts"
    "$HOME/.config/microsoft-edge/NativeMessagingHosts"
  )
fi

# ── Dry-run summary ──────────────────────────────────────────────────────────

if [[ "$DRY_RUN" == true ]]; then
  echo "[dry-run] Would register native messaging host:"
  echo "  Bridge binary : $BRIDGE_BIN"
  echo "  Extension ID  : $EXTENSION_ID"
  echo "  Origin        : $ORIGIN"
  echo "  Primary path  : $PRIMARY_MANIFEST"
  for dir in "${SECONDARY_DIRS[@]}"; do
    echo "  Secondary     : $dir/$HOST_ID.json (if directory exists)"
  done
  exit 0
fi

# ── Primary registration via bridge --register ───────────────────────────────

echo "Registering native messaging host..."
echo "  Bridge  : $BRIDGE_BIN"
echo "  Origin  : $ORIGIN"

export KHUKRI_EXTENSION_ORIGIN="$ORIGIN"
if ! "$BRIDGE_BIN" --register; then
  echo "error: khukri-bridge --register failed." >&2
  exit 2
fi

echo "  Written : $PRIMARY_MANIFEST"

# ── Copy manifest to secondary browser paths ──────────────────────────────────

for dir in "${SECONDARY_DIRS[@]}"; do
  if [[ -d "$dir" ]]; then
    cp "$PRIMARY_MANIFEST" "$dir/$HOST_ID.json"
    echo "  Copied  : $dir/$HOST_ID.json"
  fi
done

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "Done. Restart Chrome/Chromium/Brave/Edge for the change to take effect."
echo ""
echo "If downloads are not intercepted, verify:"
echo "  1. The extension is loaded and enabled."
echo "  2. The extension ID matches: $EXTENSION_ID"
echo "  3. The bridge binary is executable: $BRIDGE_BIN"
