[Home](../README.md) · [Investor Path](README.md) · **06. Data Market MVP**

# 6. Data Market MVP

> _"The current Data Market is a real secondary MVP slice, not just a spec."_
>
> — [`README.md`, OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents/blob/main/README.md)

**You will learn:**

- How NIP-90 kinds **5960 / 6960 / 31990** define request, result, and handler
- The live relay set (`wss://relay.damus.io`, `wss://relay.primal.net`)
- What an open, contestable machine-service market looks like on Nostr

## The second shipped market

Compute is market #1. Data is market #2, and it already ships in-repo alongside Autopilot. From the [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"What exists now:_
>
> - _`Data Seller`: a dedicated conversational seller lane for drafting, exact preview, confirm, publish, grant issuance, payment-required feedback, delivery, and revocation_
> - _`Data Market`: a read-only market snapshot and operator-facing lifecycle pane that now surfaces packaging posture, redacted Codex-export markers, and recent fulfillment activity_
> - _`Data Buyer`: a narrow buyer surface that selects a visible asset/default offer, shows the bundle/posture being purchased, and publishes a targeted request_
> - _`autopilotctl data-market ...`: full shell-first control over the same app-owned seller/buyer state machine_
> - _`autopilot_headless_data_market`: a no-window runtime for scripts, operators, and agents_
> - _repo-owned skills for both conversational and CLI-first seller flows"_

## Why a Data Market at all

The five-market framing ([Chapter 2](02-five-markets.md)) is only coherent if data can be priced and settled like compute can. Without a Data Market, agents silently scrape, and the labor they produce is built on un-permissioned context. The Data Market is how OpenAgents brings the data layer into the _verifiable outcomes under uncertainty_ primitive.

Kernel objects behind the market, from [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"Kernel authority owns `DataAsset`, `AccessGrant`, `DeliveryBundle`, and `RevocationReceipt`. Desktop, CLI, and skills all drive the same app-owned data-market logic through typed desktop-control actions."_

Every transaction produces a signed receipt. Every revocation produces one too. If a delivery fails, the `RevocationReceipt` is the canonical audit artifact.

## The NIP-90 data-vending profile

From [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"Transport is a targeted NIP-90 data-vending profile:_
>
> - _request kind `5960`_
> - _result kind `6960`_
> - _handler/capability kind `31990`"_

These are Nostr's data-vending kinds reused for a permissioned, paid, targeted access flow. The strict public-relay verification path runs live against:

- `wss://relay.damus.io`
- `wss://relay.primal.net`

That's two public relays — independent of any OpenAgents-controlled infrastructure — where the full buyer-to-seller flow has been verified end to end. The Data Market is not a localhost demo; it is live on the open Nostr network.

## Three ways to drive the same state machine

The Data Market is intentionally read-heavy in the UI and mutation-heavy on the shell. From the [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"The panes are intentionally read-heavy: `autopilotctl` and headless/skill flows steer mutations, while the UI exposes the exact preview, package, posture, request, payment, delivery, and revocation truth."_

| Surface                        | Who uses it                              |
| ------------------------------ | ---------------------------------------- |
| `Data Seller` pane             | Human operators, drafting and reviewing  |
| `autopilotctl data-market`     | Shell-first operators, CI, scripted QA   |
| `autopilot_headless_data_market` | No-window daemons, agents, skills runtime |
| `seller-prompt "<prompt>"`     | Terminal-driven agent-seller automation  |

All four routes drive the same app-owned seller/buyer state machine with identical acceptance semantics. No side-car truth, no shadow state.

## What ships, concretely

For operators who want to drive the loop today:

- Implementation and status: [`docs/kernel/markets/data-market.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/kernel/markets/data-market.md)
- CLI and headless runbook: [`docs/headless-data-market.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/headless-data-market.md)
- Latest seller-prompt paid-flow proof: [`docs/audits/2026-03-21-data-seller-one-sentence-prompt-paid-flow-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-03-21-data-seller-one-sentence-prompt-paid-flow-audit.md)
- Implementation spec and backlog: [`docs/plans/data-market-mvp-implementation-spec.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/plans/data-market-mvp-implementation-spec.md)
- Repo-owned skills: [`skills/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/skills/README.md)

## What does not ship (honest scope)

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"We are intentionally not shipping… broad public Data Market discovery, catalog search, or rich buyer procurement UX beyond the current narrow targeted-request flow… a broad end-user finetuning product, raw chat-log upload flow, or multi-family finetuning platform claim."_

The current Data Market is not Amazon for datasets. It is a narrow targeted-request lane that proves the kernel primitives work under live relay conditions. Broader discovery UX, open cataloging, and multi-family fine-tuning are explicit post-MVP lanes.

## How the Data Market compounds the Compute Market

If Compute is supply-side and Autopilot is the wedge, Data is the _second_ revenue surface a Pylon operator can opt into. A machine that already earns Bitcoin for training work can next sell packaged local data — stored conversations, curated artifacts, project context — at a price the kernel settles, under grants it can revoke.

That is the architectural reason Compute and Data are the first two markets live: they're the two supply-side lanes a single operator can turn on, in any order, without rearchitecting anything above them.

Labor (market #3) consumes compute and data; Liquidity (market #4) moves value across rails; Risk (market #5) underwrites outcomes. They are all programmable extensions of the same kernel primitive — and they all inherit the same receipt + revocation model that the Data Market proves out today.

---

**← Previous:** [05. Pylon, the Provider](05-pylon-provider.md) · **Next:** [07. Economy Kernel](07-economy-kernel.md) **→**
