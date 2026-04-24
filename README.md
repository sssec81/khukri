# Khukri

> Khukri exists because I wanted a downloader that does not phone home, does not eat RAM, and does not turn into a lag machine under parallel downloads.

---

## Status

| Sprint | Deliverable | Status |
|---|---|---|
| 1 - The Steel | Download engine (segmenting, SQLite, retry, queue, throttle) | Complete |
| 2 - The Sniffer | Browser extension + Native Messaging bridge | Complete |
| 3 - The Handle | Tauri GUI | In Progress |
| 4 - The Scabbard | yt-dlp + FFmpeg integration | Planned |
| 5 - Distribution | CI/CD, code signing, reproducible builds | Planned |

Sprint 2 is implemented and verified in the current `main` branch. Sprint 3 now has a working local scaffold in `src-tauri/` + `src/`, and `cargo check -p khukri-app` passes in Ubuntu 24.04 / WSL once the Tauri system packages are installed. See [docs/sprint-2-status.md](docs/sprint-2-status.md) and [docs/sprint-3-status.md](docs/sprint-3-status.md) for the ticket-by-ticket boards.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust (backend), JavaScript (extension + current Tauri frontend) |
| Framework | Tauri 2.0 |
| Async | Tokio |
| HTTP | Reqwest (rustls-tls, HTTP/2) |
| Database | SQLite via sqlx |
| Extension | Manifest V3 (Chromium) |

---

## Why Rust?

Because this project is mostly systems work disguised as a downloader.

- Parallel segment writes are easy to get wrong: one bad offset or shared mutable state bug can silently corrupt files.
- NVMe can hide bad design for a while, then punish it at scale when concurrent writes and allocation churn kick in.
- If you do not pre-allocate up front, the filesystem allocator keeps searching for free extents while segments are writing, which increases I/O jitter and can fragment write patterns under load.
- Rust gives strong guarantees around ownership and concurrency, so the engine can run many async tasks without data races while still keeping throughput high.

Khukri leans into this: deterministic segment ranges, pre-allocation before writes, explicit retry/cancel paths, and SQLite-backed state so crashes do not destroy progress.

---

## Architecture

```text
khukri/
|-- crates/
|   |-- khukri-engine/     # Core download engine (Rust library)
|   `-- khukri-bridge/     # Native Messaging bridge (Rust binary)
|-- extension/             # MV3 Chrome extension (Sprint 2)
|-- src-tauri/             # Tauri backend (Sprint 3)
|-- src/                   # Frontend UI (Sprint 3)
|-- sidecar/               # yt-dlp + FFmpeg binaries (Sprint 4)
`-- docs/
    |-- adr/
    |-- khukri-prd.md
    |-- khukri-jira-tickets.md
    |-- integration-hardening.md
    |-- sprint-2-status.md
    `-- sprint-3-status.md
```

---

## Getting Started

### Prerequisites

- Rust toolchain (`rustup`, `cargo`)
- Linux, WSL2, or macOS for the current native-host development flow
- Chrome or another Chromium browser for extension testing
- For Tauri on Ubuntu/WSL: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, and `librsvg2-dev`

### Build

```bash
cargo build --workspace
```

### Test

Current workspace verification:

- `khukri-bridge`: 1 native protocol integration test
- `khukri-engine`: 18 unit tests
- `khukri-engine`: 6 integration tests

```bash
cargo test --workspace
```

### Tauri app

Current local verification:

- `cargo check -p khukri-app` passes on Ubuntu 24.04 / WSL after installing the required GTK/WebKit packages
- `cargo tauri info` reports a valid Rust/Tauri toolchain once those system packages are present
- A placeholder tray/app icon currently lives at `src-tauri/icons/icon.png` so `tauri::generate_context!()` can build cleanly

Run the desktop shell:

```bash
cargo tauri dev
```

Key paths used by the Tauri shell:

- State DB: `LOCALAPPDATA/Khukri/state.db` on Windows
- Settings: `LOCALAPPDATA/Khukri/settings.json` on Windows
- On Linux/macOS, the app falls back to the platform data directory or temp dir when no explicit `KHUKRI_DATA_DIR` override is set

### Engine smoke test

```bash
# Streaming download (no Content-Length)
cargo run -p khukri-engine --example download -- "https://speed.cloudflare.com/__down?bytes=10485760" /tmp/test.bin

