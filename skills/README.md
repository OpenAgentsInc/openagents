# OpenAgents Skills Registry

This directory is the project-owned Agent Skills registry.

## Skills Table Of Contents

| Skill | Description |
| --- | --- |
| [charms](./charms/SKILL.md) | Charms workflows for Bitcoin app contracts, spell proving, and UTXO asset operations. |
| [mezo](./mezo/SKILL.md) | Mezo integration workflows for apps, autonomous agents, and Mezo Earn operations. |
| [moneydevkit](./moneydevkit/SKILL.md) | Money Dev Kit workflows for Lightning checkout and agent wallets. |
| [neutronpay](./neutronpay/SKILL.md) | Neutronpay MCP and SDK workflows for Lightning, stablecoin, and fiat payments. |

## Why Skills Matter

Skills are capability bundles that let agents execute repeatable, domain-specific work safely and consistently.

In OpenAgents, skills are the practical bridge between:
- user intent ("do this task"),
- agent execution ("run this workflow"),
- and shared trust ("this is the same skill identity/version everywhere").

This aligns with the SKL protocol direction in [`crates/nostr/nips/SKL.md`](../crates/nostr/nips/SKL.md): stable skill identity, versioned manifests, and explicit capability declarations.

## How This Registry Works

- This `skills/` directory is the repo-authoritative local registry for skills used by OpenAgents.
- Skills are authored as `SKILL.md` units with progressive disclosure (body + optional `references/`, `scripts/`, `assets/`).
- Runtime surfaces discover skills from this registry and derive SKL-compatible metadata from frontmatter.
- The registry is validated by `scripts/skills/validate_registry.sh` to keep shape and metadata consistent.

## Access Model

All Autopilot agents in this OpenAgents environment are expected to have access to the skills in this registry, so these skills function as shared capabilities rather than per-agent private prompts.

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

Readability guidance:

- Keep frontmatter values short for GitHub preview readability.
- Use a concise one-line `description`.
- Put detailed usage and environment requirements in the markdown body.

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
