# Local vs Runtime/Nexus/Swarm Execution Audit

Status: draft-audit snapshot
Date: 2026-02-25
Owner: repo audit (Codex)

## Question this answers

Given the current codebase, what already runs on a user machine, what is currently coupled to shared OpenAgents runtime/control, and what should be placed in:

1. local machine execution,
2. OpenAgents shared runtime/control authority,
3. Nexus coordination fabric,
4. swarm-dispatched external compute.

## Method

This audit is code-first and contract-first (code wins if docs conflict). It was produced by traversing active app/crate surfaces and inspecting the concrete call paths, route ownership, and execution boundaries.

Primary evidence surfaces:

- `apps/autopilot-desktop/src/main.rs`
- `apps/autopilot-desktop/src/runtime_auth.rs`
- `crates/codex-client/src/client.rs`
- `crates/autopilot/src/app/codex_runtime.rs`
- `crates/autopilot-core/src/agent.rs`
- `crates/autopilot-core/src/verification.rs`
- `crates/openagents-app-state/src/command_bus.rs`
- `apps/openagents.com/service/src/lib.rs`
- `apps/openagents.com/service/src/config.rs`
- `apps/openagents.com/service/src/route_split.rs`
- `apps/openagents.com/service/src/runtime_routing.rs`
- `crates/openagents-runtime-client/src/lib.rs`
- `apps/runtime/src/server.rs`
- `apps/runtime/src/marketplace.rs`
- `apps/runtime/src/config.rs`
- `apps/runtime/src/bridge.rs`
- `crates/pylon/src/config.rs`
- `crates/pylon/src/cli/compute.rs`
- `crates/dsrs/src/adapter/swarm_dispatch.rs`
- `crates/dsrs/src/core/lm/pylon.rs`
- `crates/compute/src/services/relay_service.rs`
- `docs/core/ARCHITECTURE.md`

Heuristic code-scan signals (not semantic proof, but useful pressure indicators):

- `local_command_invocations` in selected local client crates: `27`
- desktop references to runtime/khala sync APIs: `10`
- runtime internal routes in `apps/runtime/src/server.rs`: `57`
- Nexus/swarm references in Rust code: `62`
- runtime-related API references in Rust code (`/api/runtime`, `/internal/v1`, sync/khala): `492`

## Executive Summary

OpenAgents already has substantial local execution capability, but key product lanes remain tightly coupled to shared control/runtime APIs.

What is already local-capable:

1. Desktop UI and local process/tool execution.
2. Local Codex app-server spawning and IPC (`stdin/stdout`) via `codex-client`.
3. Local verification loops (git/cargo/sqlite checks), local inference backends, and local Pylon daemon modes.
4. Runtime service itself can run locally (default bind `127.0.0.1:4100`) for local deployments.

Where coupling is strongest today:

1. Desktop auth/session and runtime sync depend on control API (`/api/auth/*`, `/api/khala/token`, `/api/runtime/*`, `/sync/socket/websocket`).
2. Control service still hosts several "runtime" lanes in-process/in-memory (`/api/runtime/codex/workers*`, `/api/runtime/threads*`, `/api/runtime/tools/execute`, `/api/runtime/skills*`) while other runtime worker lanes proxy to runtime internal APIs.
3. NIP-90/Nexus defaults are broadly embedded across Pylon/DSRS/compute paths.

Recommended target split:

1. Local machine: UI, user-owned session orchestration, local Codex/tool runs, local model inference, local provider participation.
2. OpenAgents shared runtime/control: canonical multi-device authority, identity/session, replay/sync auth, receipts/treasury/credit/liquidity policy.
3. Nexus: coordination and discovery fabric (relay + metadata + routing signals), not canonical authority.
4. Swarm providers: paid/parallelizable compute execution lanes, never domain authority source of truth.

## Current Placement Matrix (Observed)

