# Docs Index

This directory is organized around a small number of canonical entry points.

## Start Here

- `MVP.md`: product authority for the current MVP.
- `OWNERSHIP.md`: crate/app ownership boundaries.
- `kernel/README.md`: marketplace and kernel architecture overview.
- `kernel/markets/data-market.md`: canonical current Data Market implementation status.
- `headless-data-market.md`: no-window and `autopilotctl` Data Market runbook.
- `autopilot-earn/README.md`: consolidated Autopilot Earn docs.
- `../skills/README.md`: repo-owned agent skills, including the current Data Market seller skills.
- `deploy/NEXUS_GCP_RUNBOOK.md`: stateful Nexus deployment runbook.
- `plans/deprecated/`: archived historical plans kept for provenance.
- `audits/`: repo audits, gap analyses, and architecture reviews.

## Directory Map

- `adr/`: accepted architectural decisions.
- `audits/`: audits and gap analyses.
- `autopilot-earn/`: Autopilot Earn product, ops, rollout, and verification docs.
- `charms/`: CAST/Charms-specific docs and runbooks.
- `codex/`: Codex-specific docs and plans.
- `deploy/`: deployment runbooks and packaging notes.
- `kernel/`: current kernel and marketplace architecture docs.
- `plans/deprecated/`: archived historical plans and superseded design drafts.
- `reports/`: generated or captured report artifacts.
- `wgpui/`: WGPUI-specific docs.

## Top-Level Docs

Top-level files in `docs/` are reserved for cross-cutting repo authority or shared platform references.

Examples:

- `MVP.md`
- `OWNERSHIP.md`
- `headless-data-market.md`
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
