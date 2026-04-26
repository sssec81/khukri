# Code-Level Review — Sprint 3

Date: 2026-04-25
Scope: Full codebase inspection at Sprint 3 near-complete state

## Verification Refresh (2026-04-26)

The notes below were re-checked against the current repository state and recent local test output on Ubuntu/WSL.

- `Cargo.lock` is now present at the repo root, so the earlier lockfile concern no longer applies.
- `cargo check -p khukri-engine` passed.
- `cargo test -p khukri-engine` passed: 17 unit tests and 6 integration tests.
- `cargo test -p khukri-bridge` passed: 1 bridge integration test.
- Current Linux build warning: unused variable `fd` in `crates/khukri-engine/src/engine/prealloc.rs`.
- `cargo test -p khukri-app` was not completed in this verification pass, so Tauri app test coverage remains unverified here.

## Security & Correctness Fixes Applied (2026-04-26)

All implementable items from the Priority Refresh were addressed in one pass. See `sprint-3-status.md` for the full table.

| Item | Status |
|---|---|
| `allowed_origins` placeholder | **Fixed** — `validate_extension_origin()` fails registration when placeholder/invalid |
| Bridge header forwarding | **Fixed** — hop-by-hop and credential headers stripped in `browser_headers()` |
| `max_threads_by_size` no-op | **Fixed** — min-segment-size cap (1 MiB) enforced in `resolved_thread_count()` |
| Resume formula compatibility | **Fixed** — `SEGMENT_FORMULA_VERSION` stored in DB; mismatch forces fresh segmentation |
| macOS host registration | **Fixed** — writes manifest to `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` |
| Unused `fd` warning | **Fixed** — dead `let fd` line removed from `prealloc.rs` |
| Missing tests | **Added** — `resolved_thread_count`, `can_reuse_segments`, `browser_headers`, `validate_extension_origin`, `sanitize_filename`, `filename_from_url`, and three `preallocate` scenarios |

---

## Overall Grade: B+

| Category | Grade | Notes |
|---|---|---|
| Architecture | A | Clean separation, good abstractions |
| Code Quality | B+ | Rust idioms, good error handling, minor issues |
| Security | B | Path traversal protected, headers and origins need hardening |
| Testing | B- | Good coverage of core logic, missing edge cases |
| Documentation | A | Excellent README, ADRs, sprint tracking |
| Browser Integration | B+ | MV3 done right, permissions are broad |
| Cross-Platform | B | Linux solid, Windows unverified, macOS gaps |
| Release Readiness | C+ | Sprint 3 near complete — Windows verification and polish needed |

---

## Architecture & Crate Structure

The workspace separation is correct and well-executed:

| Crate | Role | Assessment |
|---|---|---|
| `khukri-engine` | Pure download library | Excellent — no Tauri, no browser, just HTTP + SQLite + filesystem |
| `khukri-bridge` | Native Messaging host | Good — handles framing, registration, and engine handoff |
| `khukri-app` (Tauri) | Desktop shell | Solid — queue orchestration, tray, settings, scheduler |

The engine is truly decoupled. Both the Tauri app and bridge depend on it as a library. CLI or headless daemon can be added later without rewriting core logic.

---

## Engine Deep Dive

### Download Orchestration (`engine/download.rs`)

**What's correct:**
- HEAD probe first to check `Content-Length` and `Accept-Ranges` before committing to segmented mode
- Range-unaware server fallback: if `Accept-Ranges` is missing, falls back to single-thread streaming
- 200 vs 206 detection: if server ignores `Range` and returns 200, engine returns `NoRangeSupport` rather than writing at the wrong offset and corrupting the file
- Streaming fallback for no `Content-Length` (live streams, chunked responses)
- Resume: compares existing segment rows in SQLite against newly calculated segments; if they match, resumes; if not, wipes and restarts
- Fail-fast: uses a `CancellationToken` to abort all other segments immediately when one fails
- Pre-allocation before any segment task starts
- Progress tracking via atomics + watch channel — no mutex contention on progress updates from concurrent segment tasks

