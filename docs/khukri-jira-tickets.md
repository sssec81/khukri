# Khukri — Jira Tickets

> Generated from PRD v1.1 (LOCKED). Each ticket maps to a sprint deliverable.
> Labels: `khukri`, `rust`, `tauri`

---

## EPIC-01 · Sprint 1 — The Steel (Download Engine)

---

### KHU-101 · Set up `khukri-engine` Rust crate scaffold

**Type:** Task  
**Priority:** Highest  
**Labels:** `rust`, `engine`, `setup`

**Description:**  
Bootstrap the `khukri-engine` library crate. Configure `Cargo.toml` with all Sprint 1 dependencies: `tokio` (full features), `reqwest` (with HTTP/2 + stream), `sqlx` (sqlite + runtime-tokio), `thiserror`, `tracing`.

**Acceptance Criteria:**
- [ ] `cargo build` passes with zero warnings (`clippy --deny warnings`)
- [ ] Project structure follows: `src/engine/`, `src/db/`, `src/error.rs`, `src/lib.rs`
- [ ] `README.md` documents how to run tests

---

### KHU-102 · Implement dynamic segment thread-count formula

**Type:** Story  
**Priority:** Highest  
**Labels:** `rust`, `engine`, `segmenting`

**Description:**  
Implement the thread-count calculation:  
```
threads = clamp(floor(file_size_MB / 50), 4, 64)
```
This should be a pure function in `src/engine/segment.rs`. Must be unit-tested with boundary cases.

**Acceptance Criteria:**
- [ ] Function `calc_thread_count(file_size_bytes: u64) -> u8` exists and is exported
- [ ] Unit tests cover: `<200 MB → 4`, `500 MB → 10`, `3200 MB → 64`, `10 GB → 64` (clamped)
- [ ] Thread count is overridable via a `DownloadConfig` struct field (`override_threads: Option<u8>`)

---

### KHU-103 · Implement segmented HTTP download with `reqwest`

**Type:** Story  
**Priority:** Highest  
**Labels:** `rust`, `engine`, `http`

**Description:**  
Implement parallel byte-range segment downloading using a shared `reqwest::Client` with HTTP/2 multiplexing and keep-alive. Each segment downloads its assigned byte range via `Range: bytes=X-Y` header. Segments write to pre-allocated file positions concurrently via `tokio::fs`.

**Acceptance Criteria:**
- [ ] Shared `reqwest::Client` instantiated once per download session (not per segment)
- [ ] Segments spawn as `tokio::task`s and download concurrently
- [ ] Final file is byte-for-byte identical to source (verified by SHA-256 in integration test)
- [ ] Falls back to single-thread if server doesn't return `Accept-Ranges: bytes`

---

### KHU-104 · Atomic file pre-allocation before segment writes

**Type:** Task  
**Priority:** High  
**Labels:** `rust`, `engine`, `io`

**Description:**  
Before any segment writes, pre-allocate the full expected file size on disk to prevent fragmentation and partial writes.

- Linux: `fallocate(2)` via `nix` crate  
- Windows: `SetEndOfFile` via `windows-sys` or `winapi`  
- macOS: `fcntl(F_PREALLOCATE)` fallback to `ftruncate`

**Acceptance Criteria:**
- [ ] Pre-allocation runs on all three target OSes (conditional compilation via `#[cfg(target_os)]`)
- [ ] If pre-allocation fails (e.g., insufficient disk space), download is aborted immediately with a `DiskSpaceError` variant — no partial write
- [ ] Integration test confirms pre-allocated file size matches `Content-Length`

---

### KHU-105 · SQLite state persistence for pause/resume

**Type:** Story  
**Priority:** High  
**Labels:** `rust`, `engine`, `sqlite`, `pause-resume`

**Description:**  
Persist byte-range segment state in SQLite so downloads can be paused and resumed instantly without re-fetching completed segments.

**Schema (minimum):**
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

**Acceptance Criteria:**
- [ ] Pausing a download persists all incomplete segments to DB
- [ ] Resuming re-spawns only incomplete segments (completed ones are skipped)
- [ ] DB file stored at `$APP_DATA/khukri/state.db`
- [ ] `sqlx::migrate!` used for schema migrations

