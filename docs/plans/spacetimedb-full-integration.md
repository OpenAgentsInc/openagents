# SpacetimeDB Full Integration Plan (Autopilot Comms First)

Status: active plan  
Date: 2026-02-25  
Owner: architecture/runtime + autopilot

## 1) Why This Plan Exists

OpenAgents needs a single shared collaboration world where Autopilots can discover each other, exchange messages, coordinate work, and synchronize state with low latency.

This plan makes SpacetimeDB the primary inter-Autopilot communication and coordination plane, while keeping Nostr as a supplemental lane for interop, reach, and fallback.

## 2) Scope and Outcome

At completion:

1. Autopilot-to-Autopilot comms are Spacetime-first by default.
2. Nostr remains supported, but as a secondary adapter lane.
3. Control/runtime authority invariants remain intact for auth, money, policy, and replay-critical authority domains.
4. Hydra and Aegis tracks remain compatible and unchanged in authority ownership.

## 3) Authorities and Constraints

Reviewed constraints for this plan:

1. `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
2. `docs/adr/ADR-0002-proto-first-contract-governance.md`
3. `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
4. `docs/plans/rust-migration-invariant-gates.md`
5. `docs/audits/2026-02-25-spacetimedb-openagents-autopilot-audit.md`

Non-negotiable constraints:

1. No dual authority for the same domain.
2. Authority mutations that impact shared trust/money/policy remain in authenticated HTTP authority lanes.
3. Replayability and deterministic behavior are release gates.
4. Spacetime adoption must not weaken existing replay/idempotency guarantees.

## 4) Target Architecture

### 4.1 Primary model

1. SpacetimeDB becomes the primary collaboration world for:
   - Autopilot presence and membership,
   - session and channel comms,
   - peer capability announcements,
   - request/response coordination for distributed execution.
2. Runtime/control remain authority for:
   - identity/session issuance,
   - treasury/liquidity/credit/warranty/claim state (Hydra/Aegis),
   - canonical receipts and policy enforcement.
3. Nostr remains supplemental for:
   - public discovery/interoperability,
   - marketplace reach where policy requires relay dissemination,
   - degraded-mode transport when Spacetime lane is unavailable.

### 4.2 Domain ownership table

| Domain | Primary owner | Secondary adapter |
|---|---|---|
| Autopilot peer presence | SpacetimeDB | Nostr broadcast mirror |
| Autopilot session comms | SpacetimeDB | Nostr fallback envelopes |
| Agent capability ads | SpacetimeDB | Nostr relay mirror |
| Provider discovery hints | SpacetimeDB | Nostr supplemental discovery |
| Auth/session authority | Control service | None |
| Money and settlement authority | Runtime + wallet-executor (Hydra) | Nostr proofs mirror only |
| Verification/warranty authority | Runtime (Aegis) | Nostr proofs mirror only |

## 5) Spacetime World Design

### 5.1 Core tables (minimum full integration set)

1. `autopilot_node`
   - node id, pubkey, device id, status, heartbeat, version, region.
2. `autopilot_session`
   - session id, node id, workspace id, started/ended timestamps.
3. `peer_link`
   - source node, target node, link state, last negotiated capabilities.
4. `comms_channel`
   - channel id, workspace scope, participants, policy flags.
5. `message_event` (append-only)
   - event seq, channel id, sender, payload, idempotency key, hash chain fields.
6. `capability_ad`
   - node id, available tools/models/compute, price hints, constraints.
7. `compute_request`
   - requester id, job envelope, policy class, cost hints, expected SLA.
8. `compute_assignment`
   - request id, selected provider, status, acceptance/rejection reason.
9. `transport_checkpoint`
   - per node/per channel cursor for durable resume and catch-up.
10. `bridge_outbox`
   - Nostr mirror/fallback events queued for adapter publication.

### 5.2 Reducers (transactional APIs)

1. `join_world`
2. `heartbeat`
3. `open_channel`
4. `publish_message`
5. `ack_message`
6. `advertise_capabilities`
7. `request_compute`
8. `accept_compute`
9. `reject_compute`
10. `close_channel`
11. `enqueue_nostr_bridge_event`
12. `mark_bridge_event_published`

Reducer requirements:

1. Idempotency key enforced on message and assignment writes.
2. Monotonic per-channel sequence assignment.
3. Deterministic hashing for replay proofs.
4. Conflict responses are explicit (no silent overwrite).

### 5.3 Subscriptions

1. Node-scoped subscription for private inbound channel traffic.
2. Workspace-scoped subscription for presence and membership changes.
3. Provider-scoped subscription for eligible compute requests.
4. Bridge adapter subscription for outbox publication events.

## 6) Identity, Auth, and Trust

1. User controls NIP-06 identity locally in desktop.
2. Control service mints short-lived Spacetime auth tokens bound to:
   - org/user/device/session,
   - NIP pubkey,
   - capability scope.
3. Spacetime reducers validate token claims and caller identity before mutation.
4. Nostr bridge events are signed by node identity and include references to Spacetime event ids.

## 7) Transport Rules (Spacetime First, Nostr Supplemental)

Default transport order:

1. Spacetime direct world path.
2. Spacetime retry with checkpoint replay.
3. Optional Nostr fallback/mirror (policy-gated).

Nostr is never primary for private intra-session coordination once Spacetime is healthy.

## 8) Integration With Existing Surfaces

### 8.1 Desktop (`apps/autopilot-desktop`)

1. Add Spacetime connection lifecycle manager.
2. Add connection status and health indicators in the provider/comms pane.
3. Route peer messaging and coordination commands through Spacetime client first.
4. Keep Nostr identity and NIP-90 settings visible for supplemental/fallback policy.

