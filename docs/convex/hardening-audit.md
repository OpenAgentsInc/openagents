# Convex Bootstrap & Reliability Audit

Date: 2025-10-29
Owner: OpenAgents (oa-bridge + Expo app)

## Summary

Bootstrapping the local Convex backend from the bridge is fragile. In practice we see:
- Long or indefinite “Connection refused” loops while the backend initializes.
- Health probe timeouts (20s) even though the backend eventually starts.
- Noisy client workers attempting WS sync long before the server is ready.
- Limited diagnostics unless run with a debug flag.

This document diagnoses the issues and proposes a hardening plan to make the Convex path reliable, observable, and non‑blocking for ACP/Codex flows.

## Current Flow (as‑implemented)

- Bridge (oa-bridge) spawns the Convex local backend binary at `~/.openagents/bin/local_backend` with:
  - DB path: `~/.openagents/convex/data.sqlite3`
  - Flags: `--db sqlite --interface 0.0.0.0 --port 7788 --site-proxy-port 7789 --local-storage ~/.openagents/convex/storage --disable-beacon`
- Health probe: GET `http://127.0.0.1:7788/instance_version` every 500ms, max ~20s.
- If healthy: run `bun run convex:dev:once` (one‑shot functions deploy).
- Meanwhile: various parts of the bridge may instantiate `ConvexClient` (WS) to upsert threads/messages.
- App reads Convex via Convex React client once everything is deployed.

References: docs/convex/convex.md, docs/convex/bridge-setup.md, docs/convex/sidecar.md

## Observations from Logs

- The backend starts and prints:
  - “Starting a Convex backend”
  - “Connected to SQLite …”
  - “Searchlight starting …”
  - Later: “Bootstrapping indexes… Loading tables … Bootstrapping table metadata …”
- During this window, health checks to `/instance_version` fail with connection refused.
- The internal Rust `convex` clients (used for upserts) concurrently attempt to connect to `ws://127.0.0.1:7788/api/sync` and error with `ProtocolFailure` repeatedly.
- The health probe times out (40×500ms) before the backend finishes its first‑run bootstrapping.

## Diagnosed Issues

1) Health probe too aggressive and narrow
- Only `/instance_version` is probed; first‑run init often exceeds the 20s window.
- We do not first check for a bound TCP listener; we immediately try HTTP and log “connection refused”.
- We don’t fall back to alternative endpoints (e.g., `/health_check`) if `/instance_version` differs across builds.

2) Eager Convex clients cause noise and backoff storms
- Bridge code attempts to construct/connect `ConvexClient` while the backend is not yet healthy.
- This spams the logs with WS backoffs and can mask real bootstrap problems.

3) Opaque backend start‑up
- Until recently, child stdout/stderr were suppressed; any CLI argument errors or panics were lost.
- We didn’t check for early child exit during the health loop.

4) Interface/URL assumptions not explicit
- We bind to `0.0.0.0` (to support LAN/VPN) but health probes hard‑code loopback. That’s fine in practice, but the code silently restarts when it detects a loopback‑only process; this increases churn during first‑run.

5) Functions deploy is attempted regardless of health state
- If the backend becomes healthy just after our 20s timeout, we skip the deploy and end up with an empty API surface for the app.

## Hardening Plan

A. Robust health & readiness detection
- Step 1: TCP listen probe. Try connecting to `127.0.0.1:port` for up to N seconds with exponential backoff and jitter.
- Step 2: HTTP probe cascade:
  - First: GET `/instance_version` (existing path).
  - Fallback: GET `/health_check` (used by some builds), treat any 2xx as healthy.
- Step 3: Child liveness. During the wait loop, poll child with `try_wait()` and log early exit status.
- Step 4: Adaptive timeout.
  - Increase window to 60–120s on first run (empty DB) and 20–30s on subsequent runs.
  - Persist a marker file (e.g., `~/.openagents/convex/.initialized`) after the first successful health check to switch profiles.

B. Start backend stdout/stderr in structured “debug” mode
- KEEP default quiet mode (suppressed IO) for normal runs.
- ENABLE stdout/stderr passthrough with `OPENAGENTS_CONVEX_DEBUG=1` to bubble logs.
- Always log the exact child argv and DB path; log metadata for the binary (path, size, exec bit).

