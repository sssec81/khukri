# Khukri Native Messaging Bridge

Implements the Chrome Native Messaging protocol for Khukri (KHU-203).

- Reads 4-byte little-endian length headers plus JSON payloads from stdin
- Writes framed JSON progress events to stdout
- Routes bridge logging to stderr so stdout stays protocol-safe
- Hands browser-initiated downloads off to `khukri-engine`

## Usage

- Build as a standalone binary for use with the Chrome extension
- Run `khukri-bridge --register` or `khukri-bridge --repair` to install or rewrite the native host manifest
- Handles `queue_download` messages and emits progress events until completion

## Current Sprint 2 State

- Engine handoff is implemented and covered by the native protocol integration test
- Custom browser headers from the extension are forwarded into `khukri-engine`
- Registration writes a host manifest with the bridge's absolute path
- Manual QA is currently easiest with Linux Chrome/WSL or a Windows Rust toolchain that can produce `khukri-bridge.exe`
- Known deferred item: stable extension-ID / exact `allowed_origins` wiring still needs to be finalized before packaging

## Verification

- Verified by `cargo test --workspace`
- `khukri-bridge` native protocol test passes end to end against a local HTTP server

## Sprint 2 - KHU-203 Acceptance Criteria

- [x] 4-byte native-endian length header logic for Chrome protocol
- [x] Accepts `CustomHeaders` from the extension and forwards them to `khukri-engine`
- [x] No extra stdout logging that could corrupt Native Messaging framing
- [x] Bridge logging is routed to stderr
- [x] Integration with `khukri-engine` for download handoff
