# Docs Index

This directory is organized around a small number of canonical entry points.

## Start Here

- `MVP.md`: product authority for the current MVP.
- `OWNERSHIP.md`: crate/app ownership boundaries.
- `autopilot-earn/README.md`: consolidated Autopilot Earn docs.
- `plans/`: forward-looking system and protocol plans.
- `audits/`: repo audits, gap analyses, and architecture reviews.

## Directory Map

- `adr/`: accepted architectural decisions.
- `audits/`: audits and gap analyses.
- `autopilot-earn/`: Autopilot Earn product, ops, rollout, and verification docs.
- `charms/`: CAST/Charms-specific docs and runbooks.
- `codex/`: Codex-specific docs and plans.
- `deploy/`: deployment runbooks and packaging notes.
- `plans/`: broader system plans and normative design docs.
- `reports/`: generated or captured report artifacts.
- `wgpui/`: WGPUI-specific docs.

## Top-Level Docs

Top-level files in `docs/` are reserved for cross-cutting repo authority or shared platform references.

Examples:

- `MVP.md`
- `OWNERSHIP.md`
- `PANES.md`
- `PROTOCOL_SURFACE.md`
- `SPACETIME_ROLLOUT_INDEX.md`
- `SOLVER.md`

## Organization Decision

Decision for the remaining top-level docs:

- Do not mass-move the rest of the top-level docs right now.
- Keep cross-cutting authority docs and shared platform docs at the top level.
- Continue moving product- or program-specific doc clusters into dedicated subdirectories when a real cluster exists.

Practical rule:

- product/program doc set with multiple related specs, runbooks, trackers, and audits: give it a subdirectory
- shared repo authority or platform reference used across multiple areas: keep it top-level

Current examples:

- `autopilot-earn/` is now the canonical home for the Earn doc set.
- `codex/`, `charms/`, `deploy/`, and `wgpui/` already follow the same pattern.
- Remaining top-level docs stay where they are until there is a stronger clustering reason than “they exist.”
