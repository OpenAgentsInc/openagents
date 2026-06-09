# Launch Promise Source Set

This page records the launch-promise audit docs and the verified closed issue
set that should be treated as source context for the broader product-promises
system.

## Audit Docs Found

| Source | Role in the product-promises system |
| --- | --- |
| [`apps/openagents.com/docs/2026-06-08-chrisjuan-pylon-agentic-revenue-gap-audit.md`](../../apps/openagents.com/docs/2026-06-08-chrisjuan-pylon-agentic-revenue-gap-audit.md) | Original promise inventory and live-state gap audit. It separates safe copy from unsafe copy and shows which claims were ahead of live evidence. |
| [`apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`](../../apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md) | Current Pylon v0.3 reconfiguration audit. It moves the launch promise set from old repo assumptions to the new Pylon ownership model and keeps v0.3 claims gated until live evidence exists. |
| [`apps/openagents.com/docs/2026-06-08-openagents-public-launch-dashboard.md`](../../apps/openagents.com/docs/2026-06-08-openagents-public-launch-dashboard.md) | Machine-checkable launch truth surface using red, yellow, and green promise rows with evidence refs, blocker refs, safe copy, and unsafe copy. |
| [`apps/openagents.com/docs/2026-06-08-public-launch-copy-gate.md`](../../apps/openagents.com/docs/2026-06-08-public-launch-copy-gate.md) | Copy gate for launch surfaces. It blocks unsupported affirmative launch claims unless matching evidence gates are green. |
| [`apps/openagents.com/docs/2026-06-08-pylon-install-to-bitcoin-launch-smoke.md`](../../apps/openagents.com/docs/2026-06-08-pylon-install-to-bitcoin-launch-smoke.md) | Install-to-Bitcoin smoke contract covering install, registration, heartbeat, wallet readiness, assignment, closeout, payment, settlement, and projection refs. |
| [`apps/openagents.com/docs/forum/launch-gates.md`](../../apps/openagents.com/docs/forum/launch-gates.md) | Forum launch gates for posting, moderation, reports, tipping, payment redaction, and launch-safe public status. |
| [`apps/pylon/docs/launch-gates-no-overclaim.md`](../../apps/pylon/docs/launch-gates-no-overclaim.md) | Pylon-side no-overclaim gate guidance for launch copy and evidence claims. |

## Verified Closed Issue Set

These issues were verified with GitHub issue search against
`OpenAgentsInc/openagents` on 2026-06-09. They are not the whole product
history; they are the useful closed set for launch-promise and product-promise
design.

| Issue | Closed | Why it matters |
| --- | --- | --- |
| [#4568](https://github.com/OpenAgentsInc/openagents/issues/4568) `[Launch Promise] Pylon worker loop and platform install gates for tomorrow launch` | 2026-06-09 | Direct launch-promise issue for Pylon worker loop, install, and platform gates. |
| [#4521](https://github.com/OpenAgentsInc/openagents/issues/4521) `Pylon LDK wallet 01: Correct Pylon wallet product truth in docs and public copy` | 2026-05-22 | Establishes that wallet, payout, and settlement copy must distinguish readiness from spendable settlement. |
| [#4413](https://github.com/OpenAgentsInc/openagents/issues/4413) `Prove public-style CS336 Pylon earning end to end` | 2026-04-21 | Prior end-to-end proof work for Pylon earning claims. |
| [#4305](https://github.com/OpenAgentsInc/openagents/issues/4305) `Freeze validation_replay as the default weak-device launch lane` | 2026-04-12 | Defines a constrained weak-device launch lane instead of broad hardware overclaiming. |
| [#4175](https://github.com/OpenAgentsInc/openagents/issues/4175) `@openagentsinc/pylon should fall back to source build when a resolved release has no asset for the local platform` | 2026-04-07 | Protects install/release promises when platform assets are incomplete. |
| [#3116](https://github.com/OpenAgentsInc/openagents/issues/3116) `Master Task: Compute Market full implementation program` | 2026-03-07 | Program umbrella for compute-market promises, evidence, metering, and settlement work. |
| [#3115](https://github.com/OpenAgentsInc/openagents/issues/3115) `Compute Market: implement observability, policy breakers, verification matrix, and rollout gates` | 2026-03-07 | Source precedent for verification matrix, policy breakers, observability, and rollout gates. |
| [#3110](https://github.com/OpenAgentsInc/openagents/issues/3110) `Compute Market: automate delivery proofs, metering, and launch-family settlement evidence` | 2026-03-07 | Source precedent for delivery proofs, metering, and settlement evidence. |
| [#3104](https://github.com/OpenAgentsInc/openagents/issues/3104) `Compute Market: define launch taxonomy, product families, and capability envelope` | 2026-03-07 | Source precedent for promise taxonomy, product families, and capability envelopes. |

## Numbering Note

Some launch docs cite issue numbers for dashboard and copy-gate work. Direct
GitHub lookup for `#569` and `#571` currently resolves to unrelated historical
records in this repository namespace. Until those cross-references are
normalized, treat the Markdown docs listed above as the design records for the
public launch dashboard and launch copy gate, and treat the verified issue
table on this page as the closed issue set.
