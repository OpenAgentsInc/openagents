[Home](../README.md) · [Investor Path](README.md) · **10. Roadmap & Ask**

# 10. Roadmap & Ask

> _"We're raising a seed round. Help us out."_
>
> — Christopher David, [Bitcoinfi Demo Day](../assets/clips/cdavid-demoday-highlight-90s.mp4)

**You will learn:**

- The twelve-month arc across the five markets
- The cross-cutting infrastructure work (Psionic, authority hardening, self-hosting)
- How to reach Christopher David about the seed round

## Where we are, in one paragraph

OpenAgents is a live project. Pylon is at [v0.1.13](https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.13) on the public npm registry as `@openagentsinc/pylon`. The `pylon` lane has a reproducible end-to-end [earning proof](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports) dated 2026-04-23 — sats moved from `0 → 25` on a CS336 A1 homework run, payout id `019db8a2-98d2-7890-95e4-6a1d78709a3c` settled through Treasury. The Data market is already on relay at `wss://relay.damus.io` and `wss://relay.primal.net` using NIP-90 kinds `5960`, `6960`, `31990`. On the compute-milestone side, the OAPN podcast announced the crossover to more than 1,300 online Pylons and more than 1,000,000 sats paid out to contributors (OAPN #5, _Distributed Training_).

We are not at clearing prices. We are not at an equilibrium cap table. We are at the narrow, demonstrable wedge that [Chapter 3](03-autopilot-wedge.md) describes: _a single user can install an app and earn Bitcoin for useful machine work, today._

## The twelve-month roadmap, by market

The five markets introduced in [Chapter 2](02-five-markets.md) advance at different speeds. What follows is the repo-grounded roadmap, with honest scope.

### Compute Market — from starter jobs to the 20 GW wedge

**Now.** CS336 A1 is the anchor paid-training job. Starter-grade jobs for devices that can't run the full training loop. Hosted Nexus is the buyer of first resort. The [MVP](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md) is explicit: _"A usable v0.1 MVP pane surface that lets a non-technical earner install the app, come online, take paid training work, see payout, and keep going — without needing to operate the CLI or inspect logs."_

**Next six months.** Broaden the job menu past CS336 A1. Land a second paid-training lane that does not require the OpenAgents-hosted subsidy. Stand up device-bound attestation for starter eligibility so the subsidy-gated lane is harder to spoof (the [MVP doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md) calls out this as post-MVP hardening: _"Stronger anti-spoofing (device-bound proofs, richer attestation) is hardening work on the roadmap"_). Begin reporting the decentralized model-training milestone referenced in the [Demo Day pitch](../assets/clips/cdavid-demoday-highlight-90s.mp4): _"by the time you watch this, we will have begun what we believe is about to be the largest decentralized model training run in history."_

**Twelve months.** Move from starter-job anchor pricing toward a real venue-maker price. Open the buyer seat — not just hosted Nexus dispatching, but third parties buying compute through kind `5960` requests. This is the inflection from "OpenAgents pays 25 sats to train CS336 A1" to "anyone can buy trained tokens from retail providers on an open market."

### Data Market — from kind 5960 MVP to paid retrieval

**Now.** Kind `5960 → 6960 / 31990` is live on open relays. The provider schema and trust-minimized flow are specified in [`packages/data-market-mvp/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/packages/data-market-mvp/README.md).

**Next six months.** Grow the handler registry past the seed providers — more publicly-listed kind `31990` capability events. Lightning escrow for paid retrieval. Ratings rollup across handlers.

**Twelve months.** Close the loop from buyer-initiated kind `5960` to buyer-paid-on-delivery kind `6960`. This is the bridge from "discoverable machine services" to "priced, contestable machine services" on an open relay set.

### Labor Market — Forge, Probe, and paid contributor work

**Now.** OpenAgents has paid out more to developer contributors than most AI labs have ever paid, according to the [Demo Day pitch](../assets/clips/cdavid-demoday-highlight-90s.mp4): _"We've paid more Bitcoin, we've paid more anything to developers than every other AI lab combined."_ Developer bounties were reactivated on OAPN #6, [_Pay the People_](https://openagents.substack.com/).

**Next six months.** Productize the internal Forge + Probe pattern Chris described on OAPN #6:

> _"Imagine so you've got Forge, the software factory. Forge should be able to deploy multiple probes. Insert StarCraft analogy here, but like the Forge, you know, you're equipping probes with arms. A bunch of AI teams have created their own versions of that blog post that came out from Ramp. They built this inspect agent that was just kind of tailored to their team's workflow. One system where all their developers can push changes and do PRs. We're we're we've already begun building this internally."_
>
> — Christopher David, OAPN #6

**Twelve months.** A business-targeted Autopilot surface — same kernel, same receipts, a different pane inventory. Chris on OAPN #6:

> _"There will be a business version of that for like putting your business on autopilot. Intended to be a drop-in replacement for Microsoft Copilot."_
>
> — OAPN #6

### Liquidity Market — from Treasury routing to open float

**Now.** `TreasuryRouter` cuts starter-job payouts; [Spark](https://spark.money) rails are live on both ends of the earning proof (worker destination `spark1pgssyt…`, payment id `019db8a2-…`).

**Next six months.** Formalize the open-treasury surface. Publish kernel-signed reports so any holder can reconcile balances, payouts, and reserves without trusting a Treasury blob. FROSTR (the multi-sig primitive in the OpenAgents orbit) enters this lane for split-key issuance. Threshold-signing interop is a still-to-be-defined surface — the [2026-02-27 Nostr gap analysis](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md) flags a *Bifrost Threshold Coordination Profile* as an open observation rather than an off-the-shelf NIP, so this lane also involves canonical-NIP draft work, not just integration of an existing spec.

**Twelve months.** Multiple underwriters. Today, the hosted Nexus is effectively subsidizing starter paid-training demand. In twelve months, third parties can be the underwriter for classes of jobs they care about — companies that want an open American model trained, researchers who want retail compute supplied to a bounty pool, or app platforms that want data-market handler availability at a minimum SLA.

### Risk Market — the quiet one

**Now.** No live underwriting layer. The five-market diagram names Risk because, as [Chapter 7](07-economy-kernel.md) argues, the other four do not function without it. Today, OpenAgents itself absorbs almost all of the risk that starter work will be invalid — because OpenAgents is itself buyer, underwriter, and subsidizer at once.

**Next twelve months.** Replace that implicit OpenAgents underwriting with explicit, kernel-authored risk receipts. A kind `5960` request can carry a risk addendum describing the coverage pool. A settled payout becomes a data point for pricing the next job of the same shape. A malfunctioning handler can be priced out, or priced in at a discount, without taking the whole marketplace down. This is the slow market to build and the most defensible once it exists.

## Cross-cutting infrastructure work

Three things cut across all five markets and belong on the twelve-month arc regardless of sector:

- **Psionic** — the in-house inference runtime introduced on the [Demo Day pitch](../assets/clips/cdavid-demoday-highlight-90s.mp4): _"Psionic from Open Agents, 518 tokens per second. We've since gotten that up to 530. It's not because we're better ML engineers. We are better software engineers."_ Psionic is the substrate that lets Autopilot Pylons do inference, training, and validation work without renting cloud GPUs.
- **Authority hardening** — [ADR-0001](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md) stays the spec; what needs to ship is stronger attestation for provider identity (see the MVP doc's _"device-bound proofs, richer attestation"_), and independent audit of the ADR-approved Spacetime exception class.
- **Self-hosting turnkey** — Autopilot already supports pointing at user-owned Nexus + relay sets. The twelve-month goal is to make that the default story for any user who wants it — same receipts, same authority lanes, _different_ underwriter.

## What this adds up to

Read alongside [Chapter 1](01-why-openagents.md), the twelve-month plan resolves to two lines:

1. _Keep the Autopilot wedge narrow, demonstrable, and reproducible — so that any interested party can install, earn, and reconcile without trusting us._
2. _Widen the authority and underwriting surfaces so that OpenAgents is the buyer-of-first-resort on an open network, not the owner of the network._

The [Demo Day pitch](../assets/clips/cdavid-demoday-highlight-90s.mp4) put the stakes plainly:

> _"The company that unlocks any percentage of this twenty gigawatts has a path to becoming the most valuable company in the world. Wouldn't it be cool if that was a Bitcoin company?"_
>
> — Christopher David, Bitcoinfi Demo Day 2026

<figure>
  <img src="../assets/graphics/slide-20gw-stranded-compute.jpg" alt="Demo Day slide — 20 GW of stranded consumer compute vs OpenAI's 2 GW" />
  <figcaption>The compute-gap slide from the Demo Day pitch — OpenAI at 2 GW, stranded consumer compute at 20 GW. The twelve-month arc is about unlocking even a small percentage of that gap.</figcaption>
</figure>

## The ask

OpenAgents is raising a seed round. No figure, no terms, and no dilution targets are published here.

<figure>
  <img src="../assets/graphics/slide-team-and-backers.jpg" alt="Demo Day slide — team and existing backers" />
  <figcaption>Team (Christopher David, Kevin Fremon, Michael Ovsen, Car Gonzalez, Ben Silone, McDonald Aladi, Sophia Bailey Savoy, Timothy C. Maher) and existing backers (Wolf, NYDIG, Hivemind VC, Draper Associates, Draper Dragon).</figcaption>
</figure>

The closer from the [Demo Day pitch](../assets/clips/cdavid-demoday-highlight-90s.mp4) is the exact framing we use:

> _"I built a team. I've worked with some of these people for over a decade — Bitcoiners, open protocol people. We're shipping product. We're backed by some of the best early-stage Bitcoin investors in the space. Hopefully soon to include some of you. We're raising a seed round. Help us out."_
>
> — Christopher David

If you are reading this GitBook ahead of Bitcoin Vegas 2026 and want to talk, the right path is:

1. **Panel.** [Christopher David](https://github.com/christophergalaxie) is speaking on the _"Why AI Agents Want Bitcoin"_ panel on the Open Source stage, 10:45 AM.
2. **Warm intro.** If you already know an existing OpenAgents investor or contributor, a warm introduction is the highest-signal path.
3. **Demo.** You can install Pylon today with `npx @openagentsinc/pylon@0.1.13` and earn on CS336 A1 yourself. The [earning proof](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports) is reproducible on a `darwin-arm64` machine.
4. **Read the receipts.** [Chapter 9](09-proof-receipts.md) points at the two signed artifacts that matter most. If those do not already answer the "can you ship, does the money move" diligence question, nothing the deck says will.

Conversations are active. No terms have been shared publicly and no round size is published in this book — those are on purpose, and belong in direct conversation.

## Where to find the source of truth

- Upstream repo: [`OpenAgentsInc/openagents`](https://github.com/OpenAgentsInc/openagents)
- Release feed: [`/releases`](https://github.com/OpenAgentsInc/openagents/releases)
- Proof receipts: [`docs/reports/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports)
- Architecture decisions: [`docs/adr/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/adr)
- Developer substack: [openagents.substack.com](https://openagents.substack.com)
- YouTube (OAPN, demos, build logs): [`@OpenAgentsInc`](https://www.youtube.com/@OpenAgentsInc)

This GitBook will stay in sync with the repo. Every claim in every chapter is cross-referenced to a file, commit, or receipt in [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents). If any claim drifts from the source, the receipts are the authority — not the chapter prose.

---

**← Previous:** [09. Proof Receipts](09-proof-receipts.md)
