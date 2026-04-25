# Compute Market

The compute market is the lane where machine work — inference, embedding, fine-tuning, image generation, and paid pre-training — is priced, matched, executed, validated, and settled in Bitcoin between an open set of providers and an open set of buyers.

On the supply side, providers are retail devices running [Pylon](https://www.npmjs.com/package/@openagentsinc/pylon) v0.1.13+ with the [Psionic](https://github.com/OpenAgentsInc/openagents) edge ML framework as the execution substrate. The canonical starter workload is Stanford's [CS336 A1](https://stanford-cs336.github.io/spring2025/) — a bounded pre-training pipeline (BPE tokenizer, transformer, Adam) that any Autopilot-eligible Pylon can complete. Each accepted contribution pays a floor of **25 sats**, capped at **6,400 sats per cycle per contributor**, with up to **256 contributors** per window.

On the demand side, the buyer of first resort today is the OpenAgents-hosted [Nexus](https://github.com/OpenAgentsInc/openagents/tree/main/crates/nexus-control) dispatcher. In the twelve-month roadmap, any party can publish a kind-`5960` NIP-90 request on the open relay set at [`wss://relay.damus.io`](wss://relay.damus.io) and [`wss://relay.primal.net`](wss://relay.primal.net) to buy compute from the same provider pool.

Prices are not clearing prices yet — they are subsidized floor prices. The [Demo Day pitch](https://bitcoinfi.network/demoday) framed the ceiling: roughly **20 GW of stranded consumer compute** against OpenAI's 2 GW, unlocked by making retail compute priced, verifiable, and payable in Bitcoin.

See also: [Chapter 2 — The Five Markets](../../investors/02-five-markets.md), [Chapter 4 — Earn Loop](../../investors/04-earn-loop.md), [Chapter 5 — Pylon, the Provider](../../investors/05-pylon-provider.md).
