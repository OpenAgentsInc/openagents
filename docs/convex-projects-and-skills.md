---
title: Projects & Skills in Convex
---

# Projects & Skills in Convex

Goal
- Keep the user’s OpenAgents home directory as the source of truth for Projects and Skills, while mirroring into Convex for multi‑client sync, search, and subscriptions.

Current state
- Filesystem formats are stable (Markdown + YAML frontmatter). See `docs/projects-and-skills-schema.md`.
- Bridge exposes WS controls for listing/saving/deleting Projects and listing Skills.
- App uses Convex for Threads/Messages and still seeds Projects/Skills via bridge WS.

Proposed model
- Source of truth: on‑disk folders `~/.openagents/projects/` and `~/.openagents/skills/` (+ repo `skills/`).
- Sync direction: Filesystem → Convex (initial). Writes from the app continue to go through the bridge to disk, and the bridge re‑syncs to Convex.
- App reads: switch Projects/Skills providers to Convex queries for live updates; keep local persisted stores for instant rehydrate.

Skill scopes
- personal (`user`): `~/.openagents/skills/<id>/SKILL.md` — available in all projects.
- registry (`registry`): repo‑bundled `skills/<id>/SKILL.md` — baseline set; overridden by personal skills on id conflicts.
- project (`project`): `<project workingDir>/skills/<id>/SKILL.md` — only active for that project.

Convex tables
- `projects(id, name, workingDir, repo, agentFile?, instructions?, createdAt, updatedAt)`
  - Indexes: `by_id(id)`, `by_name(name)`
- `skills(skillId, name, description, license?, allowed_tools?, metadata?, source, projectId?, path?, createdAt, updatedAt)`
  - Indexes: `by_skill_source_project(skillId, source, projectId)`, `by_project(projectId)`

Server functions
- Projects: `projects:list`, `projects:byId`, `projects:upsertFromFs`, `projects:remove`
- Skills: `skills:listAll`, `skills:listByScope`, `skills:upsertFromFs`, `skills:bulkUpsertFromFs`, `skills:removeByScope`

Bridge syncers (filesystem → Convex)
- On startup:
  - Scan `~/.openagents/projects/**/PROJECT.md` → call `projects:upsertFromFs`.
  - Scan personal `~/.openagents/skills/**/SKILL.md` → call `skills:upsertFromFs` with `source='user'`.
  - Scan repo registry `./skills/**/SKILL.md` → `source='registry'`.
  - For each Project `workingDir`, if `skills/` exists, scan as `source='project'` with `projectId`.
- On file events: re‑call the relevant upsert/remove function with `source` and `projectId`.

Implemented
- Projects watcher: watches `~/.openagents/projects` recursively and syncs Projects (and re‑scans project‑scoped `skills/`).
- Skills watcher: watches both personal `~/.openagents/skills` and repo `./skills` and syncs to Convex on changes.
- Initial sync: runs at bridge startup for Projects + all skill scopes.
- Deletion handling: on each sync, the bridge queries Convex and removes any `projects` or `skills` rows not present on disk (by id and scope).

Controls
- Set `OPENAGENTS_CONVEX_SYNC=0` to disable FS→Convex sync/watchers (enabled by default).

App wiring
- Projects: replace WS seeding in `expo/providers/projects.tsx` with `useQuery('projects:list', {})` (keep persisted store for instant rehydrate).
- Skills: replace WS subscription in `expo/providers/skills.tsx` with `useQuery('skills:listAll', {})` or project‑filtered queries.
- Preface injection (`buildHumanPreface`): read from Convex store instead of the local skills store.

Notes & tradeoffs
- We avoid blanket find/replace or touching lockfiles; only schema and functions under `convex/` and docs are added.
- Two‑way editing: initial phase keeps disk as canonical; later we can add Convex mutations that write to disk (via bridge) then re‑upsert.
- De‑duplication precedence: personal (`user`) wins over `registry` for the same `skillId`. Project‑scoped entries are separate and only resolved when a project is active.

References
- Filesystem schema: `docs/projects-and-skills-schema.md`
- Convex schema: `convex/schema.ts`
- Convex functions: `convex/projects.ts`, `convex/skills.ts`
