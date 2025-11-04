# OpenAgents Skills (Claude-compatible)

This doc explains how Skills work in OpenAgents, aligned with Anthropic’s Agent Skills design so you can drop in Claude-format skills and they “just work”. It also captures best practices distilled from Anthropic’s posts and docs.

- References
  - Anthropic overview: https://www.anthropic.com/news/skills
  - Engineering deep‑dive: https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills
  - Claude Code skills docs: https://docs.claude.com/en/docs/claude-code/skills
- Our schema details: docs/projects-and-skills-schema.md
- Convex integration plan: docs/convex/projects-and-skills.md

## What is a Skill?
A Skill is a folder of instructions, scripts, and resources that agents can discover and load dynamically to perform better at specific tasks. Skills make agents composable and portable: instead of baking procedures into a single prompt, you package reusable, task‑specific knowledge as files.

- Directory layout (Claude-compatible):
  - `~/.openagents/skills/<skill-id>/`
  - Entrypoint: `SKILL.md`
  - Optional additional files referenced from `SKILL.md` (progressive disclosure)

## SKILL.md format
- YAML frontmatter (required):
  - `name`: lowercase hyphen-case identifier, must match the folder name
  - `description`: what the skill does and when to use it
- YAML frontmatter (optional):
  - `license`: short license id or filename
  - `allowed-tools`: list of tools pre‑approved to use (e.g. `bash`, `node`)
  - `metadata`: free‑form key/values for extensions
- Markdown body: instructions, examples, workflows, links to additional files.

Example frontmatter

```markdown
---
name: repo-audit
description: Audit a repository for structure, dependencies, and risks
license: Apache-2.0
allowed-tools:
  - bash
metadata:
  tags: ["engineering", "audit"]
---

# Repository Audit
…instructions…
```

## Progressive disclosure (why folders matter)
From Anthropic’s approach: agents should see only what they need when they need it.
- Startup: agent reads only `name` and `description` of all installed skills.
- On demand: when relevant, the agent loads the full `SKILL.md` into context.
- Deep dives: `SKILL.md` can reference additional files (e.g., `forms.md`, `reference.md`). The agent follows links as needed, keeping context lean while supporting arbitrarily rich skills.

OpenAgents follows this model: we keep skills as folders with `SKILL.md` and let the agent progressively load the rest as needed.

## Validation and tooling
- Validate all local skills:
  - `cargo run -p oa-validate --`
- Validate one skill:
  - `cargo run -p oa-validate -- ~/.openagents/skills/<id>/SKILL.md`
- Our validator enforces full JSON Schema (name, description; optional license, allowed-tools, metadata). See `crates/codex-bridge/schemas/skill.schema.json`.
- Bridge WS control `skills` returns the list of installed skills: `{ "control": "skills" }` → `{ "type": "bridge.skills", "items": Skill[] }`.

## How skills are injected into prompts
OpenAgents sends a concise, human‑readable preface at the top of user prompts (when “Attach preface” is enabled in Settings). This preface includes:

1) A short description of the environment (filesystem, network, approvals)
2) Active project details (name, repo, workingDir) if a project is selected
3) A concise skills summary:
   - One‑line explainer that OpenAgents supports Claude‑compatible Skills
   - A list of installed skills as “name — description” (truncated), up to 10
   - A pointer to full contents: `~/.openagents/skills/<skill-id>/SKILL.md`

This matches Anthropic’s progressive disclosure guidance: the model sees enough to know that skills exist and when to use them, but not the full skill contents by default. When the model decides to use a skill, it can read the full SKILL.md (and any linked files) from disk on demand.

Implementation details
- Code: `expo/providers/projects.tsx` in `buildHumanPreface()` composes the preface.
- It reads the current set of skills from the persisted store via `listSkills()`.
- It truncates descriptions to keep the prompt small and limits the list to 10 entries.
- If more are installed, it adds “(and N more)”.

Disabling or changing the preface
- Toggle the “Attach preface” setting if you prefer to send raw messages.
- If you want more or fewer skills listed by default, adjust the `max` value in `buildHumanPreface()`.

## Projects vs. Skills
- Projects bind an agent to a repo/workspace and configuration.
  - Location: `~/.openagents/projects/<project-id>/PROJECT.md`
  - Required fields: `name`, `workingDir`; optional `description`, `repo` metadata.
- Skills provide reusable task procedures, independent of a specific repo.
  - Location: `~/.openagents/skills/<skill-id>/SKILL.md`

Both use Markdown + frontmatter and validate via `oa-validate`.

## Best practices (adapted from Anthropic’s guidance)
- Clear triggers: describe exactly when to use the skill; include example prompts.
- Deterministic steps: numbered workflows; precise commands; expected inputs/outputs.
- Safety and guardrails: state limits, preconditions, and verification steps.
- Tool usage: if commands or runtimes are required, list them under `allowed-tools`.
- Progressive linking: split large guidance into topical files and link from `SKILL.md`.
- Observability: suggest log points or artifacts agents should produce (e.g., `audit.json`).
- Maintenance: include `license`, `version`, and `metadata.tags` to aid discovery.

## Interoperability notes
- Claude‑format drop‑in: any folder with `SKILL.md` (frontmatter described above) in `~/.openagents/skills/` will be recognized by OpenAgents.
- Legacy single‑file formats are still validated, but folders are preferred.

## Roadmap and optional enhancements
- UI surfacing: list + search skills in the app; tap to view source and run sample flows.
- Live reloads: watch the `~/.openagents/skills` directory and broadcast updates to clients.
- Editing from mobile: WS controls to save/delete skills, with schema validation on the bridge.
- Tool policy integration: map `allowed-tools` to execution policy/approvals in the bridge.
- Skill metadata conventions: adopt optional `version`, `authors`, `tags`, `category` keys.

## Examples installed
- `~/.openagents/skills/skill-creator/` is installed locally as a working example (copied from your skills repo).

## See also
- docs/projects-and-skills-schema.md (full schemas, WS payload shapes and examples)