# Segmented download (parallel, with range support)
cargo run -p khukri-engine --example download -- https://proof.ovh.net/files/10Mb.dat /tmp/test.bin
```

### Extension / bridge notes

- Extension files live in [extension/README.md](extension/README.md)
- Bridge details live in [crates/khukri-bridge/README.md](crates/khukri-bridge/README.md)
- Stable packaged extension ID / final `allowed_origins` wiring is still deferred

---

## Sprint 1 - The Steel

### Implemented

| Module | Description |
|---|---|
| `engine/segment.rs` | Thread count formula: `clamp(floor(file_size_mb / 50), 4, 64)` |
| `engine/download.rs` | Segmented parallel download plus streaming fallback, deterministic download IDs, cancellation-aware entrypoint |
| `engine/download.rs` API | Public `spawn_download` plus `DownloadHandle` progress watcher |
| `engine/retry.rs` | Exponential backoff with jitter; permanent errors (403, 404) are not retried |
| `engine/prealloc.rs` | Platform-specific pre-allocation before writes |
| `engine/throttle.rs` | Shared token-bucket rate limiter |
| `engine/queue.rs` | Priority queue with hot-configurable concurrency |
| `db/mod.rs` | SQLite persistence for download and segment state |
| `config.rs` | Early validation for URL, path, threads, and custom headers |

### Test coverage

| Type | Count | What |
|---|---|---|
| Unit | 18 | Segment sizing, retry logic, throttle behavior, queue ordering, and config validation |
| Integration | 6 | SHA-256 segmented download, streaming fallback, retry handling, permanent failure handling, resume behavior, and spawned progress reporting |

---

## Sprint 2 - The Sniffer

### Implemented

| Area | Description |
|---|---|
| `extension/manifest.json` | MV3 extension with download interception, storage, webRequest, and native messaging permissions |
| `extension/service-worker.js` | Browser download interception, stream candidate tracking, and Native Messaging handoff |
| `extension/content-script.js` | Blob/video fallback detection with page-context forwarding |
| `extension/blade-ui.js` | IDM-style player-adjacent Blade pill with SPA reinjection |
| `crates/khukri-bridge/src/main.rs` | Native Messaging framing, stderr-safe logging, engine handoff, and `--register` / `--repair` flows |
| `crates/khukri-bridge/tests/native_protocol.rs` | End-to-end bridge protocol test against a local HTTP server |

### Current notes

- The service worker remembers the best stream candidate and queues on Blade click rather than auto-queuing every observed request.
- Custom browser headers are forwarded from the extension into `khukri-engine`.
- The current dev workflow resets Blade dismissal state on extension install/startup to make manual QA easier.
- Full YouTube extractor-grade support is still future work for Sprint 4.

---

## Key Design Decisions

- Thread count: `clamp(floor(file_size_mb / 50), 4, 64)`
- TLS: `rustls`
- Segment writes: each task opens the file independently and seeks to its own offset
- Pause/resume: deterministic download ID plus SQLite segment state
- Throttling: token bucket shared across all segment tasks
- Pre-allocation: reserve full disk space before writes begin
- Cancellation: cooperative cancellation support through the engine entrypoints
- Progress API: `spawn_download` returns a handle with watch-based updates
- Config safety: invalid runtime config is rejected early with explicit errors

---

## Notes

- `Cargo.lock` remains excluded while the repo is still centered on library crates and in-progress application work.
- Integration risks and mitigations are documented in [docs/integration-hardening.md](docs/integration-hardening.md).
- The current Sprint 3 UI is intentionally plain JavaScript + HTML + CSS. A richer frontend toolchain can be added later without changing the Rust command surface.
- The current Tauri frontend keeps its locale file at `src/i18n/en.json` so it ships with the static frontend bundle cleanly.

---

## License

GPLv3. See [LICENSE](LICENSE).

Bundled yt-dlp (Sprint 4): Unlicense.  
Bundled FFmpeg (Sprint 4): GPL-compatible build only (no non-free codecs).
