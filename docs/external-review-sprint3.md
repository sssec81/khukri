# External Review — Sprint 3

Date: 2026-04-25
Scope: README and project structure review at Sprint 3 near-complete state

## Verification Refresh (2026-04-26)

This review has been refreshed against the current repository state and recent local Ubuntu/WSL verification.

- `Cargo.lock` is present in the repo root.
- `cargo check -p khukri-engine` passed.
- `cargo test -p khukri-engine` passed: 17 unit tests and 6 integration tests.
- `cargo test -p khukri-bridge` passed: 1 integration test.
- `cargo test -p khukri-app` was started but not completed, so desktop-app test status is still not confirmed here.
- Current Linux build warning: unused variable `fd` in `crates/khukri-engine/src/engine/prealloc.rs`.

---

## Overall Assessment

Well-scoped, ambitious project with strong architectural decisions and clear product positioning. Architecture and Rust choices are justified, not cargo-culted. Foundation is solid but not release-ready. The gap between "Sprint 3 near complete" and "shippable product" is real and acknowledged.

**Grade: B+** — Strong foundation, needs hardening and cross-platform validation before public release.

---

## Strengths

### Architecture is Clean and Modular
Crate separation is sensible: `khukri-engine` as a pure library, `khukri-bridge` as a standalone binary, Tauri shell decoupled from core logic, browser extension as a separate artifact. Engine-as-library means CLI, headless daemon, or alternative frontends can be added without rewriting core logic.

### Rust is the Right Choice
Pre-allocation, deterministic segment writes, concurrent file I/O with ownership guarantees, and crash-resilient SQLite state are real problems where Rust provides genuine value. `tokio` + `reqwest` + `rustls` is a modern, secure default stack.

### Product Positioning is Sharp
"Local-first, no account, no phone-home" is a genuine differentiator. The comparison table against traditional download managers is honest and hits real user pain points.

### State Management is Thoughtful
SQLite-backed segment state with deterministic download IDs means resume actually works across crashes. Many download managers get this wrong by storing state in memory or fragile JSON files.

### Browser Integration Architecture is Correct
Manifest V3 + Native Messaging is the only viable approach for modern Chromium. Blade UI and service worker candidate tracking show UX awareness beyond just intercepting all requests.

### Testing Exists and is Documented
The currently verified set is 17 engine unit tests + 6 engine integration tests + 1 bridge protocol test. Covers the right things: SHA-256 segmented downloads, streaming fallback, retry behavior, permanent failures, resume, and progress reporting.

---

## Areas of Concern

### 1. No `Cargo.lock` in Version Control
Status refresh (2026-04-26): this specific concern is now closed because `Cargo.lock` is present in the repository.

The README justifies this by pointing to library-crate status. **This is a mistake.** `khukri-app` is a binary. Binaries need lockfiles for reproducible builds. A future `cargo update` can silently break the build or introduce a vulnerable dependency. Commit `Cargo.lock` before any public release.

**Action:** Commit `Cargo.lock`. Already tracked in Sprint 5 scope but should move earlier.

---

### 2. Windows Runtime Verification Narrowed, But Not Fully Closed
Native Windows verification has now cleared the big first hurdle: `cargo test --workspace` passes on Windows and `cargo tauri dev` launches on a native Windows machine. That materially reduces Sprint 3 risk. The remaining Windows work is narrower and should focus on native messaging registration, tray behavior under real hide/show flows, and one end-to-end browser download handoff.

**Action:** Before Sprint 4, finish one native Windows manual pass covering host registration, extension connection, queue handoff, one successful real download, and a quick cold-start / RAM measurement.

---

### 3. Extension `allowed_origins` Wiring is Deferred — Security Risk
The native messaging bridge is a privileged surface. If `allowed_origins` is too permissive or not pinned to a stable extension ID, any extension can talk to the bridge. Additionally, the extension forwards custom browser headers into the engine — if a malicious page can influence the content script via `postMessage`, it could inject arbitrary headers. The bridge should reject dangerous headers (`Host`, `Content-Length`, `Authorization`) before passing them to the engine.

The `--register` / `--repair` flows should not be triggerable by the extension without explicit user action.

**Action:** Lock down `allowed_origins` to the final stable extension ID before any public release. Add header sanitization in the bridge. Document the threat model.

---

