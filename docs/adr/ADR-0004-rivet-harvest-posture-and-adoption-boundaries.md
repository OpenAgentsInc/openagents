# ADR-0004: Rivet Harvest Posture and Adoption Boundaries

## Status

Accepted

## Date

2026-02-21

## Owner Lane

`owner:contracts-docs`

## Context

Rivet contains high-signal Rust patterns relevant to replacing BEAM-era operational behavior (long-lived sessions, reconnect lifecycle handling, durable workflow history discipline, and pubsub seams).  
Without explicit boundaries, selective reuse can drift into platform-level adoption that conflicts with OpenAgents authority and protocol architecture.

OpenAgents requires a formal posture that defines what is harvested, what is rejected, and how provenance is governed.

## Decision

OpenAgents adopts a **selective Rivet harvest posture**:

1. Harvest subsystem patterns and implementation ideas where they strengthen Rust-era runtime/Khala behavior.
2. Do **not** adopt Rivet’s platform authority model as OpenAgents’ core architecture.
3. Keep OpenAgents authority boundaries, proto-first contracts, and service topology as the governing model.

Approved harvest categories:

1. Guard/hibernation lifecycle patterns for websocket/session stability.
2. Durable workflow history discipline patterns (history compatibility and replay safety).
3. Universal pubsub/database seam patterns that preserve transport/storage abstraction.

Rejected platform-level adoptions:

1. Actor-per-entity as authority source of truth.
2. Full Rivet runtime/platform surface as OpenAgents core.
3. Replacing proto-first OpenAgents contracts with Rivet-native wire authority.

## Rust-Era Boundary Impact

- Control-plane boundary: unchanged; remains OpenAgents-owned.
- Runtime authority boundary: preserved; authority remains runtime/control planes, not actor memory.
- Khala delivery boundary: pattern reuse allowed, authority role unchanged.
- Client/runtime contract boundary: remains proto-first, independent of harvested internals.
- Deployment/runtime ops boundary: component reuse cannot introduce implicit service coupling.

## Invariant Gate Mapping

Source: `docs/plans/active/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-01` (proto-first contract authority)
   - `INV-04` (control/runtime plane boundaries)
   - `INV-05` (no implicit in-memory coupling across services)
   - `INV-06` (Khala remains delivery/replay only)
2. Preservation/change:
   - Preserves architecture authority by explicitly limiting reuse scope.
   - Prevents authority-model drift toward actor-memory ownership.
3. Follow-up gate requirements:
   - Harvest proposals must show invariant impact and explicit non-goals.
   - Any borrowed subsystem with boundary implications requires ADR or ADR addendum.

## Compatibility and Migration Plan

1. Compatibility posture:
   - Harvested internals must be transparent to wire contracts and protocol compatibility.
2. Rollout sequence:
   - Introduce harvest pattern behind explicit module seams.
   - Validate behavior with replay/history/chaos harnesses before promotion.
3. Migration requirements:
   - Preserve current runtime/Khala authority and topic ordering semantics during adoption.

## Rollback and Failure Impact

1. Rollback triggers:
   - Boundary drift (`INV-04`/`INV-05`/`INV-06`) or replay regression after harvested integration.
2. Rollback procedure:
   - Remove/revert harvested integration while preserving OpenAgents protocol and data-plane contracts.
3. Residual risk:
   - Mis-scoped reuse can still occur without review discipline; governance checks are mandatory.

## Verification

Required for each harvested component rollout:

```bash
./scripts/run-cross-surface-contract-harness.sh
apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh
./scripts/local-ci.sh proto
```

Review evidence must include:

1. Source provenance (Rivet path/revision/license note).
2. Boundary impact checklist (`INV-*` mapping).
3. Rollback plan and operational runbook impact.

## Consequences

### Positive

- Allows OpenAgents to benefit from proven Rust operational patterns.
- Reduces BEAM-replacement risk while preserving architecture discipline.
- Creates enforceable governance for future harvested components.

### Negative

- Additional review overhead for harvested-pattern proposals.
- Slower “lift and shift” velocity due to boundary enforcement.

### Neutral

- Rivet may still be used as research context even when no direct code reuse occurs.

## Alternatives Considered

1. Full Rivet platform adoption.
   - Rejected: conflicts with OpenAgents authority/contract model.
2. No Rivet harvesting at all.
   - Rejected: leaves high-signal Rust operational patterns unused.
3. Case-by-case harvesting without ADR boundary policy.
   - Rejected: too much ambiguity and high drift risk.

## Licensing and Provenance Policy

1. Rivet is Apache-2.0; selective reuse is permitted with notice preservation.
2. Any copied/adapted code must include provenance notes in PR/issue context.
3. License obligations must be preserved in repository notices where required.

## Future Harvest Decision Process

1. Open a proposal issue referencing source paths and intended boundary impact.
2. Map proposal to affected invariants and non-goals.
3. Require owner-lane approval before merge.
4. Add/update ADR when harvest changes architecture or operational guarantees.

## References

- `docs/ARCHITECTURE-RUST.md` (Rivet integration exploration section)
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- Related issue: `OA-RUST-077` / `#1892`
