# ADR-0001: Rust-Only Architecture Baseline

## Status

Accepted

## Date

2026-02-21

## Owner Lane

`owner:contracts-docs`

## Context

OpenAgents has reset ADR authority for the Rust-era migration. Multiple transitional lanes still exist in the repo, but architecture authority now requires one baseline ADR that locks:

1. Rust-only implementation direction.
2. Authority boundaries between control plane, runtime, and Khala.
3. Transport and coupling invariants that prevent architectural drift.
4. Explicit non-goals that are disallowed even when expedient.

Without this baseline ADR, migration work risks reintroducing implicit coupling, non-proto contracts, and mixed transport semantics.

## Decision

OpenAgents adopts the Rust-only architecture baseline defined in `docs/ARCHITECTURE-RUST.md` as the canonical architecture authority for all new migration and closure work.

Normative constraints:

1. Cross-process/client-server contracts are proto-first (`proto/`) and generated into Rust.
2. Authority mutations are HTTP API only; Khala WebSocket transport is subscription/replay only.
3. Khala remains delivery/replay infrastructure and never becomes an authority write path.
4. Control and runtime authority planes remain separate, with no cross-plane SQL joins in production code.
5. Production service boundaries (`openagents.com` control service, runtime service, Khala service) must communicate via explicit network/proto contracts and not via in-memory coupling.
6. Rust-only endstate requirements in `docs/ARCHITECTURE-RUST.md` are the migration completion criteria.

## Rust-Era Boundary Impact

- Control-plane boundary: preserved and explicit.
- Runtime authority boundary: preserved and explicit.
- Khala delivery boundary: constrained to WS replay/delivery only.
- Client/runtime contract boundary: proto-first enforced.
- Deployment/runtime ops boundary: deploy/migrate sequencing and runbooks are mandatory evidence.

## Invariant Gate Mapping

Source: `docs/plans/active/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-01` through `INV-10` (baseline ADR for all migration gates).
2. Preservation/change:
   - Preserves all listed invariants as mandatory, no exceptions introduced.
   - Tightens enforcement by making Rust architecture baseline authoritative for ADR review.
3. Follow-up gate requirements:
   - Any architecture-affecting issue must reference impacted `INV-*` gates.
   - Release promotion requires gate evidence in roadmap-linked issues.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - Transitional hybrid deployments are allowed only while preserving proto contract compatibility and authority boundaries.
2. Rollout sequence:
   - Follow `docs/ARCHITECTURE-RUST-ROADMAP.md` issue ordering and phase gates.
3. Data/schema/protocol migration:
   - Keep proto-first compatibility checks (`buf lint`, breaking checks).
   - Preserve control/runtime plane ownership during migration.

## Rollback and Failure Impact

1. Rollback triggers:
   - Any `INV-01` to `INV-06` violation discovered in release candidates.
   - Critical replay/auth boundary regressions.
2. Rollback procedure:
   - Halt promotion, revert offending change, re-run invariant gates and release checks.
3. Residual risk:
   - Transitional code paths may persist until closure issues complete; no-go decision remains valid until closure gates pass.

## Verification

Required baseline checks:

```bash
./scripts/local-ci.sh proto
./scripts/local-ci.sh runtime-history
./scripts/run-cross-surface-contract-harness.sh
```

For runtime deploy evidence:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

## Consequences

### Positive

- Single architecture source of truth for Rust-era decisions.
- Stronger prevention of boundary drift during migration.
- Clear release/go-no-go evaluation against explicit invariants.

### Negative

- Reduced flexibility for ad hoc shortcuts across service boundaries.
- Additional documentation/verification burden for architecture-affecting changes.

### Neutral

- Historical hybrid docs remain available as archival context only.

## Alternatives Considered

1. Keep hybrid architecture doc (`docs/ARCHITECTURE.md`) as co-equal authority.
   - Rejected: creates ambiguity and inconsistent gate interpretation.
2. Defer baseline ADR until all closure issues are complete.
   - Rejected: allows drift during the highest-risk migration window.
3. Use issue threads as architecture authority without ADR baseline.
   - Rejected: not durable/auditable enough for cross-team enforcement.

## References

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/plans/active/rust-migration-invariant-gates.md`
- Related issue: `OA-RUST-074` / `#1889`
