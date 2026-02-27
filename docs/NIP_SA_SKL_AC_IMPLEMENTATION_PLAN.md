# NIP-SA, NIP-SKL, NIP-AC Implementation Plan (MVP)

## 1. Goal

Implement all three NIP drafts in `crates/nostr/nips/`:

- `SA.md` (NIP-SA)
- `SKL.md` (NIP-SKL)
- `AC.md` (NIP-AC)

including prerequisite NIP support, with Rust protocol code pulled from archive only where needed, and product panes in `apps/autopilot-desktop` for full operator interaction.

This plan is constrained by:

- `docs/MVP.md` as product authority
- `docs/OWNERSHIP.md` boundaries (`crates` own reusable protocol/UI primitives, app owns orchestration/UX)

## 2. Current State Snapshot

### 2.1 In-repo protocol status

- `crates/nostr/core` currently only provides identity/NIP-06 helpers (`identity.rs`, `nip06.rs`).
- `crates/nostr/nips` contains draft specs for SA/SKL/AC but no corresponding runtime modules in `crates/nostr/core`.
- No in-repo implementation currently exists for SKL core kinds (`33400`, `33401`) or AC kinds (`39240` to `39245`).

### 2.2 Desktop status

- `apps/autopilot-desktop` already has pane infrastructure and many relevant placeholders:
  - `Go Online`, `Job Inbox`, `Active Job`, `Job History`, `Network Requests`, `Activity Feed`, `Relay Connections`, `Nostr Keys`, `Spark Wallet`.
- Current pane state is mostly local/mock and not yet wired to real Nostr event lanes for SA/SKL/AC.

### 2.3 Archive availability

Archive path:

- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`

Available and reusable:

- Full `crates/nostr/core` with `nip_sa/*` and prerequisite NIPs.
- Full `crates/nostr/client` relay/DVM client stack (needs pruning/adaptation before reuse in this pruned repo).

Not available as ready-to-pull Nostr modules:

- No `nip_skl` or `nip_ac` Rust module in archive `crates/nostr/core`.

## 3. Canonical Protocol Surface To Implement

## 3.1 NIP-SA (from `SA.md`)

- `39200` Agent Profile
- `39201` Agent State
- `39202` Agent Schedule
- `39203` Agent Goals
- `39210` Agent Tick Request
- `39211` Agent Tick Result
- `39220` Skill License
- `39221` Skill Delivery
- `39230` Trajectory Session
- `39231` Trajectory Event

## 3.2 NIP-SKL (from `SKL.md`)

- `33400` Skill Manifest
- `33401` Skill Version Log
- Optional profile: `5390` Skill Search DVM Request
- Optional profile: `6390` Skill Search DVM Result
- Reused kinds: `1985` (NIP-32 attestation), `5` (NIP-09 revocation), `30402` (NIP-99 listing), `39220/39221` (NIP-SA fulfillment), `39230/39231` (NIP-SA trajectory audit)
- Canonical cross-NIP scope tag to emit: `skill_scope_id = 33400:<skill_npub>:<d-tag>:<version>`

## 3.3 NIP-AC (from `AC.md`)

- `39240` Credit Intent
- `39241` Credit Offer
- `39242` Credit Envelope (addressable)
- `39243` Credit Spend Authorization
- `39244` Credit Settlement Receipt
- `39245` Credit Default Notice
- `scope=skill` must resolve to canonical SKL skill identity:
  - `scope = skill:<skill_scope_id>:<constraints_hash>`

## 3.4 Prerequisite NIPs (union across SA/SKL/AC)

Mandatory for MVP implementation:

- NIP-01
- NIP-06 (already present, needs SA/SKL derivation extension)
- NIP-09
- NIP-32
- NIP-40
- NIP-44
- NIP-59
- NIP-90
- NIP-99

Feature-gated/optional rails in first rollout:

- NIP-26 (delegated signing profile)
- NIP-57
- NIP-60
- NIP-61
- NIP-87
- NIP-98

## 4. Archive Pull Plan (Minimal Required Code Only)

Rule: do not restore whole archived crates blindly. Pull only modules required for the protocol surface above.

## 4.1 Pull set for `crates/nostr/core`

Archive source root:

- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/nostr/core/src`

Pull in Phase 1 (foundation + prerequisites):

- `nip01.rs` (event model/signing/verification primitives)
- `nip09.rs`
- `nip26.rs`
- `nip32.rs`
- `nip40.rs`
- `nip44.rs`
- `nip59.rs`
- `nip90.rs`
- `nip99.rs`

Pull in Phase 2 (SA):

- `nip_sa/mod.rs`
- `nip_sa/profile.rs`
- `nip_sa/state.rs`
- `nip_sa/schedule.rs`
- `nip_sa/goals.rs`
- `nip_sa/tick.rs`
- `nip_sa/trajectory.rs`
- `nip_sa/skill.rs`
- `nip_sa/budget.rs`
- `nip_sa/wallet_integration.rs` (behind `spark-integration` feature)

Optional pull in Phase 4 (AC optional rails):

- `nip57.rs`
- `nip60.rs`
- `nip61.rs`
- `nip87.rs`
- `nip98.rs`

Do not pull:

- Unused NIP modules outside this scope.
- Entire archive `lib.rs` export surface (rebuild local exports explicitly to keep MVP small).

## 4.2 Pull set for tests/examples

Pull and adapt:

- `crates/nostr/core/tests/nip_sa_exports.rs`
- `crates/nostr/core/tests/nip_sa_e2e.rs`
- `crates/nostr/core/tests/nip44_integration.rs`
- `crates/nostr/core/tests/nip90_integration.rs`

Optional examples:

- `crates/nostr/core/examples/nip90_customer.rs`
- `crates/nostr/core/examples/nip90_provider.rs`
- `crates/nostr/core/examples/nip90_types.rs`

## 4.3 Pull set for client transport (if reused)

Archive source root:

- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/nostr/client/src`

Pull only if needed for real relay IO in MVP:

- `lib.rs`, `error.rs`, `dvm.rs`, `relay.rs`, `pool.rs`, `subscription.rs`

Optional later:

- `connection_pool.rs`, `recovery.rs`, `outbox.rs`, `queue.rs`, `cache.rs`

Important adaptation note:

- Archive `nostr-client` depends on crates not in this pruned workspace (for example `openagents-utils`); strip/replace those dependencies during import.

## 5. Target Code Layout After Implementation

## 5.1 `crates/nostr/core`

- Keep `identity.rs` and `nip06.rs`.
- Add prerequisite NIP modules listed above.
- Add `pub mod nip_sa;` (archive-derived).
- Add new modules for this repo:
  - `pub mod nip_skl;`
  - `pub mod nip_ac;`
- Re-export only the required public types/constants for app/runtime use.

## 5.2 New modules to create (net-new in this repo)

`crates/nostr/core/src/nip_skl/`:

- `mod.rs`
- `manifest.rs` (33400)
- `version_log.rs` (33401)
- `discovery.rs` (optional 5390/6390 profile)
- `trust.rs` (NIP-32 attestation evaluation + optional NIP-26 delegated-signing checks)
- `trust.rs` must include kill-flag authority/quorum evaluation, not label-presence-only checks
- `revocation.rs` (NIP-09 integration)
- `revocation.rs` must support pre-signed manifest revocation by `a`/`d` identity without requiring unknown future license ids
- `yaml_derivation.rs` (SKILL.md frontmatter -> deterministic event derivation)

`crates/nostr/core/src/nip_ac/`:

- `mod.rs`
- `intent.rs` (39240)
- `offer.rs` (39241)
- `envelope.rs` (39242 state authority rules, replaceable by `d` tag)
- `spend.rs` (39243)
- `settlement.rs` (39244)
- `default_notice.rs` (39245)
- `scope_hash.rs` (canonical scope hash rules)
- `reputation.rs` (NIP-32 label conversion helpers)

## 5.3 App integration (`apps/autopilot-desktop`)

- Add background workers/channels for:
  - SA lifecycle lane
  - SKL discovery/trust lane
  - AC credit lane
- Replace mock pane state transitions with Nostr event-backed state updates.

## 6. Phased Implementation Plan

## Phase 0: Protocol lock and scaffolding (1-2 days)

1. Keep `docs/PROTOCOL_SURFACE.md` as the canonical kind/tag lock for runtime.
2. Confirm SA kind numbers (`39200+`) and freeze SKL/AC ranges used by this implementation.
3. Expand `crates/nostr/core/Cargo.toml` features:
   - `full`, `minimal`, `spark-integration`.
4. Add module scaffolding/tests compile gate in `nostr/core`.

Exit criteria:

- Kind numbers are stable in-repo and referenced by code/tests, not only draft docs.

## Phase 1: Foundation + prerequisite NIPs (3-5 days)

1. Pull required modules from archive (Section 4.1 foundation list).
2. Rebuild `nostr/core/src/lib.rs` exports narrowly for MVP.
3. Ensure NIP-06 derivation remains backward compatible and add SA/SKL derivation helpers:
   - Agent account derivation.
   - Skill key derivation path extension from SKL spec.
4. Import/adapt `nip44` and `nip90` integration tests.

Exit criteria:

- `nostr/core` compiles with required prerequisites.
- Tests pass for NIP-44 and NIP-90 basics.

## Phase 2: NIP-SA implementation (3-5 days)

1. Pull `nip_sa/*` modules from archive and adapt to current crate features.
2. Wire `wallet_integration.rs` behind `spark-integration`.
3. Add SA exports and kind constants.
4. Import/adapt SA tests (`nip_sa_exports`, `nip_sa_e2e`).
5. Wire app runtime states to SA events:
   - Agent profile/state/schedule publish/read.
   - Tick request/result lifecycle.
   - Trajectory session/event append-only feed.

Exit criteria:

- Full SA event set is publishable/parseable from app flows.
- Existing job/provider panes can link to SA trajectory/tick state.

## Phase 3: NIP-SKL implementation (net-new) (5-8 days)

1. Implement SKL core data models and serializers for 33400/33401.
2. Implement optional NIP-90 discovery profile types for 5390/6390.
3. Implement SKILL.md YAML frontmatter parser and deterministic derivation helper:
   - deterministic tags/content
   - canonical payload hashing
   - caller-supplied `created_at` handling
4. Implement trust gate engine:
   - NIP-32 attestation aggregation.
   - optional NIP-26 delegation checks when delegated-signing profile is enabled.
   - kill-flag authority/quorum enforcement.
   - NIP-09 revocation handling.
5. Implement fulfillment bridge to SA:
   - SKL trust must pass before `39220`/`39221` flow can execute.
   - actor alignment: marketplace issues `39220`, skill provider/delegate emits `39221`.
6. Add unit tests for manifest validation, trust evaluation, and revocation.

Exit criteria:

- Skill discovery, trust, and fulfillment gating work end-to-end in local flows.

## Phase 4: NIP-AC implementation (net-new) (5-8 days)

1. Implement AC event models and state transitions for 39240-39245.
2. Implement envelope authority logic:
   - `d`-tag replacement semantics for 39242.
   - cap/expiry/spend validation.
   - canonical `skill_scope_id` parsing for `scope=skill`.
3. Implement settlement mapping:
   - AC receipt linkage to NIP-90 result events and payment pointers.
4. Implement reputation hooks via NIP-32 labels.
5. Add optional rail adapters behind feature flags (zap/cashu/http auth).

Exit criteria:

- Credit envelope lifecycle runs deterministically with auditable settlement/default outcomes.

## Phase 5: Desktop pane integration (5-8 days)

1. Extend existing panes (Section 7.1).
2. Add new panes (Section 7.2).
3. Connect pane actions to typed commands/events, not direct mutable side effects.
4. Ensure source badges/load states reflect runtime truth (`runtime`, `wallet`, `local`).

Exit criteria:

- Operator can complete SA + SKL + AC flows entirely from desktop panes.

## Phase 6: Validation, hardening, and release gate (3-5 days)

1. Add integration tests for complete path:
   - Skill discovery -> trust gate -> license/delivery -> job execution -> credit settlement.
2. Add replay/idempotency tests for AC envelope and SA trajectory updates.
3. Add failure-mode tests (relay loss, stale cursor, wallet errors).
4. Document runbooks in `docs/` for incident/recovery.

Exit criteria:

- MVP confidence for first-run earning loop with SA/SKL/AC in place.

## 7. Pane Plan

## 7.1 Existing panes to extend

1. `Nostr Keys (NIP-06)`
   - Add agent account index controls.
   - Add skill derivation preview (author/agent/skill key lineage).
2. `Go Online`
   - Add SA runner status and SKL trust gate status.
   - Show AC credit lane availability and blockers.
3. `Network Requests`
   - Add skill-scoped request composition and optional credit-envelope references.
4. `Job Inbox`
   - Show required skill manifest references and envelope eligibility.
5. `Active Job`
   - Show linked tick ID, trajectory session ID, and credit envelope state.
6. `Job History`
   - Show immutable links to SKL manifest/version, settlement receipt/default event.
7. `Activity Feed`
   - Add event-kind filters for SA/SKL/AC classes.
8. `Alerts and Recovery`
   - Add trust-failure, revocation, envelope-expiry, and settlement-failure incidents.

## 7.2 New panes required

1. `Agent Profile and State` (NIP-SA)
   - Manage/read `39200`, `39201`, `39203`.
   - Actions: publish profile, publish encrypted state, update goals.
2. `Agent Schedule and Tick` (NIP-SA)
   - Manage/read `39202`, `39210`, `39211`.
   - Actions: set heartbeat/triggers, trigger manual tick, inspect last result.
3. `Trajectory Audit` (NIP-SA)
   - Read `39230`, `39231`.
   - Actions: open session, filter by step type, verify trajectory hash.
4. `Skill Registry` (NIP-SKL)
   - Read `33400`, `30402`, optional `5390/6390` results.
   - Actions: discover skills, inspect manifests, install/select skill.
5. `Skill Trust and Revocation` (NIP-SKL)
   - Read/write `1985`, `5`, delegation graph checks.
   - Actions: view trust tier, inspect attestations, evaluate kill-flag quorum, revoke/suspend.
6. `Credit Desk` (NIP-AC)
   - Read/write `39240`, `39241`, `39242`, `39243`.
   - Actions: request intent, review offers, accept/revoke envelope, authorize spend.
7. `Credit Settlement Ledger` (NIP-AC)
   - Read `39244`, `39245` and linked NIP-90 outcomes.
   - Actions: verify settlement, inspect defaults, emit reputation labels.

## 7.3 Pane interaction map (critical flows)

1. Skill acquisition flow
   - `Skill Registry` -> `Skill Trust and Revocation` -> SA skill license/delivery reflected in `Agent Profile and State`.
2. Paid compute flow with credit
   - `Credit Desk` creates envelope -> `Network Requests` submits NIP-90 request with scope tags (including canonical skill scope when applicable) -> `Active Job` tracks lifecycle -> `Credit Settlement Ledger` finalizes.
3. Audit flow
   - `Trajectory Audit` + `Job History` + `Credit Settlement Ledger` provide end-to-end proof chain.

## 8. Data Contracts Between App and Nostr Core

App command surface should be explicit and typed (not ad-hoc string commands), for example:

- `PublishAgentProfile`
- `PublishAgentState`
- `ConfigureAgentSchedule`
- `PublishTickRequest`
- `PublishTickResult`
- `PublishSkillManifest`
- `PublishSkillVersionLog`
- `SubmitSkillSearch` (optional profile path)
- `PublishCreditIntent`
- `PublishCreditOffer`
- `PublishCreditEnvelope`
- `PublishCreditSpendAuth`
- `PublishCreditSettlement`
- `PublishCreditDefault`

Each command should return:

- event id
- authoritative status (`accepted`, `rejected`, `retryable`)
- typed error class

## 9. Testing Strategy

## 9.1 Unit tests

- Kind/tag/content validation for every SA/SKL/AC event type.
- Delegation/trust/revocation logic for SKL.
- Kill-flag quorum logic for SKL trust enforcement.
- Envelope cap/expiry/state-machine logic for AC.

## 9.2 Integration tests

- Relay publish/subscribe roundtrip for all new kinds.
- SA tick + trajectory consistency.
- SKL trust gate blocks SA delivery when invalid.
- SKL pre-signed revocation works via manifest identity even before per-buyer license ids exist.
- AC settlement references valid NIP-90 result and payment pointer.

## 9.3 Desktop tests

- Pane action to command dispatch correctness.
- Deterministic UI state across restart/reconnect.
- Failure UX for trust failures, expired envelopes, and wallet/relay errors.

## 10. Delivery Order and Acceptance Criteria

## 10.1 Delivery order

1. Foundation NIPs + SA.
2. SKL registry/trust.
3. AC envelope/settlement.
4. Pane completion and E2E hardening.

## 10.2 Final acceptance criteria

1. All SA/SKL/AC kinds above are implemented in `crates/nostr/core`.
2. Required prerequisite NIPs are implemented and wired.
3. Desktop has pane coverage for create/read/update/audit flows across SA/SKL/AC.
4. End-to-end flow works:
   - discover skill -> trust gate -> license/delivery -> run job -> settle credit -> reflect wallet/job history.
5. Archive imports remain minimal and explicit; no full historical code resurrection.

## 11. Explicit Non-Goals (for this implementation pass)

- Full marketplace productization beyond MVP loop.
- External LP/liquidity product surface in desktop.
- Mobile/web parity for SA/SKL/AC panes.
- Pulling unrelated archived crates back into workspace.
