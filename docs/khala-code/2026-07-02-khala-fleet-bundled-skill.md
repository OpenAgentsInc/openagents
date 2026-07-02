# Khala Fleet Bundled Skill

Date: 2026-07-02
Status: shipped. Khala Code now bundles a `khala-fleet` skill — generic
fleet-management best practices (connect, dispatch ladder, claims,
closeout/token verification, failure diagnosis, hard guardrails) in the
standard `SKILL.md` shape any skills-aware agent can consume.

## Canonical source

`<repo root>/.agents/skills/khala-fleet/SKILL.md`

Because it lives in the repo's `.agents/skills/` root, Codex sessions working
inside this repository already discover it at **repo scope** with no install
step. The skill is a launcher, not the law: it points at the delegation
runbook in `AGENTS.md`/`CLAUDE.md`, the ops runbooks, and the fleet spec,
which stay authoritative.

## Default install in the Khala Code flow

On startup (desktop and headless), Khala Code Desktop materializes bundled
skills into the user-scope shared skills root:

- Target: `~/.agents/skills/khala-fleet/SKILL.md` — the `$HOME/.agents/skills`
  convention the Codex harness loads as a user-scope skill root, alongside
  `$CODEX_HOME/skills` and repo `.agents/skills` roots.
- Wiring: `src/bun/index.ts` → `ensureKhalaCodeDesktopBundledSkillsInstalled`
  (`src/bun/khala-bundled-skills.ts`). Fail-soft: a write failure logs a
  warning and never blocks startup.
- The Codex harness picks the skill up via its normal skills discovery;
  it appears in `skills/list` (surfaced through the ecosystem/Inbox
  diagnostics) and is invocable like any other skill.

### Toggle

Default-on. Disable with `KHALA_CODE_DESKTOP_BUNDLED_SKILLS=0` (also accepts
`false`/`off`). The env key is registered in `khala-code-config.ts`; a
Settings-panel toggle can layer on later without changing the seam.

### Overwrite policy

Every bundled copy carries the marker `managed-by: khala-code`:

- absent file → installed
- managed file with stale content → upgraded in place
- file **without** the marker → treated as user-owned and never touched
  (`user_owned` status)

## Updating the skill

1. Edit `<repo root>/.agents/skills/khala-fleet/SKILL.md` (bump the version
   in the managed-by comment).
2. Regenerate the embedded content module:
   `bun clients/khala-code-desktop/scripts/sync-bundled-skills.ts`
   (writes `src/bun/khala-bundled-skill-content.generated.ts`).
3. `bun test tests/khala-bundled-skills.test.ts` — the suite pins the
   generated module byte-for-byte against the canonical file, checks the
   frontmatter contract (`name`, `description`, managed marker), and covers
   install/upgrade/user-owned/disabled/no-home behavior.

## Invariants kept

- The skill text is public-safe: no owner paths, no secrets, no private
  refs; it teaches the same guardrails the runbooks enforce (isolated homes,
  never `~/.codex`, exact-only token accounting, approval-per-run-start,
  counter movement is never proof).
- Installation only ever writes inside `~/.agents/skills/<name>/` and only
  files it owns by marker.
