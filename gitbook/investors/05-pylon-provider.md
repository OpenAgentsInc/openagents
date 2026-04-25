[Home](../README.md) · [Investor Path](README.md) · **05. Pylon, the Provider**

# 5. Pylon — The Provider

> _"Pylon is the standalone provider node for the current paid training path. It is the easiest way to put a machine online for OpenAgents jobs without running the full Autopilot desktop app."_
>
> — [`OpenAgentsInc/openagents` README](https://github.com/OpenAgentsInc/openagents/blob/main/README.md)

**You will learn:**

- The one-line install that turns any machine into a Bitcoin-paying node
- Why Pylon exists alongside Autopilot
- Psionic — the ML engine that makes it all possible

## The one-line install

```bash
npx @openagentsinc/pylon
```

That's the install. No account creation. No build-from-source. No desktop UI. Keep the process running, and your machine starts taking real work.

From the public README:

> _"Pylon creates local node identity and wallet state, marks the node online, advertises eligible capabilities to Nexus, asks for available work, executes assigned jobs locally, and watches for accepted-work payouts. The user-facing contract is: run `pylon`, stay online, and get paid when assigned work closes out as accepted."_

Run it on a gaming PC. Run it on a server. Run it on a fleet. The contract is the same.

## Two doors into the same house

Autopilot is the door for individual users — the people who want a personal AI agent on their desktop _and_ a Bitcoin earn loop. Pylon is the door for everyone else:

- Bitcoiners with idle compute who don't want to run a UI
- Operators with cloud VMs and headless rigs
- Developers building integrations against the network
- Institutions running fleets of provider nodes

Chris on the design intent, _OAPN #2_:

> _"If everyone, like literally everybody can, using these Bitcoin, Lightning, Nostr little tech based on top of it, can do something really cool like plug in any spare compute that you have and make that available into this open network, we are gonna be using it. We are buyer number one. We want your compute. But guess what? All this is on an open network. It's Nostr and NIP-90. If someone else wants to come and outbid us for your compute and pay more than we do, we want that to happen."_
>
> — [_OAPN #2, Pylon Launch_](https://www.youtube.com/watch?v=uvRO-E9SXI8)

That's the distinction from Bittensor and the shitcoin training projects: Pylon is _open-market_ supply. OpenAgents is the buyer of first resort, not the only buyer. If someone else shows up and pays more for your machine's time, the protocol routes them to you.

## Current build: Pylon v0.1.13

We ship frequently. Every release ships with a signed receipt — commit hash, npm package hash, the verification commands that ran before cut, and an explicit scope statement of what changed.

The current recommended public build is [`pylon-v0.1.13`](https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.13), shipped April 23, 2026, on commit `8590d04a`. The matching release receipt is in [Chapter 9](09-proof-receipts.md).

**Why this matters for diligence.** A 0.1.x project that can publish a signed, verifiable, reproducible release receipt _for every cut_ is a project that won't be caught lying to itself about readiness later. Release discipline is the cheapest leading indicator of engineering discipline. We hold ourselves to it.

## Psionic — the ML engine underneath

Pylon executes paid training and inference work _locally_. That is only possible because we built our own ML engine.

Psionic is our Rust-first edge inference framework. From the [Demo Day pitch](../assets/clips/cdavid-demoday-highlight-90s.mp4):

> _"Open agents' Psionic: 518 tokens per second on the lightest-weight Qwen model. We've since gotten that up to 530. Here's Ollama: 338 tokens per second. We are beating them by 30 percent. It's not because we're better ML engineers. We are better software engineers."_

<figure>
  <img src="../assets/graphics/slide-psionic-vs-ollama.jpg" alt="Demo Day slide — Psionic vs Ollama benchmark across Qwen3.5 0.8B, 2B, 4B, 9B sampled tok/s" />
  <figcaption>Psionic vs. Ollama, RTX 4080, sampled top_k=40 / temperature=0.8 / top_p=0.9 / seed=42. Source: <code>psionic/docs/QWEN35_OLLAMA_COMPARISON.md</code>, March 29, 2026.</figcaption>
</figure>

Thirty percent faster, on the same hardware, on every model size we've measured. That's the supply-side unlock. Faster local inference means cheaper compute means a real price-per-token a small machine can sell into the network without burning power on slow runtimes.

Psionic is the reason a Pylon on someone's home GPU is competitive with rented cloud capacity for the kinds of jobs the Compute Market clears. Without Psionic, the supply side would be too slow to matter.

## The decentralized training ambition

The 25-sat earn loop is the price floor. The ambition is much bigger.

> _"The multi-billion dollar training runs, the multi, multi hundreds of billions of dollars that are allocated by the massive AI companies for compute, a lot of that goes to training. It's like instead of that going out to Nvidia and these big cloud people, like why don't we pay that to actual people?"_
>
> — Christopher David, [_OAPN #5_](https://openagents.substack.com/p/oapn-5-distributed-training-101)

Pylon is how that money gets paid to actual people. As of OAPN #5, the network crossed **1,300 Pylons online and 1,000,000+ sats paid out**. Those are the first two milestones on the ramp toward the largest decentralized training run in history.

We have begun it this month. The architecture holds.

---

{% hint style="info" %}
**Under the hood.** Engineers can read the full version ladder (0.1.8 → 0.1.13), the operator inspection commands (`pylon status`, `pylon training status`, `pylon wallet history`), runtime prerequisites, and the `--no-launch` bootstrap path in the [Developer Path → Quickstart](../developers/quickstart.md). The release-receipt template lives in [`docs/reports/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports).
{% endhint %}

---

**← Previous:** [04. The Earn Loop](04-earn-loop.md) · **Next:** [06. Data Market MVP](06-data-market-mvp.md) **→**
