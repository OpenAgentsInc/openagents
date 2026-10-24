# 0001: One decision record folder

Status: Accepted (owner, 2026-10-24).

## Decision

`docs/adr/` is the only place for architecture decisions. Every earlier
decision record is deprecated:

- Aethyrforge `docs/decisions/0001`–`0008`.
- The 14 reconciled records proposed in the
  [consolidation audit](../aethyr/imported/2026-10-24-consolidation-audit.md#reconciled-decision-records).
  They were never adopted. Records here carry forward the parts the owner has
  decided.
- `apps/openagents/docs/adr/0001` and `0002` (theme families and surface
  preference).
- Decision sections in older design documents, such as
  `docs/verse/engine/architecture.md`, where they conflict with a record here.

A deprecated record is history. It explains why code looks the way it does,
but nothing new is built to it. When a deprecated record still describes
behavior we want, a record here restates it.

## Consequences

- A change that needs a decision adds a record here, or names the open
  decision in [the merge plan](../aethyr/merge.md#decisions-for-the-owner).
- Old record files stay in place with a deprecation note pointing here.
