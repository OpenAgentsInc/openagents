# Documentation

OpenAgents builds Coder, shared agent infrastructure, decision services, and
Nostr contracts. Start with the [master roadmap](roadmap.md) for the complete
direction, the [glossary](glossary.md) for terms and implementation status, or
the [document catalog](catalog.md) to find a specific reference.

## Find the right guide

| Goal | Start here |
| --- | --- |
| Reach every surface from one command | [The `openagents` command](cli/README.md) |
| Use the terminal and follow the workbench release | [Terminal user guide](terminal/README.md), [Grid/standalone and Everglade roadmap](terminal/workbench-roadmap.md), [issue directory and project](terminal/issue-roadmap.md), [smart terminal specification](terminal/smart-terminal.md) |
| Use or develop Coder | [Coder](coder/README.md), [installation](coder/guides/install.md), [task commands](coder/guides/tasks.md) |
| See every UI system and component across platforms | [UI inventory](ui/inventory.md) |
| Build shared terminal, native, and web interfaces | [Rust Native](../crates/rust-native/README.md), [styling](coder/rust-native/styling-design.md), [adoption plan](coder/rust-native/adoption.md) |
| Follow suite delivery and ownership | [Migration tracker](coder/migration-status.md), [master roadmap](roadmap.md) |
| See what ships to playtesters and what comes next | [Launch roadmap, 2026-09-29](roadmap/2026-09-29-launch-roadmap.md), [playtesting program](game/playtesting.md) |
| Design the phone app's screens and user flow | [App wireframe specification](product/2026-09-28-app-wireframe.md) |
| Grow the playtest cooperative and the following | [Indie studio viral roadmap](growth/2026-09-28-indie-studio-viral-roadmap.md) |
| Plan the Wallet and agent payments | [Breez and Spark](breez/README.md), [Bitcoin](bitcoin/README.md) |
| Understand the coding and network thesis | [Coder design index](coder/design/README.md), [networked Coder](coder/design/networked-coder-plan.md) |
| Understand test-time compute and the capabilities agents gain at run time | [Test-Time Capabilities](essays/2026-09-29-test-time-capabilities.md) (essay) |
| Present a talk from the desktop | [OpenAgents deck](../crates/openagents-deck/README.md), [test-time capabilities slides](decks/test-time-capabilities/) |
| Observe and control a task over Nostr | [Scoped control host and client](coder/runtime/nostr-task-control.md) |
| Make, test, and publish a plugin | [Plugins](plugins/README.md) |
| Reuse knowledge and components | [Knowledge](coder/guides/knowledge-base.md), [extension packages](extensions/README.md), [programs](programs.md) |
| Build agent labor | [Market infrastructure](agents/market-infrastructure.md), [free labor host](coder/runtime/free-labor.md) |
| Inspect decision and coding evidence | [Gym](gym/README.md), [Terminal-Bench](terminal-bench/README.md) |
| Make delegated runs cheaper and faster than raw Codex or Claude Code | [System One cost efficiency audit](cost/2026-10-02-system-one-cost-efficiency-audit.md) |
| Run everything on your own OpenRouter, Vercel AI Gateway, or TypeSafe key (BYOK) | [BYOK design](byok/README.md) |
| Put OpenAgents itself behind an API for partner apps, websites, other agents, and self-hosters | [OpenAgents API](api/README.md): plain HTTP, x402 payment |
| Receive every payment centrally, split it with plugin authors, pay them out, and watch it live | [Payments](payments/README.md): one receiver, a split ledger, payouts, `/live` |
| Earn revenue from Coder: what businesses want, pricing, referrals, partners, and the sales roadmap | [Sales](sales/README.md), [revenue roadmap](sales/revenue-roadmap.md), [agent sales floor in Everglade](sales/agent-sales-floor.md) |
| Call or operate decision services | [Decision models](decision-models/README.md), [caller guide](decision-models/guides/caller.md), [gateway](decision-models/service/gateway.md) |
| Call the Pro inference door (GPT-5.6 Sol, Terra, and Luna) | [Pro inference door](gateway/README.md) |
| Plan our own inference gateway: every model account behind one Open Responses API, for our apps and the public | [Inference gateway spec](inference/gateway.md) |
| Work on model implementations | [Kev](kev/README.md), [Lev](lev/README.md), [Laya](laya/README.md) |
| Understand Nostr support | [Protocol index](protocol/README.md), [coverage](protocol/2026-09-26-nip-implementation-coverage.md), [specifications](../nips/README.md) |
| Operate the relay | [Deployment](deployment/README.md) |
| Plan CoderOS | [CoderOS index](os/README.md), [audit of what moves from the private tree](os/2026-09-28-coderos-audit.md) |
| Run reliable, user-defined background processes such as disk cleanup | [Background processes](background/README.md) |
| Fan Coder runs out onto Google Cloud machines | [Cloud](cloud/README.md), [parallel execution audit](cloud/2026-10-02-cloud-parallel-execution-audit.md) |
| Sign people in: accounts, GitHub sign-in, sessions, and what comes next | [Authentication](auth/README.md), [GitHub sign-in](auth/github.md) |
| Build Coder Cloud and the openagents.com work, Verse, billing, and sales interfaces | [Coder Cloud web specification](cloud/coder-cloud.md) |
| Build general agents and optimization | [Agent architecture](agents/README.md), [optimization](optimization/README.md) |
| Explore world interfaces | [Voyager](voyager/README.md), [Minecraft](minecraft/README.md), [Verse](verse/README.md), [Gym building](verse/gym.md), [Unreal source study](research/unreal/README.md) |
| Delegate work to Alice, the workshop agent, and supervise her | [Alice runbook](verse/alice-runbook.md), [workshop agent](verse/workshop-agent.md) |
| Delegate coding to the local Devin CLI through Coder | [Devin runbook](verse/devin-runbook.md), [the Devin route](coder/runtime/devin.md) |
| Verify a change | [Targeted development and release verification](verification.md) |
| Understand earlier decisions | [Historical surveys](history/README.md), [audits](audits/README.md), [transcript archive](transcripts/README.md) |
| Trace the history of selling spare compute for bitcoin: GPUtopia, Pylon, and the compute market | [Compute](compute/README.md), [compute for bitcoin history](compute/compute-for-bitcoin.md) |
| Plan compute in the Verse: the Pylon Field, the Wellspring, paid jobs, and the agent market in Everglade | [Verse compute vision and spec](compute/verse-compute.md), [NIP-PYLON](../nips/openagents/NIP-PYLON.md) |

## Read claims at their stated scope

Runtime guides describe supported code paths and limits. Design documents
describe intended behavior, including work that is not implemented. Dated
measurements retain a particular configuration and result; they do not certify
today's default. A NIP specifies a contract, while its implementation coverage
report identifies the roles that code actually supports.

Rust Native's experimental core implements bounded semantic views, typed UI
intents, and deterministic generic style composition. Coder's amber palette
lives separately in `coder-ui`. The [Coder iOS reader](coder/guides/mobile-readonly.md)
implements SwiftUI lists and transcripts over Rust-owned retained history.
The existing UIKit probe is separate. Web adapters, complete task control, and
the wider cross-platform client remain roadmap work.

The master roadmap owns cross-project priorities. The migration tracker owns
suite packages and issue claims. Each domain index links its current runtime
guides and retained evidence. Avoid copying changing result tables between
these pages.

See [documentation maintenance](documentation.md) for ownership, consolidation,
and archival rules, and the [September 26 review](maintenance/2026-09-26-documentation-review.md)
for this organization pass. Transcripts and raw evidence are retained sources,
not material to rewrite when updating current guidance.