---

### KHU-106 · Retry logic with exponential back-off

**Type:** Task  
**Priority:** High  
**Labels:** `rust`, `engine`, `error-handling`

**Description:**  
On transient HTTP failures (5xx, network timeout, connection reset), automatically retry the failed segment. Default: 3 retries with exponential back-off starting at 1s.

**Acceptance Criteria:**
- [ ] `RetryConfig { max_retries: u8, base_delay_ms: u64 }` is configurable per download
- [ ] Back-off formula: `delay = base_delay * 2^attempt` with jitter (`± 10%`)
- [ ] 404 / 403 responses are **not** retried — surface `PermanentError` immediately
- [ ] Unit test simulates 2 transient failures then success; asserts exactly 2 retries used

---

### KHU-107 · Priority queue and concurrent download management

**Type:** Story  
**Priority:** Medium  
**Labels:** `rust`, `engine`, `queue`

**Description:**  
Implement a priority-based download queue. Priorities: `High`, `Normal`, `Low`. Max concurrent downloads defaults to 3, configurable.

**Acceptance Criteria:**
- [ ] `DownloadQueue` struct manages active and pending downloads
- [ ] High-priority downloads pre-empt Normal/Low when a slot opens
- [ ] `max_concurrent` is runtime-configurable without restart
- [ ] Queue state is persisted in SQLite (`status = 'queued'`)

---

### KHU-108 · Bandwidth throttling (per-download and global)

**Type:** Task  
**Priority:** Medium  
**Labels:** `rust`, `engine`, `throttling`

**Description:**  
Implement token-bucket rate limiting for bandwidth caps. Configurable at global level and per-download level (per-download overrides global).

**Acceptance Criteria:**
- [ ] `ThrottleConfig { bytes_per_sec: Option<u64> }` accepted by download task
- [ ] Token bucket implemented without external crate dependency (or use `governor`)
- [ ] Setting cap to `None` disables throttling (full speed)
- [ ] Integration test confirms measured throughput stays within `±10%` of configured cap

---

## EPIC-02 · Sprint 2 — The Sniffer (Browser Integration)

---

### KHU-201 · MV3 Chrome extension scaffold + `chrome.downloads` interceptor

**Type:** Story  
**Priority:** Highest  
**Labels:** `extension`, `mv3`

**Description:**  
Create the MV3 extension scaffold with a service worker that intercepts browser downloads and hands them off to the Native Messaging bridge.

**Acceptance Criteria:**
- [ ] `manifest.json` targets MV3 with `downloads` and `nativeMessaging` permissions.
- [ ] Service worker intercepts `onCreated` and cancels browser download to hand-off.

---

### KHU-202 · Automated Native Messaging Host Registration (The "Self-Installer")

**Type:** Task  
**Priority:** Highest  
**Labels:** `native-messaging`, `installer`

**Description:**  
Add a `--register` (or `--install`) flag to the Rust bridge binary that automatically detects the OS and writes the Manifest JSON/Registry keys.

**Acceptance Criteria:**
- [ ] Windows: Auto-write `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.khukri.host`.
- [ ] Linux: Auto-write JSON to `~/.config/google-chrome/NativeMessagingHosts/`.
- [ ] Path detection: Binary must find its own absolute path for the manifest `path` field.

---

### KHU-203 · Rust Native Messaging Bridge (stdin/stdout protocol)

**Type:** Story  
**Priority:** Highest  
**Labels:** `rust`, `ipc`

**Description:**  
Implement the Rust Native Messaging bridge over stdin/stdout with protocol-safe framing and output handling.

**Acceptance Criteria:**
- [ ] Implement 4-byte native-endian length header logic for Chrome protocol.
- [ ] `khukri-engine` updated to accept `CustomHeaders` (Cookies/User-Agent) from bridge.
- [ ] STDOUT logic ensures ZERO extra `println!` calls (to prevent protocol corruption).

---

### KHU-206 · Native Messaging Mock Test Suite

**Type:** Task  
**Priority:** Medium  
**Labels:** `testing`, `python`

