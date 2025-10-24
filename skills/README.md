# OpenAgents Skills Registry

This folder contains trusted, Claude‑compatible skills that ship with the OpenAgents repo. Bridges can expose these to the mobile app so users always have a baseline set of high‑quality skills available without installing anything locally.

Structure
- Each skill lives in `skills/<skill-id>/` and includes a `SKILL.md` with YAML frontmatter and Markdown instructions (Claude format).
- The bridge lists both registry skills here and user‑installed skills at `~/.openagents/skills` and de‑duplicates by `skill-id` (user‑installed overrides registry on conflicts).
- The app’s Skills screen shows the merged set.

How to add a new registry skill
1. Create a new folder under `skills/your-skill-id/`.
2. Add a `SKILL.md` with at least this frontmatter:

```markdown
---
name: your-skill-id
description: A clear description of what the skill does and when to use it
---

# Your Skill Title
Instructions go here…
```

3. Run the validator:
   - `cargo run -p oa-validate -- skills/your-skill-id/SKILL.md`
4. Commit to the repo.

Runtime behavior
- The bridge watches both `~/.openagents/skills` and the repo `skills/` folder and broadcasts updates to connected clients whenever files change.
- Clients merge skills and persist them locally for instant render.

See also
- docs/skills.md — overview, best practices, and prompt integration
- docs/projects-and-skills-schema.md — schema details and WS payloads