**Issue — `max_threads_by_size` is a no-op:**
```rust
let max_threads_by_size = total_bytes.min(64) as u8;
```
For any file over 64 bytes, `total_bytes.min(64)` = 64. This is effectively always 64 and provides no size-based capping. The intent appears to be enforcing a minimum segment size, but as written it does nothing. Should be something like `total_bytes / (5 * 1024 * 1024)` to enforce at least 5MB per segment.

**Issue — `can_reuse_segments` is version-brittle:**
If the thread count formula changes between engine versions, all existing partial downloads will fail the segment match and restart from zero with no warning to the user. A version or formula-hash field in the download row would allow detecting this and informing the user.

### Segment Sizing (`engine/segment.rs`)

`clamp(floor(file_size_mb / 50), 4, 64)` — for a 10MB file, this returns 4 threads with ~2.5MB each. The HTTP overhead (TCP handshake, TLS, headers) for 4 parallel connections to download 2.5MB each may be slower than 1 connection on high-latency links. A minimum segment size threshold (e.g., 5MB per segment) would be safer than a minimum thread count.

### Retry Logic (`engine/retry.rs`)

Permanent failures (400, 401, 403, 404, 405, 410) are correctly not retried. Exponential backoff with ±10% jitter is correct.

`rand::rng()` creates a new RNG on every backoff call. Inefficient and may have weak statistical properties for jitter. A thread-local or shared RNG would be better.

### Throttling (`engine/throttle.rs`)

Token bucket with 1-second burst capacity. Slices large requests into ≤1-second chunks. `bytes_per_sec = 0` means unlimited.

The bucket is `Arc<tokio::sync::Mutex<TokenBucket>>` — 64 segment tasks contending on one mutex every chunk write. Under high throughput (NVMe, gigabit), this mutex could become a bottleneck. A sharded or atomic token approach would scale better.

### Pre-allocation (`engine/prealloc.rs`)

- **Linux**: `fallocate(2)` via `nix` in `spawn_blocking`. Handles `EOPNOTSUPP` gracefully. Correct.
- **Windows**: `set_len` → `SetEndOfFile`. Does not physically allocate blocks on NTFS. `SetFileValidData` would give true pre-allocation but requires `SE_MANAGE_VOLUME_NAME` privilege. Current approach is acceptable but suboptimal.
- **macOS**: Explicit stub. `fcntl(F_PREALLOCATE)` is TODO. Acceptable for now.

### Database Layer (`db/mod.rs`)

`upsert_download` uses `ON CONFLICT DO UPDATE` but does not update `status`. If a download is retried with the same URL and path, the old status persists until explicitly overwritten. `failure_reason = NULL` on conflict is correct.

Transaction safety for segment insertion is correct — all-or-nothing via `tx.commit()`.

---

## Native Messaging Bridge

### Message Framing

4-byte little-endian length prefix + JSON body. Correct Chrome NM protocol. Critically, all logs go to stderr — no stdout pollution that would corrupt the framing.

### Registration

Uses `HKCU` registry key on Windows (no admin needed). Generates JSON programmatically with no hand-escaped paths. Linux path is standard. **macOS registration is not implemented** — returns an error.

### Security — `allowed_origins` Placeholder

```rust
fn extension_origin_from_env() -> String {
    std::env::var("KHUKRI_EXTENSION_ORIGIN")
        .unwrap_or_else(|_| "chrome-extension://replace-with-your-extension-id/".to_string())
}
```

**Release blocker.** The placeholder will never match a real extension. Bridge will be broken out of the box until `KHUKRI_EXTENSION_ORIGIN` is set or the ID is hardcoded to the stable published extension ID.

### Header Forwarding

