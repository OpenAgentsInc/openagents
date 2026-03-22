# OpenAgents Skills Registry

This directory is the project-owned Agent Skills registry.

## Skills Table Of Contents

| Skill | Description |
| --- | --- |
| [blink](./blink/SKILL.md) | Blink Lightning wallet for agents — balances, invoices, payments, QR codes, price conversion, and transaction history. |
| [autopilot-cad-builder](./autopilot-cad-builder/SKILL.md) | Deterministic CAD design/build orchestration for Autopilot Chat using `openagents.cad.*` and pane tools. |
| [autopilot-pane-control](./autopilot-pane-control/SKILL.md) | OpenAgents desktop pane and CAD control for Codex via `openagents.*` tool calls. |
| [autopilot-data-seller](./autopilot-data-seller/SKILL.md) | Conversational seller-authoring policy for DS-first Data Market listings in the dedicated `Data Seller` pane. |
| [autopilot-data-seller-cli](./autopilot-data-seller-cli/SKILL.md) | Shell-first DS-first Data Market packaging, publication, and lifecycle control through `autopilotctl` and the headless runtime. |
| [autopilot-data-market-control](./autopilot-data-market-control/SKILL.md) | Typed OpenAgents DS-first Data Market tool contract for seller publication and authority read-back. |
| [charms](./charms/SKILL.md) | Charms workflows for Bitcoin app contracts, spell proving, and UTXO asset operations. |
| [cast](./cast/SKILL.md) | Charms CAST DEX workflows for order creation, cancellation/replacement, partial fulfillment, signing, and transaction verification on Bitcoin. |
| [l402](./l402/SKILL.md) | L402 agent commerce workflows with lnd, lnget, scoped macaroons, aperture, and MCP. |
| [maestro](./maestro/SKILL.md) | Maestro Symphony blockchain query operations for OpenAgents agents, including tip freshness checks and address/UTXO/runes queries against deployed Symphony endpoints. |
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

## Blink Quickstart

Required env vars live in `.env.local` (see `.env.example` for placeholders).

```bash
# Check balances
source ~/.profile && node skills/blink/scripts/balance.js

# Create a 1,000 sat invoice (auto-subscribes for payment)
source ~/.profile && node skills/blink/scripts/create_invoice.js 1000 "Payment for service"
```

## Data Market Skill Quickstart

The repo currently ships three first-party Data Market skills with distinct jobs:

- `autopilot-data-seller`
  - use when the agent is operating inside the dedicated `Data Seller` conversational pane
  - governs seller drafting, exact preview, confirm-before-publish, and lifecycle discipline
- `autopilot-data-market-control`
  - use when the agent needs the typed `openagents.data_market.*` tool contract
  - keeps seller publication and authority read-back bounded to the app-owned data-market tools
- `autopilot-data-seller-cli`
  - use for shell-first or no-window operation through `autopilotctl` and `autopilot_headless_data_market`
  - packages local files/directories for sale and drives the same seller lifecycle without bootstrapping the visible UI
  - the deterministic publish path is local `nexus-control` plus packaged CLI flow, with `seller-prompt` reserved for intentional conversational terminal use
  - now also includes a dedicated redacted Codex conversation packager for selling `.codex/sessions` material without publishing raw rollout files

Current supporting docs:

- [../docs/headless-data-market.md](../docs/headless-data-market.md)
- [../docs/kernel/markets/data-market.md](../docs/kernel/markets/data-market.md)
- [../docs/audits/2026-03-21-ds-first-headless-data-market-paid-e2e-audit.md](../docs/audits/2026-03-21-ds-first-headless-data-market-paid-e2e-audit.md)

Current transport truth for the Data Market MVP:

- targeted DS-DVM request kind `5960`
- targeted DS-DVM result kind `6960`
- NIP-89 handler/capability kind `31990`

Current verification truth for the Data Market MVP:

- `scripts/autopilot/verify-data-market-cli-headless.sh` is the portable local DS-first launch gate
- repo-owned harnesses set `OPENAGENTS_DISABLE_CODEX=true` because they exercise the typed CLI path rather than the conversational seller lane
- normal operator runs keep Codex enabled so `autopilotctl data-market seller-prompt ...` remains usable

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