**Description:**  
Create a Python or Rust script to simulate a browser. It sends 4-byte-prefixed JSON to the bridge via `stdin` and validates the response.

**Acceptance Criteria:**
- [ ] Script successfully triggers a 10MB download through the bridge.
- [ ] Validates JSON response from bridge for progress updates.

---

### KHU-204 · Content-script HLS/DASH stream detector

**Type:** Story  
**Priority:** High  
**Labels:** `extension`, `media-detection`, `hls`, `dash`

**Description:**  
Inject a content script that monitors network requests on video platforms for `.m3u8` (HLS) and `.mpd` (DASH) manifest URLs.

**Acceptance Criteria:**
- [ ] Content script uses `chrome.devtools`-free approach (observe `<video>` `src` and XHR/fetch patterns)
- [ ] Detected manifest URL and page origin are sent to service worker via `chrome.runtime.sendMessage`
- [ ] Works on YouTube (DASH manifests) in manual testing
- [ ] Does not fire on pages with no video element

---

### KHU-205 · Floating Blade UI pill overlay

**Type:** Story  
**Priority:** High  
**Labels:** `extension`, `ui`, `blade`

**Description:**  
Inject a subtle pill-shaped overlay in the bottom-right of detected video players. Appears after 1.5s of playback. Single click queues the highest quality stream. Dismissible per-site (stored in `chrome.storage.local`).

**Acceptance Criteria:**
- [ ] Pill uses Khukri brand colors: Gurkha Green `#2D5A27`, Tiger Amber `#FF9F1C`
- [ ] Appears after exactly 1.5s delay (use `setTimeout`, reset on page unload)
- [ ] "×" dismiss button sets `dismissed_sites: [origin]` in storage; pill never shows again on that origin
- [ ] Clicking pill sends `{ type: "queue_download", source: "blade" }` to service worker
- [ ] Does not shift page layout (positioned `fixed`, `z-index: 2147483647`)

---

## EPIC-03 · Sprint 3 — The Handle (Tauri GUI)

---

### KHU-301 · Tauri 2.0 app scaffold + engine integration

**Type:** Task  
**Priority:** Highest  
**Labels:** `tauri`, `rust`, `scaffold`

**Description:**  
Bootstrap the Tauri 2.0 application. Wire the `khukri-engine` crate as a dependency of the Tauri backend. Define Tauri commands for: `start_download`, `pause_download`, `cancel_download`, `get_queue`.

**Acceptance Criteria:**
- [ ] `tauri dev` launches with no errors on Windows and Linux
- [ ] `invoke('get_queue')` returns current download list from engine
- [ ] Cold-start time ≤ 800ms on Windows (measured from process spawn to first paint)
- [ ] RAM ≤ 80 MB with 5 active downloads (Khukri process only, not yt-dlp children)

---

### KHU-302 · "All Downloads" list view with progress bars

**Type:** Story  
**Priority:** Highest  
**Labels:** `tauri`, `ui`, `downloads-list`

**Description:**  
Main window shows all downloads (active, queued, paused, complete, failed) as a list. Each row shows: filename, progress bar, speed (MB/s), ETA, status badge, and action buttons (pause/resume/cancel/open folder).

**Acceptance Criteria:**
- [ ] Progress updates via Tauri events (`emit('progress', payload)`) at 500ms intervals
- [ ] Speed displayed as `X.X MB/s`; ETA formatted as `Xm Xs`
- [ ] Completed downloads show "Open Folder" button that opens OS file explorer
- [ ] Failed downloads show error reason inline (e.g., "404 Not Found")
- [ ] List is keyboard navigable (arrow keys, Enter to open, Delete to cancel)

---

### KHU-303 · Settings panel

**Type:** Story  
**Priority:** High  
**Labels:** `tauri`, `ui`, `settings`

**Description:**  
Settings panel accessible from the sidebar/nav. Sections: General (default download path, max concurrent), Performance (thread override, bandwidth cap), Scheduler (time window), Proxy (HTTP/SOCKS5).

