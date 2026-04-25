[Home](../README.md) · [Investor Path](README.md) · **04. The Earn Loop**

# 4. The Earn Loop

> _"Autopilot Earn starts with the OpenAgents Compute Market. You run the desktop app, press `Go Online`, and offer standardized compute products into the network."_
>
> — [`OpenAgentsInc/openagents` README](https://github.com/OpenAgentsInc/openagents/blob/main/README.md)

**You will learn:**

- What real machine work pays today, and why the price is 25 sats
- Why we don't pay for being online
- How a single 25-sat tick scales to the largest training run in history

## The first dollar of an open compute market

The earn loop is not a subsidy. It's a price floor on real work.

When Pylon first launched, the network paid users for being online. We've moved off that. Chris on _OAPN #5_:

> _"As of today, we're no longer paying you for being online. We're gonna be paying you for the real work that we're sending to your Pylon."_
>
> — [_OAPN #5, Distributed Training 101_](https://openagents.substack.com/p/oapn-5-distributed-training-101)

Today the worker only gets paid when their contribution is _accepted_ by an independent validator. No paid uptime, no participation prizes. Real work or no payment.

## The economics, on one card

| | |
|---|---|
| **Pay per accepted contribution** | **25 sats** |
| **Cap per dispatch cycle** | 6,400 sats |
| **Max contributors per cycle** | 256 |
| **Cycle cadence** | Roughly every 10 minutes |
| **Paid work class today** | Stanford CS336 Assignment 1 (distributed training) |
| **Paid for being online?** | No |

Twenty-five sats is small on purpose. The number is a price floor — the smallest amount of real Bitcoin that proves the entire stack works end-to-end. If we can pay 25 sats reliably, automatically, with a signed receipt every time, we can pay 25,000.

The rest of the company is the work it takes to scale that floor up.

## Why CS336 A1 is the starter lane

Stanford's CS336 Assignment 1 is a well-defined distributed-training exercise. It has an objective acceptance criterion — the worker either produces a valid training contribution or doesn't. There's no judgment call. That's exactly what a starter market needs: a paid task we can validate without a human in the loop.

Once the loop works on CS336 A1, the same plumbing extends to inference, to embeddings, to bigger training runs, to any unit of machine work where acceptance can be checked in code. The starter lane is the first member of a class.

## Why the loop is big

Every prior decentralized-training network has had a fundamental problem: it pays in a token that didn't exist before the network. Bittensor's biggest training run ever had 70 contributors. From _OAPN #5_:

> _"They've taken the ideas come out of DeepMind for Diloco and some of these algorithms for decentralized training, and they've put them to use. Now, they've generally attached these shitcoins that I think are completely unnecessary, but hey, if you have 70 people that will contribute compute to be paid in your shitcoin, if we make it easy for Bitcoin, the massive, biggest, most secure, coolest, awesomest network, why don't we get 700 people or 7,000 or 70,000 people contributing their compute to this?"_
>
> — Christopher David, OAPN #5

The loop isn't impressive in absolute payout today. It's impressive because we built it this month, the architecture holds a path to the largest decentralized training run in history, and **every sat that moves is real Bitcoin** — not a token that needs a market to make it valuable.

## What the network looks like already

As of OAPN #5: **1,300+ Pylons online. 1,000,000+ sats paid out.** Both numbers are growing weekly, on a base of zero marketing spend, before the desktop wedge has even been pitched at Bitcoin Vegas.

Twenty-five sats per contribution sounds small until you do the multiplication. A thousand contributors hitting the per-cycle cap, six cycles an hour, twenty-four hours a day, is a real Bitcoin payroll for a real research compute pool. We are not at that scale yet. We are at the part where the rails work and the price floor holds.

## The honesty layer

> _"The app must never 'feel like it paid you' unless it actually did. The architecture exists to enforce that honesty."_
>
> — [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md)

The wallet only updates when the kernel says money moved. There is no pending-balance hallucination, no "you'll be paid soon" toast, no inferred payouts. If payment fails, the UI says so plainly. That's why the [proof receipts](09-proof-receipts.md) reconcile so cleanly: the wallet history matches the kernel's payout id, exactly.

## The first earning proof

The first signed end-to-end proof is dated **April 23, 2026**. One bounded run, one validator, one settled payout, one wallet tick: `0 → 25 sats`. Payout id `019db8a2-98d2-7890-95e4-6a1d78709a3c`.

That's the smallest possible real earning. Full receipt, with reproducible commands, in [Chapter 9](09-proof-receipts.md).

---

{% hint style="info" %}
**Under the hood.** The full eight-step lifecycle (`Go Online → advertise capabilities → Nexus dispatches → local runtime executes → validator accepts → Treasury pays → wallet updates → withdraw`), the assignment-lease semantics, and the open-market race model for non-starter jobs all live in the [Developer Path → Quickstart](../developers/quickstart.md). The user-facing operator runbook is [`docs/autopilot-earn/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/README.md). The end-to-end loop in v0.1 was produced by the headless `pylon` lane, not the desktop UI panes — wiring the panes to that same runtime is the next slice of product work.
{% endhint %}

---

**← Previous:** [03. Autopilot — The Wedge](03-autopilot-wedge.md) · **Next:** [05. Pylon, the Provider](05-pylon-provider.md) **→**
