# Khukri

Khukri is a local-first download manager built with Rust and Tauri. It supports segmented HTTP downloads, pause and resume, browser handoff, and media downloads through `yt-dlp` and FFmpeg.

The project is under active development. The core download path, desktop app,
and a free macOS beta package work. A stable public release still needs Apple
notarization or store distribution, Chrome Web Store distribution, and broader
Windows and Linux validation.

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

Build a free, ad-hoc-signed macOS beta DMG and unpacked extension package:

```bash
bash sidecar/fetch-ffmpeg.sh macos
bash scripts/build-macos-beta.sh
```

See [docs/macos-beta.md](docs/macos-beta.md) for installation and Gatekeeper
instructions. This beta is not Apple-notarized.

The app stores its state in the platform data directory. Set `KHUKRI_DATA_DIR` to use an isolated directory during development.

## Browser extension

Load `extension/` as an unpacked extension from `chrome://extensions`. Its
committed public key keeps the beta extension ID stable. Packaged macOS beta
builds register the bundled native host automatically on app startup.

For development builds, the host can still be registered manually:

```bash
cargo build --release -p khukri-bridge
./extension/register-host.sh <extension-id>
```

The host registration manifest is browser-specific. See [extension/README.md](extension/README.md) for setup and troubleshooting, and [docs/extension-architecture.md](docs/extension-architecture.md) for the message flow.

## Media downloads

Khukri uses `yt-dlp` for extractor-backed media URLs. FFmpeg is required when a
site provides video and audio as separate streams. The app checks its managed
sidecar directory and packaged executable directory before falling back to
`PATH` and common installation paths on macOS and Linux.

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

- the free macOS beta is ad-hoc signed and requires **Open Anyway** approval
- the extension's ask-mode prompt may open as a normal tab in Chromium browsers
- the extension must be loaded unpacked until it is distributed through the Chrome Web Store
- the macOS FFmpeg beta sidecar is Intel-only and requires Rosetta on Apple Silicon
- tray and shell integration need broader Windows and Linux testing
- Windows and Linux installers are not part of the current beta pipeline

Implementation risks and verification notes are tracked in [docs/integration-hardening.md](docs/integration-hardening.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) before submitting changes.

## License

Khukri is available under the [MIT License](LICENSE). Bundled `yt-dlp` uses the Unlicense. FFmpeg distributions must remain GPL-compatible and must not include non-free codecs.
