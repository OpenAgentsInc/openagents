# A2A Bridge Crate and Module Breakdown

**Status:** implementation planning handoff  
**Authorities:** `docs/A2A_INTEROP_PROFILE.md`, `docs/plans/a2a-sovereign-agent-integration-plan.md`, `docs/PROTOCOL_SURFACE.md`, `docs/OWNERSHIP.md`

## V1 invariants

- A2A is the wire/discovery/orchestration surface.
- NIP-SKL, NIP-SA, and NIP-AC remain canonical state.
- `Task.id == SA session d-tag`.
- `saSessionRef = 39230:<agent_hex_pubkey>:<task_id>`.
- `agentProfileRef = 39200:<agent_hex_pubkey>:`.
- Initial funding handoff is by-reference only through `osceEnvelopeRef`.
- Generic A2A clients must still function if they ignore OpenAgents extension params.
- Only material lifecycle/funding/artifact transitions become `39231`; do not mirror every stream chunk.

## Ownership boundaries

### `crates/a2a-bridge`
Owns A2A types, HTTP handlers, AgentCard projection, auth bridge, task runtime facade, funding gate, DVM projection, and bridge-local errors.

### `crates/nostr/core`
Owns reusable SKL/SA/AC/NIP-90 semantics and typed helpers for manifests, profiles, trajectory, envelopes, spends, settlements, and data-vending events.

### `crates/nostr/client`
Owns relay publish/subscribe/wait helpers used by the bridge state store.

### `apps/autopilot-desktop`
Owns hosting the first local A2A bridge, configuration, provider-lane integration, and optional headless serve mode.

### `apps/nexus-relay`
Remains relay infrastructure only; it must not own A2A HTTP surface logic.

## New crate and host files

- `crates/a2a-bridge/Cargo.toml`
- `crates/a2a-bridge/src/lib.rs`
- `crates/a2a-bridge/src/types.rs`
- `crates/a2a-bridge/src/errors.rs`
- `crates/a2a-bridge/src/agent_card.rs`
- `crates/a2a-bridge/src/auth.rs`
- `crates/a2a-bridge/src/tasks.rs`
- `crates/a2a-bridge/src/funding.rs`
- `crates/a2a-bridge/src/dvm.rs`
- `crates/a2a-bridge/src/artifacts.rs`
- `crates/a2a-bridge/src/http.rs`
- `apps/autopilot-desktop/src/a2a_server.rs`
- `apps/autopilot-desktop/src/lib.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

## Core traits

### `SovereignStateStore`
Relay-backed reads/writes for SA/SKL/AC/NIP-90 state. Reuse `crates/nostr/core/src/nip_sa/*`, `nip_skl/*`, `nip_ac/*`, `nip90/data_vending.rs`, plus `crates/nostr/client`.

Required methods:
- `get_agent_profile`
- `get_skill_manifests`
- `get_skill_manifest_by_ref`
- `get_credit_envelope`
- `publish_trajectory_session`
- `publish_trajectory_event`
- `publish_auth_challenge`
- `await_auth_response`
- `publish_credit_spend_auth`
- `publish_credit_receipt`
- `publish_credit_cancel`

### `A2aTaskRuntime`
Owns in-memory task/session index, ordered subscribers, and A2A lifecycle projection.

Required methods:
- `create_task`
- `send_task_message`
- `stream_task`
- `get_task`
- `list_tasks`
- `cancel_task`

### `FundingPolicy`
Classifies requests, validates by-reference AC envelopes, and decides when guardian approval is required.

### `DvmRouter`
Routes long-running or billable work into NIP-90 and projects feedback/results back into A2A updates and artifacts.

## Module breakdown

| Module | Owns | Reuses | Acceptance focus |
| --- | --- | --- | --- |
| `types.rs` | A2A DTOs, auth payloads, constants | A2A wire shape | stable camelCase JSON |
| `errors.rs` | bridge-local error taxonomy | `thiserror` | clean 401/403/404/422 mapping |
| `agent_card.rs` | SA+SKL -> AgentCard projection | `nip_sa/profile.rs`, `nip_skl/manifest.rs` | valid generic A2A card |
| `auth.rs` | SKL challenge bridge + bearer sessions | `nip_skl`, relay wait helpers | verified principal bound to pubkey |
| `tasks.rs` | task store, session projection, SSE | `nip_sa/trajectory.rs` | `Task.id == session d-tag` |
| `funding.rs` | envelope validation + receipt/cancel projection | `nip_ac/*` | reject invalid funding before work |
| `dvm.rs` | NIP-90 request/feedback/result bridge | `nip90/data_vending.rs` | `7000` -> status, `6000-6999` -> artifacts |
| `artifacts.rs` | artifact shaping + sovereign refs | task/DVM outputs | final outputs carry audit refs |
| `http.rs` | axum routes, auth extraction, SSE framing | all bridge modules | routes match AgentCard |
| `lib.rs` | public exports and bootstrap helpers | all bridge modules | reusable crate surface |

## Endpoint ownership

- `GET /.well-known/agent-card.json` -> `agent_card.rs`, `http.rs`
- `POST /auth/nostr/challenge` -> `auth.rs`, `http.rs`
- `POST /auth/nostr/complete` -> `auth.rs`, `http.rs`
- `POST /message:send` -> `tasks.rs`, `funding.rs`, `dvm.rs`, `http.rs`
- `POST /message:stream` -> `tasks.rs`, `dvm.rs`, `http.rs`
- `GET /tasks/:id` -> `tasks.rs`, `http.rs`
- `GET /tasks` -> `tasks.rs`, `http.rs`
- `POST /tasks/:id:cancel` -> `tasks.rs`, `funding.rs`, `http.rs`
- `POST /tasks/:id:subscribe` -> `tasks.rs`, `http.rs`

## Required tests

New tests:
- `crates/a2a-bridge/tests/types_roundtrip.rs`
- `crates/a2a-bridge/tests/agent_card_contract.rs`
- `crates/a2a-bridge/tests/auth_bridge.rs`
- `crates/a2a-bridge/tests/task_projection.rs`
- `crates/a2a-bridge/tests/funding_policy.rs`
- `crates/a2a-bridge/tests/funded_dvm_flow.rs`

Existing tests to extend:
- `crates/nostr/core/tests/nip_sa_e2e.rs`
- `crates/nostr/core/tests/nip_sa_skl_ac_integration_matrix.rs`
- `crates/nostr/core/tests/nip_ac_skill_linkage.rs`
- `crates/nostr/core/tests/nip90_integration.rs`

## Delivery order

1. crate skeleton and shared types
2. AgentCard projection
3. discovery-only desktop mount
4. auth bridge
5. task/session projection
6. funding gate
7. NIP-90 routing
8. full desktop serve mode + docs