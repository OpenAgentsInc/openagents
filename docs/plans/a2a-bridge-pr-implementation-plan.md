# A2A Bridge PR Implementation Plan

**PR title:** A2A bridge: project sovereign SA/SKL/AC state into A2A discovery, auth, task, and funding flows

## Goal
Add a bridge layer that exposes OpenAgents sovereign agents over standard A2A while keeping Nostr canonical for identity, skills, lifecycle, and funding.

## Architectural rules
- A2A is the public wire protocol and discovery surface.
- NIP-SKL owns skill identity, trust, and optional auth challenge.
- NIP-SA owns profile, session, trajectory, guardianship, and delegation semantics.
- NIP-AC owns credit, spend, settlement, default, and cancel-window semantics.
- The bridge only projects sovereign state into A2A; it must not create a second authority model.

## V1 implementation locks
- `Task.id == SA session d-tag`
- `saSessionRef = 39230:<agent_hex_pubkey>:<task_id>`
- `agentProfileRef = 39200:<agent_hex_pubkey>:`
- request funding starts by-reference only via `osceEnvelopeRef`
- generic A2A clients still work when they ignore OpenAgents metadata and extension params

## Component boundaries

### `crates/a2a-bridge`
Create a new workspace crate for bridge-local types, router construction, auth bridge, task projection, funding gate, DVM routing, artifact projection, and error mapping.

### `crates/nostr/core`
Reuse existing protocol modules instead of reimplementing protocol logic:
- `crates/nostr/core/src/nip_sa/profile.rs`
- `crates/nostr/core/src/nip_sa/trajectory.rs`
- `crates/nostr/core/src/nip_skl/manifest.rs`
- `crates/nostr/core/src/nip_skl/discovery.rs`
- `crates/nostr/core/src/nip_ac/envelope.rs`
- `crates/nostr/core/src/nip_ac/spend.rs`
- `crates/nostr/core/src/nip_ac/settlement.rs`
- `crates/nostr/core/src/nip90/data_vending.rs`

### `apps/autopilot-desktop`
Host the initial bridge surface and wire it into existing runtime and NIP-90 flows:
- `apps/autopilot-desktop/src/a2a_server.rs`
- `apps/autopilot-desktop/src/lib.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`

### Nexus integration
Use `crates/nostr/client` against configured relays, including Nexus relay where appropriate. `apps/nexus-relay` remains relay infrastructure only and does not host the A2A surface.

## New files
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

## Core contracts
- `SovereignStateStore`: relay-backed SA/SKL/AC/NIP-90 reads and writes
- `A2aTaskRuntime`: task lifecycle, task index, and subscriber streaming
- `FundingPolicy`: request classification and envelope validation
- `DvmRouter`: NIP-90 start/watch/finalize bridge

## HTTP surface
Discovery:
- `GET /.well-known/agent-card.json`

Auth:
- `POST /auth/nostr/challenge`
- `POST /auth/nostr/complete`

A2A core:
- `POST /message:send`
- `POST /message:stream`
- `GET /tasks/:id`
- `GET /tasks`
- `POST /tasks/:id:cancel`
- `POST /tasks/:id:subscribe`

## Metadata contract
Client-provided:
- `osceEnvelopeRef`
- `preferredNostrRelays`
- `requestedProfileUris`

Server-emitted:
- `saSessionId`
- `saSessionRef`
- `sklSkillRefsUsed`
- `guardianApprovalRefs`
- `osceEnvelopeRef`
- `acReceiptRefs`
- `trajectoryEventRefs`

## Implementation sequence
1. Add workspace crate and shared A2A bridge types.
2. Project SA profile + SKL manifests into `AgentCard`.
3. Implement SKL challenge-backed auth and verified principal extraction.
4. Map A2A tasks onto SA sessions and `39231` milestones.
5. Add by-reference AC funding validation and settlement projection.
6. Route long-running/billable work through NIP-90.
7. Mount the bridge router in Autopilot Desktop and add headless serve mode.
8. Update docs and rollout notes.

## Testing requirements
New tests:
- `crates/a2a-bridge/tests/types_roundtrip.rs`
- `crates/a2a-bridge/tests/agent_card_contract.rs`
- `crates/a2a-bridge/tests/auth_bridge.rs`
- `crates/a2a-bridge/tests/task_projection.rs`
- `crates/a2a-bridge/tests/funding_policy.rs`
- `crates/a2a-bridge/tests/funded_dvm_flow.rs`

Extend existing tests:
- `crates/nostr/core/tests/nip_sa_e2e.rs`
- `crates/nostr/core/tests/nip_sa_skl_ac_integration_matrix.rs`
- `crates/nostr/core/tests/nip_ac_skill_linkage.rs`
- `crates/nostr/core/tests/nip90_integration.rs`

## Acceptance criteria
- `AgentCard` is generated from sovereign state, not a parallel config store.
- Auth binds A2A access to a verified Nostr key using the SKL challenge profile.
- Every bridge task is losslessly correlated to an SA session and trajectory chain.
- Funded execution validates envelopes before compute begins and exposes settlement refs back through A2A metadata.
- Long-running work can be surfaced to generic A2A clients without requiring them to understand raw NIP-90 events.

