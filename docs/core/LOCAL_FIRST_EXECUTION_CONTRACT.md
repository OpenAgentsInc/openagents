# Local-First Execution Contract

Status: active (2026-02-25)

This contract defines how Autopilot execution routes work on user machines.

## Execution Lanes

1. `local_codex` (authoritative default)
2. `shared_runtime` (optional fallback through authenticated control/runtime APIs)
3. `swarm` (optional fallback through NIP-90 network execution)

Fallback order is explicit and policy-driven:

- `local_only`
- `local_then_runtime`
- `local_then_runtime_then_swarm`

Policy is resolved from:

- `OPENAGENTS_EXECUTION_FALLBACK_ORDER`

Default policy is `local_then_runtime_then_swarm`.

Implementation guardrail:

- Lane ordering is centralized in `crates/openagents-client-core/src/execution.rs` via
  `execution_lane_order(...)` and is always `local_codex` first for every policy.

## Runtime Endpoint Resolution (Portable, Env-Driven)

Runtime sync/control base URL resolution is centralized in
`crates/openagents-client-core/src/execution.rs` and used by desktop.

Resolution precedence:

1. `OPENAGENTS_RUNTIME_SYNC_BASE_URL`
2. `OPENAGENTS_RUNTIME_BASE_URL`
3. `OPENAGENTS_CONTROL_BASE_URL` / `OPENAGENTS_AUTH_BASE_URL`
4. persisted runtime-auth base URL
5. local default `http://127.0.0.1:8787`

No production base URL is hardcoded in execution-critical runtime client paths.

## Desktop Behavior

For `UserAction::Message`:

1. Attempt local Codex `turn/start`.
2. If local execution fails and runtime fallback is enabled, submit a control request to:
   - `POST /api/runtime/codex/workers/:worker_id/requests`
   - method: `turn/start`
3. If runtime fallback fails and swarm fallback is enabled, submit a NIP-90 text-generation job.
4. If all enabled lanes fail, emit structured error with lane failure details.

This preserves local-first operation while allowing optional remote/network continuity.

Remote control safety:

1. Runtime sync control requests (`thread/start`, `thread/resume`, `turn/start`, `turn/interrupt`)
   execute through local `AppServerClient` first-party calls on desktop.
2. Remote control dispatch telemetry labels lane as `local_codex` to make precedence auditable.

Regression evidence:

- `cargo test -p openagents-client-core execution_lane_order_keeps_local_codex_first_for_all_policies -- --nocapture`
- `cargo test -p openagents-client-core execution_lane_order_matches_policy_shape -- --nocapture`