### 4. No CI Yet
The project is at Sprint 3 near-complete with no CI pipeline. Issues found in Sprint 5 CI setup will be expensive to fix retroactively. At minimum needed now:
- `cargo test` on Ubuntu, Windows, macOS
- `cargo clippy --deny warnings`
- `cargo audit` for security advisories
- Tauri build verification on all three platforms

**Action:** Move basic CI (test + clippy + audit) earlier — do not wait for Sprint 5.

---

### 5. yt-dlp + FFmpeg Sidecar Strategy is Under-Defined
Operational complexity of Sprint 4:
- **yt-dlp** updates frequently (sometimes daily). The 24h update check is in the PRD, but what happens when an update breaks the API Khukri calls? Pinned versions go stale; auto-updates add attack surface.
- **FFmpeg** binary distribution across Windows/Linux/macOS with GPL attribution is non-trivial.
- **Binary size**: standalone yt-dlp binary (not Python) is viable, but FFmpeg adds significant size.

**Action:** Write an ADR for the yt-dlp update strategy before Sprint 4 begins. Define: pinned vs auto-update, rollback behavior, and what version is shipped at launch.

---

### 6. Thread Count Formula Upper Bound is Aggressive
`clamp(floor(file_size_mb / 50), 4, 64)` — the 64-thread ceiling is significantly above what IDM (8–16) and aria2c (5 default) use. More segments means more connection overhead, more SQLite contention, more filesystem seek pressure, and higher per-download memory. The formula does not account for connection latency or disk type (HDD vs SSD).

| File size | Current threads | IDM typical |
|---|---|---|
| 200 MB | 4 | 8 |
| 1 GB | 20 | 8–16 |
| 3 GB | 64 | 16 |

**Action (deferred):** Benchmark 16 vs 64 segments on a real consumer connection and HDD. Consider lowering the cap to 16 as a conservative default. Runtime detection of disk type is Sprint 4+ complexity — cap reduction is a one-line change with no downside risk.

---

### 7. Pre-allocation Needs Platform Scrutiny
Platform-specific pre-allocation behavior:
- Linux: `fallocate` / `posix_fallocate` — may fail silently on network drives or FAT32
- Windows: `SetEndOfFile` or `SetFileValidData` — the latter requires `SE_MANAGE_VOLUME_NAME` privilege and can expose uninitialized disk data if misused
- macOS: `fcntl(F_PREALLOCATE)`

**Action:** Verify the current `engine/prealloc.rs` implementation handles failures gracefully (fall through to no pre-allocation rather than hard error). Add an ADR if not already documented.

---

### 8. Frontend Scalability at Large Queue Sizes
Vanilla JS DOM manipulation without a virtual list will degrade with hundreds of downloads. Real-time progress updates (500ms interval) hitting a large DOM list will cause jank before 1.0.

**Action (deferred):** Not urgent for Sprint 3. Consider a lightweight virtual list (no framework required) if queue size regularly exceeds ~100 rows. Revisit before 1.0.

---

## Recommendations by Priority

| Priority | Action |
|---|---|
| 1 | Commit `Cargo.lock` |
| 2 | Verify Windows runtime before Sprint 4 |
| 3 | Lock down extension `allowed_origins` and bridge header sanitization |
| 4 | Set up basic CI (test + clippy + audit) now, not Sprint 5 |
| 5 | Write ADR for yt-dlp update strategy before Sprint 4 |
| 6 | Benchmark and consider lowering thread cap from 64 to 16 |
| 7 | Verify pre-allocation failure handling per platform |

### Recommendation Refresh (2026-04-26)

The original priority table above predates the current repository state. `Cargo.lock` is now present, so that item should be considered closed.

Updated practical order:
- Verify Windows runtime before Sprint 4
- Lock down extension `allowed_origins` and bridge header sanitization
- Set up basic CI (test + clippy + audit) now
- Write ADR for yt-dlp update strategy before Sprint 4
- Benchmark and consider lowering thread cap from 64 to 16
- Verify pre-allocation failure handling per platform
- Clear the current Linux build warning in `engine/prealloc.rs`

---

## Bottom Line

Biggest risks before public release:
- **Windows polish** — where the target users are
- **Security hardening** — native messaging is a privileged surface
- **yt-dlp operational complexity** — hardest long-term maintenance problem

If those three are addressed competently, this is a credible modern open-source download manager.
