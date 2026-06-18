# World-Firsts Verification — Launch L-3

- Date: 2026-06-18
- Scope: independent verification of the two "world first" claims to be used
  on camera for the Tassadar Run launch (`docs/transcripts/238.md`).
- Tracking: Launch L-3 (#5395), under EPIC #5392.
- Method: web research (search + source fetch), adversarial — actively hunting
  for the counterexample that would defeat each claim.
- Posture: a claim stays GATED until it is either defensible as-worded or
  narrowed to wording that is defensible. Better we find the counterexample
  than a critic does on camera.

This document records the exact claims, the prior art checked, what each prior
project actually did, whether it defeats the claim, and an honest verdict with
narrowed wording where a qualifier fails.

---

## The two claims (as stated on camera)

From `docs/transcripts/238.md`:

1. "The first AI model training run paid in **Bitcoin**. Specifically, to
   **consumer compute**. Anybody can plug in a computer and get paid Bitcoin
   for helping with this." (Worker runs a job; a validator on the network
   replays it; once verified, both worker and verifier get paid in sats.)

2. "The first **public LLM-computer** (Percepta-class) **training run**."
   LLM-computer = programs compiled directly into transformer weights, executed
   exactly (no gradient descent in the core), as defined by AI lab Percepta.

### Distinguishing conjunction for Claim 1

The claim is only a "first" if **all** of these hold *together*:

- paid in **Bitcoin** — the asset/sats, not a project token, not fiat, not
  off-platform credits;
- for **training / execution compute** — not data collection, labeling,
  annotation, or inference/hosting;
- on contributors' **own consumer devices** — decentralized, not rented
  datacenter GPU;
- **verified** — replay-verified work, not trust-me self-reporting.

### Distinguishing conjunction for Claim 2

- the **compiled-program-in-weights** ("LLM-computer" / Percepta-class)
  paradigm;
- run as a **public, open contributor network** (anyone can join), not a
  single-org closed research artifact;
- as an actual **training run** with paid + verified contribution.

---

## Claim 1 — prior art checked

### Spirit of Satoshi (Laier Two Labs) — the closest Bitcoin-paid AI precedent

- What it actually did: built a "Bitcoin-centric" LLM (Satoshi 7B). It ran a
  Lightning-enabled crowdsourcing tool ("LECS-LLM") that **paid people in sats
  to curate / create / rank Bitcoin data** — ~50M sats paid out, ~40,000
  responses from ~280 contributors, feeding a "Nakamoto Repository." The model
  was then trained / fine-tuned on that data.
- Verdict on the conjunction: **does not defeat Claim 1.** The Bitcoin
  payments were for **data collection and human feedback/curation**, not for
  **training (execution) compute**, and not on contributors' compute at all —
  contributors supplied *labels/answers*, the org supplied the training
  compute. No replay-verified compute attestation. This is precisely the
  distinction the claim is built around.
- Sources: https://www.spiritofsatoshi.ai/ ,
  https://www.spiritofsatoshi.ai/satoshi-7b ,
  https://bitcoinmagazine.com/markets/spirit-of-satoshi-releases-its-first-annual-bitcoin-and-ai-industry-report ,
  https://geyser.fund/project/spiritofsatoshi

### Token-paid decentralized training networks

These are the strongest "decentralized training + pay for compute" precedents.
Every one pays a **project token**, not Bitcoin:

- **Bittensor / Templar (Subnet 3)** — miners contribute commodity/home GPU
  compute and gradients; validators score; rewards paid in **TAO** (and subnet
  alpha token). Templar completed "Covenant-72B," the largest decentralized
  LLM pretraining run, ~70 contributors on home internet. Pays TAO, not BTC.
  - https://www.ainvest.com/news/bittensor-tao-surges-subnet-3-templar-completes-largest-decentralized-llm-pretraining-2603/ ,
    https://blockeden.xyz/blog/2026/03/13/templar-covenant-72b-bittensor-largest-decentralized-llm-pretraining/
- **Gensyn** — verifiable-compute training marketplace; nodes earn the **$AI**
  token for executing training tasks. Pays $AI, not BTC.
  - https://www.gensyn.ai/ ,
    https://www.gate.com/learn/articles/gensyn-ai-tokenomics-analysis-compute-incentives-fee-mechanism-and-ai-value-logic
- **Prime Intellect** — trustless training network with verifiable rewards
  (PRIME-RL / TOPLOC / SHARDCAST); aggregates idle GPUs; rewards in protocol
  token/credits, not BTC. First to train a 10B-param distributed LLM (2024).
  - https://www.gate.com/learn/articles/open-ai-founding-members-invest-a-quick-dive-into-the-decentralized-ai-breakthrough-prime-intellect/7323
