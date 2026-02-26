# OpenAgents Agent Contract (MVP Mode)

## Scope

- This repository is intentionally pruned for MVP work.
- Primary authority is `docs/MVP.md`.
- If guidance conflicts, direct user instruction wins.

## Current Working Set

- Active implementation focus is `crates/wgpui/`.
- Keep `docs/MVP.md` as the product/spec authority.
- Architecture ownership boundaries are defined in `docs/OWNERSHIP.md`.

## Archived Backroom Code

- Most historical code/docs were moved to `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`.
- Do not pull archived code back by default.
- Restore pieces only when user explicitly directs it.

## Execution Rules

- Before edits, read `docs/MVP.md` and align changes to MVP scope.
- Confirm changes respect `docs/OWNERSHIP.md` crate boundaries.
- Prefer deletion/simplification over expansion unless requested.
- Keep changes small, verifiable, and directly tied to current MVP goals.
- Do not add `.github/workflows/` automation in this repo.