**Acceptance Criteria:**
- [ ] Settings persisted to `$APP_DATA/khukri/settings.json`
- [ ] Changes apply immediately without restart (hot-reload config)
- [ ] "Reset to defaults" button per section
- [ ] All inputs are keyboard accessible with visible focus rings

---

### KHU-304 · System tray integration

**Type:** Task  
**Priority:** High  
**Labels:** `tauri`, `tray`

**Description:**  
Persistent system tray icon. Right-click menu: Pause All, Resume All, Open Dashboard, Quit. App minimises to tray instead of closing.

**Acceptance Criteria:**
- [ ] Tray icon uses Khukri logo (SVG → PNG at 16×16, 32×32, 64×64)
- [ ] "Pause All" / "Resume All" toggles based on current queue state
- [ ] Closing the window hides to tray; does not terminate the process
- [ ] Quit from tray persists queue state to SQLite before exit

---

### KHU-305 · Dark mode + light mode theming

**Type:** Task  
**Priority:** Medium  
**Labels:** `tauri`, `ui`, `theming`

**Description:**  
Implement both dark (default) and light themes using CSS custom properties. Dark: Obsidian `#0B0C10` background. Tiger Amber `#FF9F1C` accents must meet WCAG AA on both backgrounds.

**Acceptance Criteria:**
- [ ] Theme toggle in Settings; persisted to `settings.json`
- [ ] Follows OS preference on first launch (`prefers-color-scheme`)
- [ ] Tiger Amber on Obsidian: contrast ≥ 4.5:1 (verify with `axe` or manual calc)
- [ ] Tiger Amber on light background: verified manually, flag if it fails AA

---

## EPIC-04 · Sprint 4 — The Scabbard (yt-dlp + FFmpeg)

---

### KHU-401 · Bundle pinned yt-dlp sidecar binary

**Type:** Task  
**Priority:** Highest  
**Labels:** `yt-dlp`, `sidecar`, `bundling`

**Description:**  
Bundle a pinned yt-dlp release binary in `sidecar/`. Pin version is committed to `sidecar/yt-dlp.version`. Include platform-specific binaries for Windows x64, macOS (universal), Linux x64.

**Acceptance Criteria:**
- [ ] `sidecar/yt-dlp.version` contains the pinned tag (e.g., `2025.01.15`)
- [ ] SHA-256 checksums committed to `sidecar/yt-dlp.sha256`
- [ ] Tauri `externalBin` config references the sidecar correctly
- [ ] `cargo tauri build` includes correct binary for current platform

---

### KHU-402 · yt-dlp invocation from Rust + quality selection

**Type:** Story  
**Priority:** Highest  
**Labels:** `rust`, `yt-dlp`, `sidecar`

**Description:**  
Invoke the bundled yt-dlp binary from Rust via `std::process::Command` (or `tokio::process::Command`). Support quality options: `best`, `1080p`, `720p`, `audio-only`. Parse yt-dlp JSON output for progress reporting.

**Acceptance Criteria:**
- [ ] `YtDlpJob { url, quality, output_path }` struct drives invocation
- [ ] Progress parsed from yt-dlp `--progress-template` JSON output and emitted as Tauri events
- [ ] Process is killed cleanly on user cancel (no zombie processes)
- [ ] Audio-only mode uses `-x --audio-format mp3`

---

### KHU-403 · yt-dlp auto-updater (tagged releases, SHA-256 verification, hot-swap)

**Type:** Story  
**Priority:** High  
**Labels:** `rust`, `yt-dlp`, `auto-update`

**Description:**  
Background worker checks GitHub releases API for new **tagged** yt-dlp releases every 24 hours. On new release: download binary, verify SHA-256 from release assets, hot-swap. On failure: retain last known good binary, notify user once.

**Acceptance Criteria:**
- [ ] Checks `https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest` (tagged only, never `master`)
- [ ] SHA-256 verified against checksum published in release assets before swap
- [ ] Hot-swap is atomic: write to `yt-dlp.new`, verify, rename (no window where binary is missing)
- [ ] On any failure: existing binary untouched; one system notification sent (not repeated)
- [ ] Worker does not block UI thread or active downloads

---

