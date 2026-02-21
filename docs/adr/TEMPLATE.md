# ADR-XXXX: Title

## Status

`Proposed` | `Accepted` | `Superseded` | `Deprecated` | `Archived`

## Date

YYYY-MM-DD

## Owner Lane

`owner:<lane>`

## Context

Describe the problem, constraints, and why an ADR is needed now.

## Decision

State the decision as a clear, normative statement.

## Rust-Era Boundary Impact

Indicate which architecture boundaries are impacted:

- Control-plane boundary
- Runtime authority boundary
- Khala delivery boundary
- Client/runtime contract boundary
- Deployment/runtime ops boundary

## Invariant Gate Mapping

Map this ADR to migration invariants (`INV-*`) from:

- `docs/plans/active/rust-migration-invariant-gates.md`

Required:

1. Invariants affected
2. How the decision preserves or changes each invariant
3. Follow-up gate requirements

## Compatibility and Migration Plan

1. Backward/forward compatibility expectations.
2. Rollout sequence and dependency ordering.
3. Data/schema/protocol migration requirements.

## Rollback and Failure Impact

1. Rollback trigger conditions.
2. Rollback procedure.
3. Residual risk if rollback is incomplete.

## Verification

List concrete checks required before status moves to `Accepted`.

Example:

```bash
./scripts/local-ci.sh changed
buf lint
buf breaking --against .git#branch=main,subdir=proto
```

## Consequences

### Positive

- ...

### Negative

- ...

### Neutral

- ...

## Alternatives Considered

1. Alternative A (why rejected)
2. Alternative B (why rejected)

## References

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- Related issue(s): `OA-RUST-...`

