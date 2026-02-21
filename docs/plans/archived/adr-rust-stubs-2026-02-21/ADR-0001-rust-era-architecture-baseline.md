# ADR-0001: Rust-Era Architecture Baseline (Stub)

## Status

`Proposed`

## Date

2026-02-21

## Owner Lane

`owner:contracts-docs`

## Context

The legacy ADR corpus was archived in OA-RUST-004. The Rust-era architecture now requires a fresh ADR baseline anchored to `docs/ARCHITECTURE-RUST.md`.

This stub reserves ADR-0001 in the new numbering scheme and establishes the work item for final baseline ratification.

## Decision

Reserve ADR-0001 for the Rust-era architecture baseline and complete full decision content in OA-RUST-074.

## Rust-Era Boundary Impact

- Control-plane boundary: in scope for final ADR content.
- Runtime authority boundary: in scope for final ADR content.
- Khala delivery boundary: in scope for final ADR content.
- Client/runtime contract boundary: in scope for final ADR content.
- Deployment/runtime ops boundary: in scope for final ADR content.

## Invariant Gate Mapping

This stub tracks the invariant mapping requirement; detailed mappings are completed in OA-RUST-074.

Target invariants:

- `INV-01` through `INV-10` from `docs/plans/active/rust-migration-invariant-gates.md`.

## Compatibility and Migration Plan

Detailed compatibility/migration plan is part of OA-RUST-074 completion criteria.

## Rollback and Failure Impact

No runtime behavior change in this stub ADR.

## Verification

1. ADR appears in `docs/adr/INDEX.md`.
2. Numbering starts at ADR-0001 for Rust-era ADR set.
3. Template/process docs in `docs/adr/` align with Rust-era workflow.

## Consequences

### Positive

- New ADR set can start without legacy numbering ambiguity.

### Negative

- Baseline decision content is deferred until OA-RUST-074.

### Neutral

- Historical ADR archive remains accessible for context.

## Alternatives Considered

1. Reuse legacy ADR numbering (rejected: ambiguous authority boundary).
2. Start at a high offset (rejected: weak readability for new ADR set).

## References

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/plans/archived/adr-legacy-2026-02-21/CATALOG.md`
- Related issues: `OA-RUST-004`, `OA-RUST-005`, `OA-RUST-074`

