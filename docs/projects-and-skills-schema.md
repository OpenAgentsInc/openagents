# Projects and Skills Schema

This document defines the on‑disk and over‑WS formats used by the bridge and app for Projects and Skills.

## Locations

- OPENAGENTS_HOME: defaults to `~/.openagents` (override via env var `OPENAGENTS_HOME`).
- Projects directory: `~/.openagents/projects/`
- Skills directory: `~/.openagents/skills/`

## File formats: folder + YAML frontmatter

Both Projects and Skills are Markdown files that begin with a YAML frontmatter block delimited by `---` lines. The bridge parses only the frontmatter for metadata and preserves a free‑form Markdown body for humans and agents.

### Project folder

- Path: `~/.openagents/projects/{id}/`
- Entrypoint file: `PROJECT.md`
- Frontmatter (camelCase keys; validated by `project.schema.json`):

```yaml
---
name: string                   # human name, used for display
workingDir: string             # absolute path for agent `cd`
repo:                          # optional repository metadata
  provider: github|gitlab|other
  remote: owner/name
  url: https://…
  branch: main
agentFile: string              # optional, e.g. path to an agent config file
instructions: string           # optional free‑text guidance
# Optional extras for future versions
# approvals: never|on-request|on-failure
# model: string
# sandbox: danger-full-access|workspace-write|read-only
# createdAt: 0
# updatedAt: 0
---

# Overview
...free‑form Markdown body...
```

- Notes:
  - The bridge maps `name` (or the file stem) to `id` when needed.
  - `instructions` may be derived from `description` if present.

### Skill folder (Claude-compatible)

- Path: `~/.openagents/skills/{id}/`
- Entrypoint file: `SKILL.md`
- Frontmatter (validated by `skill.schema.json` and compatible with Anthropic Agent Skills):

```yaml
---
name: string                   # lowercase hyphen-case; must equal folder name
description: string            # what the skill does / when to use
license: string                # optional
allowed-tools:                 # optional
  - bash
  - node
metadata: {}                   # optional; arbitrary key/values
---

## Instructions
Step‑by‑step instructions the agent can follow.

## Workflow
Bullet list of the recommended flow, commands to run, files to read, etc.
```

- Notes:
  - Skills are conceptually distinct from Projects. Use a Project to bind an agent to a repo and working directory; use a Skill to define reusable procedures.

## Bridge WS payloads

- List projects: `{ "control": "projects" }`
  - Response: `{ "type": "bridge.projects", "items": Project[] }`
  - Project fields (snake_case when coming from Rust): `id, name, working_dir, repo, agent_file, instructions, todos?, approvals?, model?, sandbox?, created_at?, updated_at?`

- Save project: `{ "control": "project.save", "project": Project }`
  - Writes `{id}.project.md` and replies with the updated list.

- Delete project: `{ "control": "project.delete", "id": string }`
  - Removes the file and replies with the updated list.

(WS endpoints for Skills mirror the above and can be added where needed.)

- List skills: `{ "control": "skills" }`
  - Response: `{ "type": "bridge.skills", "items": Skill[] }`
  - Skill fields (snake_case from Rust): `id, name, description, license?, allowed_tools?, metadata?`

## App mapping

- ProjectsProvider requests the project list via WS on mount and seeds the persisted store, preserving instant rehydrate. See `expo/providers/projects.tsx`.
- The app currently reads Projects from WS; Skills are local files used by humans/agents (future UI can expose them with the same pattern).

## Convex mapping (proposed + in progress)

To unify persistence and enable multi‑client sync, Projects and Skills are also mirrored into Convex tables. Files under the user’s OpenAgents home remain the source of truth; the bridge syncs them into Convex on startup and on file changes.

- Source of truth: on‑disk folders under `~/.openagents/projects/` and `~/.openagents/skills/` (plus repo `skills/` for registry defaults).
- Sync direction: Filesystem → Convex (initial). Future two‑way edits will write to disk first, then update Convex.
- App reads: via Convex queries/subscriptions for Projects/Skills once the migration is complete.

Tables (Convex)
- `projects`
  - Fields: `id` (string), `name`, `workingDir`, `repo` (object), `agentFile?`, `instructions?`, `createdAt`, `updatedAt`.
  - Indexes: `by_id (id)`, `by_name (name)`.
- `skills`
  - Fields: `skillId` (string), `name`, `description`, `license?`, `allowed_tools?`, `metadata?`, `source` ('user' | 'registry' | 'project'), `projectId?`, `path?`, `createdAt`, `updatedAt`.
  - Indexes: `by_skill_source_project (skillId, source, projectId)`, `by_project (projectId)`.

Server functions (Convex)
- `projects:list`, `projects:byId`, `projects:upsertFromFs`, `projects:remove`
- `skills:listAll`, `skills:listByScope`, `skills:upsertFromFs`, `skills:bulkUpsertFromFs`, `skills:removeByScope`

Scopes for Skills
- `user`: personal skills under `~/.openagents/skills`.
- `registry`: repo‑bundled skills under `skills/` in this repo (baseline set; personal overrides on id conflicts).
- `project`: skills bundled inside a project’s repo (e.g., `<project workingDir>/skills/…`). These are only active for that project; stored with `projectId`.

Migration plan
1) Bridge watchers read folders and call `skills:bulkUpsertFromFs` and `projects:upsertFromFs` on changes.
2) App switches Projects/Skills providers to use Convex queries (keep local store for instant rehydrate).
3) Optional: add Convex mutations for in‑app editing that write to disk via bridge, then re‑sync into Convex.

## Examples

- Project example folder: `~/.openagents/projects/tricoder/PROJECT.md`
- Skill example folder: `~/.openagents/skills/skill-creator/SKILL.md`

## Validation CLI

Use the validator to check that frontmatter conforms to the JSON Schemas:

- Validate all projects and skills (default):
  - `cargo run -p oa-validate --`
- Validate specific files:
  - `cargo run -p oa-validate -- ~/.openagents/projects/tricoder.project.md`
  - `cargo run -p oa-validate -- ~/.openagents/skills/repo-audit.skill.md`

Notes:
- The validator performs full JSON Schema validation using the bundled schemas:
  - `crates/codex-bridge/schemas/project.schema.json`
  - `crates/codex-bridge/schemas/skill.schema.json`
- Exit code is non‑zero if any files fail; output lists per‑file errors.

## Versioning

- The frontmatter is intentionally simple. If future fields are added, keep them optional and consider adding `schemaVersion: 1` to frontmatter for migrations.
