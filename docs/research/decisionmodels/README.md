# Decision Models and REAX: what they built, and our better version (2026-10-14)

Owner request: read our Pylon plan and NIP-PYLON, study
<https://mine.decisionmodels.io/>, work out exactly how their hosting of
"Jiv" models works, and plan our own version inside Verse and the desktop
app, paid with Bitcoin revenue share instead of a token.

"Jiv" is **Jev**: TypeSafe AI's closed System One decision model. Decision
Models does not host Jev. It sells a *Jev-compatible* API (the same
`POST /v1/systemone` shape we already speak in `crates/jev`) backed by open
decision models, and it is starting a Bittensor subnet so other people's
GPUs can serve those models. Their footer says it directly: "Jev is a
trademark of TypeSafe AI. Decision Models is not affiliated with TypeSafe."

Source-by-source notes, with links, are in [evidence.md](evidence.md).

## Summary

- **Who.** Decision Models (decisionmodels.io) and REAX (reax.co, reax.dev)
  are brands of one small German company: productivity-boost.com Betriebs
  UG (haftungsbeschränkt) & Co. KG in Passau, run by managing director
  Florian Standhartinger. Their own evidence page says the founder also
  runs **JevBench**, the leaderboard their marketing cites. Their evidence
  pages are written in the first person ("I measured all of it myself").
  We found no funding announcement.
- **What they sell.** A hosted, Jev-compatible decision API: one forward pass
  per request, a probability for each option, and billing per input token
  in euros through Stripe. s1-fast costs **$0.019 per million input tokens**,
  about half of Jev's price. Models: s1-fast (Blink 26B-A4B, NVFP4), s1-pro
  (vLLM-SR d3 27B, FP8), s1-vision, and Quyet-1.0-Large. It runs on their
  own rented GPUs: UpCloud in the EU, plus Hetzner for the gateway. There is
  also an EU/GDPR tier, dedicated endpoints, and a local installer.
- **The "mining".** REAX is Bittensor subnet 92 on mainnet (Finney):
  - A miner runs two Docker containers on Linux with one NVIDIA GPU of at
    least 24 GB. One container serves **Plumb-4B**, a 4B Qwen3.5 fine-tune,
    in bf16 behind loopback. The other answers validators on public TCP
    port 8091.
  - Every 2 minutes a validator sends 8 synthetic decisions that are new
    for each miner and round. It recomputes them on its own copy of the
    pinned model and compares the probability vectors in logit space.
    Matches score; up to 20 % of the score is for speed.
  - Pay is in the subnet's **alpha token**, credited as stake on the
    miner's key.
- **What is real today (2026-10-14).**
  - Real: the netuid, public source, a careful verification design with
    published GPU calibration, and a small open-source prefill-only engine.
  - Not running yet:
    - No emissions. The chain flag `subnet_emission_enabled` is false.
    - The miner container images are still private, so nobody outside can
      mine.
    - Only one validator exists, theirs, and only in a local test setup.
    - No customer traffic goes to miners. Release 1.1 sends synthetic
      items only.
  - Their legal addendum dated 12 October allows opt-in "network
    processing" of s1-fast-1 requests. Even then, every network answer is
    re-checked on their own GPU before it is returned, so the network does
    not save them compute yet.
- **Our position.** We already have most of the pieces they are still
  designing:
  - a signed, open market protocol (NIP-PYLON, NIP-DEC);
  - a live decision lane on a pylon (Clef-Flash on the CoderOS 4080);
  - a provider revenue share in the ledger (8,500 bps);
  - a Lightning receiver and payout worker;
  - a world that already draws pylons and paid receipts (the Verse Pylon
    Field and Wellspring);
  - our own inference engine, Psionic, that runs on CUDA, Metal and CPU.

  What we lack, and what they did well, is **a calibrated
  answer-verification rule for decisions** and **a one-click way to join**.
- **Our better version.**
  - A contributor turns on "Earn" in OpenAgents Desktop and serves Clef
    decisions. It works on NVIDIA and also on Apple Silicon, which they
    cannot support.
  - No public port, wallet seed, registration fee or token. Our pylons dial
    out to a Nostr relay.
  - We check answers with the same recompute-and-compare method (logit
    distance on a sample of real and canary jobs).
  - We pay sats from real customer revenue to a Lightning address, and the
    contributor sees their pylon and its gold receipts in the Verse.

