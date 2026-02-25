# ADR-0009: SpacetimeDB Canonical Sync Transport

## Status

Accepted

## Date

2026-02-25

## Owner Lane

`owner:protocol-runtime`

## Context

OpenAgents currently runs Spacetime as the production sync/replay delivery lane. The active architecture direction is full replacement of Spacetime transport with SpacetimeDB.

Without an explicit superseding ADR, the repository has split authority:

1. ADR-0003 still defines Spacetime as the canonical WS sync doctrine.
2. Active plan docs define SpacetimeDB as the target canonical sync lane.
3. Migration gates and compatibility policy are ambiguous during execution.

OpenAgents needs one canonical transport ADR that:

1. declares the replacement target as normative,
2. preserves authority-mutation boundaries and replay guarantees,
3. defines Spacetime retirement as a gated outcome rather than an implicit assumption.

## Decision

OpenAgents adopts SpacetimeDB as the canonical sync/replay transport for retained client surfaces, superseding ADR-0003 as long-term sync doctrine.

Normative constraints:

1. Sync delivery uses Spacetime subscribe/apply semantics as the target canonical protocol.
2. Authority mutations remain authenticated HTTP API commands (`INV-02`); sync transport is delivery/projection, not authority mutation.
3. Replay/idempotent apply remains mandatory with ordered keys (current `(topic, seq)`, target `(stream_id, seq)`).
4. Spacetime remains implemented legacy lane only until cutover gates pass, then is removed from retained production paths.
5. Transport replacement must not degrade Hydra/Aegis authority correctness or receipt integrity.

This ADR supersedes:

1. `docs/adr/ADR-0003-spacetime-ws-only-replay-transport.md`

## Rust-Era Boundary Impact

- Control-plane boundary: token/claim issuance expands to Spacetime session claims.
- Runtime authority boundary: unchanged ownership; runtime remains authority source of domain events.
- Sync delivery boundary: canonical doctrine moves from Spacetime to Spacetime.
- Client/runtime contract boundary: protocol shifts to Spacetime sync envelopes while preserving replay semantics.
- Deployment/runtime ops boundary: cutover and rollback runbooks become required promotion evidence.

## Invariant Gate Mapping

Source: `docs/plans/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-01` proto-first contracts
   - `INV-02` HTTP-only authority mutations
   - `INV-03` transport doctrine (superseded from Spacetime-specific to Spacetime-specific sync doctrine)
   - `INV-06` delivery-not-authority (retained, lane-agnostic)
   - `INV-07` replay/idempotency
   - `INV-08` deploy isolation
2. Preservation/change:
   - Preserves `INV-02`, `INV-06`, and `INV-07`.
   - Replaces Spacetime-specific transport wording in `INV-03` with Spacetime canonical transport wording.
3. Follow-up gate requirements:
   - invariant gate doc update must merge before claiming full replacement complete.
   - compatibility negotiation policy and fixtures must include Spacetime protocol versions.
   - cutover evidence must include replay/resume/stale recovery correctness and rollback drills.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - Spacetime remains available during migration cohorts only.
   - retained clients gain Spacetime protocol negotiation before Spacetime removal.
2. Rollout sequence:
   - land ADR + invariant updates,
   - land proto contracts and client/runtime integrations,
   - run dual-lane parity and chaos gates,
   - flip default transport and retire Spacetime.
3. Data/schema/protocol migration requirements:
   - introduce sync v2 proto contracts for Spacetime transport,
   - preserve replay and watermark continuity across migration.

## Rollback and Failure Impact

1. Rollback trigger conditions:
   - replay divergence, stale cursor loops, or auth-scope regressions in staged rollout.
2. Rollback procedure:
   - disable Spacetime default routing and re-enable Spacetime emergency lane for affected cohorts.
   - restore known-good sync profile while preserving authority event integrity.
3. Residual risk:
   - temporary dual-lane complexity persists during migration windows.

## Verification

Required baseline checks:

```bash
./scripts/local-ci.sh docs
./scripts/local-ci.sh proto
./scripts/run-cross-surface-contract-harness.sh
```

Required rollout evidence before Spacetime retirement:

```bash
apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh
cargo test -p openagents-runtime-service server::tests::spacetime_topic_messages -- --nocapture
```

## Consequences

### Positive

- Removes doctrine ambiguity about sync transport end state.
- Preserves authority and replay constraints while enabling full replacement execution.
- Makes Spacetime retirement an explicit, test-gated milestone.

### Negative

- Requires coordinated updates across protocol, runtime, control, and desktop lanes.
- Increases migration complexity until full Spacetime removal lands.

### Neutral

- Spacetime remains an implemented lane until cutover gates pass.

## Alternatives Considered

1. Keep ADR-0003 as canonical and treat Spacetime as optional add-on.
   - Rejected: conflicts with accepted full replacement direction.
2. Replace Spacetime immediately without superseding ADR.
   - Rejected: removes architecture auditability and gate discipline.
3. Keep dual-primary sync lanes permanently.
   - Rejected: doubles operational and compatibility burden.

## References

- `docs/core/ARCHITECTURE.md`
- `docs/plans/spacetimedb-full-integration.md`
- `docs/plans/2026-02-25-spacetimedb-autopilot-primary-comms-integration-plan.md`
- Related issue: `OA-SPACETIME-001` / `#2231`