| Surface | Current behavior | Runtime/control dependency | Recommended primary placement |
|---|---|---|---|
| `apps/autopilot-desktop` | Native app runs local UI logic, local file/tool actions, local Codex integration | Uses control/runtime APIs for auth, worker sync, khala token/websocket, and runtime worker ops | Local-first execution with optional sync mirror |
| `crates/codex-client` + `crates/autopilot/src/app/codex_runtime.rs` | Spawns local `codex-app-server` process and speaks JSON-RPC over stdio | None required for local spawn path | Local machine |
| `crates/autopilot-core` | Local verification and orchestration (git/cargo/sqlite/pylon checks) | Optional external provider/network use depending on mode | Local machine |
| `apps/openagents.com/service` | Control-plane auth/session/sync token issuance; download distribution; many runtime-facing API routes | Calls runtime internal APIs for worker lanes; also hosts several in-memory runtime-like lanes | Shared control authority only (and thin runtime proxy where needed) |
| `apps/runtime` | Runtime internal authority APIs, worker registry, marketplace dispatch, hydra/credit/liquidity/treasury/verifications | May dispatch to provider `base_url`s and optional Nostr bridge relays | Shared runtime authority |
| `apps/lightning-wallet-executor` | Rust HTTP signing/execution service (mock or Spark) | Depends on Spark/secret backends when enabled | Shared custody service (can run local in dev) |
| `apps/lightning-ops` | Rust ops CLI/service with mock/API modes | API mode depends on control/gateway endpoints | Shared ops service (local CLI allowed) |
| `crates/pylon` | Local daemon host/provider modes + local bridge + relay connectivity | Default relays include Nexus; optional cloud APIs | Local provider/host agent runtime |
| `crates/dsrs` swarm adapters | Can dispatch jobs via NIP-90 (DvmClient) or local/hybrid LM modes | Swarm mode depends on relays/providers | Local by default; swarm optional for expensive work |
| `crates/local-inference`, `crates/gpt-oss`, `crates/lm-router` | Local model/backends abstraction with localhost defaults | Optional remote backends where configured | Local machine |
| `crates/openagents-app-state` | Maps intents to control/runtime HTTP commands | Hardcoded routing to `/api/runtime/codex/workers/*` for turn requests | Keep local state; abstract remote adapter behind transport boundary |

## What Should Run Locally (Target)

The user machine should be authoritative for user-owned immediate execution state, then optionally mirror to shared runtime.

Run local by default:

1. Conversation turn orchestration and local Codex app-server calls.
2. Repository/workspace operations and local tool execution.
3. Local model inference selection (`ollama`, `llama.cpp`, `apple_fm`, local gateways).
4. Pylon host/provider local participation.
5. Local replay spool/queue for offline operation and delayed sync.

Push to shared runtime asynchronously:

1. Worker lifecycle snapshots/events.
2. Thread projection updates when multi-device continuity is needed.
3. Receipts/telemetry required for shared policy or billing.

## What Should Stay in OpenAgents Shared Runtime/Control

Keep centralized authority for cross-device/cross-tenant correctness:

1. Identity/session/device revocation (`/api/auth/*`) and token minting (`/api/sync/token`, `/api/khala/token`).
2. Canonical runtime event store and replay ordering (`(topic, seq)` invariants).
3. Treasury/credit/liquidity policy and settlement receipts.
4. Risk and verification adjudication that affects shared trust/budget state.
5. Org-level permissions and policy enforcement.

## What Should Be in Nexus vs Swarm

Nexus (coordination fabric):

1. Relay connectivity and discovery metadata.
2. High-throughput event distribution and interop bridge lanes.
3. Marketplace signaling, routing hints, and provider ads.

Swarm providers (execution fabric):

1. Sandbox-run and heavy compute jobs that are parallelizable or price-sensitive.
2. Model execution beyond local machine limits.
3. Specialized providers exposed through NIP-90 contracts.

Non-negotiable:

