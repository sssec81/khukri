# ADR 001: Native Messaging vs WebSockets

- Status: Accepted
- Date: 2026-04-15
- Owner: Khukri maintainer

## Context

Sprint 2 needs a browser-to-Rust bridge for handing off downloads with low overhead and predictable behavior.

Two options were considered:

1. Native Messaging (browser-managed stdio process, length-prefixed JSON).
2. Local WebSocket server (browser extension connects over localhost).

## Decision

Adopt Native Messaging.

## Rationale

WebSockets were rejected because they add extra runtime overhead and a larger security surface:

- Requires a long-lived local server endpoint and port lifecycle management.
- Increases attack surface via local network exposure and origin/protocol validation complexity.
- Adds additional connection and reconnection state that is unnecessary for this use case.

Native Messaging gives a direct, low-latency stdio pipe with browser-managed process lifecycle and simpler trust boundaries.

## Consequences

Positive:

- Lower moving-part count for Sprint 2.
- Lower latency path between extension and bridge.
- No localhost port management.

Trade-offs:

- OS-specific registration and path handling must be robust.
- Installer and repair tooling are mandatory for a good user experience.

## Follow-ups

- Implement self-installer and repair flow (KHU-202).
- Add mock test harness for protocol-level debugging (KHU-206).
- Add bridge stress test: kill browser mid-transfer and verify resumable state.
