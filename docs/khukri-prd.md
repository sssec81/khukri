# 🗡️ Khukri PRD: Sovereign Download Manager

> **Status: LOCKED — v1.1**  
> **Lead Dev Note:** This is the definitive source of truth. Every feature, metric, and sprint decision traces back to the Sovereign + High-Performance goals. Changes require a version bump and a rationale comment.

---

## 1. Vision & Executive Summary

**Khukri** is a high-performance, open-source download manager designed to replace legacy tools like IDM. Built with **Rust** and **Tauri 2.0**, it targets extreme throughput, memory safety, and a minimalist "Himalayan Tiger" aesthetic — zero bloat, zero telemetry, zero compromises.

---

## 2. Target Audience

| Segment | Primary Need |
|---|---|
| **Power Users** | Multi-threaded speed for large files (ISOs, archives, datasets) |
| **Media Consumers** | One-click YouTube / streaming site download experience |
| **Developers / Privacy-Conscious** | Open-source, auditable alternative to cracked proprietary tools |

---

## 3. Technical Stack

| Layer | Technology |
|---|---|
| **Language** | Rust (backend), TypeScript (browser extension) |
| **Framework** | Tauri 2.0 — native WebView, low RAM footprint |
| **I/O Engine** | Tokio (async runtime) + Reqwest (HTTP client) |
| **Persistence** | SQLite via `sqlx` (async) or `rusqlite` |
| **Native Messaging Bridge** | Named Pipes (Windows) / Unix Domain Sockets (macOS, Linux) — used exclusively for the browser-extension ↔ Rust host IPC |
| **Internal App IPC** | Tauri's built-in command/event system |
| **Browser Extension** | Manifest V3 (Chromium); MV2 fallback planned for Firefox |

---

## 4. Supported Platforms

| OS | Minimum Version | Architecture |
|---|---|---|
| Windows | 10 (build 19041+) | x64, ARM64 |
| macOS | 12 (Monterey) | x64, Apple Silicon (universal binary) |
| Linux | Ubuntu 22.04 LTS or equivalent | x64 |

---

## 5. Core Features & Requirements

### A. The "Steel" — Download Engine

- **Dynamic Segmenting:** Thread count calculated as `threads = clamp(floor(file_size_MB / 50), 4, 64)`. Rationale: one thread per 50 MB of file size, bounded between 4 (minimum parallelism) and 64 (practical socket ceiling). Override configurable in settings.

  > ⚠️ **v1.0 note removed:** The previous formula `file_size_GB × bandwidth_Mbps / 8` produced dimensional units of `GB·MB/s`, not a thread count. The new formula is dimensionally correct and empirically tunable.

- **Connection Reuse:** Shared `reqwest::Client` with HTTP/2 multiplexing and keep-alive connection pooling for maximum throughput.

- **Atomic Writing:** Pre-allocate full file space on disk via `fallocate` (Linux) / `SetEndOfFile` (Windows) before segment writes to prevent fragmentation.

- **Pause/Resume:** Byte-range state persisted in SQLite. Resume is instantaneous — no re-request of completed segments.

- **Queue Management:** Priority-based download queue (High / Normal / Low). Max concurrent downloads configurable (default: 3).

- **Bandwidth Throttling:** Per-download and global speed caps, configurable in settings. Respects OS-level network conditions.

- **Scheduler:** Optional time-window scheduling (e.g., "only download 10pm–6am").

- **Proxy Support:** HTTP/HTTPS/SOCKS5 proxy, with per-download override.

- **Error Handling:** Automatic retry on transient failures (configurable, default: 3 retries with exponential back-off). Permanent failures (404, 403) surface immediately with a clear error message — no silent drops.

---

### B. The "Sniffer" — Browser Integration

- **Active Interception:** Capture download events via the `chrome.downloads` API and route them through the Native Messaging bridge to the Rust host.

- **Media Detection:** Detect `.m3u8` (HLS) and `.mpd` (DASH) stream manifests on video platforms via content-script URL pattern matching.

- **Floating Blade UI:** A subtle, non-intrusive pill overlay appears near the active video player after 1.5 s of playback. Single click queues the best available quality. Dismissible per-site during a session, with development-time reset on extension install/startup to keep manual QA repeatable.

