# 101. Maya's recipe: V4.1 Flash on one card

> **Status: unimplemented — design draft.** Nothing here runs yet.

[Project Maya](https://github.com/mw00/project-maya) (MIT) serves DeepSeek
V4.1 Flash from one PC with a hot-expert cache and 2–3-bit experts. The
pre-reset Rust stage did the same on one rented RTX PRO 6000 (96 GB, 176 GiB
of RAM, a `g4-standard-48`), not on an RTX 4080. This chapter rebuilds that
setup in Coax and then measures what a 16 GB card can do.

## What this chapter covers

- The exact components from chapters 94, 96, 97 and 98: Engram from mapped
  files, the expert cache, layer-major prefill and the packed cache.
- Routed experts requantized to about 2.6 bits a weight (178 GB) by
  importance-weighted GPTQ, as a separate `consumer-q` profile.
- The exact skip of layers 21–39 for all but the last
  19 × 127 + 1 = 2,414 prompt tokens. Layers 21–39 read earlier tokens only
  through layer 20's compressed state and their own 128-token window.
- A per-machine calibration of the cache size and the miss lane (host CPU
  threads or mapped files).
- The `consumer-q` profile is never qualified and never joins a qualified
  group. It has its own quality card against the reference: top-1 agreement,
  KL divergence and G2 answers.
- The 16 GB card case: on a card such as the RTX 4080, 289 GB of routed
  experts and 203 GB of Engram must live beside the card, so you measure that
  card as its own device class or record a refusal.

## Dependencies

- [Chapter 94](ch094-memory-by-lookup.md),
  [chapter 96](ch096-hot-experts-in-fast-memory.md),
  [chapter 97](ch097-prefill-layer-by-layer.md) and
  [chapter 98](ch098-pack-the-cache-exactly.md).
- [Chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md): the exact
  reference that the quality card compares against.
- [Chapter 50](ch050-quantized-weights-and-caches.md): the rule that a
  quantized profile never inherits the original's evidence.
- A rented large-memory GPU host is an owner budget decision.

## Done means

- The old run's numbers are targets, not results: 24.5 tokens/s decode,
  91% top-1 agreement and mean KL 0.117 for `consumer-q`. Maya's own engine
  on the same machine reached 35 tokens/s, and 54 with drafts (upstream
  measurement).
- The exact prompt skip gives the same logits as running every layer, on
  prompts longer than 2,414 tokens.
- The quality card reports top-1 agreement, mean KL and G2 answers against
  chapter 100's reference, with the host, cache size and miss lane named.
- The 16 GB card result is a measured decode rate with the host RAM named,
  or a stated refusal.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md)
