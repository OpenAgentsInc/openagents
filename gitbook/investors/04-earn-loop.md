[Home](../README.md) · [Investor Path](README.md) · **04. The Earn Loop**

# 4. The Earn Loop

> _"Autopilot Earn starts with the OpenAgents Compute Market. You run the desktop app, press `Go Online`, and offer standardized compute products into the network."_
>
> — [`README.md`, OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents/blob/main/README.md)

**You will learn:**

- How **25 sats / 6,400 cap / 256 contributors** anchors the economy
- The role of CS336 A1 as the starter paid-training lane
- How TreasuryRouter cuts and reconciles each payout

## What ships today

The canonical user-facing guide is [`docs/autopilot-earn/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/README.md). Chris summarized the current production truth on _OAPN #5 — Distributed Training 101_:

> _"As of today, we're no longer paying you for being online. We're gonna be paying you for the real work that we're sending to your Pylon. So in this video, we're gonna go into, like, what we're actually sending, why we're doing this."_

— [_OAPN #5, Distributed Training 101_, ~minute 3](https://openagents.substack.com/p/oapn-5-distributed-training-101)

The earn loop is not a subsidy-for-uptime loop. It is an _accepted-work_ loop: real training work, validated, paid only when accepted.

## The loop, end to end

```text
1. User installs Autopilot (or runs `pylon` directly)
2. User presses Go Online  →  node advertises eligible capabilities to Nexus
3. Nexus dispatches a bounded CS336 A1 homework/training run (~every 10 min)
4. Local Psionic runtime executes the training window
5. A separate validator Pylon accepts or rejects the contribution
6. Treasury dispatches accepted-work payout (25 sats)
7. Worker wallet balance increases (Spark Lightning wallet)
8. User can withdraw over Lightning — does not need to go offline first
```

{% hint style="info" %}
**Which path produced the 2026-04-23 proof.** Steps 2–7 above are accurate for the **`pylon` headless lane** — the way the receipts in [Chapter 9](09-proof-receipts.md) were generated (a `pylon` provider + a separate validator `pylon` on `wss://relay.damus.io` and `wss://relay.primal.net`). Those same steps are **not yet** what happens when a user drives the `Job Inbox` / `Active Job` panes in the desktop UI today; those panes currently run on locally seeded state for the v0.1 cut (see the `Source` column in [Chapter 3](03-autopilot-wedge.md#the-pane--command-surface)). Wiring the desktop panes to the same runtime that the `pylon` binary already ships is the next slice of product work.
{% endhint %}

Every step has a visible pane in Autopilot and a matching `autopilotctl` or `pylon` command for scripted proof.

## The quantitative floor today

From the [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

| Parameter                         | Current production value                                  |
| --------------------------------- | --------------------------------------------------------- |
| Paid work class                   | Bounded hosted homework/training (CS336 A1 starter lane)  |
| Automatic pacing target           | `pylon-v0.1.12+`, online-only                             |
| Max contributors per cycle        | `256`                                                     |
| Rate per accepted contribution    | `25 sats`                                                 |
| Cap per automatic cycle           | `6,400 sats`                                              |
| Cycle cadence                     | ~every 10 minutes                                         |
| Placeholder / liveness payouts    | _not part of the current claim_                           |

This matters for investor diligence: the rate is intentionally small, the cycle cap is bounded, and nothing is paid for empty uptime. The loop is a proof of payment rails, not a subsidy.

## The current paid-work class

From [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"The live paid work class is bounded hosted homework/training work, currently the CS336 A1 starter lane used to prove the earning loop. Work is not yet a fully open demand marketplace. Hosted Nexus currently paces paid homework jobs from the server side."_

CS336 A1 is Stanford CS336's Assignment 1 — a well-defined distributed-training exercise that gives us a reproducible, verifiable work class with an objective acceptance criterion. It's the scaffolding on which the broader Compute Market is validated before the open-demand marketplace turns on.

## Authority model for the loop

From [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"Authority does **not** live in the desktop client. Authority lives in backend services: **TreasuryRouter** and the **Kernel Authority API**. The app sends authenticated HTTPS requests to TreasuryRouter, which evaluates policy and invokes kernel authority operations. Money movement, settlement, verdict finalization, and other authoritative state changes happen there and are recorded as canonical receipts._
>
> _**Nostr** and **Spacetime** are used for coordination, sync, identity, and projections. They are not authority lanes for money, liability, or verdict changes."_

This separation is the reason the loop is auditable. The desktop runs the work; the backend mutates economic truth; Nostr and Spacetime project progress; receipts provide the canonical audit trail. See [Chapter 8 — Authority & Ownership](08-authority-model.md) for the full authority matrix.

## What the UI must never do

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"The app must never 'feel like it paid you' unless it actually did. The architecture exists to enforce that honesty."_

Wallet updates are reflected in the UI as _authoritative_, not inferred. If payment fails, the UI must say so plainly. There is no pending-balance hallucination.

## Job assignment: not a race on the starter lane

For starter jobs, from [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"The OpenAgents-hosted Nexus should assign each starter job to one eligible provider at a time using a short-lived assignment lease derived from provider liveness and current load. The desktop auto-accepts the starter job only when it holds the active lease."_

That lease is aggressive about confirming the provider actually started — on the order of `10-15s` to emit a start/progress signal. If the provider drops or fails to acknowledge, Nexus reassigns. No duplicate-work waste, no silent no-pay for honest providers.

For _open-market_ contention (public NIP-90 jobs on public relays), the current default is a `race` model — first valid result wins — bounded by strict local admission controls: low `max_inflight` (default `1`), TTL freshness checks, minimum reward thresholds, per-buyer limits, cheap preflight validation.

## Why the loop is big

OAPN #5 spells out the ambition. On the same episode, Chris walks through the Bittensor comparison:

> _"We note that like, you know, the biggest training run ever from, um, Bittensor subnet 300 jillion, I don't know what it is, but like they had 70 people contribute compute to a training run. Um, Pylon, which right now we're doing very basic inference, your Pylon will load a Gemma 4 model."_

— [_OAPN #2, Pylon Launch_](https://www.youtube.com/watch?v=uvRO-E9SXI8)

Translated to the Vegas investor read: the loop is not impressive today in absolute payout. It is impressive because _we only built it this month_, and the architecture holds a path to the largest decentralized training run in history. The Compute Market is supply-side led by design — one machine at a time, each paid in Bitcoin, each verified.

## First earning proof, signed

The [_2026-04-23 Autopilot-Controlled Pylon Production Earning Proof_](https://github.com/OpenAgentsInc/openagents/blob/main/docs/reports/nexus/2026-04-23-autopilot-pylon-production-earning-proof.md) is the audit-grade artifact of this loop. Selected facts:

- Repo state: `openagents` `main` at `96295609b`
- Dispatch: `POST /v1/admin/homework/cs336-a1/dispatch` with `1 run`, `1 contributor`, `25-sat budget`
- Window status: `reconciled`, `payout eligible: true`, `accepted contributions: 1`
- Payout state: `confirmed`, reconciliation status: `settled`, amount: `25 sats`
- Wallet balance delta: **`0 → 25 sats`** (Spark wallet)

That is the smallest possible _real_ earning: one job, one validator, one payout receipt, one wallet tick-up. Full receipt in [Chapter 9 — Proof Receipts](09-proof-receipts.md).

---

**← Previous:** [03. Autopilot — The Wedge](03-autopilot-wedge.md) · **Next:** [05. Pylon, the Provider](05-pylon-provider.md) **→**
