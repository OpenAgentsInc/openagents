# Rust-Era ADR Process

This directory is the active ADR authority namespace for Rust-era architecture decisions.

## Scope

Use ADRs for decisions that change interfaces, invariants, boundaries, compatibility rules, or rollout guarantees.

Do not use ADRs for routine implementation details that do not alter cross-team contracts.

## Lifecycle

Valid status values:

1. `Proposed`
2. `Accepted`
3. `Superseded`
4. `Deprecated`
5. `Archived`

## Authoring Workflow

1. Copy `docs/adr/TEMPLATE.md` into a new ADR file under `docs/adr/` named `ADR-XXXX-title`.
2. Use the next number from `docs/adr/INDEX.md`.
3. Fill all required sections, including:
   - invariant gate impact (`INV-*`),
   - migration/rollback impacts,
   - verification evidence.
4. Add the ADR entry to `docs/adr/INDEX.md` with `Status: Proposed`.
5. Land via PR with reviewer signoff from the owning lane.
6. Promote status to `Accepted` once decision is ratified.

## Review Requirements

Every ADR PR must show:

1. Why existing ADRs cannot be reused.
2. Which Rust architecture invariant(s) are affected.
3. How compatibility and rollback are handled.
4. Which tests/runbooks validate the decision.

## Legacy ADR Archive

Legacy (pre-Rust-reset) ADR corpora were moved to backroom and are historical context only:

- `/Users/christopherdavid/code/backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/docs/plans/archived/`