Bridge adds `Referer` from page URL if not already present. Custom headers are forwarded from the extension to the engine without sanitization. Dangerous headers (`Host`, `Content-Length`, `Connection`) should be filtered before passing to `reqwest`.

### Bridge Integration Test

The test in `crates/khukri-bridge/tests/native_protocol.rs` is comprehensive: spawns an axum HTTP server with range support, spawns the bridge binary, sends a framed `queue_download` message, reads progress events until complete, verifies SHA-256 and file size. Validates the entire bridge → engine → HTTP → filesystem pipeline in one test.

---

## Tauri Desktop App (`src-tauri/src/main.rs`)

### Queue Orchestration

Priority-first scheduling with FIFO tiebreaker. Respects `max_concurrent`. Checks scheduler window before promoting. Scheduler correctly handles overnight windows (e.g., 23:00 to 06:00).

### Download Lifecycle

Progress events are throttled to 500ms intervals but emitted immediately on terminal states (Complete, Failed, Paused). Correct.

### Startup Recovery

```rust
sqlx::query("UPDATE downloads SET status = 'paused' WHERE status = 'active'")
```

Downloads marked `active` from a previous crash are reset to `paused` at startup so they can be resumed. Correct.

### Pause/Resume/Cancel

`pause_download` on a completed or failed download will set its status to `paused` in the DB — no guard against this. Not a data corruption issue but a UI state concern.

`remove_download` requires manual cancel before remove. Safe but a UX friction point — auto-cancel-then-remove would be cleaner.

### Busy-Wait on Download Start

```rust
for _ in 0..80 {
    let snapshot = refresh_download_snapshot(pool, id).await?;
    if snapshot.is_some() { return Ok(snapshot); }
    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

8-second polling loop waiting for the DB row to appear after starting a download. A channel-based notification would be cleaner.

### Tray

Menu items are created once and never updated — `Pause All` / `Resume All` do not reflect dynamic state. Acknowledged in sprint-3-status.md.

### Database Bootstrap

WAL mode and foreign keys enabled. `max_connections = 5` for the Tauri app, `1` for the bridge. Prevents database locked errors under concurrent access.

---

## Browser Extension

### Permissions

`host_permissions: ["<all_urls>"]` with content scripts on `<all_urls>` and `nativeMessaging` is a powerful combination. `allowed_origins` in the bridge manifest must be locked to the stable extension ID before public release to prevent other extensions from connecting to the bridge.

### Service Worker

Intercepts browser downloads, cancels them, and hands off to Khukri via Native Messaging. Long-lived `connectNative()` port keeps the service worker alive. Stream candidate tracking scores candidates per tab and queues the best one on Blade click.

### Content Script

Monkey-patches `window.fetch` and `XMLHttpRequest.prototype.open` on every page to detect stream URLs. May conflict with sites that have CSP or integrity checks, or with other extensions that patch the same APIs.

### Blade UI

Glassmorphism pill injected near video players. Has `role="button"`, `tabindex="0"`, `aria-label`, and keyboard handlers for Enter, Space, Escape. Re-injects on YouTube SPA navigation. Dismissal persists per-origin via `chrome.storage.local`.

---

## Security Summary

| Vector | Status | Notes |
|---|---|---|
| Path traversal | Mitigated | `allowed_root` canonicalization in `config.rs` |
| Header injection | **Fixed** | `browser_headers()` strips `Host`, `Content-Length`, `Connection`, `Authorization`, and all hop-by-hop headers |
| Native messaging origin | **Fixed** | `validate_extension_origin()` rejects placeholder and non-extension schemes; registration fails fast |
| macOS host registration | **Fixed** | Writes manifest to Chrome NativeMessagingHosts path on macOS |
| Extension permissions | Broad | `<all_urls>` content script + nativeMessaging — acceptable for a download manager |
| SQLite injection | Safe | Parameterized queries via `sqlx` throughout |
| Stdout pollution | Safe | Bridge logs to stderr only |
| URL scheme validation | Safe | `reqwest` rejects `file://` — `DownloadConfig::validate()` only checks non-empty |

