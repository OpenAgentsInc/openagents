# Plans

Plans are versioned, in-repo execution artifacts intended to make long-running work reproducible and agent-legible.

Structure:
- `docs/plans/active/` (in-progress work)
- `docs/plans/completed/` (finished work; keep for history)
- `docs/plans/TEMPLATE.md` (recommended structure)

Conventions:
- Use concrete verification steps (commands, expected outputs).
- Record decisions as they happen (short decision log).
- Prefer linking to ADRs and canonical specs instead of duplicating them.
- Codex cross-surface architecture planning is canonical in `docs/codex/unified-runtime-desktop-plan.md`; avoid creating competing Codex plan docs here.
