# Khukri — CLAUDE.md

## Project Overview

**Khukri** is a high-performance, open-source download manager built to replace IDM. Written in Rust + Tauri 2.0. Zero telemetry, zero bloat.

- **PRD:** `docs/khukri-prd.md` (LOCKED v1.1 — source of truth)
- **Tickets:** `docs/khukri-jira-tickets.md`

---

## Architecture

```
khukri/
├── crates/
│   └── khukri-engine/       # Core download engine (Rust library crate)
│       ├── src/
│       │   ├── engine/      # Segmenting, HTTP, throttling, queue
│       │   ├── db/          # SQLite persistence (sqlx)
│       │   ├── error.rs
│       │   └── lib.rs
├── src-tauri/               # Tauri 2.0 backend (Tauri commands, IPC, tray)
├── src/                     # Frontend (WebView UI — downloads list, settings)
├── extension/               # MV3 Chrome extension (service worker, content script)
├── sidecar/                 # Bundled yt-dlp + FFmpeg binaries
│   ├── yt-dlp.version
│   └── yt-dlp.sha256
└── docs/
    ├── khukri-prd.md
    └── khukri-jira-tickets.md
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust (backend), JavaScript (extension + current frontend shell) |
| Framework | Tauri 2.0 |
| Async runtime | Tokio (full features) |
| HTTP client | Reqwest (HTTP/2 + keep-alive + stream) |
| Persistence | SQLite via `sqlx` (async, migrations via `sqlx::migrate!`) |
| Browser extension | Manifest V3 (Chromium); MV2 Firefox planned Sprint 2.5 |
| IPC (browser ↔ Rust) | Named Pipes (Windows) / Unix Domain Sockets (Linux/macOS) |
| Internal IPC | Tauri command/event system |
| Sidecars | Pinned yt-dlp (tagged release) + minimal FFmpeg (GPL build) |

---

## Key Formulas & Constants

- **Thread count:** `threads = clamp(floor(file_size_MB / 50), 4, 64)`
- **Retry back-off:** `delay = base_delay_ms * 2^attempt ± 10% jitter` (default: 3 retries, base 1s)
- **Max concurrent downloads:** 3 (configurable)
- **Progress emit interval:** 500ms
- **Blade UI delay:** 1.5s after playback starts
- **yt-dlp update check interval:** 24h

---

## Performance Targets

| Metric | Target |
|---|---|
| RAM (Khukri process, 10 concurrent downloads) | ≤ 80 MB (excludes yt-dlp child processes) |
| Cold-start (Windows) | ≤ 800 ms to interactive UI |
| Time-to-first-segment | ≤ 500 ms from user initiating download |

---

## Brand Colors

| Name | Hex | Usage |
|---|---|---|
| Gurkha Green | `#2D5A27` | Primary actions, logo |
| Obsidian | `#0B0C10` | Background (dark mode) |
| Tiger Amber | `#FF9F1C` | Speed indicators, warnings, accents |

Tiger Amber on Obsidian must meet WCAG AA contrast (≥ 4.5:1). Verify on light backgrounds.

---

## SQLite Schema

State DB path: `$APP_DATA/khukri/state.db`

```sql
CREATE TABLE downloads (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  file_path TEXT NOT NULL,
  total_bytes INTEGER,
  status TEXT NOT NULL,  -- queued | active | paused | complete | failed
  created_at INTEGER NOT NULL
);

CREATE TABLE segments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  download_id TEXT NOT NULL REFERENCES downloads(id),
  start_byte INTEGER NOT NULL,
  end_byte INTEGER NOT NULL,
  completed INTEGER NOT NULL DEFAULT 0
);
```

Settings path: `$APP_DATA/khukri/settings.json`

