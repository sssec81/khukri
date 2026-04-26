# Khukri — Code Review Fix Tracker

> Review date: 2026-04-26  
> Source: structured codebase review (B+ grade)

Legend: ✅ Fixed · ❌ Not Fixed · ⚠️ Partial

---

## Priority Fixes

| # | Issue | Status | File | Notes |
|---|-------|--------|------|-------|
| 1 | Commit `Cargo.lock` | ❌ Not Fixed | `.gitignore:3` | `Cargo.lock` is explicitly gitignored — must be committed for binary crates to ensure reproducible builds |
| 2 | `allowed_root` canonicalize on non-existent parent | ✅ Fixed | `crates/khukri-engine/src/config.rs` | Added `canonicalize_with_nonexistent_tail` helper that walks up to the deepest existing ancestor before canonicalizing |
| 3 | Symlink traversal test | ✅ Fixed | `crates/khukri-engine/src/config.rs` | Added `test_validate_rejects_symlink_escape` (gated `#[cfg(unix)]`) and `test_validate_accepts_nonexistent_output_file_inside_root` |
| 4 | Bridge SQLite WAL + busy timeout | ✅ Fixed | `crates/khukri-bridge/src/main.rs` | Added `PRAGMA journal_mode = WAL` and `PRAGMA busy_timeout = 5000` after pool connect |
| 5 | Replace `wait_for_download_snapshot` polling | ✅ Fixed | `src-tauri/src/main.rs` | Replaced 80×100 ms polling loop with a single DB read; call site already has a synthesized fallback for `None` |
| 6 | Scope extension `host_permissions` | ✅ Fixed | `extension/manifest.json`, `extension/service-worker.js` | Moved `<all_urls>` to `optional_host_permissions`; added `scripting` permission; content scripts now registered dynamically via `chrome.scripting`; permission requested on action click (user gesture required by Chrome) |
| 7 | Content script isolation (`world: ISOLATED`) | ✅ Fixed | `extension/content-script-main.js` (new), `extension/content-script.js` | Fetch/XHR patching moved to new `content-script-main.js` (world: MAIN); `content-script.js` is now a pure ISOLATED relay that listens to `window.postMessage` and forwards to `chrome.runtime.sendMessage` |
| 8 | `innerHTML` in `blade-ui.js` | ✅ Fixed | `extension/blade-ui.js` | Replaced with `document.createElement` chain; SVG icons still set via `innerHTML` (hardcoded, no user data) |
| 9 | Bridge missing WAL mode (duplicate of #4) | ✅ Fixed | `crates/khukri-bridge/src/main.rs` | Covered by fix #4 |

---

## Other Review Concerns (non-priority)

| Area | Issue | Status | Notes |
|------|-------|--------|-------|
| Engine | Throttle mutex contention (64 segments) | ❌ Open | Replace `Arc<Mutex<TokenBucket>>` with atomic token bucket or periodic-refill task |
| Bridge | Threading model (mixed std thread + Tokio) | ❌ Open | Consider `task::spawn_blocking` for stdin to simplify; not a bug but adds complexity |
| Bridge | Filename collision in `~/Downloads` | ❌ Open | Two URLs mapping to same filename will overwrite — append short hash or counter |
| Bridge | Race on port drop during browser download cancel | ❌ Open | If port drops between `isBridgeConnected()` check and `sendToNative`, download is lost with no retry |
| Tauri | `pause_all_downloads` DB update not in transaction | ❌ Open | Download transitioning `active` between cancel and DB update could be missed |
| Tauri | Proxy credentials stored in plain-text settings | ❌ Open | Document; consider user warning |
| Tauri | `DownloadConfig::validate()` not called on all paths | ❌ Open | Audit all `DownloadConfig` construction sites |
| DB | No unique constraint on `(url, file_path)` | ❌ Open | Same-path re-add silently resets progress via `ON CONFLICT(id)` upsert |
| Extension | `<all_urls>` host permissions justification | ❌ Open | Linked to #6 above |
| Testing | No Tauri UI / E2E tests | ❌ Open | Add before Sprint 5 distribution |
| Testing | No extension unit tests | ❌ Open | Add `jest` + `webextensions-api-mock` harness |

---

## Security Summary (from review)

| Area | Grade | Status |
|------|-------|--------|
| Path traversal | Good | Partial — symlink edge case open (#2, #3) |
| Header injection | Good | ✅ |
| Extension origin validation | Good | ✅ |
| Proxy credentials | Acceptable | ❌ Undocumented |
| Native Messaging framing | Good | ✅ |
| Content script isolation | Weak | ❌ Main-world patching (#7) |
| TLS (rustls, no native fallback) | Good | ✅ |

---

## Fix Log

| Date | # | Action |
|------|---|--------|
| 2026-04-26 | 6 | `<all_urls>` moved to `optional_host_permissions`; dynamic content script registration; permission requested on action click |
| 2026-04-26 | 7 | Created `content-script-main.js` (MAIN world); `content-script.js` rewritten as isolated-world postMessage relay |
| 2026-04-26 | 2, 3 | `canonicalize_with_nonexistent_tail` helper + two new tests in `config.rs` |
| 2026-04-26 | 4, 9 | Bridge `make_pool` now sets WAL + 5 s busy timeout |
| 2026-04-26 | 5 | `wait_for_download_snapshot` reduced from 80-iteration polling to single DB read |
| 2026-04-26 | 8 | `blade-ui.js` pill built with `createElement`; no more `innerHTML` for structure |
