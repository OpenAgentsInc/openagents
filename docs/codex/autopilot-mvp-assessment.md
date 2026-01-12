# Autopilot scope update: M1-M7 complete, CM1-3 done

## Sources reviewed
- `crates/autopilot/docs/ROADMAP.md`
- `docs/codex/CODEXMONITOR_PARITY_PLAN.md`
- `crates/autopilot/Cargo.toml`
- `git log --oneline -n 30 -- crates/autopilot docs/codex`

## Status update (post-M7)
Recent commits indicate:
- M1-M7 delivered: app-server transport + event mapping, approvals, sessions + review,
  config/auth/models/MCP/skills, autopilot loop routing, and Codex runtime consolidation
  on the app-server path.
- CodexMonitor parity track has CM1-CM3 delivered (workspace orchestration, layout parity,
  timeline item mapping). CM4-CM6 remain.

Evidence (commit refs):
- M1: `9c8198cab` (app-server transport)
- M2: `cd6d4d694` (event mapping)
- M3: `bf357856d` (approvals)
- M4: `28ee37b89` (sessions + review)
- M5: `292d36d58` (config/auth/models/MCP/skills)
- M6: `80bdda251` (autopilot routing)
- M7: `bc0e35b31` (app-server-only Codex path)
- CM1: `82b68d675`
- CM2: `91ef8b3ce`
- CM3: `11764b7b7`

## Legacy or superseded (not part of the current app-server UI)
- `crates/codex-agent-sdk` was removed; app-server JSONL is the sole Codex path now.
- `crates/acp-adapter`: JSON-RPC bridge tied to the deprecated SDK path.
- `crates/autopilot-core`: legacy CLI/tunnel runner around SDK workflows.
- `crates/autopilot-service`: service layer for `autopilot-core` snapshots.
- `crates/autopilot-shell`: legacy WGPUI shell used by `src/bin/autopilot.rs`,
  separate from `crates/autopilot`.
- `crates/autopilot-wasm`: optional replay viewer; not required for current parity work.
- `crates/autopilot/docs/ROADMAP-old`: superseded by `ROADMAP.md`.

## Still out of scope for current Autopilot UI + CodexMonitor parity

### Other product apps and UIs
- `crates/pylon`, `crates/pylon-desktop`
- `crates/gitafter`
- `crates/onyx`
- `crates/nexus`, `crates/nexus/client`, `crates/nexus/worker`
- `crates/web`, `crates/web/client`, `crates/web/worker`, `crates/web/wallet-worker`
- `crates/orderbook`

### Marketplace, economy, and network stack
- `crates/marketplace`
- `crates/compute`
- `crates/protocol`
- `crates/runtime` (needed for marketplace/host flows, not the app-server UI)
- `crates/agent`, `crates/agent-orchestrator`
- `crates/wallet`, `crates/neobank`, `crates/frostr`
- `crates/nostr/relay`, `crates/relay`, `crates/relay-worker`

### Alternative inference backends (not required for Codex app-server parity)
- `crates/gpt-oss`, `crates/gpt-oss-agent`, `crates/gpt-oss-metal`
- `crates/fm-bridge`, `crates/fm-bridge-agent`
- `crates/local-inference`, `crates/ml`, `crates/ml/candle-wgpu`
- `crates/rlm`, `crates/frlm`

### Benchmarks and research tooling
- `crates/bench-harness`, `crates/bench-datasets`, `crates/bench-runner`,
  `crates/rlm-methods`
- `docs/research`, `docs/rlm`, `docs/metrics`, `docs/bazaar`, `docs/blog`,
  `docs/company`, `docs/reports`, `docs/transcripts`

## Notes and risks
- `crates/adjutant`, `crates/dsrs`, `crates/oanix`, `crates/issues`,
  `crates/wgpui`, `crates/gateway`, `crates/lm-router`, `crates/spark`,
  and `crates/nostr/core` + `crates/nostr/client` remain in-scope for the
  current app-server + Autopilot loop path.
- `src/bin/autopilot.rs` still boots the legacy `autopilot-shell`; packaging should
  align on whether the product entrypoint is `crates/autopilot` or the legacy shell.
