# Sprint 2 Status

Date: 2026-04-16
Scope: EPIC-02 - The Sniffer

## Board

| Ticket | Title | Status | Notes |
|---|---|---|---|
| KHU-201 | MV3 extension scaffold + downloads interceptor | Done | MV3 manifest, browser download interception, and long-lived native port flow are implemented in `extension/`. |
| KHU-202 | Native host self-installer | Done, with follow-up | `khukri-bridge` supports `--register` and `--repair` with absolute-path manifest generation. Stable extension-ID / `allowed_origins` wiring is intentionally deferred for now. |
| KHU-203 | Rust Native Messaging bridge | Done | Native framing, stderr-only logging, custom-header handoff, engine integration, and progress forwarding are implemented. |
| KHU-204 | HLS/DASH stream detector | Done | MV3 service worker observes stream patterns via `webRequest`, content script provides blob/video fallback, and worker logic now remembers the best stream candidate instead of auto-queuing every match. |
| KHU-205 | Floating Blade UI pill | Done | Pill appears after 1.5s, anchors near the player like IDM, dismisses per-origin during a session, and queues downloads through the service worker. |
| KHU-206 | Native Messaging mock test suite | Done | A bridge protocol integration test was added and passes under `cargo test --workspace`. |

## Verification

- `cargo test --workspace` passed after the Sprint 2 fixes:
- `khukri-bridge`: 1 protocol integration test passed
- `khukri-engine`: 18 unit tests passed
- `khukri-engine`: 6 integration tests passed

## Reviewed State

- The Rust and extension code paths for Sprint 2 are implemented and test-verified.
- Stream auto-queuing was reduced so the service worker now remembers the best candidate and lets Blade-triggered queueing drive the actual handoff.
- The Blade pill is currently tuned for development QA: dismissal state is reset on extension install/startup so reloads do not permanently hide the UI while testing.
- Temporary bridge debug breadcrumbs used during test bring-up have been removed.
- Known deferred item: native host `allowed_origins` still needs stable extension-ID wiring before packaging/distribution.