- **Browser Support:**
  - **Chromium (MV3):** Full support — Chrome, Edge, Brave, Opera GX.
  - **Firefox (MV2/MV3 hybrid):** Planned for Sprint 2.5. Feature-flagged; native messaging host registration differs on Linux.

---

### C. The "Scabbard" — Sidecar Management

- **yt-dlp Integration:** Bundle a pinned yt-dlp release binary at build time. The version is committed to `sidecar/yt-dlp.version` and tracked in lockfile.

- **Self-Updating Blade:** A background worker checks for new **tagged releases** of yt-dlp (not `master` HEAD) every 24 hours. On a new release: download, verify SHA-256, hot-swap. On failure: retain last known good binary and notify the user once via system notification.

  > ⚠️ **v1.0 risk removed:** The previous spec tracked `master` branch. Shipping HEAD on a 24-hour cadence means shipping untested code. Tagged releases are stable checkpoints with checksums.

- **FFmpeg Stitching:** Automatic merging of separate video/audio streams. Bundle a minimal FFmpeg build (libavformat + libavcodec only) to keep binary size down.

- **Legal / ToS Notice:** yt-dlp functionality is provided as a technical capability. Compliance with the Terms of Service of any streaming platform is the user's responsibility. Khukri ships no credentials, no DRM bypass, and no circumvention of technical protection measures.

---

### D. The "Sheath" — Shell & System Integration

- **System Tray:** Persistent tray icon with quick-access menu (pause all, resume all, open dashboard, quit). App minimises to tray by default.

- **Notifications:** OS-native notifications for: download complete, download failed, yt-dlp update applied.

- **File Association:** Register as the default handler for `.metalink` and `.torrent` files (torrent support: Sprint 6+).

- **Startup Behaviour:** Optional launch-at-login, configured in Settings. Not enabled by default.

---

## 6. Design & Brand Identity