1. Neither Nexus nor swarm providers should become canonical authority for control/runtime domain state.
2. Authority writes stay in authenticated HTTP authority lanes (control/runtime), with Khala/Nexus as delivery/coordination.

## Potential Simplifications for the Two-Sided Marketplace Focus

To match the near-term product goal, the platform can be simplified to one clear loop:

1. user runs Autopilot agent,
2. user can sell compute supply into the network via NIP-90 provider mode,
3. that same Autopilot can consume compute either locally or from the wider network.

Simplified profile for the next versions:

1. Treat NIP-90 as the single marketplace execution lane for externalized compute.
2. Keep local-first Autopilot execution as default, with explicit opt-in to remote/swarm dispatch.
3. Make local Codex control explicit: when `codex-app-server` is present locally, Autopilot should fully command it (turn execution, tool invocation, repo operations) as the primary path.
4. Keep control/runtime surfaces minimal and focused on auth/session, sync/replay, and canonical receipts.
5. Keep Nexus as relay/coordination fabric only, not a second authority plane.

What can wait for later versions:

1. Expanded multi-driver runtime routing complexity not needed for the two-sided marketplace MVP.
2. Broad surface area under `/api/runtime/*` that does not directly support:
   - local Autopilot execution,
   - NIP-90 provider enrollment/advertisement,
   - NIP-90 compute consumption and settlement.
3. Non-essential operator/admin workflows that are orthogonal to buy/sell compute and core agent operation.

Hydra and Aegis compatibility posture under this simplification:

1. Keep `docs/plans/hydra-liquidity-engine.md` and `docs/plans/aegis.md` as forward-compatible program tracks.
2. In the simplified near-term shape:
   - Hydra remains the economic settlement/policy substrate for compute pricing, liquidity, and receipts.
   - Aegis remains the verification/underwriting substrate for trust, risk, and claims on higher-assurance lanes.
3. Defer broad Aegis/Hydra feature expansion until core two-sided marketplace flow is stable, but do not break their contract lanes, schema namespaces, or receipt model.

Practical guardrail for implementation during this simplified phase:

1. Any new feature should be prioritized only if it directly improves one of:
   - provider-side compute selling via NIP-90,
   - local Autopilot-to-Codex command/control quality when Codex is available on-device,
   - agent-side compute buying/consumption (local and network),
   - economic/verifiability integrity required for that loop (Hydra/Aegis compatible).

## Local Codex Control Requirement

For this product phase, local Codex command/control should be treated as a first-class requirement:

1. If `codex-app-server` is installed and healthy on the user machine, Autopilot should prefer local Codex by default and fully orchestrate it.
2. "Fully orchestrate" means Autopilot owns the local session loop: prompt/turn dispatch, tool-use requests, workspace actions, and response streaming/collection.
3. Shared runtime is optional for this lane and should be used for sync, remote compute augmentation, and shared policy/receipt needs, not as a prerequisite for local Codex operation.
4. If local Codex is unavailable or unhealthy, Autopilot should degrade gracefully to configured remote/network compute paths without changing shared authority boundaries.

## Notable Gaps and Inconsistencies

1. Mixed ownership under `/api/runtime/*` in control service:
   - `runtime_workers_*` routes use `RuntimeInternalClient` and runtime internal HTTP APIs.
   - `runtime_codex_workers_*`, `runtime_threads_*`, `runtime_tools_execute`, and runtime skill registry paths are currently handled in control-service in-memory/domain-store code paths.
   - This split obscures where runtime authority truly lives.

2. Runtime routing model still carries `Legacy`/`Elixir` driver nomenclature in `apps/openagents.com/service/src/runtime_routing.rs`, which is inconsistent with current Rust-only direction.

3. Local clients still hardcode centralized API assumptions in places:
   - `crates/autopilot/src/app/spark_wallet.rs` uses `https://openagents.com/api` constant.

4. Swarm discovery is incomplete in some local lanes:
   - `crates/autopilot-core/src/pylon_integration.rs` discovery helpers are stubs returning empty provider sets.