### KHU-404 · Bundle minimal FFmpeg for stream stitching

**Type:** Task  
**Priority:** High  
**Labels:** `ffmpeg`, `sidecar`, `bundling`

**Description:**  
Bundle a minimal FFmpeg build (libavformat + libavcodec, no non-free codecs, GPL-compatible). Used exclusively for merging separate video + audio streams from yt-dlp.

**Acceptance Criteria:**
- [ ] FFmpeg binary < 30 MB per platform
- [ ] Only GPL-compatible codecs included (no libfdk-aac, no OpenH264 non-free)
- [ ] `ffmpeg -version` output logged at startup for auditability
- [ ] Stitching invoked automatically when yt-dlp produces separate video/audio files

---

### KHU-405 · Quality selector in Floating Blade UI

**Type:** Story  
**Priority:** High  
**Labels:** `extension`, `ui`, `blade`, `yt-dlp`

**Description:**  
Extend the Floating Blade UI with a quality picker: Best / 1080p / 720p / Audio Only. Selection is remembered per-site in `chrome.storage.local`.

**Acceptance Criteria:**
- [ ] Dropdown appears on hover over the pill (not a click — avoid accidental trigger)
- [ ] Default selection is `Best`
- [ ] Per-site quality preference persisted and restored on next visit
- [ ] Selection sent as `quality` field in the `queue_download` message

---

### KHU-406 · Legal/ToS notice in onboarding

**Type:** Task  
**Priority:** Medium  
**Labels:** `ui`, `legal`, `onboarding`

**Description:**  
On first launch, display a one-time onboarding screen with the yt-dlp legal notice: yt-dlp capability is provided as a technical tool; compliance with platform ToS is the user's responsibility; Khukri ships no credentials and no DRM bypass.

**Acceptance Criteria:**
- [ ] Shown exactly once (flag in `settings.json: onboarding_complete: true`)
- [ ] User must click "I Understand" to proceed (no silent dismiss)
- [ ] Full notice text matches PRD Section 5C verbatim
- [ ] Accessible: readable at 200% zoom, keyboard-focusable button

---

## EPIC-05 · Sprint 5 — Distribution (CI/CD + Audit)

---

### KHU-501 · GitHub Actions build matrix (Windows x64/ARM64, macOS universal, Linux x64)

**Type:** Task  
**Priority:** Highest  
**Labels:** `ci`, `github-actions`, `build`

**Description:**  
Set up GitHub Actions workflow that builds signed Tauri installers for all target platforms on every tagged release.

**Matrix:**
| OS | Arch | Output |
|---|---|---|
| Windows | x64 | `.msi` + NSIS `.exe` |
| Windows | ARM64 | `.msi` |
| macOS | universal | `.dmg` |
| Linux | x64 | `.AppImage` + `.deb` |

**Acceptance Criteria:**
- [ ] Workflow triggers on `push: tags: ['v*']`
- [ ] All four matrix targets build successfully
- [ ] Artifacts uploaded to GitHub Release
- [ ] Build time per matrix leg < 15 minutes (cache `~/.cargo/registry`)

---

### KHU-502 · Code-signed binaries (Windows EV cert + macOS Developer ID)

**Type:** Task  
**Priority:** Highest  
**Labels:** `ci`, `codesigning`, `security`

**Description:**  
Sign all release binaries. Windows: EV code-signing certificate (stored as GitHub secret). macOS: Developer ID Application cert + notarisation via `notarytool`.

**Acceptance Criteria:**
- [ ] Windows installer passes SmartScreen without "Unknown Publisher" warning
- [ ] macOS `.dmg` passes Gatekeeper on a clean macOS 12+ system
- [ ] Signing secrets stored only in GitHub repository secrets (never in code)
- [ ] CI fails loudly if signing step fails (no unsigned release slips through)

---

### KHU-503 · `cargo audit` + `clippy --deny warnings` SAST gate

**Type:** Task  
**Priority:** High  
**Labels:** `ci`, `security`, `sast`

**Description:**  
Add security and lint gates to CI that block merge/release on failure.

