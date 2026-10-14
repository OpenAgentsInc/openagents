# Decision Models and REAX: source notes (read 2026-10-14)

This file collects the sources behind the assessment in
[README.md](README.md), with what each one shows. We did not run any of
their downloads. The site files and the public GitHub repositories were
fetched into a scratch directory and read only.

## Sites

**<https://mine.decisionmodels.io/>** is the landing page for miners. It
also publishes machine-readable files for agents: `/agent.md`,
`/llms-full.txt`, `/mine.json`, `/releases/current.json`, `/install.sh`,
`/hardware.md`, `/rent.md` and `/eu.md`.

- Title: "Bittensor Subnet 92 · REAX · Decision Models".
- Kit 0.1.5; subnet release 1.1.0rc2.
- `install.sh` is a short POSIX script. It downloads
  `kit/reax-miner-kit-0.1.5.tar.gz`, checks sha256 `6b321d28…02bf9`, and
  extracts it. It uses no `sudo` and touches no wallet. A comment says
  "Checksum integrity is not an independent release signature."
- Hardware page, s1-fast pool:
  - Plumb-4B in bf16, about 9 GB of weights.
  - Needs at least 24 GB of VRAM, R580 or newer, a public inbound TCP
    port 8091, and the private sidecar ports 8101, 8102 and 8000.
  - Measured decisions/s: 3090 about 27, 4090 about 30, 5090 about 42,
    A6000 about 19, L40S about 31, RTX PRO 6000 about 56, H100 about 61.
- Rent page: lists Lium (itself a Bittensor subnet, which takes TAO),
  Hetzner, Scaleway, UpCloud and others, with prices checked
  12 October 2026.

**`/releases/current.json`** (schema `reax-release/1`, updated 2026-10-14)
holds a live chain readback:

| Field | Value |
| --- | --- |
| Network, netuid | finney, 92 |
| Block | 9,282,858 at 2026-10-14T16:55Z |
| `owner` | `5GMxpAvB9iJkDtNDELhVn8GmbBz5XChnWcLaThWTJ38QmU6B` |
| `first_emission_block` | 9,280,309 |
| `subnet_emission_enabled` | **false** |
| `registration_allowed` | true |
| `images.public` | **false**, for `ghcr.io/reaxlabs/reax-miner`, `reax-s1-fast-runtime` and `reax-s1-pro-runtime` (tag 1.1.0-rc2; digests recorded) |
| `source` | `github.com/reaxlabs/reax-subnet` at `e5fbb76f`, public |
| s1-fast pool | weights `crh225/plumb-4b` at revision `55de0378…`; manifest `ff6e01b983d48024` |
| s1-pro pool | weights `surogate/rune-26b-a4b-GGUF` at `c6b360d4…` (gated); `enabled_at_launch: false` |

**<https://decisionmodels.io/>** is the customer product: a
"Jev-compatible decision API".

- `/llms.txt` covers models and prices:
  - s1-fast: $0.019 per million input tokens (Peer-to-Peer tier) or $0.021
    (EU tier). Output tokens are free.
  - s1-pro: $0.080 (Peer-to-Peer) or $0.084 (EU).
- Endpoints: `POST https://api.decisionmodels.io/v1/systemone`; a hosted
  MCP server; question types `noul`, `choice` and `score`.
- Hosting: the gateway and database run on Hetzner in the EU, and EU
  inference runs on UpCloud.
- Prepaid top-ups through Stripe, from EUR 1.
- `/evidence`: speed and quality numbers, self-measured on 11–12 October
  2026. It states "JevBench is run by the founder of Decision Models."
- `/legal/p2p-network`: the Peer-to-Peer Network Processing Addendum, v1.0,
  dated 12 October 2026.
  - Opt-in per key.
  - Covers `s1-fast-1` single-question text requests.
  - "Every network answer is checked against our own reference GPU."
  - Providers "see request content in clear text" and "may store content".
- `/legal/imprint`:
  - productivity-boost.com Betriebs UG (haftungsbeschränkt) & Co. KG,
    Reichenberger Str. 2, 94036 Passau, HRA 12725.
  - General partner HRB 8453, managing director Florian Standhartinger.
  - "Jev is a trademark of TypeSafe AI. Decision Models is not affiliated
    with TypeSafe AI."
- `/local/`: a `dm-local` installer whose releases are cosign-signed from
  `github.com/fstandhartinger/decisionmodels-local`. Free for small teams;
  USD 1,000 plus USD 100 a month for larger companies.
- `/hosting/`: dedicated endpoints on rented GPUs, in the EU or global.

**<https://reax.co/>** is the network's marketing site ("AI by the people,
for the people").

- It describes a four-step loop: serve, verify, reward, improve.
- The REAX Engine is described as "our gift to open source".
- Inference pools are 80 % of miner emission and planned "competition
  mining" is 20 %.
- Roadmap, "gates, not dates": testnet unassigned; Peer-to-Peer tier
  routing "design only".
- "Subnet ownership and mainnet operating arrangements are not announced."

**<https://reax.dev/>** holds the developer docs, spec revision 0.6, last
updated 2026-10-11.

- `/mine/`: the seven-step miner guide, including "Can I forward requests
  to an API instead of running the model? Yes, the rules allow it." It
  also says miner pay "is expected to go down over time".
- `/verification/`:
  - The distance formula, thresholds 0.20, 0.50 and 2.0, and calibration
    over 871 decisions on three GPU types.
  - s1-fast results:
    - Honest pairs across GPUs: median distance 0.071, max 0.708, top
      choice agrees 98.7 %.
    - FP8 build: median 0.250.
    - Closest sibling model: median 0.89.
    - Base Qwen3.5-4B: median 1.43.
  - s1-pro: a deterministic vLLM 0.30.0 build (`VLLM_BATCH_INVARIANT=1`)
    gives bit-identical answers across H100 and RTX PRO 6000.
