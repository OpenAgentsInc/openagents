# Launch Promise Source Set

This page records the launch-promise audit docs and the verified closed issue
set that should be treated as source context for the broader product-promises
system.

## Audit Docs Found

| Source                                                                                                                                                                 | Role in the product-promises system                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md`](../../apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md)               | Original promise inventory and live-state gap audit. It separates safe copy from unsafe copy and shows which claims were ahead of live evidence.                                             |
| [`apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`](../../apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md) | Current Pylon v0.3 reconfiguration audit. It moves the launch promise set from old repo assumptions to the new Pylon ownership model and keeps v0.3 claims gated until live evidence exists. |
| [`apps/openagents.com/docs/2026-06-08-openagents-public-launch-dashboard.md`](../../apps/openagents.com/docs/2026-06-08-openagents-public-launch-dashboard.md)         | Machine-checkable launch truth surface using red, yellow, and green promise rows with evidence refs, blocker refs, safe copy, and unsafe copy.                                               |
| [`apps/openagents.com/docs/2026-06-08-public-launch-copy-gate.md`](../../apps/openagents.com/docs/2026-06-08-public-launch-copy-gate.md)                               | Copy gate for launch surfaces. It blocks unsupported affirmative launch claims unless matching evidence gates are green.                                                                     |
| [`apps/openagents.com/docs/2026-06-08-pylon-install-to-bitcoin-launch-smoke.md`](../../apps/openagents.com/docs/2026-06-08-pylon-install-to-bitcoin-launch-smoke.md)   | Install-to-Bitcoin smoke contract covering install, registration, heartbeat, wallet readiness, assignment, closeout, payment, settlement, and projection refs.                               |
| [`apps/openagents.com/docs/forum/launch-gates.md`](../../apps/openagents.com/docs/forum/launch-gates.md)                                                               | Forum launch gates for posting, moderation, reports, tipping, payment redaction, and launch-safe public status.                                                                              |
| [`apps/pylon/docs/launch-gates-no-overclaim.md`](../../apps/pylon/docs/launch-gates-no-overclaim.md)                                                                   | Pylon-side no-overclaim gate guidance for launch copy and evidence claims.                                                                                                                   |

## Recent Transcript Sources

The product-promise backlog in [`registry.md`](registry.md) started with a
2026-06-09 sweep of transcript files from Episode 199 onward and now includes
later transcript updates as they land. Episode 199 is included as historical
launch framing with an explicit caveat: the old Claude Code-first mech-suit
direction is not current public positioning. Current implementation work
should be understood as Codex-oriented, with applicable ideas folded into
Probe/Pylon. These transcripts are source material for explicit and implied
promise candidates, not proof that a promise is green.

| Transcript                        | Promise areas surfaced                                                                                                                                                                            |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`199.md`](../transcripts/199.md) | Historical Autopilot launch framing, autonomous coding loop claims, old Claude Code-first mech-suit direction, Codex successor framing, Probe/Pylon folding, agent learning, and paid skills.     |
| [`200.md`](../transcripts/200.md) | Autonomous agents that learn from long-lived work, repeated task completion, approval loops, high-level objective handling, and explicit risks around autonomy and lock-in.                       |
| [`201.md`](../transcripts/201.md) | Issue-to-PR Autopilot loops, agents learning from other agents, local model evaluation, task execution traces, and economic incentives for productive software work.                              |
| [`202.md`](../transcripts/202.md) | Skill marketplaces, royalties for reusable agent assets, agent groups that form around tasks, Nostr-like distribution, and agent-created products that other agents can buy or reuse.             |
| [`203.md`](../transcripts/203.md) | Budget-bound autonomous agents, revenue-split workrooms, agents creating and selling work, human control boundaries, and marketplace primitives for agent work.                                   |
| [`204.md`](../transcripts/204.md) | Autopilot as a two-sided task market, open-source agents taking paid work, agent memory and learning claims, and open marketplace direction.                                                      |
| [`205.md`](../transcripts/205.md) | Local Apple Silicon orchestration, spare-device compute use, cluster-style runtime direction, and user-visible promises around running useful agent work on owned hardware.                       |
| [`206.md`](../transcripts/206.md) | Pylon/open compute market framing, wallets, device registration, model/runtime support, and earning money by contributing compute or agent labor.                                                 |
| [`207.md`](../transcripts/207.md) | Compute-market plumbing, receipts, routing, evidence, dashboards, Nostr relay direction, and revenue settlement surfaces.                                                                         |
| [`208.md`](../transcripts/208.md) | Pylon/Nexus/NIP-90 style network design, request and fulfillment flow, open relay participation, and measurable agent-service marketplace behavior.                                               |
| [`209.md`](../transcripts/209.md) | Mainnet Bitcoin transition, live payments, public payout evidence, and strict need to separate experimental/devnet claims from production settlement claims.                                      |
| [`210.md`](../transcripts/210.md) | Provider independence, no-break userspace migration constraints, Autopilot full-auto desktop direction, agent-owned keys, and wallet-linked agent operation.                                      |
| [`211.md`](../transcripts/211.md) | Agent Lightning/L402 API access, public docs for paid APIs, account/wallet setup, and agent-consumable payment flows.                                                                             |
| [`212.md`](../transcripts/212.md) | Forum as open agent social layer, agent comments and reports, protocol-native reputation, Bitcoin/Nostr integration, and social routing for loose product feedback.                               |
| [`213.md`](../transcripts/213.md) | Nostr protocol primitives, sovereign agent keys, portable identity, public/private separation, and agent-to-agent coordination constraints.                                                       |
| [`214.md`](../transcripts/214.md) | Agent credit, microloans, repayment evidence, economic identity, and caution around claims that require underwriting or durable repayment systems.                                                |
| [`215.md`](../transcripts/215.md) | Hatchery/OpenClaw setup, automated environment bootstrap, installing agent runtime dependencies, and self-hosted agent onboarding.                                                                |
| [`216.md`](../transcripts/216.md) | Autopilot auto-upgrade of personal agents, release channels, user-owned persistent agents, and upgrade safety promises.                                                                           |
| [`217.md`](../transcripts/217.md) | Liquidity, yield, bonds, risk markets, underwriting, and storage/sandbox primitives as future economic layers that must stay clearly aspirational until implemented.                              |
| [`218.md`](../transcripts/218.md) | Permissionless skill registry, Nostr-based publishing, agent discovery, and reusable skill/resource economics.                                                                                    |
| [`219.md`](../transcripts/219.md) | Apple Silicon Pylon button-money flow, public install instructions for agents, and setup instructions that should be directly usable by automated agents.                                         |
| [`220.md`](../transcripts/220.md) | NIP-90 service-provider market direction, agent service requests, relay publication, payments, and fulfillment proofs.                                                                            |
| [`221.md`](../transcripts/221.md) | Codex/CloudCode wrapper direction, paid task execution, agent trace/data commercialization, and the distinction between current wrappers and future network products.                             |
| [`222.md`](../transcripts/222.md) | Idle-agent labor markets, Bitcoin earning claims, public live stats, payout truth, and constraints around claims of largest decentralized training network.                                       |
| [`223.md`](../transcripts/223.md) | Rebuilding large lab-style product surfaces as open Bitcoin/agent products, OpenAI-compatible compute services, Bitcoin/Nostr agent economy, and Discord/support feedback routing.                |
| [`224.md`](../transcripts/224.md) | Pylon real-work payments, distributed training, Psionic training runtime, public stats/API expectations, contributor settlement, and self-serve compute-market direction.                         |
| [`225.md`](../transcripts/225.md) | Bounties, strict contributor intake, open product work, Autopilot superapp, Probe/provider-account fallbacks, Forge, mobile agent control, and business revenue-share direction.                  |
| [`226.md`](../transcripts/226.md) | Product philosophy for starting simple, buying spare compute for Bitcoin, and turning low-cost consumer compute into an economic flywheel.                                                        |
| [`227.md`](../transcripts/227.md) | Pylon v0.2/payout caveats, Lightning/LDK upgrade direction, Qwen/legal fine-tuning, benchmark dashboard, secure legal-document workflows, business dashboard, and no-lock-in export expectations. |
| [`228.md`](../transcripts/228.md) | Autopilot coding-task beta, public traces, user-paid upgrades, trace/data reuse, and delayed/asynchronous fulfillment expectations.                                                               |
| [`229.md`](../transcripts/229.md) | Autopilot Sites, hosted Site handoff, revision queue, public Site URLs, referral attribution, and future referral payout.                                                                         |
| [`230.md`](../transcripts/230.md) | Agent instructions, pay-the-people revenue sharing, five agent markets, Autopilot as entry point, Nostr/Bitcoin protocol direction, agent commerce, and open agent network claims.                |
| [`231.md`](../transcripts/231.md) | Forum launch, agent participation, specialized agents communicating publicly, Bitcoin-ranked reputation, Forum tips, and agent wallet earning claims.                                             |
| [`232.md`](../transcripts/232.md) | Energy-aware compute orchestration, agentic inference flexibility, accepted outcomes per kilowatt hour, miner economics, and unified profitability/orchestration model direction.                 |
| [`233.md`](../transcripts/233.md) | OpenAgents monorepo consolidation, Bun/Effect/TypeScript/Cloudflare Worker direction, product homes for Autopilot/Pylon/Forum/Sites, and repo issue/PR routing.                                   |
| [`236.md`](../transcripts/236.md) | Planned large decentralized training run, Pylon v0.3 as Bitcoin-paid node software, Percepta Executor Class model support, and Tassadar training/model direction.                                 |
| [`242.md`](../transcripts/242.md) | Khala as the flagship collective-intelligence product, the free OpenAI-compatible API, single public Khala model, open-source infrastructure, contributor compute/data/labor/verification markets, pay-for-privacy direction, and confidential-compute aspirations. |
| [`243.md`](../transcripts/243.md) | Khala dogfooding through OpenCode, free self-serve API key limits, public token counter honesty, stale `khala-mini` model-id correction, provider/model mix, stats-page direction, and free-tier trace/data disclosure constraints. |
| [`244.md`](../transcripts/244.md) | Khala -> Pylon -> Codex own-capacity delegation, caller-owned Pylon/account linking, no-resale boundary, typed/semantic routing requirement, exact token-counter growth, `/stats` model mix, and Khala CLI sneak peek. |

## Verified Closed Issue Set

These issues were verified with GitHub issue search against
`OpenAgentsInc/openagents` on 2026-06-09. They are not the whole product
history; they are the useful closed set for launch-promise and product-promise
design.

| Issue                                                                                                                                                                                | Closed     | Why it matters                                                                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ---------------------------------------------------------------------------------------------------------- |
| [#4568](https://github.com/OpenAgentsInc/openagents/issues/4568) `[Launch Promise] Pylon worker loop and platform install gates for tomorrow launch`                                 | 2026-06-09 | Direct launch-promise issue for Pylon worker loop, install, and platform gates.                            |
| [#4521](https://github.com/OpenAgentsInc/openagents/issues/4521) `Pylon LDK wallet 01: Correct Pylon wallet product truth in docs and public copy`                                   | 2026-05-22 | Establishes that wallet, payout, and settlement copy must distinguish readiness from spendable settlement. |
| [#4413](https://github.com/OpenAgentsInc/openagents/issues/4413) `Prove public-style CS336 Pylon earning end to end`                                                                 | 2026-04-21 | Prior end-to-end proof work for Pylon earning claims.                                                      |
| [#4305](https://github.com/OpenAgentsInc/openagents/issues/4305) `Freeze validation_replay as the default weak-device launch lane`                                                   | 2026-04-12 | Defines a constrained weak-device launch lane instead of broad hardware overclaiming.                      |
| [#4175](https://github.com/OpenAgentsInc/openagents/issues/4175) `@openagentsinc/pylon should fall back to source build when a resolved release has no asset for the local platform` | 2026-04-07 | Protects install/release promises when platform assets are incomplete.                                     |
| [#3116](https://github.com/OpenAgentsInc/openagents/issues/3116) `Master Task: Compute Market full implementation program`                                                           | 2026-03-07 | Program umbrella for compute-market promises, evidence, metering, and settlement work.                     |
| [#3115](https://github.com/OpenAgentsInc/openagents/issues/3115) `Compute Market: implement observability, policy breakers, verification matrix, and rollout gates`                  | 2026-03-07 | Source precedent for verification matrix, policy breakers, observability, and rollout gates.               |
| [#3110](https://github.com/OpenAgentsInc/openagents/issues/3110) `Compute Market: automate delivery proofs, metering, and launch-family settlement evidence`                         | 2026-03-07 | Source precedent for delivery proofs, metering, and settlement evidence.                                   |
| [#3104](https://github.com/OpenAgentsInc/openagents/issues/3104) `Compute Market: define launch taxonomy, product families, and capability envelope`                                 | 2026-03-07 | Source precedent for promise taxonomy, product families, and capability envelopes.                         |

## Numbering Note

Some launch docs cite issue numbers for dashboard and copy-gate work. Direct
GitHub lookup for `#569` and `#571` currently resolves to unrelated historical
records in this repository namespace. Until those cross-references are
normalized, treat the Markdown docs listed above as the design records for the
public launch dashboard and launch copy gate, and treat the verified issue
table on this page as the closed issue set.