C. Gate internal Convex clients on readiness
- Add a `convex_ready: AtomicBool` (or shared state) set by `ensure_convex_running()`.
- Defer constructing `ConvexClient` until ready; or wrap client construction in a helper that no‑ops and logs a single line when not ready.
- Quiet the WS worker backoff loop until ready.

D. Resilience and recovery
- If health probe times out but child still runs:
  - Continue app/bridge operation in “degraded” mode (ACP rendering continues; Convex optional paths disabled).
  - Keep retrying health in the background (long backoff) and fire an event when healthy to run the one‑shot functions deploy.
- On port conflict:
  - Detect listeners via `lsof` (Unix) or a TCP bind attempt. If busy, either kill or pick the next available port (and advertise it to the app via WS status).

E. Correctness checks and defaults
- Confirm CLI flags against the actual `local_backend` usage. If flags change upstream, fail fast with a helpful message.
- Consider defaulting to loopback (`127.0.0.1`) for the backend and exposing to LAN/VPN only when explicitly configured.
- Verify health endpoint compatibility across our pinned backend build.

F. Developer ergonomics
- Add a `cargo bridge --convex-log` (or env var) to enable debug logs without editing env each time.
- Add a `bridge.convex_status` WS control that returns: bin path + metadata, argv, health state, last error, and recent child stderr tail.
- Improve app status bar: separate “WS connected” from “Convex ready” and show a concise state timeline.

## Concrete Changes to Implement

- bootstrap.rs
  - [x] Child exit polling during health loop; log early exit.
  - [x] Optional stdout/stderr passthrough (`OPENAGENTS_CONVEX_DEBUG=1`).
  - [ ] TCP listen probe (before HTTP).
  - [ ] Health cascade: probe `/instance_version` then `/health_check`.
  - [ ] Adaptive timeout: 60–120s on first run; lower for subsequent runs.
  - [ ] Persist `.initialized` marker on first success.

- main.rs / ws.rs
  - [ ] Add `convex_ready` flag in `AppState`; only instantiate/use `ConvexClient` once ready.
  - [ ] Soft‑disable FS→Convex sync until ready; queue or log pending ops instead of spamming errors.
  - [ ] Background retry for delayed health + run `convex:dev:once` when healthy.

- providers/ws.tsx (app)
  - [x] Log inbound `bridge.acp` and `bridge.*` lines for observability.
  - [ ] Add a lightweight `/convex` status in header with clear healthy/degraded states.

- Docs
  - [ ] Update docs/convex/convex.md to reflect fallback endpoint and adaptive timeouts.
  - [ ] Add a Troubleshooting section for first‑run (indexing) latency.

## Risk Assessment

- Increasing the timeout without a fallback can hide real failures. We mitigate with child exit polling and optional stdout/stderr.
- Delaying internal clients until ready reduces early mirror writes but avoids noisy backoffs and race conditions.
- Background retries + degraded mode ensures ACP/Codex UX remains responsive even when Convex is slow.

## Open Questions

- Should we vendor a specific Convex backend build and pin flags/endpoints to eliminate drift?
- Do we need the `site-proxy-port` in our self‑hosted case? If not, remove it to simplify.
- Would a small `oa-convex` wrapper (submodule) give us a more stable CLI contract than supervising `local_backend` directly?

## Immediate Action Items (P0)

1) Implement TCP probe + HTTP health cascade (+ adaptive timeouts) in bootstrap.
2) Gate internal Convex client construction on a `convex_ready` flag.
3) Background health retry and delayed functions deploy.
4) Keep ACP path fully functional when Convex is unavailable.
5) Ship improved logs by default (including the child exit diagnostics we added).

## Appendix: Why `/instance_version` may fail

- Some local backend builds respond with `unknown` (200 OK). Others may serve liveness at `/health_check`.
- First‑run index bootstrapping can take >30s on large/slow disks; during that window, the HTTP listener may not be ready.
- Using TCP probe + longer adaptive timeout makes this robust without hiding real startup failures.
