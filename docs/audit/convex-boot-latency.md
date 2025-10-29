# Convex Boot Latency Audit (why ~30–40s before “ready”)

Date: 2025-10-29
Owner: OpenAgents (oa-bridge + Expo app)

## Summary

When running `cargo bridge`, the bridge supervises a local Convex backend. On some machines it takes ~30–40 seconds before the backend responds healthy and functions deploy can run. The delay is almost entirely in the Convex backend process becoming ready; the one‑shot functions deploy (`convex dev --once`) typically completes in <1 second once the backend is healthy.

This doc explains the startup sequence, what the logs mean, likely causes for the delay, how to observe it, and concrete mitigations.

## What you see

From a fresh `cargo bridge` run:

```
INFO oa_bridge::bootstrap: convex.ensure: begin port=7788 interface=0.0.0.0
INFO oa_bridge::bootstrap: convex.ensure: starting local backend ...
INFO oa_bridge::bootstrap: convex.ensure: waiting for health attempt=11 url=http://127.0.0.1:7788 first_run=false
...
INFO oa_bridge::bootstrap: convex.ensure: waiting for health attempt=81 url=http://127.0.0.1:7788 first_run=false
INFO oa_bridge::bootstrap: convex.ensure: healthy after start url=http://127.0.0.1:7788 pid=...
INFO oa_bridge: convex health watcher: ready url=http://127.0.0.1:7788
INFO oa_bridge::bootstrap: convex dev:once spawned pid=...
$ bash ./scripts/convex-cli.sh dev:once
✔ Convex functions ready! (687.72ms)
```

Each “attempt” happens every ~500ms and logs every 10 attempts. Attempt ~81 ≈ ~40.5s of waiting.

## Startup sequence (current implementation)

Reference code:
- Bridge bootstrapper: `crates/oa-bridge/src/bootstrap.rs`
- Health watcher + bootstrap: `crates/oa-bridge/src/main.rs`
- Docs: `docs/convex/convex.md`, `docs/convex/bridge-setup.md`, `docs/convex/hardening-audit.md`

What happens:
- Bridge spawns `~/.openagents/bin/local_backend` with SQLite DB at `~/.openagents/convex/data.sqlite3`, binding to `OPENAGENTS_CONVEX_INTERFACE` (default `0.0.0.0`) and port `7788`.
- The bridge polls readiness with an adaptive loop:
  - TCP probe → HTTP GET `/instance_version` (fallback `/health_check`) every 500ms.
  - First‑run timeout up to 120s; subsequent runs up to 60s (marker: `~/.openagents/convex/.initialized`).
- Once healthy, a background task marks `convex_ready=true` and runs a one‑shot functions deploy (`bun run convex:dev:once`).

## Why the backend takes ~30–40s

The health wait reflects the Convex backend’s internal initialization, not the bridge. Common contributors:
- Database initialization and migration
  - Open SQLite DB, validate schema/metadata, run any startup migrations.
- Search/index services warm‑up
  - Text search and (optionally) vector index subsystems initialize on startup. If any indexes exist, loading/mmap/warm‑up can take noticeable time proportional to index size.
- Table/catalog bootstrap
  - Load table metadata, constraints, and caches before serving.
- First request readiness
  - The HTTP readiness endpoints are only served after the runtime settles; until then, TCP may accept but `/instance_version` won’t return 2xx.

Notes from this repo’s logs and code:
- `first_run=false` indicates the `.initialized` marker exists, so the longer delay is not a first‑ever boot. It is consistent with warm‑up dominated by index/catalog work on an existing DB.
- The functions deploy itself is fast (<1s) and not the bottleneck.

## Observability (how to see what’s happening)

- Enable backend logs during bootstrap:
  - `OPENAGENTS_CONVEX_DEBUG=1 cargo bridge`
  - This passes through the backend’s stdout/stderr so you can see DB open, search/index boot, and any warnings.
- Confirm health probe behavior:
  - The bridge probes TCP, then `/instance_version` with a fallback to `/health_check` and logs every 10 tries.
- Check DB size and storage:
  - DB path: `~/.openagents/convex/data.sqlite3`
  - Storage path: `~/.openagents/convex/storage`
  - Large files or slow disks can amplify warm‑up.

## Mitigations and options

Short‑term (no code changes):
- Keep the backend running across sessions
  - Start Convex once and reuse it: `OPENAGENTS_MANAGE_CONVEX=0 cargo bridge` (bridge won’t stop/start Convex). Run the backend separately to avoid per‑session warm‑up costs.
- Restrict to loopback if you don’t need LAN access
  - `OPENAGENTS_CONVEX_INTERFACE=127.0.0.1 cargo bridge`
  - Binding to loopback avoids any LAN/VPN exposure; impact on warm‑up is typically negligible, but it simplifies networking.
- Trim DB/indexes if experimenting
  - If the DB has grown large during experiments, consider archiving `~/.openagents/convex/` and letting it rebuild. Only do this if you can afford to lose local Convex data; Codex JSONLs remain intact and can be mirrored again.

Bridge/runtime improvements (tracked in codebase):
- Adaptive health and better readiness checks
  - Already implemented: TCP probe + `/instance_version` fallback + longer initial timeout. Marker file reduces future timeouts.
- Defer internal Convex clients until ready
  - Implemented via `convex_ready` gating to avoid noisy WS backoffs before health.
- Background health retry + delayed deploy
  - If the initial window were to time out, continue in “degraded” mode and bootstrap once healthy later.

Potential upstream/structural improvements:
- Back‑compat “early‑ready” endpoint in the backend so clients can connect sooner while heavy features (e.g., search) finish warming.
- Configurable startup of optional subsystems (e.g., delaying vector/search init until first use) for local dev modes.

## Answers to “why 30 seconds?” in this context

- The log shows ~81 half‑second attempts → ~40s. That time is spent inside the Convex backend until it serves readiness.
- It is not the functions deploy or bridge; those execute after health and complete in <1s.
- The most likely causes for a non‑first‑run 30–40s delay are index/catalog warm‑up on an existing DB and general runtime initialization. Enabling `OPENAGENTS_CONVEX_DEBUG=1` will confirm the precise step on your machine.

## Recommendations

- Day‑to‑day dev: start Convex once and reuse it across `cargo bridge` runs; set `OPENAGENTS_MANAGE_CONVEX=0` to have the bridge leave it alone.
- If you need the bridge to manage Convex:
  - Use loopback bind unless you actively need LAN access.
  - Set `OPENAGENTS_CONVEX_DEBUG=1` temporarily to verify where the time is spent.
  - If warm‑up consistently exceeds a minute, inspect DB size under `~/.openagents/convex/` and consider pruning archives.
- Track improvements in `docs/convex/hardening-audit.md`; the bootstrapper already includes longer timeouts, TCP probing, and readiness gating.

## References

- Code: `crates/oa-bridge/src/bootstrap.rs`, `crates/oa-bridge/src/main.rs`
- Docs: `docs/convex/convex.md`, `docs/convex/bridge-setup.md`, `docs/convex/hardening-audit.md`
- CLI wrapper: `scripts/convex-cli.sh`

