# 🗡️ Khukri

> High-performance, open-source download manager. Built with Rust + Tauri 2.0.  
> Zero telemetry. Zero bloat. Zero compromises.

---

## Status

| Sprint | Deliverable | Status |
|---|---|---|
| 1 — The Steel | Download engine (segmenting, SQLite, retry, queue, throttle) | ✅ Complete |
| 2 — The Sniffer | Browser extension + Native Messaging bridge | 🔜 |
| 3 — The Handle | Tauri GUI | 🔜 |
| 4 — The Scabbard | yt-dlp + FFmpeg integration | 🔜 |
| 5 — Distribution | CI/CD, code signing, reproducible builds | 🔜 |

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust (backend), TypeScript (extension + UI) |
| Framework | Tauri 2.0 |
| Async | Tokio |
| HTTP | Reqwest (rustls-tls, HTTP/2) |
| Database | SQLite via sqlx |
| Extension | Manifest V3 (Chromium) |

---

## Architecture

```
khukri/
├── crates/
│   └── khukri-engine/     # Core download engine (Rust library)
│       ├── src/
│       │   ├── engine/    # Segmenting, download, retry, throttle, queue
│       │   ├── db/        # SQLite persistence
│       │   ├── config.rs
│       │   └── error.rs
│       ├── migrations/    # sqlx migrations
│       └── examples/
│           └── download.rs  # CLI smoke-test
├── src-tauri/             # Tauri backend (Sprint 3)
├── src/                   # Frontend UI (Sprint 3)
├── extension/             # MV3 Chrome extension (Sprint 2)
├── sidecar/               # yt-dlp + FFmpeg binaries (Sprint 4)
├── i18n/
│   └── en.json            # All UI strings
└── docs/
    ├── khukri-prd.md      # Product requirements (LOCKED v1.1)
    └── khukri-jira-tickets.md
```

---

## Getting Started

### Prerequisites

- Rust 1.75+ (`rustup`)
- WSL2 or Linux / macOS for development

### Build

```bash
cargo build -p khukri-engine
```

### Test

```bash
cargo test -p khukri-engine
```

### CLI smoke-test

```bash
# Streaming download (no Content-Length)
cargo run --example download -- "https://speed.cloudflare.com/__down?bytes=10485760" /tmp/test.bin

# Segmented download (parallel, with range support)
cargo run --example download -- https://proof.ovh.net/files/10Mb.dat /tmp/test.bin

# With speed cap (500 KB/s)
cargo run --example download -- https://proof.ovh.net/files/10Mb.dat /tmp/test.bin 512000
```

---

## Key Design Decisions

- **Thread count formula:** `clamp(floor(file_size_MB / 50), 4, 64)`
- **TLS:** rustls (pure Rust — no system OpenSSL dependency)
- **Pause/resume:** byte-range state persisted in SQLite; resume skips completed segments
- **Throttling:** shared token bucket across all segments enforces per-download rate cap
- **Pre-allocation:** `fallocate` (Linux) / `SetEndOfFile` (Windows) before any writes

---

## Notes

- `Cargo.lock` is excluded from git (correct for a library crate). When the Tauri binary is added in Sprint 3, `Cargo.lock` will be committed since application binaries should lock their dependencies.

---

## License

GPLv3. See [LICENSE](LICENSE).

Bundled yt-dlp (Sprint 4): Unlicense.  
Bundled FFmpeg (Sprint 4): GPL-compatible build only (no non-free codecs).
