# OpenAgents Skills Registry

This directory is the project-owned Agent Skills registry.

## Listing Policy

- We generally welcome skills across domains.
- We do not list crypto-related skills unless they are centered on Bitcoin.
- Accepted crypto-adjacent examples include: using Bitcoin directly, wrapping Bitcoin, bridging to Bitcoin, or integrating with legitimate Bitcoin layers.

## Directory Contract

- Root layout: `skills/<project>/<skill-name>/SKILL.md`
- One skill per skill directory.
- `SKILL.md` `name` must match `<skill-name>`.
- Use uppercase `SKILL.md` for canonical file naming.

Example:

```text
skills/
  mezo/
    integration/
      SKILL.md
      references/
      scripts/
      assets/
```

## Authoring Contract

`SKILL.md` frontmatter must stay compatible with Agent Skills protocol.

Required top-level fields:

- `name`
- `description`

Allowed optional top-level fields:

- `license`
- `compatibility`
- `metadata`
- `allowed-tools`

Do not add non-spec top-level fields.

## OpenAgents Nostr Mapping Contract

OpenAgents-specific SKL bridge metadata lives under `metadata.oa.*` only.

Recommended keys:

- `metadata.oa.project`
- `metadata.oa.nostr.identifier`
- `metadata.oa.nostr.version`
- `metadata.oa.nostr.expiry_unix`
- `metadata.oa.nostr.capabilities_csv`
- `metadata.oa.nostr.author_npub` (optional)

## Validation Contract

Validate changed skills:

```bash
scripts/skills/validate_registry.sh
```

The validator uses `skills-ref validate` and falls back to running local `agentskills/skills-ref` via `uv` when `skills-ref` is not on PATH.

Validation fails on:

- invalid frontmatter
- unexpected top-level fields
- name/directory mismatch

## Versioning Contract

- Bump `metadata.oa.nostr.version` when behavior changes.
- SKL publish flow should emit updated `33400` manifest + `33401` version log for releases.

## Contribution Workflow

1. Add or update skill files under `skills/<project>/<skill-name>/`.
2. Run `scripts/skills/validate_registry.sh`.
3. Include the validation result in your issue/PR notes.
