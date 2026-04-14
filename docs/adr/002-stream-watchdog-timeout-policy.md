# ADR 002: Stream Watchdog Timeout Policy

- Status: Accepted (temporary)
- Date: 2026-04-15
- Owner: Khukri maintainer

## Context

The engine now wraps response stream reads with a watchdog timeout to prevent stalled transfers from hanging forever.

A policy decision is needed: fixed timeout now vs fully configurable timeout in user-facing settings.

## Decision

Use a fixed 30-second stream watchdog timeout in Sprint 1/2, and defer user-configurable tuning.

## Rationale

- Immediate safety: stale connections fail fast instead of blocking queue progress.
- Simplicity: one known timeout avoids premature config complexity while bridge and UI are still being built.
- Debuggability: fixed behavior makes early integration failures easier to reproduce.

## Consequences

Positive:

- Segment and streaming tasks no longer hang indefinitely on stalled reads.
- Queue throughput is protected from dead sessions.

Trade-offs:

- 30s may be too strict for some unstable/very slow links.
- No user override yet for environment-specific tuning.

## Trigger to Revisit

Revisit this ADR when any of the following is true:

1. Repeated field reports show false timeouts on healthy slow networks.
2. Bridge-level telemetry (local only, no telemetry upload) indicates frequent watchdog aborts.
3. Sprint 3 settings UI is ready to expose networking profiles safely.

## Planned Follow-up

- Add configurable network profile in settings (conservative/default/aggressive).
- Keep a safe minimum bound and validate values.
- Add integration tests for timeout behavior under controlled delayed streams.