**Acceptance Criteria:**
- [ ] `cargo audit` runs on every PR; fails CI on any `RUSTSEC` advisory at severity ≥ Medium
- [ ] `cargo clippy --all-targets --deny warnings` runs on every PR
- [ ] `cargo fmt --check` enforced (no formatting drift)
- [ ] Results posted as PR check annotations

---

### KHU-504 · Installer auto-registers Native Messaging host on all three OSes

**Type:** Task  
**Priority:** High  
**Labels:** `installer`, `native-messaging`

**Description:**  
The Tauri installer (NSIS/MSI on Windows, `.pkg`/`.dmg` postinstall on macOS, `.deb` postinstall on Linux) must automatically register the Native Messaging host without any user action.

**Acceptance Criteria:**
- [ ] Fresh install → open Chrome → extension connects to host with zero manual steps (manual QA on each OS)
- [ ] Uninstall removes the registration cleanly
- [ ] Re-install over existing install does not break the registration
- [ ] Registration path is correct for Chrome, Edge, and Brave on each OS

---

### KHU-505 · Reproducible build verification

**Type:** Task  
**Priority:** Medium  
**Labels:** `ci`, `reproducible-builds`, `security`

**Description:**  
Run the build matrix twice independently and compare SHA-256 hashes of output binaries to confirm reproducibility.

**Acceptance Criteria:**
- [ ] CI workflow includes a second independent build run
- [ ] Hash comparison script fails CI if any binary hash differs between runs
- [ ] Hashes published to GitHub Release as `checksums.sha256`

---

### KHU-506 · README benchmark table (Khukri vs. IDM)

**Type:** Task  
**Priority:** Medium  
**Labels:** `docs`, `benchmark`

**Description:**  
Add a benchmark section to `README.md` comparing Khukri vs. IDM on a standardised 1 GB file download over a 100 Mbps and 1 Gbps connection.

**Acceptance Criteria:**
- [ ] Benchmark run on Windows 10 x64 (reference platform)
- [ ] Metrics: total download time, peak speed (MB/s), RAM usage (Task Manager peak)
- [ ] Methodology documented (file URL, connection, number of runs averaged)
- [ ] Khukri result meets DoD: speed ≥ IDM on same connection

---

## Cross-Cutting / Non-Functional Tickets

---

### KHU-601 · i18n scaffold — all strings in `i18n/en.json`

**Type:** Task  
**Priority:** Medium  
**Labels:** `i18n`, `frontend`

**Description:**  
From day one, no hardcoded UI strings. All strings externalised to `i18n/en.json`. RTL-ready layout (no fixed-direction CSS).

**Acceptance Criteria:**
- [ ] `i18n/en.json` exists and covers 100% of UI strings by end of Sprint 3
- [ ] `t('key')` helper used throughout frontend
- [ ] No hardcoded English strings in component files (enforced by ESLint rule or grep CI check)

---

### KHU-602 · Full keyboard navigation + accessibility audit

**Type:** Task  
**Priority:** Medium  
**Labels:** `accessibility`, `a11y`

**Description:**  
All interactive elements (buttons, inputs, download rows, tray menu) must be keyboard navigable with visible focus indicators.

**Acceptance Criteria:**
- [ ] Tab order is logical throughout the app
- [ ] All actions triggerable without a mouse
- [ ] Tiger Amber on Obsidian: contrast ≥ 4.5:1 (WCAG AA)
- [ ] `axe` automated scan reports zero critical violations on the downloads list view

---

### KHU-603 · Zero-telemetry audit

**Type:** Task  
**Priority:** High  
**Labels:** `privacy`, `security`

**Description:**  
Verify that Khukri makes zero outbound network requests except: user-initiated downloads, yt-dlp update check (24h interval, opt-out toggle), and GitHub Releases API for self-update (if implemented).

**Acceptance Criteria:**
- [ ] Network traffic captured with Wireshark on fresh install with no user action
- [ ] Zero unexpected outbound connections observed
- [ ] yt-dlp update check has an opt-out toggle in Settings
- [ ] Privacy policy / `PRIVACY.md` documents all network activity

---

*End of Khukri Jira Tickets — v1.1*
