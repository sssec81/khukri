# Khukri

> A modern, local-first download manager built in Rust.
> Fast segmented downloads, resumable state, browser handoff, and a desktop shell without the usual bloat.

---

## Project Signals

- License: GPLv3
- Current status: Sprint 4 in progress
- Last verified locally before current Sprint 4 media work: `cargo check -p khukri-engine`, `cargo check -p khukri-app`
- Platform direction: Windows, Linux, macOS

Note:
CI/build badges are intentionally not shown yet because the repo does not have a finalized public release pipeline for them.

---

## Demo

Desktop shell status:

- working Tauri desktop shell
- queue view with progress, speed, ETA, and row actions
- browser handoff architecture already in place

Screenshot/demo asset:

- a polished public screenshot or GIF should be added here before launch
- the current repo includes the working app icon at `src-tauri/icons/icon.png`, but no final showcase image yet

---

## Why Khukri

Most download managers still feel stuck in another era: Windows-only, heavy, ad-filled, opaque, or fragile under real parallel load.

Khukri is the opposite:

- local-first: no account, no tracking, no phone-home dependency
- performance-focused: segmented downloads, throttling, retry logic, and resumable state
- engineered for reliability: Rust core, SQLite-backed progress, deterministic resume behavior
- modern architecture: browser extension + native bridge + desktop shell
- cross-platform direction: engine and app are being built for Windows, Linux, and macOS

If IDM is the benchmark people still compare against, Khukri is aiming to be the modern open alternative with a cleaner architecture and a better trust model.

---

## Why Now

| Question | Traditional Download Managers | Khukri |
|---|---|---|
| Trust model | Often opaque, bundled, ad-heavy, or legacy-first | Local-first, no account, no phone-home dependency |
| Core engine | Mature but often closed and hard to inspect | Rust engine with explicit retry, throttling, resume, and SQLite-backed state |
| Browser integration | Usually works, but tied to old UX assumptions | Extension + native bridge + desktop app architecture |
| Cross-platform future | Often Windows-first or Windows-only | Built with cross-platform direction from the start |
| Media roadmap | Often bolted on | Sprint 4 plans `yt-dlp` + FFmpeg as a first-class capability |
| Developer confidence | Hard to audit and hard to extend | Codebase is inspectable, documented, and built in the open |

This project exists because the downloader space still has demand, but much of the software in it still feels untrusted, bloated, or outdated.

---

## Current Status

| Sprint | Deliverable | Status |
|---|---|---|
| 1 - The Steel | Download engine (segmenting, SQLite, retry, queue, throttle) | Complete |
| 2 - The Sniffer | Browser extension + Native Messaging bridge | Complete |
| 3 - The Handle | Tauri GUI | Near Complete |
| 4 - The Scabbard | yt-dlp + FFmpeg integration | In Progress |
| 5 - Distribution | CI/CD, code signing, reproducible builds | Planned |

Sprint 2 is implemented and verified in the current `main` branch. Sprint 3 now has a working desktop shell in `src-tauri/` + `src/`, `cargo test --workspace` passes on native Windows, and both `cargo check -p khukri-engine` and `cargo check -p khukri-app` pass in Ubuntu 24.04 / WSL once the Tauri system packages are installed. See [docs/sprint-2-status.md](docs/sprint-2-status.md) and [docs/sprint-3-status.md](docs/sprint-3-status.md) for the ticket-by-ticket boards.
Sprint 4 is now active in the codebase, not just in planning. Pinned `yt-dlp` sidecars, Rust media job wiring, Blade quality selection, onboarding/legal gating, FFmpeg handoff hooks, and the first pass of the `yt-dlp` updater are all tracked in [docs/sprint-4-status.md](docs/sprint-4-status.md).

---

## What It Already Does

- segmented parallel downloads with retry and resume
- SQLite-backed download and segment state
- pause, resume, cancel, remove, and queue management
- bandwidth caps and per-download priority
- browser extension plus native bridge handoff
- desktop UI with progress bars, speed, ETA, settings, and tray integration
- scheduler gating and proxy-aware download requests

Planned next:

- end-to-end validation of the Sprint 4 media flow on real devices
- richer packaging and release workflows
- production polish across install, branding, and release workflows

Sprint 4 work already landed in the tree:

- pinned `yt-dlp` sidecar assets in `sidecar/`
- Tauri `externalBin` wiring for platform-specific media sidecar packaging
- Rust-side `yt-dlp` media invocation with progress parsing and quality mapping
- Blade hover quality picker with per-site persistence
- desktop onboarding/legal notice for media tooling
- FFmpeg sidecar discovery and `yt-dlp --ffmpeg-location` handoff
- background `yt-dlp` updater scaffolding with managed app-data sidecars

---

## Roadmap

### Sprint 3 Finish Line

- finish stabilization and edge-case QA
- verify native host registration and end-to-end browser handoff on Windows
- polish tray-state behavior
- replace placeholder branding assets
- measure Windows cold-start and RAM budget

### Sprint 4: Media Support

- finish validation of bundled `yt-dlp` media downloads across bridge and desktop flows
- ship and verify FFmpeg sidecars for split audio/video stitching
- prove updater behavior against live GitHub Releases data
- polish notifications, packaging behavior, and failure recovery around media tooling

Detailed planning lives in [docs/sprint-4-status.md](docs/sprint-4-status.md).

---

## Why It Can Win