- **Nous Research / Psyche (DisTrO)** — decentralized training on idle GPUs,
  incentivized via a **token mechanism on Solana**. Not BTC.
  - https://nousresearch.com/nous-psyche , https://psyche.network/
- **Pluralis** — collaborative/protocol-learning decentralized training;
  ecosystem/token framing, not BTC payment for compute.
  - (covered in the Nous/Psyche ecosystem coverage above)

Verdict: **none defeat Claim 1.** They satisfy "verified-ish / decentralized /
consumer-device training compute" but pay a **token**, never **Bitcoin**. The
asset qualifier is the discriminator and it holds.

### Generic "earn crypto for your GPU" marketplaces

- **Salad** — closest non-token threat. Installs on idle gaming PCs, runs AI
  inference / rendering / *and model training* workloads, pays "**Salad
  Balance**" redeemable for gift cards, games, or crypto. This is the one to
  be careful about.
  - Why it does **not** defeat Claim 1: (a) the unit of account is **Salad
    Balance** (an off-platform credit), with crypto only as one redemption
    off-ramp — it is not "paid in Bitcoin"; (b) the work is **renter-supplied
    jobs on a compute marketplace**, not a single coordinated, verified
    **training run** where contributions are **replay-verified**; (c) there is
    no per-task replay-verification settlement in sats. It is GPU-rental
    earnings, not Bitcoin-for-verified-training-compute.
  - https://salad.com/ , https://community.salad.com/sell-gpu-power/
- **Akash / Render (RNDR) / Vast.ai / RunPod** — compute or GPU-rental
  marketplaces. Akash/Render use their own tokens; Vast.ai/RunPod are fiat
  rental. None pay BTC for verified training compute.
  - https://www.kucoin.com/blog/top-ai-depin-projects-2025-2026-decentralized-infrastructure

Verdict: **none defeat Claim 1.**

### Bitcoin/Lightning + AI projects (payment direction matters)

- **LightPhon** — global GPU network where consumers **buy** AI access/inference
  minutes **with** Lightning sats. This is payment *into* the network for
  inference, the **opposite direction** from "contributors get *paid* BTC for
  training compute." Does not defeat the claim.
  - https://www.lightphon.com/
- **L402 / Lightning Labs agent payments** — a protocol letting agents pay sats
  per API call. Payment rails, not a training-compute contributor network.
  Does not defeat the claim.
  - https://www.kucoin.com/news/articles/ai-agent-payment-paradigm-how-lightning-labs-l402-protocol-reshapes-bitcoin-lightning-network-ecosystem

### Older "Bitcoin mining rigs pivot to AI"

Hardware-utilization pivots (miners renting GPUs to AI) earn fiat or token
rental income for *inference/rendering*; none constitute a verified
training-run paid in BTC to consumer devices. No counterexample found.

---

## Claim 2 — prior art checked

### Percepta (the originator of "LLM-computer")

- What it actually did: published "Can LLMs Be Computers?" (March 11, 2026) and
  open-sourced `Percepta-Core/transformer-vm` (Apache-2.0) — a WebAssembly
  interpreter compiled directly into transformer weights, executing programs
  token-by-token with 100% accuracy (Sudoku, arithmetic), ~33k tok/s on CPU.
- Why it does **not** defeat Claim 2: it is a **single-organization research
  artifact** — a paper plus a repo. There is **no public contributor network,
  no payment, no Bitcoin, no distributed/verified training run**. Percepta
  *defined and demonstrated the paradigm*; it did not run an open paid public
  training network on it. (The transcript itself credits Percepta as the
  paradigm's origin — the claim is about being the first *public network* on
  it, not the first to invent it.)
  - https://www.percepta.ai/blog/constructing-llm-computer ,
    https://github.com/Percepta-Core/transformer-vm ,
    https://towardsdatascience.com/i-built-a-tiny-computer-inside-a-transformer/

### Tracr / RASP / "Thinking Like Transformers" (DeepMind, 2023)

- What it actually did: Tracr is a compiler that converts RASP programs into
  transformer weights, intended as ground-truth models for interpretability
  research. A **technique/tool**, open-sourced by DeepMind.
- Why it does **not** defeat Claim 2: it is a research compiler, not a public
  paid contributor **training run/network**. No contributors, no payment, no
  Bitcoin, no run.
  - https://arxiv.org/pdf/2301.05062 , https://github.com/google-deepmind/tracr

### Any other public compiled-transformer contributor network

Targeted searches for a "public contributor network / compiled transformer /
LLM-computer / paid distributed training run" surface only Percepta (closed
research) and Tracr (research tool). **No competing public paid contributor
network on this paradigm was found.**

---

## Verdicts

### Claim 1 — paid in Bitcoin, for verified training compute, on consumer devices

**Verdict: defensible as a first, with the full conjunction stated explicitly.**