## REAX's code, read in depth, and what we built from it (2026-10-14)

We read REAX's three public repositories at pinned commits: the subnet
(MIT), the engine (Apache-2.0) and the miner kit (MIT). The full notes are
in [reax.md](reax.md); the calibration we then ran on Clef-Flash is in
[calibration.md](calibration.md).

- **Their approach.**
  - **The subnet.** A validator recomputes fresh synthetic decisions on
    its own copy of the pinned model and compares in centred-log space.
    Limits come from GPU calibration, and a big miss is a strike.
  - **The engine.** A prefill-only server for a 27B decision model, about
    3x the reference: packed varlen batches, FP8 with fused activation
    quantisation, last-layer pruning, and warm-up before health.
  - **The miner kit.** A stdlib-only, agent-first CLI: a doctor whose
    blocks carry fixes, pinned digests, a self-check against shipped
    reference answers, and updates that never silently change the served
    model.
- **What we adapted and shipped**, in the `openagents` CLI:
  - the distance and round rule, with limits measured on Metal, CPU and
    CUDA fixtures (#11632);
  - `pylon check decide`: fresh items, a verified subset drawn after the
    answers arrive, and NIP-32 verdicts (#11633);
  - `pylon selfcheck`, enforced by `serve --decide` (#11634);
  - `pylon doctor`, `setup`, `start` and `update` (#11635);
  - the anti-gaming rules in NIP-PYLON's abuse table (#11637).

  Engine ideas for Psionic's Clef lane are in #11636.
- **What we reject.** Token emissions and an owner cut, registration
  burns, coldkeys, inbound port 8091, Docker as a requirement, and pay
  that is not from revenue.
- **Build order.** Steps 1–3 below are built, and a step 3b (one-command
  setup) is added and built. That makes step 5 (the Desktop Earn switch) a
  thin UI over `pylon doctor`, `setup` and `start`. Step 4 (price and
  mainnet pay) is still the next owner decision.

## Their technology: verified versus inferred

| Claim | Status | Evidence |
| --- | --- | --- |
| REAX is Bittensor netuid 92 on Finney | **Verified** | `releases/current.json` chain readback, block 9,282,858, owner `5GMxpAvB9…QmU6B` |
| Emissions are running | **False today** | Same readback: `subnet_emission_enabled: false`; `first_emission_block` 9,280,309 is set but not active |
| Anyone can mine now | **False today** | `images.public: false`; the agent playbook stops installs at the "source/images" gate |
| Registration is open | **Contradictory** | `registration_allowed: true` on chain, and the mine page says registration is open; reax.dev and reax.co (updated 11 Oct) still say "registration remains closed pending a separate release gate" |
| Miners host the models customers use | **Partly** | The miner pool serves Plumb-4B. The API's s1-fast is Blink 26B-A4B. Their docs say only `s1-fast-1` on network-enabled keys "may be answered by its original model", which looks like Plumb-4B |
| Customer traffic reaches miners | **Not yet** | reax.co: "Validators score with synthetic items; no Decision Models customer traffic is routed through REAX yet." The P2P addendum (12 Oct) creates the legal path, and the gateway change is still listed as "design only" on the roadmap |
| Verification by recomputation in logit space | **Verified in source** | `reaxlabs/reax-subnet` `s1scoring.py`, `s1validator.py`, `docs/CALIBRATION.md`; the formula is on reax.dev/verification |
| Honest miners pass across GPUs; FP8 and substitutes fail | **Measured by them** | 871 decisions on RTX PRO 6000, H100 and RTX 4090. Cross-GPU median distance 0.071; FP8 0.250; nearest sibling model 0.89. Nobody has reproduced this |
| REAX Engine about 3x the reference server | **Code is public; numbers are theirs** | `reaxlabs/reax-engine`, Apache-2.0, about 2,000 lines of Python and Triton; RESULTS.md has 15.7 vs 5.0 decisions/s on one RTX PRO 6000 |
| "#1 on Decision Index" and JevBench capability claims | **About open models, self-reported** | Their evidence page; they disclose that the founder runs JevBench |
| A miner can relay to the paid API instead of running a GPU | **Admitted** | Threat model S15 is an "accepted residual risk"; reax.dev says "the rules allow it" |
| Sybil (many hotkeys on one GPU) | **Admitted** | Threat model A12, risk "R – Medium". A 40 % per-operator cap is designed; the spec admits it can be evaded with unlinked coldkeys |

### What a miner runs

- **Hardware.** One NVIDIA GPU with at least 24 GB per miner. Driver R580
  or newer (CUDA 13). Linux x86_64; WSL2 is "conditional". 4 cores, 32 GB
  RAM, 20 GB disk. Public IPv4 or IPv6 with inbound TCP 8091, and NTP.
  Apple, AMD, Intel and 16 GB cards are refused.
- **Precision.** No quantised builds: FP8 and smaller formats drift past
  the tolerance and score zero.
- **Measured speed** (Plumb-4B, batch 1, from their calibration): RTX 3090
  about 27 decisions/s, 4090 about 30, 5090 about 42, H100 about 61.
- **Software.**
  - `reaxctl`, a stdlib-only Python kit (`reax-miner-kit`, MIT), with
    `doctor`, `plan`, `install`, `selfcheck`, `register`, `start`,
    `verify`, `service` and `update`.
  - It pulls `ghcr.io/reaxlabs/reax-miner` and `reax-s1-fast-runtime` by
    digest. The runtime is FastAPI on loopback port 8101. It verifies the
    sha256 of every weight file and of the runtime source against a
    manifest, then runs the `jevk5` reference readout with CUDA graphs at
    batch 1: letter logits divided by T = 2.07, then softmax.
  - The miner publishes an on-chain commitment
    `reax-s1/1:s1-fast:ff6e01b983d48024` that names the manifest it serves.
- **Models.**
  - Plumb-4B (`crh225/plumb-4b`, Apache-2.0) is a Qwen3.5-4B fine-tune of
    `alibiserikbay/JevK5`, a small open Jev imitation.
  - The planned s1-pro pool, which is off, is Surogate Rune 26B-A4B v3, a
    Gemma-4 26B-A4B fine-tune that needs an 80–96 GB GPU.
- **Onboarding is agent-first.** The site is written for a coding agent:
  `/agent.md`, `/mine.json`, `/llms-full.txt` and WebMCP tools. It is
  "tested with Claude Code". The rules are strong on key custody: the agent
  never sees the seed, and a human approves every payment.

### How work is assigned and verified

- **Items.** Synthetic typed decisions (choice, noul, score) built from
  word pools with a 128-bit seed per miner and round, so there is no answer
  key to leak. Requests are signed with Ed25519 over RFC 8785 JSON and
  bound to the miner's hotkey, so one miner's answer can't be passed off as
  another's.
- **Distance.** `d(p,q) = T · max_i |centred ln p_i − centred ln q_i|`,
  averaged over the round's verified items (D).
- **Thresholds.**
  - D ≤ 0.20 scores 0.8 + 0.2 × speed.
  - D > 0.20 loses the round.
  - D > 0.50, or any single item above 2.0, is a strike that resets the
    moving average to 0.
  - s1-fast verifies all 8 items. s1-pro would verify a secret 3 of 8,
    drawn after the answers arrive.
- **Turning scores into pay.** Validators submit weights about every 72
  minutes, and the chain's Yuma consensus turns them into emission shares.
  The subnet owner takes an 18 % default cut. If no miner qualifies, the
  weight goes to the owner UID.
- **Older design, still documented.** Spec 0.6 has hidden probes, an
  "organic identity shadow" (2 % of real traffic re-run on a reference),
  and capacity bursts. The changelog says release 1.1 replaced it and that
  "nothing in the code implements 0.6 yet".

### How rewards are paid

- **What miners get.** Bittensor emissions in the subnet-92 alpha token,
  credited as stake on the hotkey. To get money, a miner unstakes alpha
  into TAO through the subnet's pool (slippage, about 0.05 % fee), sends
  the TAO to an exchange, and sells it.
- **Costs.** The miner pays a non-refundable TAO registration burn, the
  GPU or rent, and power.
- **Rewards will shrink.** Their own guide says miner pay "is expected to
  go down over time": they plan to divert emission and admit many miners.
- **Billing and emissions are separate by design.** Customer euros go to
  the company, and miners are paid in emissions, not revenue.

### Their EU and privacy story

- **The EU tier never touches the network.** EU-tier requests stay with
  the company's own EU GPUs.
- **Contracted EU miners are planned.** They would sign a GDPR Art. 28
  sub-processing agreement online, and that EU routing is off.
- **The P2P addendum is blunt about privacy.** Network providers see
  state, question and options "in clear text" and "may store content", so
  customers must not send personal data on network-enabled keys.

### What we think is really going on

The network is not doing much for them yet. Every answer is re-run on
their own GPU, customer traffic is off, and the reward is a new subnet
token.

What it buys them:

- a story ("AI by the people");
- a distribution channel into the Bittensor miner community;
- a token whose owner cut accrues to the subnet owner.

The parts worth taking are the technical ones:

- The **verification rule** suits decision models. A decision is one
  prefill and a short probability vector, so recomputing it is cheap, and
  the logit gaps act as a fingerprint of the weights and the prompt.
- The **calibration discipline**: measure honest cross-hardware noise, and
  set the tolerance between honest noise and the cheapest cheat.
- The **"deterministic or nothing" serving rule**: they reject stock vLLM
  for s1 because it flips 6 of 300 answers between identical passes.

## Side by side

| | Decision Models / REAX (SN92) | OpenAgents Pylon (NIP-PYLON) today | Our better version |
| --- | --- | --- | --- |
| Contract | Jev-compatible `/v1/systemone` | Same: NIP-DEC, `/v1/systemone` and `/api/v1/decisions` | Same |
| Models served by contributors | Plumb-4B bf16 (s1-pro pool off) | Clef-Flash 9B Q4_K_M through Psionic; Qwen 0.8B for text | Clef-Flash first, Clef 27B for big machines; open Jev-compatible models where licences allow |
| Hardware | NVIDIA 24 GB+, Linux, CUDA 13 | CUDA, Metal or CPU | NVIDIA 8 GB+ and Apple Silicon 16 GB+ (Clef-Flash Q4 is about 5 GB) |
| Network | Public IP plus inbound port 8091 | Outbound to a Nostr relay; no inbound port | Same; works behind home NAT |
| Join flow | Coding agent plus wallet, TAO purchase, registration burn, Docker | CLI (`openagents pylon serve …`); key allowlist (`DECISION_PYLONS`) | One "Earn" switch in Desktop; a Lightning address or our wallet |
| Work | Synthetic validator items only | Free decision jobs from our gateway; 1-in-20 shadow re-ask | Real paid decision traffic plus hidden canaries |
| Verification | Recompute every item; logit distance ≤ 0.20; strikes | Canaries, exact-text redundancy, artifact digest, shape checks, NIP-32 verdicts | Recompute a sample at a reference door; logit distance with per-hardware calibrated tolerance; NIP-32 verdicts; forfeit unpaid balance |
| Pay | Alpha token emissions; 18 % owner cut; shrinking by design | Ledger share 8,500 bps; test networks only | Sats from the customer's payment, 85 % provider share, swept to Lightning |
| Sybil | Per-hotkey reward; admitted Sybil risk | Receipts only from trusted buyers count | Pay per verified decision, so extra keys earn nothing extra |
| Relay cheat | Allowed ("the rules allow it") | Artifact digest named in answers | Unprofitable by price (below), plus latency and canary checks |
| Privacy | Clear text to anonymous miners; opt-in | Encrypted transport (NIP-44) to the pylon; pylon sees content | Opt-in "community" lane only; sealed lane (NIP-ATT, TDX) for private work |
| Visible to users | Docs, metagraph | Verse Pylon Field, Wellspring, gold coin-light on paid receipts | Same, from live beacons and receipts |

## Our better version

### Built into Desktop and Verse

- **Earn switch** in OpenAgents Desktop. It starts the bundled Psionic
  decision server and `pylon serve --decide … --decisions-only` as child
  processes on a background-priority lease. It downloads Clef-Flash Q4 and
  checks its digest. It drains when the person's own work needs the
  machine. On a laptop it pauses on battery and when the machine is hot.
  It shows: today's verified decisions, sats earned and paid, and whether
  the machine is serving or paused.
- **Payout setup** in the same panel: a Lightning address or Spark
  address, or the built-in wallet. No seed handling for the "earn" part at
  all.
- **Verse.** The Pylon Field already draws one pylon per machine, and the
  Wellspring already draws the pool aggregate. We switch the field from
  `Sim` to `RelaySource` for real beacons, and the gold coin-light flares
  on each paid, verified receipt. A contributor can find their own pylon
  in the world. This is the "dots going back and forth" from Episode 289,
  but for compute.

### Bitcoin rewards and revenue share instead of a token

- **Pay comes from revenue.** A customer pays for a decision (sats per
  million input tokens on the rate card). The pylon that answered gets the
  `pylon_job` share in `crates/pay-ledger/rules/v2.toml` (8,500 bps), and
  we keep the rest. Balances sweep through the existing payout worker (at
  1,000 sats owed, or once the oldest share is ten minutes old). The
  owner's mainnet `grant.json` caps each payment and each day.
- **No token, no emissions, no registration fee.** Contributor income
  tracks real demand. We say plainly that it can be zero when there's no
  traffic.
- **Optional launch bonus** paid from our own share, like the plugin
  launch bonus. It is never minted.
- **Later: a reward for faster serving.** The equivalent of their planned
  "competition mining" is our existing `verified_optimization`
  contribution class (`crates/pay-ledger/src/markets/contribution.rs`). It
  pays a fixed reward for a Gym-confirmed speed or accuracy improvement to
  the Psionic Clef path.

### How verification works on our rails

1. **Pinned identity.** The beacon and every answer name
   `clef-flash@sha256:<artifact digest>`, as the gateway already requires.
2. **Recompute a sample.** The gateway already re-asks 1 in 20 answers at
   the next door. For pylon answers we make that the verification door: a
   reference Clef run on our hosted TPU or H100. We compare the probability
   vectors with the centred-logit distance. The tolerance is set per
   hardware class (CUDA, Metal, CPU) from a calibration run, the same
   method REAX used. Recompute is cheap: about $0.0058 per million tokens
   on TPU v6e spot (`docs/inference/decision-profitability.md`), so a 5 %
   sample costs a tiny fraction of what we pay the pylon.
3. **Hidden canaries.** `pylon check canary` already sends known-answer
   jobs through the normal path. We add decision canaries with stored
   reference vectors.
4. **Consequences** use the existing machinery:
   - A failed check is a NIP-32 `check-fail` label.
   - The pylon leaves the checked pool and forfeits its unpaid balance
     for the failed jobs.
   - Three strikes and it needs a fresh calibration self-check before
     serving again.
   - Payment is held until the sampling window closes, so a cheat loses
     more than it gains.
5. **Self-check before serving.** When the switch turns on, the app runs a
   few hundred reference decisions locally and refuses to serve if this
   machine's distance is above the line. This is their `reax-selfcheck`,
   which is a good idea.

Open technical risk: we use Q4_K_M weights on three backends, and they
avoid quantisation entirely. Psionic's own measurements show Q4 against f32
at mean cosine 0.989 on hidden rows, and 99.0 % top-choice agreement on the
4080 CUDA run. We need the cross-backend distance table (CUDA vs Metal vs
CPU, all on the same Q4 GGUF) before we pick a tolerance. If the noise is
too high, we pin one runtime per hardware class or add a batch-invariant
deterministic mode to Psionic.

### Which models contributors serve

| Model | Who can serve it | Why |
| --- | --- | --- |
| **Clef-Flash** 9B, Q4_K_M (about 5 GB) | NVIDIA 8 GB+, Apple Silicon 16 GB+, fast CPUs | Our first door; Apache-2.0; Psionic already serves it on CUDA, with Metal in progress |
| **Clef** 27B, Q4_K_M (19.2 GB) | NVIDIA 24 GB+, Mac 32 GB+ | The bigger tier for higher-stakes decisions |
| Open Jev-compatible models (Plumb-4B, Blink 26B-A4B, vLLM-SR d3, Perplexity Decider) | Same classes by size | Catalog breadth; licence review needed per model |
| **Jev** | Nobody | Closed weights, hosted by TypeSafe only. Jev stays a backup door, never a contributor model |
| Kev, Lev, Laya | The user's own device | Local decision implementations for that person's own work, not market serving |

### Safety and abuse

- **Relay to a hosted API.** A pylon could forward work to our own API, or
  to Cloudflare's Clef at $0.038 per million tokens, instead of computing.
  Relaying to our API loses money because the provider share is below our
  price. Relaying to a cheaper outside API is the real risk. Our price
  floor and the share must keep the provider payout below the cheapest
  public price for the same model. We also flag pylons whose latency
  pattern looks like an upstream call.
- **Sybil.** We pay per verified decision, not per key, so ten keys on one
  GPU earn the same as one. Pool counts accept receipts from trusted buyers
  only, and receipts from the provider or its owner never count. This is
  already in the NIP-PYLON abuse table.
- **Content exposure.** The pylon sees the decision's state in clear text.
  - Contributor serving is opt-in per customer key, as a "community
    compute" lane.
  - Our own internal decisions use it by default only for non-personal
    content.
  - Private work goes to the sealed NIP-ATT lane or our hosted doors.
  - The gateway strips account and key identifiers before dispatch.
- **Malicious contributor binaries or models.** Contributors run our
  signed Desktop build and a digest-checked GGUF. No third-party
  containers.
- **Contributor safety.** No seed phrases, no inbound ports, no `sudo`.
  The app pauses on battery and when hot. Earnings are shown as "earned so
  far", never as projections.
- **Payout compliance.** Paying many people for services may need payee
  records above a threshold. This is an owner decision (open question 4).
- **Copy.** The earn panel and Verse labels follow the no-machine-talk
  rule: "serving", "paused", "paid", not "admitted" or "aggregate".

## Build order, smallest first

| # | Step | Reuses | Done when |
| --- | --- | --- | --- |
| 1 | **Decision distance and calibration.** Add the centred-logit distance to `nostr::pylon` or the gateway's shadow compare. Run 800 reference decisions through Psionic Clef-Flash on CUDA (4080), Metal (M-series) and CPU, against the TPU BF16 reference, and record the distance table | `crates/psionic` Clef, `clef-tpu`, the shadow re-ask in `decision_dispatch.rs` | A checked-in calibration note with the honest-noise and Q4-vs-substitute distances, and a chosen tolerance per class |
| 2 | **Decision canaries and sampled recompute** in `pylon check`, emitting NIP-32 verdicts; a failure benches the pylon and forfeits its unpaid share | `pylon check canary`, NIP-32, `pylon::broker` forfeit | A fixture pylon serving a substitute model is caught and excluded |
| 3 | **Local self-check** (`pylon selfcheck --decide`) that refuses to serve above the line | Step 1 reference set | A Metal and a CUDA machine pass; a mismatched GGUF fails |
| 4 | **Price decisions and pay pylons on mainnet.** Rate-card entry for decisions; `pylon_job` share for decision receipts; owner `grant.json`; a small allowlisted cohort first | `crates/inference` rate card, `crates/pay-ledger`, payout worker, `crates/wallet --lsp mdk` | First real sats paid for verified decisions (owner step in `NEEDS_OWNER.md`) |
| 5 | **Desktop "Earn" switch**: start and stop Psionic and the pylon, download the model, set the payout address, show earnings from `/v1/earnings`, pause on battery | `crates/openagents-desktop` (beside the Pylon League panel), `gateway/src/earnings.rs` | A new Mac user earns their first verified paid decision with no terminal |
| 6 | **Verse live field**: the Pylon Field reads `RelaySource`; gold receipts come from real paid, verified receipts | `crates/verse-zone-everglade` compute and `pylon_field` | The contributor sees their own pylon and its paid receipts in the world |
| 7 | **Open admission**: replace the `DECISION_PYLONS` allowlist with checked-pool admission plus per-owner rate limits | NIP-PYLON pools and aggregates | Unknown pylons join, get checked, and are paid without manual allowlisting |
| 8 | Later: Clef 27B tier, sealed contributor lane (NIP-ATT), `verified_optimization` speed rewards | `pylon serve --attested`, `contribution.rs` | Each its own issue |

## Open questions for the owner

1. **Decision price and share.** Decisions are free today. What sats price
   per million input tokens, and do we keep 8,500 bps for decision pylons?
   It must keep relaying to the cheapest outside API unprofitable.
2. **Customer traffic to contributors.** Opt-in per key only, as Decision
   Models does? Or should our own internal non-personal decisions (router,
   Coder judges) also run on contributor pylons by default?
3. **Apple laptops.** Allow serving on battery-powered Macs at all, or
   only when plugged in? We propose plugged in only.
4. **Payout records.** At what yearly amount do we ask a contributor for
   payee details?
5. **Launch bonus.** Fund a first-sats bonus for contributors from our
   share, like the plugin launch bonus?
6. **Third-party open models.** Should contributors also serve open
   Jev-compatible models (for example Plumb-4B or vLLM-SR d3), or only
   Clef?
7. **Public positioning.** Name Decision Models and REAX in public copy
   ("no token, no registration fee, works on a Mac"), or just ship?
