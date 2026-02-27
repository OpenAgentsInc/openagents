# OpenAgents Skills Registry

This directory is the project-owned Agent Skills registry.

## Listing Policy

- We generally welcome skills across domains.
- We do not list crypto-related skills unless they are centered on Bitcoin.
- Accepted crypto-adjacent examples include: using Bitcoin directly, wrapping Bitcoin, bridging to Bitcoin, or integrating with legitimate Bitcoin layers.

## Directory Contract

- Single-skill project layout: `skills/<project>/SKILL.md`
- Multi-skill project layout: `skills/<project>/<skill-name>/SKILL.md`
- Do not mix layouts within the same project namespace.
- `SKILL.md` `name` must match the containing skill directory:
  - single-skill projects: `<project>`
  - multi-skill projects: `<skill-name>`
- Use uppercase `SKILL.md` for canonical file naming.

Example:

```text
skills/
  mezo/
    SKILL.md
    references/
    scripts/
  neutron/
    validator-ops/
      SKILL.md
    wallet-ops/
      SKILL.md
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
- `metadata.oa.identifier`
- `metadata.oa.version`
- `metadata.oa.expires_at_unix`
- `metadata.oa.capabilities`
- `metadata.oa.author_npub` (optional)
- `metadata.oa.author_pubkey` (optional)
- `metadata.oa.previous_manifest_event_id` (optional)

Deprecated but still parsed for compatibility:

- `metadata.oa.nostr.*` keys

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

- Bump `metadata.oa.version` when behavior changes.
- SKL publish flow should emit updated `33400` manifest + `33401` version log for releases.

## Contribution Workflow

1. Add or update skill files under `skills/<project>/` (single-skill) or `skills/<project>/<skill-name>/` (multi-skill).
2. Run `scripts/skills/validate_registry.sh`.
3. Include the validation result in your issue/PR notes.