- **Colors:**
  - **Gurkha Green (#2D5A27):** Primary actions, logo.
  - **Obsidian (#0B0C10):** Background.
  - **Tiger Amber (#FF9F1C):** High-speed indicators, warning alerts.

- **Modes:** Full dark mode (default) and light mode. Tiger Amber meets WCAG AA contrast on Obsidian; verify on light backgrounds during Sprint 3.

- **Philosophy:** Subtle, professional, minimalist. No modal spam, no upsell banners, no feature discovery tours.

---

## 7. Success Metrics

| # | Metric | Target | How Measured |
|---|---|---|---|
| 1 | RAM during active downloads | ≤ 80 MB (engine + UI, excluding yt-dlp sidecars) | `heaptrack` / Windows Task Manager |
| 2 | Zero-configuration setup | Browser bridge auto-configured on install after packaged extension-ID wiring is finalized | Manual QA: fresh install → first intercept |
| 3 | Download speed vs. IDM | Match or exceed on same connection | Parallel benchmark, same 1 GB file |
| 4 | Cold-start time (Windows) | ≤ 800 ms to interactive UI | `time` from process spawn to first paint |
| 5 | Time-to-first-segment | ≤ 500 ms from user initiating a download | Instrumented log timestamps |

> ⚠️ **v1.0 metric removed:** "≤ 30 MB RAM during 10 concurrent 4K yt-dlp streams" was not achievable — each yt-dlp subprocess alone allocates 20–50 MB. The new RAM target (80 MB) covers the Khukri process itself and excludes necessary child processes, which is an honest and auditable claim.

> ⚠️ **v1.0 metric removed:** "< 300 ms first-byte latency on 1 Gbps" is a network metric, not an application metric. Replaced with time-to-first-segment, which measures Khukri's own contribution to latency.

---

## 8. Non-Functional Requirements

| Category | Requirement |
|---|---|
| **Performance** | ≤ 800 ms cold start (Windows); ≤ 80 MB RAM for Khukri process during 10 concurrent downloads |
| **Privacy** | Zero telemetry; zero crash reporting; all state stored locally in SQLite |
| **Security** | Sandboxed native messaging host; signed binaries; reproducible builds via GitHub Actions; SHA-256 verification for all downloaded sidecars |
| **Accessibility** | Full keyboard navigation; Tiger Amber (#FF9F1C) meets WCAG AA on Obsidian (#0B0C10); high-contrast mode toggle |
| **Internationalization** | English first; all UI strings in `i18n/en.json` from day one; RTL-ready layout |
| **Testing** | Unit tests for the download engine (segment math, retry logic); integration tests for the Native Messaging bridge; E2E smoke test via `tauri-driver` on CI |
| **Licensing** | GPLv3. Bundled yt-dlp is Unlicense (compatible). Bundled FFmpeg subset must be GPL-compatible build (no non-free codecs). |

---

## 9. Roadmap — The 5 Sprints

### Sprint 1 — The Core (Download Engine)
**Goal:** A working headless downloader, verifiable by CLI.

**Deliverables:**
- Rust crate: `khukri-engine`
- Segmented download with dynamic thread count (new formula)
- SQLite state persistence (byte-range tracking, pause/resume)
- Atomic pre-allocation on all three target OSes
- Retry logic with exponential back-off

**Definition of Done:** `cargo test` passes; `khukri-engine` can download a 1 GB file to disk at ≥ IDM speed on a 100 Mbps connection in a manual benchmark.

---

### Sprint 2 — The Handshake (Browser Integration)
**Goal:** Browser captures a download and it appears in the engine queue.

**Deliverables:**
- MV3 extension (Chrome/Edge) with `chrome.downloads` interceptor
- Native Messaging host registration flow in Rust
- Native Messaging bridge in Rust
- Content-script HLS/DASH detector
- Floating Blade UI pill

**Definition of Done:** On Windows and Linux development setups, clicking a file in Chrome triggers a download in the engine through the bridge, and the Blade UI appears on YouTube after 1.5 s. Final zero-config install behavior remains deferred until stable packaged extension-ID / `allowed_origins` wiring is completed.

---

### Sprint 3 — The Handle (Tauri GUI)
**Goal:** A usable graphical interface over the engine.

**Deliverables:**
- Tauri 2.0 shell with "All Downloads" list view
- Progress bars, speed readout, ETA
- Settings panel (threads, bandwidth, scheduler, proxy)
- System tray integration
- Dark mode + light mode

**Definition of Done:** Cold-start ≤ 800 ms on Windows. RAM ≤ 80 MB with 5 active downloads. All interactive elements keyboard-accessible.

---

### Sprint 4 — Media Mastery (yt-dlp + FFmpeg)
**Goal:** One-click YouTube/stream download from the browser.

**Deliverables:**
- yt-dlp sidecar bundled and invoked from Rust
- Auto-updater (tagged releases, SHA-256 verification, hot-swap)
- FFmpeg stitching for split audio/video
- Quality selector in Floating Blade UI (best / 1080p / 720p / audio-only)
- Legal/ToS notice in onboarding

**Definition of Done:** User can download a YouTube video in best quality via the Blade UI with no configuration. Auto-updater runs in background without blocking the UI or crashing on network failure.

---

### Sprint 5 — Distribution (CI/CD + Audit)
**Goal:** Signed, reproducible, multi-OS releases on GitHub.

**Deliverables:**
- GitHub Actions: build matrix (Windows x64/ARM64, macOS universal, Linux x64)
- Code-signed binaries (Windows: EV cert; macOS: Developer ID + notarisation)
- Reproducible build verification (hash comparison across two independent build runs)
- SAST scan (e.g., `cargo audit`, `clippy --deny warnings`)
- Installer auto-registers Native Messaging host on all three OSes
- README benchmark table (Khukri vs. IDM)

**Definition of Done:** A tagged release on GitHub produces signed installers for all three OSes via CI. `cargo audit` reports zero known vulnerabilities. Benchmark table shows Khukri ≥ IDM speed.

---

## 10. Glossary

| Term | Meaning |
|---|---|
| **The Steel** | The core Rust download engine (`khukri-engine` crate) |
| **The Sniffer** | The browser extension + content-script detection layer |
| **The Scabbard** | The yt-dlp + FFmpeg sidecar management subsystem |
| **The Sheath** | System-level shell integrations (tray, notifications, file association) |
| **Floating Blade UI** | The pill-shaped overlay that appears on video players in the browser |
| **Native Messaging Bridge** | The Named Pipe / UDS channel between the browser extension and the Rust host process |
| **Tagged Release** | A versioned, checksummed yt-dlp release from its GitHub releases page (not `master` HEAD) |