---

## Testing

| File | Type | Covers |
|---|---|---|
| `engine/segment.rs` tests | Unit | Thread count formula, segment building |
| `engine/retry.rs` tests | Unit | Retry logic, permanent errors, cancellation |
| `engine/throttle.rs` tests | Unit | Token bucket behavior |
| `config.rs` tests | Unit | Validation, path traversal |
| `tests/integration.rs` | Integration | SHA-256 download, streaming fallback, retry, permanent failure, resume, progress |
| `native_protocol.rs` | Integration | Full bridge → engine → HTTP → filesystem |

Verified on 2026-04-26:
- `khukri-engine`: 17 unit tests + 6 integration tests passing
- `khukri-bridge`: 1 integration test passing

New tests added 2026-04-26:
- `engine/download.rs` — `resolved_thread_count` (5 cases), `can_reuse_segments` (4 cases)
- `engine/prealloc.rs` — `preallocate` success, zero-byte, read-only file returns `DiskSpaceError`
- `bridge/main.rs` — `browser_headers` blocked headers stripped, Referer injection, Referer not duplicated; `validate_extension_origin` placeholder/valid/invalid-scheme; `sanitize_filename` path traversal, reserved chars, empty; `filename_from_url` query-strip, trailing slash

Still missing (deferred to Sprint 5):
- Scheduler window boundary conditions
- Settings persistence round-trip
- Concurrent download contention at `max_concurrent`

---

## Priority Refresh (2026-04-26)

The original priority table below predates the current repository state. In particular, `Cargo.lock` is now present, so it should no longer be treated as the top open action item.

Updated practical order (items marked ✅ are resolved):

- ✅ Lock down `allowed_origins`
- ✅ Filter dangerous headers in the bridge
- ✅ Add macOS native host registration
- ✅ Fix `max_threads_by_size`
- ✅ Add engine version/formula compatibility checks for resume
- ✅ Clear the Linux warning in `engine/prealloc.rs`
- Windows runtime verification
- Set up CI (`cargo test`, `cargo clippy -- -D warnings`, `cargo audit`, Tauri build)
- Benchmark thread count against lower caps on consumer connections
- Write ADR for yt-dlp update strategy

---

## Priority Action Items

| Priority | Item | Status |
|---|---|---|
| 1 | Commit `Cargo.lock` — binary crate needs it for reproducible builds | Done |
| 2 | Windows runtime verification — #1 blocker for IDM comparison | Open |
| 3 | Lock down `allowed_origins` — validate at registration time | ✅ Fixed |
| 4 | Filter dangerous headers in bridge (`Host`, `Content-Length`, `Connection`, `Authorization`) | ✅ Fixed |
| 5 | Add macOS native host registration | ✅ Fixed |
| 6 | Set up CI (`cargo test`, `cargo clippy -- -D warnings`, `cargo audit`, Tauri build) | Sprint 5 |
| 7 | Fix `max_threads_by_size` — enforce 1 MiB min segment size | ✅ Fixed |
| 8 | Add engine version/formula compatibility check for resume | ✅ Fixed |
| 9 | Benchmark 64-thread default against 8–16 threads on typical consumer connections | Open |
| 10 | Write ADR for yt-dlp update strategy before Sprint 4 begins | ✅ docs/adr-001-ytdlp-update-strategy.md |

---

## Bottom Line

The code matches the README's promises. Architecture is sound, Rust usage is idiomatic, and the critical paths (segmented download, resume, browser handoff, bridge protocol) are implemented correctly. The gap to a shippable product is Windows validation, CI/CD, security hardening, and release packaging — not fundamental architecture problems.

The honest sprint documentation is itself a signal. Projects that catalog their own limitations clearly are more likely to ship reliable software.