5. Aegis appears as architecture/plan authority but has limited explicit runtime namespace manifestation in active code (`aegis` strings are predominantly doc/plan references).

## Recommended Cleanup/Convergence Plan

### Phase A: Boundary hardening

1. Declare one canonical owner for each `/api/runtime/*` capability and remove mixed in-memory/proxy ambiguity.
2. Rename runtime routing drivers away from legacy-era labels to current Rust-era semantics.

### Phase B: Local-first execution lane

1. Add a local authoritative session lane for desktop/web-shell command execution.
2. Persist local replayable event queue and make remote sync optional (eventual consistency).
3. Keep remote mirror strictly as synchronization/observability path, not required for single-device execution.

### Phase C: Nexus/swarm discipline

1. Explicitly tag features as `local_only`, `shared_runtime_required`, `swarm_optional`, or `swarm_required`.
2. Route heavy compute to swarm only when policy/cost/performance thresholds justify remote dispatch.
3. Keep fallback to local execution for degraded-network operation where feasible.

### Phase D: Policy and custody clarity

1. Keep custody/signing in wallet-executor boundary.
2. Keep treasury/credit/liquidity/verification decisions centralized when they impact shared money/trust state.
3. Ensure local lanes emit deterministic receipts when mirrored to shared runtime.

## Practical Placement Rules for New Features

Use this decision order:

1. If functionality mutates shared canonical state across users/devices, place in shared control/runtime.
2. If functionality is user-private, workspace-local, and latency-sensitive, place local-first.
3. If work is expensive/parallelizable and not authority-critical, make swarm-capable with local fallback.
4. Use Nexus for discovery/distribution, never as authority write path.

## Immediate Action Items (from this audit)

1. Normalize `/api/runtime/*` ownership map in docs and code comments (explicit per-route owner).
2. Create one "local execution first" adapter boundary for desktop/web-shell command lanes.
3. Remove legacy runtime driver naming in control service routing model.
4. Replace hardcoded OpenAgents API constants in local clients with config/env-driven endpoints.
5. Implement real swarm provider discovery in Pylon/autopilot-core integration.

## Appendix: High-Signal Evidence References

Local execution evidence:

1. `crates/codex-client/src/client.rs` (`AppServerClient::spawn`, executable discovery helpers)
2. `crates/autopilot/src/app/codex_runtime.rs` (local codex app-server runtime wrapper)
3. `crates/autopilot-core/src/verification.rs` (local command-based verification checks)
4. `crates/pylon/src/cli/compute.rs` (local backend detection on localhost endpoints)

Shared runtime/control dependency evidence:

1. `apps/autopilot-desktop/src/runtime_auth.rs` (`/api/auth/email`, `/api/auth/verify`)
2. `apps/autopilot-desktop/src/main.rs` (`/api/runtime/*`, `/api/khala/token`, `/sync/socket/websocket`)
3. `apps/openagents.com/service/src/lib.rs` (route registration + runtime worker/thread/tool handlers)
4. `crates/openagents-runtime-client/src/lib.rs` (`/internal/v1/workers*`, marketplace/credit/hydra paths)
5. `apps/runtime/src/server.rs` (`/internal/v1/*` authority route surface)

Nexus/swarm evidence:

1. `crates/autopilot/src/app/nexus.rs` (`https://nexus.openagents.com/api/stats`)
2. `crates/autopilot/src/app/nip90.rs` (`wss://nexus.openagents.com/`)
3. `crates/pylon/src/config.rs` (default relay list includes Nexus)
4. `crates/dsrs/src/adapter/swarm_dispatch.rs` and `crates/dsrs/src/core/lm/pylon.rs` (DvmClient/NIP-90 dispatch modes)
5. `apps/runtime/src/marketplace.rs` + `apps/runtime/src/server.rs` (provider selection/dispatch to provider base URLs)
