#!/bin/bash
# Khukri Native Messaging Host Registration Script (Linux)
# Usage: ./register-host.sh

set -e

HOST_ID="com.khukri.host"
MANIFEST_DIR="$HOME/.config/google-chrome/NativeMessagingHosts"
MANIFEST_PATH="$MANIFEST_DIR/$HOST_ID.json"

# Find the absolute path to the bridge binary (assume ../target/release/khukri-bridge)
BRIDGE_PATH="$(cd "$(dirname "$0")/.." && pwd)/target/release/khukri-bridge"

mkdir -p "$MANIFEST_DIR"

cat > "$MANIFEST_PATH" <<EOF
{
  "name": "$HOST_ID",
  "description": "Khukri Native Messaging Host",
  "path": "$BRIDGE_PATH",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://*"
  ]
}
EOF

echo "Native messaging host registered for Chrome: $MANIFEST_PATH"