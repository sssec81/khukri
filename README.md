# Khukri

Khukri is a local-first download manager built with Rust and Tauri. It supports segmented HTTP downloads, pause and resume, browser handoff, and media downloads through `yt-dlp` and FFmpeg.

The project is under active development. The core download path and desktop app work, but packaging and cross-platform browser setup still need more testing before a stable release.

## Features

- parallel segmented downloads with a streaming fallback
- persisted queue and segment state in SQLite
- pause, resume, cancel, retry, and bandwidth limits
- Chromium extension with a native messaging bridge
- YouTube and media downloads through `yt-dlp`
- separate video and audio stream merging through FFmpeg
- dark and light desktop themes
- no accounts or telemetry

## Repository layout

```text
crates/khukri-engine/   download engine and SQLite persistence
crates/khukri-bridge/   browser native messaging host
extension/              Chromium extension
src-tauri/              Tauri backend
src/                    desktop UI
sidecar/                yt-dlp and FFmpeg tooling
docs/                   design notes and implementation history
```

## Requirements

- Rust and Cargo
- Tauri CLI 2.x
- a Chromium-based browser for extension development
- platform packages required by Tauri

On Ubuntu, install the GTK and WebKit development packages before building the desktop app:

```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev
```

## Build and run

Build the workspace:

```bash
cargo build --workspace
```

Run the desktop app:

```bash
cargo tauri dev
```

Run the test suite:

```bash
cargo test --workspace
```

The app stores its state in the platform data directory. Set `KHUKRI_DATA_DIR` to use an isolated directory during development.

## Browser extension

Load `extension/` as an unpacked extension from `chrome://extensions`. The native messaging host must be registered with the extension ID before browser handoff works.

```bash
cargo build --release -p khukri-bridge
./extension/register-host.sh <extension-id>
```

The host registration manifest is browser-specific. See [extension/README.md](extension/README.md) for setup and troubleshooting, and [docs/extension-architecture.md](docs/extension-architecture.md) for the message flow.

## Media downloads

Khukri uses `yt-dlp` for extractor-backed media URLs. FFmpeg is required when a site provides video and audio as separate streams. The app checks the managed sidecar directory first, then `PATH`, followed by common installation paths on macOS and Linux.

Useful overrides:

```text
KHUKRI_YTDLP_BIN
KHUKRI_FFMPEG_BIN
KHUKRI_YTDLP_JS_RUNTIME
```

Selected qualities such as 1080p are maximum resolutions. If the source only offers a lower resolution, Khukri downloads the best available stream below that cap.

## Engine smoke test

```bash
# Streaming response without Content-Length
cargo run -p khukri-engine --example download -- \
  "https://speed.cloudflare.com/__down?bytes=10485760" /tmp/test.bin

# Range-enabled response
cargo run -p khukri-engine --example download -- \
  "https://proof.ovh.net/files/10Mb.dat" /tmp/test.bin
```

## Current limitations

- native host registration is still a manual development step
- the extension's ask-mode prompt may open as a normal tab in Chromium browsers
- packaging, signing, and sidecar distribution are not release-ready
- tray and shell integration need broader Windows and Linux testing
- browser extension IDs are not yet stable for packaged releases

Implementation risks and verification notes are tracked in [docs/integration-hardening.md](docs/integration-hardening.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) before submitting changes.

## License

Khukri is available under the [MIT License](LICENSE). Bundled `yt-dlp` uses the Unlicense. FFmpeg distributions must remain GPL-compatible and must not include non-free codecs.
