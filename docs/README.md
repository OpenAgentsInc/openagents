# Documentation

OpenAgents builds Coder, shared agent infrastructure, decision services, and
Nostr contracts. Start with the [master roadmap](roadmap.md) for the complete
direction, the [glossary](glossary.md) for terms and implementation status, or
the [document catalog](catalog.md) to find a specific reference.

## Find the right guide

| Goal | Start here |
| --- | --- |
| Use or develop Coder | [Coder](coder/README.md), [installation](coder/guides/install.md), [task commands](coder/guides/tasks.md) |
| Follow suite delivery and ownership | [Migration tracker](coder/migration-status.md), [master roadmap](roadmap.md) |
| Understand the coding and network thesis | [Coder design index](coder/design/README.md), [networked Coder](coder/design/networked-coder-plan.md) |
| Observe and control a task over Nostr | [Scoped control host and client](coder/runtime/nostr-task-control.md) |
| Reuse knowledge and components | [Knowledge](coder/guides/knowledge-base.md), [extensions](extensions/README.md), [programs](programs.md) |
| Build agent labor | [Market infrastructure](agents/market-infrastructure.md), [free labor host](coder/runtime/free-labor.md) |
| Inspect decision and coding evidence | [Gym](gym/README.md), [Terminal-Bench](terminal-bench/README.md) |
| Call or operate decision services | [Decision models](decision-models/README.md), [caller guide](decision-models/guides/caller.md), [gateway](decision-models/service/gateway.md) |
| Work on model implementations | [Kev](kev/README.md), [Lev](lev/README.md), [Laya](laya/README.md) |
| Understand Nostr support | [Protocol index](protocol/README.md), [coverage](protocol/2026-09-26-nip-implementation-coverage.md), [specifications](../nips/README.md) |
| Operate the relay | [Deployment](deployment/README.md) |
| Build general agents and optimization | [Agent architecture](agents/README.md), [optimization](optimization/README.md) |
| Explore world interfaces | [Voyager](voyager/README.md), [Minecraft](minecraft/README.md), [Verse](verse/README.md) |
| Verify a change | [Targeted development and release verification](verification.md) |
| Understand earlier decisions | [Historical surveys](history/README.md), [audits](audits/README.md), [transcript archive](transcripts/README.md) |

## Read claims at their stated scope

Runtime guides describe supported code paths and limits. Design documents
describe intended behavior, including work that is not implemented. Dated
measurements retain a particular configuration and result; they do not certify
today's default. A NIP specifies a contract, while its implementation coverage
report identifies the roles that code actually supports.

The master roadmap owns cross-project priorities. The migration tracker owns
suite packages and issue claims. Each domain index links its current runtime
guides and retained evidence. Avoid copying changing result tables between
these pages.

See [documentation maintenance](documentation.md) for ownership, consolidation,
and archival rules, and the [September 26 review](maintenance/2026-09-26-documentation-review.md)
for this organization pass. Transcripts and raw evidence are retained sources,
not material to rewrite when updating current guidance.