No prior project satisfies all four qualifiers together. The decisive
discriminators are:
- **asset = Bitcoin** (every decentralized-training peer pays a token; Spirit
  of Satoshi paid BTC but for *data*, not compute), and
- **verified training compute on consumer devices** (Salad pays credits for
  uncoordinated marketplace jobs, not replay-verified contributions to a run).

Keep the wording tight. Recommended on-camera phrasing (load-bearing words in
bold) — this is what the evidence supports:

> "The first AI model **training run** that pays independent contributors in
> **Bitcoin** for **replay-verified** training compute on their **own consumer
> devices**."

Guardrails / do-not-drop qualifiers:
- Say **Bitcoin** (not "crypto") — that is the entire discriminator vs
  Bittensor/Gensyn/Prime Intellect/Nous.
- Say **training compute** (not just "compute" or "AI work") — keeps Spirit of
  Satoshi (data) and Salad/LightPhon (inference/rental) out.
- Say **verified / replay-verified** — keeps trust-me GPU-rental out.
- Optional but safest: "paid in **Bitcoin** (sats over Lightning)" to preempt
  the "is it really BTC?" challenge, since the nearest precedent (Spirit of
  Satoshi) also used Lightning sats.

Avoid the over-broad phrasing "first to pay Bitcoin for AI" — Spirit of Satoshi
(data) and LightPhon (buying inference) muddy that, and a critic will cite them.

### Claim 2 — first public LLM-computer (Percepta-class) training run

**Verdict: defensible as a first, with "public" / "open contributor network"
doing the work — and with Percepta explicitly credited as the paradigm's
originator.**

Percepta invented and demonstrated the paradigm but ran no public paid network;
Tracr is an interpretability tool. So "first **public** LLM-computer training
run / open contributor network" holds — but only if "public/open contributor
network" is kept in the sentence and Percepta is credited (which the script
already does).

Recommended on-camera phrasing:

> "The first **public, open contributor** **LLM-computer training run** —
> the compiled-program-in-weights paradigm defined by Percepta, run for the
> first time as a public network anyone can join and get paid for."

Guardrails:
- Do **not** say "first LLM-computer" or "we invented the LLM-computer" —
  Percepta did (March 2026). Credit them; the firstness is the **public paid
  run**, not the paradigm.
- Keep "public" / "open contributor network" explicit — that is what
  distinguishes it from Percepta's closed research artifact.

---

## One-line summary for the launch

- Claim 1: **FIRST**, with the full conjunction stated (Bitcoin + verified
  training compute + consumer devices). Do not generalize to "Bitcoin for AI."
- Claim 2: **FIRST**, narrowed to "first **public** LLM-computer training run /
  open contributor network," with Percepta credited as paradigm originator.

Both are safe to crow on camera **as worded above**. They are not safe in their
loosest form ("first to pay Bitcoin for AI"; "first LLM-computer").

---

## Sources

- Spirit of Satoshi: https://www.spiritofsatoshi.ai/ ,
  https://www.spiritofsatoshi.ai/satoshi-7b ,
  https://bitcoinmagazine.com/markets/spirit-of-satoshi-releases-its-first-annual-bitcoin-and-ai-industry-report ,
  https://geyser.fund/project/spiritofsatoshi
- Templar / Bittensor:
  https://www.ainvest.com/news/bittensor-tao-surges-subnet-3-templar-completes-largest-decentralized-llm-pretraining-2603/ ,
  https://blockeden.xyz/blog/2026/03/13/templar-covenant-72b-bittensor-largest-decentralized-llm-pretraining/
- Gensyn: https://www.gensyn.ai/ ,
  https://www.gate.com/learn/articles/gensyn-ai-tokenomics-analysis-compute-incentives-fee-mechanism-and-ai-value-logic
- Prime Intellect:
  https://www.gate.com/learn/articles/open-ai-founding-members-invest-a-quick-dive-into-the-decentralized-ai-breakthrough-prime-intellect/7323
- Nous / Psyche / DisTrO: https://nousresearch.com/nous-psyche , https://psyche.network/
- Salad: https://salad.com/ , https://community.salad.com/sell-gpu-power/
- DePIN / Render / Akash overview:
  https://www.kucoin.com/blog/top-ai-depin-projects-2025-2026-decentralized-infrastructure
- LightPhon: https://www.lightphon.com/
- L402 / Lightning Labs:
  https://www.kucoin.com/news/articles/ai-agent-payment-paradigm-how-lightning-labs-l402-protocol-reshapes-bitcoin-lightning-network-ecosystem
- Percepta: https://www.percepta.ai/blog/constructing-llm-computer ,
  https://github.com/Percepta-Core/transformer-vm ,
  https://towardsdatascience.com/i-built-a-tiny-computer-inside-a-transformer/
- Tracr / RASP: https://arxiv.org/pdf/2301.05062 , https://github.com/google-deepmind/tracr
