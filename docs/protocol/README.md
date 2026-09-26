# Nostr protocol implementation

Use this section to distinguish a pinned specification, a parser, an enforced
relay role, and a complete application. The [NIP directory](../../nips/README.md)
contains normative protocol sources; these documents explain implementation,
operations, gaps, and evidence.

## Start here

| Need | Document |
| --- | --- |
| Current role coverage and limitations | [Implementation coverage](2026-09-26-nip-implementation-coverage.md) |
| Remaining protocol work and completion criteria | [Implementation plan](implementation-plan.md) |
| Exact upstream sources | [NIP manifest](../../nips/manifest.json) and [source-sync review](2026-09-26-upstream-nip-sync.md) |
| Run the relay | [Deployment index](../deployment/README.md) |
| Current Coder application delivery | [Migration tracker](../coder/migration-status.md) and [master roadmap](../roadmap.md) |
| Cryptographic primitive evidence | [Nostr implementation notes](../nostr/README.md) |

The relay stores and routes protocol records. It does not authorize a host
operation, determine whether a task succeeded, or settle a payment. The
[free labor runtime](../coder/runtime/free-labor.md) is a separately admitted,
free-only host role with synthetic loopback evidence; it does not establish a
public marketplace or paid provider deployment.

## Implemented relay surfaces

| Document | Scope |
| --- | --- |
| [Official NIP fixture ledger](official-nip-ledger.md) | Source inventory and the narrower fixture-backed client and relay checks. |
| [Block NIP fixture ledger](block-nip-ledger.md) | Pinned Block inventory, implemented helpers, and unsupported application roles. |
| [Block handlers](block-nips.md) | Server behavior, configuration, visibility, and deliberate refusals. |
| [Protocol expansion](nip-expansion.md) | Relay subsets for groups, management, search, and related official NIPs. |
| [Media contract](media.md) | Blossom-compatible storage with the explicitly different NIP-98 authentication profile. |
| [OpenAgents retention](openagents-retention.md) | Storage classes, private visibility, acknowledgments, and optional advertisement. |

## Dated reviews and retained evidence

| Document | What it records |
| --- | --- |
| [September 26 upstream sync](2026-09-26-upstream-nip-sync.md) | Source revisions and gaps at the source-only refresh. |
| [Official lane review](2026-09-26-upstream-nip-sync-official.md) | Changed and added official specifications. |
| [Block lane review](2026-09-26-upstream-nip-sync-block.md) | Changed and added Buzz specifications. |
| [OpenAgents gap review](2026-09-26-openagents-gap-review.md) | CTRL/MKT/LAB rationale, kind allocation, and reuse across lanes. |
| [Teardown coverage](2026-09-26-teardown-coverage.md) | File-by-file historical design coverage; not a deployment inventory. |
| [September 26 verification](verification/2026-09-26-nips/README.md) | Exact tested code, failures, corrections, live Postgres acceptance, and omitted checks. |

Dated reviews retain the state they assessed. A newer index or closed issue
does not rewrite their failures, source pins, commands, or outcome denominators.
Use the implementation coverage report for later changes and the linked raw
records for what actually ran.
