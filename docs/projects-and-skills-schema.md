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
