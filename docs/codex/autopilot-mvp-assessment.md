# Autopilot MVP scope: outdated and non-MVP crates

## Sources reviewed
- `crates/autopilot/docs/ROADMAP.md`
- `crates/autopilot/docs/MVP.md`
- `crates/autopilot/docs/considerations.md`
- `crates/README.md`
- `Cargo.toml`

## MVP assumption
Based on `crates/autopilot/docs/ROADMAP.md`, the MVP is the Autopilot desktop app
(`crates/autopilot`) acting as a Codex app-server sidecar with M1-M3 scope
(transport, event mapping, approvals), while keeping local session continuity and
the Adjutant loop described in `crates/autopilot/docs/MVP.md`. Marketplace and
web products are out of scope.

## Outdated or superseded for this MVP
- `crates/codex-agent-sdk`: direct SDK streaming path; roadmap shifts to app-server
  JSONL as the primary backend.
- `crates/acp-adapter`: ACP JSON-RPC bridge tied to the SDK path; overlaps the new
  runtime adapter model in the roadmap.
- `crates/autopilot-core`: legacy CLI/tunnel runner built around SDK workflows;
  not part of the app-server UI roadmap.
- `crates/autopilot-service`: service layer for `autopilot-core` snapshots;
  superseded by the app-server runtime adapter + event stream.
- `crates/autopilot-shell`: legacy WGPUI shell (used by `src/bin/autopilot.rs`)
  separate from the current `crates/autopilot` UI target.
- `crates/autopilot-wasm`: replay viewer; aligns with M4+ (history/review), not
  M1-M3.
- `crates/autopilot/docs/ROADMAP-old`: explicitly superseded by ROADMAP.md.

## Not needed for the MVP (out of scope)

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
- `crates/runtime` (needed for marketplace/host flows, not app-server sidecar)
- `crates/agent`, `crates/agent-orchestrator`
- `crates/wallet`, `crates/spark`, `crates/neobank`, `crates/frostr`
- `crates/nostr/core`, `crates/nostr/client`, `crates/nostr/relay`, `crates/relay`

### Alternative inference backends (Codex-only MVP can ignore)
- `crates/gpt-oss`, `crates/gpt-oss-agent`, `crates/gpt-oss-metal`
- `crates/fm-bridge`, `crates/fm-bridge-agent`
- `crates/local-inference`, `crates/ml`, `crates/ml/candle-wgpu`
- `crates/lm-router`, `crates/gateway` (keep only if Adjutant multi-backend stays in scope)
- `crates/rlm`, `crates/frlm` (optional; not required for M1-M3 parity)

### Benchmarks and research tooling
- `crates/bench-harness`, `crates/bench-datasets`, `crates/bench-runner`,
  `crates/rlm-methods`
- `docs/research`, `docs/rlm`, `docs/metrics`, `docs/bazaar`, `docs/blog`,
  `docs/company`, `docs/reports`, `docs/transcripts`

## Notes and risks
- `crates/adjutant`, `crates/dsrs`, `crates/oanix`, `crates/issues`, and
  `crates/wgpui` look MVP-critical because the roadmap and MVP doc keep the
  Autopilot loop + local-first UX.
- `src/bin/autopilot.rs` currently boots `autopilot-shell`. If the MVP centers
  `crates/autopilot`, the bin wiring likely needs cleanup when deprecating the
  legacy shell path.