Khukri is not trying to win by adding random features first. It wins if it becomes the downloader people trust:

- fast enough to replace IDM for everyday use
- transparent enough that developers and power users feel safe running it
- lightweight enough to leave open all day
- reliable enough that pause/resume and crash recovery are boring

That combination is rare, and that is the opportunity.

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

## Current Limitations

- Sprint 3 is not fully release-polished yet
- tray `Pause All` / `Resume All` enable-disable state still needs refinement
- `Open Folder` can be unreliable in WSL or desktop-less Linux setups
- Windows native shell is now verified for build, test, and app launch; extension handoff and runtime polish still need a final pass
- branding, screenshots, and production packaging polish are not final
- Sprint 4 media features are implemented in-progress but not fully validated or release-ready yet

That said, the core engine, bridge, and desktop-shell path are already real and working.

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
    |-- sprint-3-status.md
    `-- sprint-4-status.md
```

---

## Getting Started

### Prerequisites

- Rust toolchain (`rustup`, `cargo`)
- Windows 10/11, Linux, WSL2, or macOS
- Chrome or another Chromium browser for extension testing
- Tauri CLI for desktop app dev: `cargo install tauri-cli --version "^2"`
- For Tauri on Ubuntu/WSL: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, and `librsvg2-dev`

### Build

```bash
cargo build --workspace
```

### Test

Current workspace verification:

- `khukri-bridge`: 1 native protocol integration test
- `khukri-bridge`: 12 unit tests
- `khukri-engine`: 29 unit tests
- `khukri-engine`: 6 integration tests

```bash
cargo test --workspace
```

### Tauri app

Current local verification:

- `cargo test --workspace` passes on native Windows 11 on 2026-04-26
- `cargo tauri dev` launches successfully on native Windows 11 on 2026-04-26
- `cargo check -p khukri-engine` passes on Ubuntu 24.04 / WSL
- `cargo check -p khukri-app` passes on Ubuntu 24.04 / WSL after installing the required GTK/WebKit packages
- `cargo tauri info` reports a valid Rust/Tauri toolchain once those system packages are present
- `cargo tauri dev` launches successfully on Ubuntu 24.04 / WSL
- A placeholder tray/app icon currently lives at `src-tauri/icons/icon.png` so `tauri::generate_context!()` can build cleanly

Run the desktop shell:

```bash
cargo tauri dev
```

Quick verification from the current repo state:

```bash
cargo check -p khukri-engine
cargo check -p khukri-app
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
| `src-tauri/src/main.rs` queue orchestration | Priority ordering, scheduler gating, and max-concurrency promotion for the desktop app |
| `db/mod.rs` | SQLite persistence for download and segment state |
| `config.rs` | Early validation for URL, path, threads, proxy URL, and custom headers |

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
- Throttling: token bucket shared across all segment tasks via `Arc<tokio::sync::Mutex>`. Burst cap = 1 second of configured bandwidth. Tokens refill continuously at `bytes_per_sec`. Setting `bytes_per_sec = 0` disables throttling entirely. The bucket is per-download, not global — concurrent downloads each have independent caps
- Pre-allocation: reserve full disk space before writes begin
- Cancellation: cooperative cancellation support through the engine entrypoints
- Progress API: `spawn_download` returns a handle with watch-based updates
- Config safety: invalid runtime config is rejected early with explicit errors
- Range-unaware servers: the engine sends a HEAD probe to check `Accept-Ranges` before committing to segmented mode. If a server returns `200 OK` instead of `206 Partial Content` in response to a `Range` request, the engine classifies it as `NoRangeSupport` and falls back to a single-thread streaming download rather than writing at the wrong offset and corrupting output
- Proxy: unauthenticated proxy URL only (`http://host:port`). Credentials can be embedded in the URL (`http://user:pass@host:port`) as supported by the underlying HTTP client, but there are no separate username/password fields in the config
- Checksum: SHA-256 is used in the engine integration tests to verify byte-for-byte download integrity. It is not currently exposed as a user-facing post-download verification step

---

## Notes

- `Cargo.lock` remains excluded while the repo is still centered on library crates and in-progress application work.
- Integration risks and mitigations are documented in [docs/integration-hardening.md](docs/integration-hardening.md).
- The current Sprint 3 UI is intentionally plain JavaScript + HTML + CSS. A richer frontend toolchain can be added later without changing the Rust command surface.
- The current Tauri frontend keeps its locale file at `src/i18n/en.json` so it ships with the static frontend bundle cleanly.
- Sprint 3 currently includes queue actions, persisted settings, scheduler gating, proxy-aware downloads, failed-download reason display, and resumable progress restoration.
- Remaining Sprint 3 gaps are mostly stabilization work: tray menu enable/disable state, Windows runtime verification, branded icons, and a final QA pass on edge-case errors.

---

## Contributing

If you are interested in download engines, Rust systems work, browser-to-native integration, or building a credible IDM alternative, this project is already at the fun stage.

Good areas to jump in:

- Sprint 3 stabilization and cross-platform QA
- Sprint 4 media pipeline (`yt-dlp`, FFmpeg, updater)
- frontend polish and onboarding
- packaging, install flows, and release automation

---

## License

GPLv3. See [LICENSE](LICENSE).

Bundled yt-dlp (Sprint 4): Unlicense.  
Bundled FFmpeg (Sprint 4): GPL-compatible build only (no non-free codecs).