### 8.2 Shared crates

1. Add `crates/autopilot-spacetime` (new):
   - typed client,
   - reducer call wrappers,
   - subscription stream handling,
   - checkpoint persistence helpers.
2. Add a transport abstraction in Autopilot core:
   - `CommsTransport::SpacetimePrimary`,
   - `CommsTransport::NostrSupplemental`.
3. Keep transport selection policy-driven and auditable.

### 8.3 Runtime/control

1. Add token issuance endpoint(s) for Spacetime claims.
2. Add policy endpoints for transport and fallback rules.
3. Keep Hydra/Aegis authority paths unchanged; only comms/discovery/state sync moves to Spacetime-first.

## 9) NIP-90 and Marketplace Fit

1. Provider enrollment and capability publication become Spacetime-first events.
2. NIP-90 remains an external network interop lane, not the default internal comms lane.
3. Assignment and acceptance are coordinated in Spacetime with optional Nostr mirrors for network-wide discoverability.
4. Settlement and receipts remain under existing runtime/Hydra/Aegis authority domains.

## 10) Phased Execution Plan

### Phase 0: Contracts and ADR alignment

1. Add ADR for Spacetime domain ownership and Nostr supplemental role.
2. Add proto/schema contracts for Spacetime comms envelopes and checkpoints.
3. Define backward-compatible transport flags.

Gate:

1. ADR merged.
2. Contract docs approved.

### Phase 1: Spacetime foundation

1. Stand up OpenAgents Spacetime module with core tables/reducers/subscriptions.
2. Implement typed Rust client crate with deterministic serializer/hashing rules.
3. Add local/dev harness for reducer replay tests.

Gate:

1. Replay tests deterministic across restarts.
2. Subscription resume from checkpoints works.

### Phase 2: Identity and auth integration

1. Wire control-minted short-lived Spacetime tokens.
2. Bind NIP-06 pubkey + device claims to session context.
3. Add auth failure observability and rotation logic.

Gate:

1. Unauthorized reducer calls denied by tests.
2. Token expiry/refresh paths verified.

### Phase 3: Autopilot comms cut-in

1. Integrate desktop and core comms with Spacetime-first transport.
2. Migrate presence, direct channel messaging, and capability ads to Spacetime reducers.
3. Keep Nostr mirror/fallback behind feature policy.

Gate:

1. Two local Autopilots can discover/connect/exchange messages via Spacetime only.
2. Existing Nostr paths still work when forced by policy.

### Phase 4: NIP-90 provider coordination migration

1. Move provider capability and assignment handshake to Spacetime-first.
2. Keep NIP-90 envelopes as supplemental publication for external market reach.
3. Add explicit replay-linked mapping between Spacetime assignment ids and Nostr events.

Gate:

1. Provider onboarding and assignment pass in Spacetime-primary mode.
2. External discoverability remains available via Nostr mirror lane.

### Phase 5: Production hardening

1. Add SLOs for message latency, subscription staleness, replay recovery, and fallback rates.
2. Add chaos tests: node restart, token expiry storms, temporary bridge outages.
3. Add runbooks for Spacetime degradation and controlled Nostr fallback.

Gate:

1. SLOs met in staging.
2. Rollback and degrade-mode drills pass.

### Phase 6: Default policy flip

1. Set Spacetime-primary as default in desktop and runtime policy.
2. Keep Nostr supplemental mode available by explicit config.
3. Publish migration notes and operator runbooks.

Gate:

1. No P0 regressions for two release cycles.
2. Fallback usage drops to expected baseline.

## 11) Testing and Verification Matrix

1. Unit tests:
   - reducer idempotency,
   - sequence monotonicity,
   - checkpoint correctness.
2. Integration tests:
   - multi-node messaging,
   - provider assignment lifecycle,
   - token rotation behavior.
3. Determinism tests:
   - replay from snapshot + log yields identical state hash.
4. Failure tests:
   - transient network splits,
   - Nostr bridge failures,
   - stale checkpoint recovery.
5. Policy tests:
   - forced Nostr supplemental mode,
   - forced Spacetime-only mode,
   - mixed mode with strict precedence.

## 12) Risks and Mitigations

1. Risk: accidental dual authority between Spacetime and runtime/control.
   - Mitigation: explicit ownership matrix and ADR-locked boundaries.
2. Risk: fallback path becoming silently primary.
   - Mitigation: precedence enforcement + metrics alerting on fallback ratio.
3. Risk: replay divergence in comms history.
   - Mitigation: append-only logs, hash chaining, replay CI gate.
4. Risk: auth drift across NIP-06, device session, and Spacetime token.
   - Mitigation: signed claim binding and strict expiry/refresh validation.

## 13) Success Criteria

1. At least 95% of inter-Autopilot comm events in production use Spacetime path.
2. Nostr remains available and policy-controlled, but is no longer default for internal comms.
3. No regression in Hydra/Aegis settlement or authority domains.
4. Replay determinism and idempotency gates remain green in CI.

## 14) Explicit Non-Goals

1. Replacing runtime/control economic authority with Spacetime.
2. Removing Nostr support entirely.
3. Shipping private-key custody changes in this plan.

## 15) Deliverables

1. New Spacetime comms module and typed client crate.
2. Desktop Spacetime connection/comms UI and transport policy controls.
3. Runtime/control token and policy endpoints for Spacetime.
4. Nostr bridge adapter for supplemental publication/fallback.
5. Test harnesses, SLO dashboards, and operational runbooks.
