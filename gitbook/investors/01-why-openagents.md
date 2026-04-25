[Home](../README.md) · [Investor Path](README.md) · **01. Why OpenAgents**

# 1. Why OpenAgents

> _"OpenAgents exists to solve one problem. America has no frontier open-source AI lab. This is a massive problem."_
>
> — Christopher David, [BitcoinFi Accelerator Demo Day, April 21, 2026](https://bitcoinfi.network/demoday)

**You will learn:**

- Why we exist — "America has no frontier open-source AI lab"
- The Bitcoin-native thesis behind open-source AI
- What we've already shipped vs. what's still to build

## The wound

Open American AI is in a bad place in 2026, and the layer below it — the compute that trains and serves frontier models — is worse.

Chris walks through the wound on stage in four beats:

- **Meta abandoned Llama** and abandoned open source. The lab that carried American open weights from 2023 to 2025 no longer ships frontier open models.
- **NVIDIA's top open models are not frontier, and they only work on NVIDIA hardware** — not AMD, not Apple. That's a vendor-hardware lock-in dressed up as open source.
- **Allen AI tried to be the "American DeepSeek" — a small, scrappy, independent lab** — and _"their core team got raided by Microsoft, acqui-hired. They now don't have the resources to continue training models."_
- **The companies that used to depend on American models like Llama are switching to Chinese models** because China is leading on open-source weights. _"In 2026, building a supply-chain dependency on China does not make sense."_

Chris names the answer in one sentence:

> _"OpenAgents is America's leading open-source AI lab. Hey, there's not many open-source AI labs, but we're leading. Proudly based in Austin, Texas, Bitcoin capital of the world."_

<figure>
  <img src="../assets/graphics/slide-problem-no-frontier-lab.jpg" alt="Slide: America has no frontier open-source AI lab">
  <figcaption>BitcoinFi Demo Day, April 21, 2026 — the wound in one slide.</figcaption>
</figure>

## The two linked problems the repo names

The public monorepo at [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) states the same wound from a product-architectural angle. The [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md) opens:

> _"OpenAgents is building the economic infrastructure for machine work. We are focused on two linked problems in AI:_
>
> - _agent misuse can create massive economic damage when output outruns verification_
> - _compute supply is constrained, so capacity has to be allocated more intelligently"_

The stage wound and the repo wound are the same door from two angles: if America has no frontier open-source lab, the cause is compute supply that concentrates in a few hands and autonomy that runs without verified settlement. Fix the compute allocation and the verification substrate, and you fix the lab problem at the layer that actually matters.

## What we are, in one breath

OpenAgents is:

1. **A lab** — currently the world's second open-source reproduction of Percepta's _"Can LLMs Be Computers?"_, shipping Psionic (the world's fastest edge inference engine, 30% faster than Ollama), Probe (our coding agent runtime), WGPUI (GPU-accelerated desktop UI), and, as of this month, the beginnings of what we believe will be the largest decentralized model-training run in history.
2. **A marketplace** — five interlocking markets (Compute, Data, Labor, Liquidity, Risk) that settle autonomous machine work in Bitcoin, on one shared economic kernel.
3. **A shipped product** — _Autopilot_, the desktop wedge that turns your machine into a Bitcoin-paying compute provider today, with live earning proofs recorded in the monorepo's [`docs/reports/nexus/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports/nexus).

The rest of this GitBook is the architecture that makes all three possible on one stack.

## Why Bitcoin, why now

On _OAPN #2 — Pylon Launch_, Chris frames why the Bitcoin-Lightning-Nostr substrate is unavoidable for this layer:

> _"We have the benefit in the Bitcoin space of everyone already speaking the same language. We speak Bitcoin at the base layer. We speak Lightning and other related L2s that all use Lightning for interop. There's increasing consensus around Nostr for this sort of like data layer that's adjacent, but not like spraying data onto the chains. So we've got all the makings of decentralized applications — all the substrate for what should be like the agentic AI layers."_

— [_OAPN #2, Pylon Launch_, ~minute 2](https://www.youtube.com/watch?v=uvRO-E9SXI8)

Agents need a native unit of account that settles across machines, crosses borders without permission, and carries a cryptographic proof of work. Bitcoin is the only asset that has all three properties in 2026.

## Why Vegas, why this panel

At Bitcoin 2026 (Las Vegas), Chris is on the **"Why AI Agents Want Bitcoin"** panel (Open Source Stage, 10:45 AM) alongside Erik Cativo (Cashu), Roland Bewick (Alby), and Eric Hadley (Hyperdope, moderating). That panel is the public version of this document. This GitBook is the long form, for investors who want the architecture behind the panel.

## The 90-second version

{% embed url="../assets/clips/cdavid-demoday-highlight-90s.mp4" %}
Christopher David, BitcoinFi Accelerator Demo Day — 90-second highlight from the April 21, 2026 pitch.
{% endembed %}

For the full 4:55 Demo Day segment, see [`assets/clips/cdavid-demoday-41m38s-full.mp4`](../assets/clips/cdavid-demoday-41m38s-full.mp4) or the broadcast at [bitcoinfi.network/demoday](https://bitcoinfi.network/demoday) starting around the 41:38 mark.

---

**Next:** [02. The Five Markets](02-five-markets.md) **→**
