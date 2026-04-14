# Integration Hardening Notes

These notes capture likely integration-tier failure modes as Khukri moves from a standalone engine to Browser -> Bridge -> Engine -> UI.

## Potential Integration Hurdles

### 1. The Zombies Problem (Process Management)

Risk:
- In Sprint 2, the Native Messaging bridge is spawned by Chrome.
- If Chrome is force-closed or the bridge crashes, downloads can keep running unexpectedly and hold file or SQLite resources.

Mitigation:
- Add a heartbeat between bridge and engine.
- If IPC is severed for more than N seconds, cancel download tasks and persist paused state.
- Add an integration test that kills the parent process mid-download and validates resume readiness.

### 2. The Authentication Wall (Session Hand-off)

Risk:
- A URL that works in browser can fail in engine with 403 if request context is incomplete.
- Cookies, User-Agent, and Referer often must match browser state.

Mitigation:
- Bridge must pass a structured header set (CustomHeaders) to the engine.
- Preserve origin and referer context in bridge payloads.
- Add negative tests for 403 fallback and explicit surfaced errors.

Note:
- Some sites also enforce IP pinning or TLS fingerprint checks; full browser parity is not always possible with reqwest alone.

### 3. SQLite Locked Errors (Multi-writer pressure)

Risk:
- Bridge and UI can issue writes while engine is persisting segment progress.
- SQLite supports one writer at a time; busy errors can appear under contention.

Mitigation:
- Enable WAL mode at DB initialization.
- Set a busy timeout and keep transactions short.
- Prefer a single write path for high-frequency progress writes.

### 4. Native Messaging Path Friction (Windows/Linux)

Risk:
- Registration paths are absolute. If binaries move, host registration breaks silently.

Mitigation:
- KHU-202 self-installer must support register/repair flows.
- Recompute absolute path on each repair run and rewrite host manifest/registry key.

## Technical Blind Spots to Monitor

| Component | Potential Issue | Why It Matters |
|---|---|---|
| Throttling | Buffer bloat at very low caps | Small caps can increase latency and destabilize long transfers |
| Disk I/O | SQLite write amplification | Frequent tiny DB updates increase write pressure and overhead |
| yt-dlp | Version drift | Site extractor changes can break media flow quickly |

## Recommended Bridge Stress Test

Before shipping extension UX, run a bridge-first stress scenario:

1. Trigger a multi-GB download through the Native Messaging bridge.
2. Kill Chrome/bridge mid-transfer.
3. Verify engine transitions to paused/failed state predictably.
4. Relaunch bridge and confirm resume only fetches incomplete segments.

Passing this test early reduces uncertainty before Sprint 3 UI integration.