- `/protocol/`:
  - Requests are signed with Ed25519 over RFC 8785 (JCS) JSON, each one
    lives at most 60 s, and probabilities must sum to 1 within 1e-6.
  - Receipts carry no payload, and the docs say a receipt "never says
    verified".
- `/incentives/`: the earlier spec 0.6 design. It has an
  empirical-Bernstein fidelity window, an organic identity shadow at 2 %,
  capacity bursts, and a 40 % operator cap. The docs say Sybil resistance
  is not claimed.
- `/changelog/`:
  - The first docs were published 2026-09-30.
  - Release 1.1.0rc2 came out 2026-10-05 with a miner security fix.
  - Netuid 92 was registered 2026-10-10.

## Code (public GitHub, `reaxlabs` organisation, created 2026-10-10)

**`reax-subnet`** (MIT; created 2026-10-10; head `e5fbb76f`, 2026-10-11).

- About 4,800 lines of Python in `src/reax_subnet`, with `miner.py`,
  `s1validator.py`, `s1scoring.py`, `s1items.py`, `router.py`, `chain.py`
  and `selfcheck.py`.
- Runtime sidecars in `runtimes/`. The s1-fast sidecar is a FastAPI server
  around the `jevk5` package, pinned to commit `allebee/jevk5@85238d7`.
  - Base image: `pytorch/pytorch:2.13.0-cuda13.0`, pinned by digest.
  - Dependencies are date-bounded with `--uploaded-prior-to` but not
    hash-locked.
- `s1items.py` builds items from word pools (names, products, issues)
  with a seeded RNG, so there is no answer key.
- `docs/THREAT-MODEL.md`:
  - S15: relaying validator items through a hosted API is an "accepted
    residual risk".
  - A12: running many hotkeys on one machine is risk "R – Medium". They
    measured 12 hotkeys behind one endpoint, each earning 1/12.
- `docs/EMISSIONS.md`:
  - The chain gives the owner an 18 % default cut.
  - `REAX_BURN_FRACTION` can send weight to the owner UID.
  - If no miner qualifies, all weight goes to the owner UID.

**`reax-engine`** (Apache-2.0; created 2026-10-10; about 2,000 lines of
Python and Triton). A prefill-only server for
`perplexity-ai/pplx-decider-v1.1-27b`, a Qwen3.8-27B hybrid with 48
Gated-DeltaNet layers and 16 noncausal attention layers. Techniques:

- packed varlen batches;
- FP8 W8A8 GEMMs with fused activation quantisation;
- last-layer pruning;
- an experimental calibrated NVFP4 mode.

RESULTS.md, measured on one RTX PRO 6000: 15.70 decisions/s against the
reference server's 5.00 on Decision-Index traffic, with p50 latency of
84 ms against 121 ms.

**`reax-miner-kit`** (MIT; created 2026-10-14).

- `reaxctl` plus `reaxlib`, standard library only.
- Includes `AGENTS.md` and `SPEC.md`, written for coding agents.

## Models (Hugging Face)

**`crh225/plumb-4b`** (Apache-2.0)

- `qwen3_5_text`, tagged `decision-model` and `jevbench`.
- `base_model: alibiserikbay/JevK5`.
- Dataset `crh225/plumb-decisions`.
- Last modified 2026-09-29, about 1.5k downloads.

**`surogate/rune-26b-a4b-GGUF`** (Apache-2.0, gated)

- A Gemma-4 26B-A4B-it fine-tune with bf16 safetensors, despite the repo
  name.
- About 6.6k downloads.

## Third-party mentions

- DataCamp, "System One models: Jev"
  (<https://www.datacamp.com/blog/system-one-models-jev>):
  - Jev is TypeSafe AI's System One model, launched September 2026 in
    early access.
  - It returns bounded choices, scores and yes/no probabilities, with no
    prose.
- A web search for REAX on subnet 92 found nothing independent. One market
  listing (FXEmpire) still labels SN92 "ReinforcedAI", which suggests the
  netuid was recently re-registered.
- The decisionmodels.io home page links a TechCrunch piece about Jev
  (2026-09-30). The site notes that the article is about Jev, not about
  Decision Models.

## Our side (paths in this repository)

- Pylon protocol and its current status:
  - `nips/openagents/NIP-PYLON.md`
  - `docs/compute/pylon.md`
  - `docs/compute/verse-compute.md`
- Decision contract and routing:
  - `nips/openagents/NIP-DEC.md`
  - `docs/api/decisions.md`
  - `docs/decision-models/README.md`
  - `crates/gateway/src/decisions.rs`
  - `crates/gateway/src/decision_dispatch.rs`
- Clef serving and economics:
  - `docs/inference/clef-self-host.md`
  - `docs/inference/clef-native.md`
  - `docs/inference/clef-tpu.md`
  - `docs/inference/decision-throughput.md`
  - `docs/inference/decision-profitability.md`
- Payments:
  - `docs/payments/2026-10-02-central-receive-and-splits.md`
  - `crates/pay-ledger/rules/v2.toml` (`[pylon_job] provider_bps = 8500`)
  - `crates/pay-ledger/src/markets/contribution.rs`
- Sealed (TEE) lane: `nips/openagents/NIP-ATT.md`.
- Verse compute field: `crates/verse-zone-everglade/src/zones/everglade/`
  (`compute.rs`, `layout/pylon_field.rs`).
- Desktop: `crates/openagents-desktop`. The Pylon League panel is in
  `src/grid/panels.rs`. The Episode 289 scenes are `route_future.rs` and
  `route_plugin.rs`.