Current Tauri bootstrap behavior:
- Windows: `%LOCALAPPDATA%\Khukri\`
- Linux: `$XDG_DATA_HOME/khukri` or `~/.local/share/khukri`
- Override for local/dev runs: `KHUKRI_DATA_DIR`

---

## Native Messaging Protocol

Chrome NM wire format: **4-byte little-endian length header + UTF-8 JSON body** (over stdin/stdout).

Key message types:
- Incoming: `{ type: "queue_download", url, filename, size, quality?, source? }`
- Outgoing: `{ type: "progress", id, bytes_done, speed_bps }`

Host ID: `com.khukri.host`

---

## Current Known State (as of Sprint 4)

### Extension — recently fixed

- `ask`-mode prompt flow is now working via `extension/prompt.html` + `extension/prompt.js`
- `web_accessible_resources` includes the prompt assets in `extension/manifest.json`
- `all_frames` is `false` on all content scripts to avoid duplicate injection
- `isExtensionAlive()` guards stale runtime messaging in `extension/content-script.js`
- `chrome.downloads.onCreated` cancels synchronously before async work in `extension/service-worker.js`
- Prompt keepalive port prevents MV3 service worker suspension while the dialog is open
- Retry queue fallback persists bridge-unavailable payloads in `chrome.storage.session`

### Known open issue

- `prompt.html` can open as a full tab instead of a popup window in Chrome/Brave
- `chrome.windows.create({ type: 'popup' })` is unreliable from MV3 service worker context
- Deferred for later investigation, likely involving an offscreen-document-based flow

### Extension file map

- `extension/service-worker.js` — download interception, stream detection state, prompt flow, native bridge handoff
- `extension/content-script.js` — isolated world relay, extension liveness guard, page message bridge
- `extension/content-script-main.js` — MAIN world fetch/XHR instrumentation for stream discovery
- `extension/blade-ui.js` — YouTube pill UI and quality picker
- `extension/prompt.html` — download interception dialog shell
- `extension/prompt.js` — dialog logic, keepalive port, TTL guard, decision handoff

---

## Non-Negotiables

- **Zero telemetry.** No outbound requests except: user-initiated downloads, yt-dlp update check (24h, opt-out toggle), self-update check.
- **No hardcoded UI strings.** For the current Tauri frontend, keep strings in `src/i18n/en.json`. Use `t('key')`-style lookup everywhere.
- **No `master` HEAD tracking for yt-dlp.** Tagged releases only, SHA-256 verified before any swap.
- **Atomic file ops.** Pre-allocate full file size before any segment writes. Hot-swap sidecars via write-to-temp → verify → rename.
- **`clippy --deny warnings` must pass.** Zero warnings policy enforced in CI.
- **License:** MIT. Bundled yt-dlp is Unlicense. FFmpeg must be GPL-compatible (no libfdk-aac, no OpenH264 non-free).

---

## Sprint Map

| Sprint | Deliverable | Key Tickets |
|---|---|---|
| 1 | Download engine (headless, CLI-verifiable) | KHU-101 → KHU-108 |
| 2 | Browser extension + Native Messaging bridge | KHU-201 → KHU-205 |
| 3 | Tauri GUI (list, settings, tray, theming) | KHU-301 → KHU-305 |
| 4 | yt-dlp + FFmpeg + auto-updater | KHU-401 → KHU-406 |
| 5 | CI/CD, code signing, reproducible builds | KHU-501 → KHU-506 |

Cross-cutting: KHU-601 (i18n), KHU-602 (a11y), KHU-603 (zero-telemetry audit)

Sprint 3 current state:
- `src-tauri/` is in the Cargo workspace as `khukri-app`
- The desktop shell currently exposes queue, start, pause, resume, cancel, folder-open, and settings commands
- The current WebView frontend is static `src/index.html` + `src/app.js` + `src/styles.css`
- `cargo check -p khukri-app` passes on Ubuntu 24.04 / WSL after installing `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, and `librsvg2-dev`
- `cargo tauri dev` is the next runtime verification step

Sprint 4 extension state:
- The Manifest V3 extension includes a working `ask` prompt flow plus stable `auto` interception mode
- Blade-triggered media downloads prefer captured `videoplayback` or playlist URLs, then fall back to the watch URL for `yt-dlp`
- Prompt payloads and the retry queue now use `chrome.storage.session` for MV3-safe transient state
- Blade dismissals persist per origin with a 7-day TTL instead of resetting on extension startup
- The main unresolved browser UX issue is Chrome/Brave opening the prompt page as a tab instead of a popup

---

## CI Gates (Sprint 5)

- `cargo audit` — fails on any RUSTSEC advisory ≥ Medium severity
- `cargo clippy --all-targets --deny warnings`
- `cargo fmt --check`
- Build matrix: Windows x64/ARM64 (`.msi`, `.exe`), macOS universal (`.dmg`), Linux x64 (`.AppImage`, `.deb`)
- Reproducible build: two independent runs, SHA-256 hashes must match
