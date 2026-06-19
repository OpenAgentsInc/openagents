# Launch Evidence Pack — "The Tassadar Run is Live" (Episode 238)

- Date: 2026-06-18
- Scope: the public, dereferenceable backing for every on-camera claim in
  `docs/transcripts/238.md`.
- Tracking: Launch L-6 (#5398), under EPIC #5392.
- Companion docs: `docs/launch/2026-06-18-world-firsts-verification.md` (L-3
  prior-art search) and `docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md`
  (claim-by-claim launch-gate audit).

This is the "evidence pack built into all this" the video promises. The
standard the video sets is explicit: *"we're not going to be making any big
claims that aren't sourced by evidence."* So for each on-camera claim this doc
records:

1. the exact claim wording (from the transcript);
2. the public, dereferenceable URL/ref that proves it; and
3. the honest status — **settled-live**, **mechanism-proven**, or **gated**.

A skeptic — human or agent — can verify every claim below by dereferencing the
listed refs. Every public endpoint here was curl-checked on 2026-06-18; route
normalization notes are recorded in §9 rather than papered over.

> Honest-scope note: this document is public-safe. It contains no secrets, no
> wallet seeds, no payment hashes, no raw Lightning/Bitcoin addresses, and no
> private data. It links only to public projections and content-addressed refs.
> Tip/receive destinations may be public; private payment material never is.

> Status legend:
> - **settled-live** — true today on the live system, with current public
>   evidence, for the scoped wording.
> - **mechanism-proven** — the mechanism is implemented and proven end-to-end at
>   least once, but the fully-autonomous / at-scale version is not yet proven.
> - **gated** — defensible only with full qualifiers and/or held behind a RED
>   product-promise pending receipt-first upgrade. Not safe as bare copy.

---

## 0. The master refs (the front door)

| Ref | URL | Status |
|---|---|---|
| Live run summary (machine-readable) | `https://openagents.com/api/public/tassadar-run-summary` | resolves (HTTP 200) |
| Per-run settlements feed | `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements` | resolves (HTTP 200) |
| Run status API | `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615` | resolves (HTTP 200) |
| Verification challenge API | `https://openagents.com/api/public/training/verification-challenges/training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c` | resolves (HTTP 200) |
| Proof replay bundle | `https://openagents.com/api/public/proof-replays?ref=first-real-settlement` | resolves (HTTP 200) |
| Public run page | `https://openagents.com/tassadar` and `https://openagents.com/training/runs/run.tassadar.executor.20260615` | resolves (HTTP 200) |
| Agent front door | `https://openagents.com/AGENTS.md` | resolves (HTTP 200) |
| Install + test guide | `https://openagents.com/INSTALL.md` | resolves (HTTP 200) |
| Product-promise registry | `https://openagents.com/api/public/product-promises` | source registry target `2026-06-19.1`; public endpoint resolves (HTTP 200) |
| Product-promise page | `https://openagents.com/docs/product-promises` | resolves (HTTP 200) |

The run summary is the spine of the pack: it is a single live projection
(`schemaVersion: openagents.public_tassadar_run_summary.v1`,
`maxStalenessSeconds: 0`, rebuilds on verification-challenge transitions) that
carries the run state, the verified/rejected replay pairs (worker + validator +
verdict refs), the reconciled real-Bitcoin settled total, and the embedded
settlement rows. Every other ref below either lives inside it or is linked from
it.

---

## 1. "The Tassadar run is live"

> **Claim (238.md):** "The Tassadar run is live. … We've begun this run. … it's
> paying people for contributing compute."

**Proof:**
- `https://openagents.com/api/public/tassadar-run-summary` →
  `runRef: run.tassadar.executor.20260615`, `runState: active`,
  `run.state: active`, one active window
  (`training.window.tassadar.executor.20260615.w1`), `verifiedWorkCount: 6`,
  `acceptedTraceCount: 6` in the verified-trace corpus.
- `https://openagents.com/tassadar` — the public run page.

**Status: settled-live (scoped).** The run is genuinely live and active, with a
verified-trace corpus of 6 replay-verified traces. Honest scope: this is a
bounded **executor-trace** run growing a verified-trace corpus
(`objective: "Grow the Tassadar verified-trace corpus via paid executor-trace
work, verified by exact replay."`), not network-scale paid training. "It's
paying people" is true for **two** distinct independent contributors so far (see
§2), not at scale. The run summary itself carries live blockers
(`blocker.training.monday_launch_self_serve_stranger_payout_pending`,
`blocker.training.live_settlement_projection_pending`,
`blocker.training.autopilot_install_bundled_node_pending`) — do not imply
network-scale earning.

---

## 2. "Real Bitcoin settlements" — real vs simulation, honestly distinguished

> **Claim (238.md):** "it's paying people for contributing compute" /
> "both worker and verifier are going to get paid. Right now, it's five Bitcoin
> sats each."

**Proof — the reconciled real total:**
- Run summary →
  `summary.metrics.providerConfirmedSettledPayoutSats: 1005`,
  `summary.settlement.reconciledState: "settling"`,
  `summary.settlement.settledReceiptCount: 2`,
  `summary.metrics.qualifiedContributorCount: 2`.
- Per-run settlements feed
  `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements`
  (`schemaVersion: openagents.training_run_settlements.v1`).

**The three settlement rows (real vs simulation distinguished honestly):**

| amountSats | movementMode | realBitcoinMoved | contributor | receipt ref | counts toward real total? |
|---|---|---|---|---|---|
| 5 | **simulation** | **false** | pylon.448ba824… | `receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2` | **No** — simulation, excluded |
| 1000 | real_bitcoin | true | pylon.448ba824… | `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` | Yes (owner-armed canary) |
| 5 | real_bitcoin | true | pylon.81f0facfe… | `receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1` | Yes (self-serve, operator-retro-settled) |

Real settled total = **1,005 sats** to **two distinct independent contributors**
(the simulation row is excluded). Each real-Bitcoin receipt is independently
dereferenceable as both an API JSON and a human receipt page:

- 1,000-sat canary receipt:
  - API: `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618`
  - Page: `https://openagents.com/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618`
  - `realBitcoinMoved: true`, `movementMode: real_bitcoin`, `adapter: spark_treasury`,
    `asset: bitcoin`, `amountSats: 1000`, tied to challenge
    `training.verification.challenge.071445c5-…`.
- 5-sat self-serve (Trigger) receipt:
  - API: `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1`
  - Page: `https://openagents.com/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1`
  - `realBitcoinMoved: true`, `amountSats: 5`, contributor `pylon.81f0facfe…`, tied
    to challenge `training.verification.challenge.10c3b01b-…`.

**The first hygiene-lane real settlement (a separate proof of the rails):**
- Ref: `receipt.nexus.hygiene_lane_settlement.sha256_c81865d82fd5d3ac33757e7935e5ed8fd895ed13ba8deff2c6e34c60d7b6d7a3`
  - API: `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.hygiene_lane_settlement.sha256_c81865d82fd5d3ac33757e7935e5ed8fd895ed13ba8deff2c6e34c60d7b6d7a3`
  - Page: `https://openagents.com/nexus-pylon/receipts/receipt.nexus.hygiene_lane_settlement.sha256_c81865d82fd5d3ac33757e7935e5ed8fd895ed13ba8deff2c6e34c60d7b6d7a3`
  - `realBitcoinMoved: true`, `movementMode: real_bitcoin`, `amountSats: 75`,
    `asset: bitcoin`, `adapter: spark_treasury`, `runRef: run.hygiene.lane.20260618`,
    `verificationBasis: hygiene_merged_reviewed`, tied to merged
    `pr.public.github.openagentsinc_openagents.5358` with reviewer acceptance
    `review.public.debt_receipt.5358.accepted`. The receipt ref is itself the
    SHA-256 idempotency key (`sha256_c81865…`); re-submitting the same key is
    idempotent and a duplicate replay is rejected. (This is a separate
    pay-for-merged-reviewed-work lane, not part of the Tassadar run total; it is
    additional evidence that real-Bitcoin settlement receipts are produced and
    are dereferenceable.)

**Status: settled-live, with one honesty caveat.** Real Bitcoin has genuinely
moved to independent contributors, and every claim is content-addressed in a
dereferenceable receipt. The honesty caveats that must travel with this claim:
(a) the 5-sat simulation row must never be counted as paid — the projection
already excludes it (`1,005`, not `1,010`); (b) the 1,000-sat row is an
owner-armed canary, and the 5-sat self-serve row was **operator-retro-settled**
(auto-stream skipped at verdict; payout-target bug, since fixed) — so neither is
yet a fully-autonomous self-serve payout (see §4). Backing promise:
`training.decentralized_training_launch.v1` (**green, bounded**).

---

## 3. World firsts (verified, owner-final wording)

> **Claim (238.md):** "We've got two world firsts. One is … the first AI model
> training run paid in Bitcoin. Specifically, to consumer compute. … And the
> second: the first public LLM-computer training run."

**Proof of verification:** `docs/launch/2026-06-18-world-firsts-verification.md`
— an adversarial prior-art search (Spirit of Satoshi, Bittensor/Templar, Gensyn,
Prime Intellect, Nous/Psyche, Salad, LightPhon, L402, Percepta, Tracr) that
hunts for the counterexample that would defeat each claim and finds both
defensible **only with their full qualifiers**.

**Owner-final defensible wording (say exactly this):**
- **#1** — "The first AI model **training run** that pays independent
  contributors in **Bitcoin** for **replay-verified** training compute on their
  **own consumer devices**." (Credit Percepta as the paradigm originator for the
  LLM-computer framing.)
- **#2** — "The first **public, open-contributor** **LLM-computer training run**
  — the compiled-program-in-weights paradigm **defined by Percepta**, run for the
  first time as a public network anyone can join and get paid for."

**Do NOT say (these are not defensible and a critic will cite a counterexample):**
- "first **decentralized** training run" — token-paid decentralized training
  (Bittensor/Templar, Gensyn, Prime Intellect, Nous/Psyche) predates us; our
  discriminator is **Bitcoin**, not decentralization.
- "first to pay **Bitcoin for AI**" — Spirit of Satoshi paid sats (for data) and
  LightPhon uses sats (buying inference); too broad.
- "first **LLM-computer**" / "we **invented** the LLM-computer" — **Percepta**
  did (Mar 2026, `https://www.percepta.ai/blog/constructing-llm-computer`,
  `https://github.com/Percepta-Core/transformer-vm`). Our firstness is the
  **public paid run**, not the paradigm.
- Dropping any qualifier from #1 (must keep **Bitcoin** + **verified/replay-
  verified** + **training compute** + **own consumer devices** together).

**Status: gated.** Both world-first claims are backed by an independent search
and are defensible **as worded above**, but are held RED in the registry pending
an owner-signed receipt-first upgrade:
- `claims.world_first_ai_training_paid_bitcoin.v1` — **red**
- `claims.world_first_public_llm_computer_training_run.v1` — **red**
Use the full qualified wording only. Credit Percepta. Accuracy boundary: the
live run is **exact-trace executor PoC** work with **"no gradient descent"** (the
transcript's own words); "training run" is defensible in the executor-
construction sense, not as gradient-descent model training — do not conflate.

---

## 4. The Live Money Loop

> **Claim (238.md):** go to openagents.com/agents.md → join the run → download
> Pylon → "CLAIM WORK" → "WORKER RUNS JOB" → "VALIDATOR REPLAYS" → "VERIFIED" →
> "PAY 5 [sats] TO W + V."

**Proof the loop ran end-to-end (worker → validator-replay → verified):**
- Run summary `realGradient.verifiedReplayPairs` — six verified
  `exact_trace_replay` pairs, each with a public `workerRef`, a public
  `validatorRef`, the `challengeRef`, and the `verdictRef`. Example pairs where
  the worker and validator are **distinct** pylon identities:
  - `training.verification.challenge.10c3b01b-…`: worker `pylon_45b58c56…`,
    validator `pylon_acdbc165…`, verdict
    `verdict.training.exact_trace_replay.verified…10c3b01b…`.
  - `training.verification.challenge.729b4bc1-…`: worker `pylon_b151f663…`,
    validator `pylon_fec27f0a…`.
  - `training.verification.challenge.071445c5-…`: worker `pylon_70e0e962…`,
    validator `pylon_3fd32a38…`.
- Run summary `realGradient.rejectedReplayPairs` — three **rejected** pairs
  (`ExecutorTraceMismatch`), evidence the verifier actually rejects bad work
  (`verifiedWorkCount: 6`, `rejectedWorkCount: 3`). The verifier is not a
  rubber stamp.
- The "pay 5 sats each" leg: the self-serve 5-sat real settlement
  (`…retro.10c3b01b.trigger.v1`, §2) is tied to challenge
  `training.verification.challenge.10c3b01b-…`, closing the loop verdict→payout
  for one real contributor.
- The first auto-stream visibility capture for the same challenge is recorded in
  `docs/launch/2026-06-19-autostream-settlement-visibility-capture.md` and
  `docs/launch/2026-06-19-autostream-settlement-clip-manifest.json`:
  `trace_submitted -> verification_verified -> real_bitcoin_moved ->
  settlement_recorded`, replay bundle
  `proof_replay_bundle.public_activity.73e66071`, and receipt
  `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker`.
- `verifierPolicy: exact_trace_replay`, `paymentMode: operator_approved_small_sats`,
  `spendCapSats: 100000` (from the run manifest in the run summary).

**Status: visibility-captured, broad claim still gated.** The full loop (claim →
run → independent replay → verified verdict → real-Bitcoin payout) is proven
end-to-end at least once in the open, with distinct worker and validator
identities and real rejections. Issue #5438 adds a public auto-stream
timeline/replay/manifest capture. The honest gap is now narrower: this is still
not enough for unqualified "anybody installs on any platform and is
automatically paid" copy. The capture preserves
`operator_approval.tassadar.autostream.worker` as owner/gate evidence and an R2
upload caveat, and the broad claim still needs scale methodology, Windows/WSL
coverage, and Spark-helper auto-start/readiness evidence. Backing promises:
`training.verification_classes.v1` (yellow),
`pylon.first_real_model_training_run.v1` (yellow),
`pylon.consumer_compute_earns_bitcoin_self_serve.v1` (**red**).

---

## 5. Install — "download Pylon"

> **Claim (238.md):** "they're going to download Pylon, our software that
> actually runs this." Front door: openagents.com/agents.md.

**Proof:**
- `https://openagents.com/AGENTS.md` — Pylon-first agent front door. It states:
  "The agent path is **Pylon**. Install the stable v1.0 headless node with
  `npx @openagentsinc/pylon`, then join the live Tassadar training run."
- npm: `@openagentsinc/pylon` dist-tags = `{ latest: '1.0.0', rc: '1.0.0-rc.33' }`.
  The default `npx @openagentsinc/pylon` now resolves the **stable `1.0.0`**
  contributor node (`https://registry.npmjs.org/@openagentsinc/pylon/-/pylon-1.0.0.tgz`),
  verified boots v1.0 — not the old `0.2.5` bootstrap stub.
- `https://openagents.com/INSTALL.md` — full install + test guide (also offers
  the signed binary via `updates.openagents.com` and Autopilot Desktop as the
  human cockpit).

**Status: settled-live.** This closes the launch-readiness audit's Gate #1: the
default `npx @openagentsinc/pylon` now hands a new contributor the stable v1.0
earning-capable node, matching the video's "download Pylon" CTA. Backing
promises: `agents.one_instruction_sheet.v1` (green),
`pylon.install_without_wallet_knowledge.v1` (green),
`pylon.v03_release_candidate.v1` (yellow — RC channel is separate).

---

## 6. Honest promise states (the "no overclaim" north star)

> **Claim (238.md):** "We're not going to be making any big claims that aren't
> sourced by evidence."

**Proof:** `https://openagents.com/api/public/product-promises`, source registry
target version `2026-06-19.1`. The relevant records and their honest states:

| promiseId | state | meaning for this launch |
|---|---|---|
| `promises.registry.v1` | green | the evidence-first registry itself is live |
| `training.decentralized_training_launch.v1` | green (bounded) | run live, 1,005 real sats to 2 contributors |
| `compute.tassadar_executor_poc.v1` | green | exact-trace executor PoC backed; no general capability claim |
| `agents.one_instruction_sheet.v1` | green | AGENTS.md front door |
| `pylon.install_without_wallet_knowledge.v1` | green | install path |
| `proof.claim_upgrade_receipts.v1` | yellow | receipt-first upgrade discipline |
| `training.verification_classes.v1` | yellow | replay verification model |
| `pylon.first_real_model_training_run.v1` | yellow | first real run, bounded |
| `pylon.v03_release_candidate.v1` | yellow | RC channel |
| `claims.world_first_ai_training_paid_bitcoin.v1` | **red (gated)** | world-first #1 — qualified wording only |
| `claims.world_first_public_llm_computer_training_run.v1` | **red (gated)** | world-first #2 — qualified wording only |
| `pylon.consumer_compute_earns_bitcoin_self_serve.v1` | **red (gated)** | default npm and one auto-stream visibility capture exist; broad "anybody plugs in → auto-paid" still gated by scale, Windows/WSL, and Spark-helper auto-start/readiness evidence |
| `marketplace.agentic_npm_module_registry.v1` | **planned** | the agentic-npm / module marketplace |

**Status: settled-live (honest).** The registry openly carries the two
world-first claims and the self-serve-earning claim as **RED**, and the
agentic-npm marketplace as **PLANNED**. Nothing that is gated is marked green.
This is the evidence-first promise honored: the strong claims are explicitly
**not** overclaimed on the public registry.

---

## 7. LLM-computer / learning-by-construction / flywheel concepts

> **Claim (238.md):** "What is LLM-computer? … out of a lab called Percepta …
> programs compiled into transformer weights … no gradient descent." Plus
> "learning by construction" (verified program → module → library grows →
> compose → more capability, "like an agentic npm") and "the flywheel"
> (verified work → better model → lower cost → more demand).

**Proof (concept provenance, internal essays):**
- Percepta credit (paradigm originator):
  `https://www.percepta.ai/blog/constructing-llm-computer` (resolves) and
  `https://github.com/Percepta-Core/transformer-vm` (resolves). The transcript
  explicitly credits Percepta; we did **not** invent the LLM-computer.
- OpenAgents essays in `docs/tassadar/`:
  - `docs/tassadar/2026-06-10-percepta-constructing-llm-computer-notes.md`
  - `docs/tassadar/2026-06-11-llm-computer-full-introduction.md`
  - `docs/tassadar/2026-06-10-tassadar-percepta-audit.md`
  - `docs/tassadar/work-that-proves-itself.md` (verification-by-replay thesis)
  - `docs/tassadar/2026-06-16-verified-work-payment-economics.md` (the
    money-loop economics)
  - `docs/tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md` and
    `docs/tassadar/2026-06-11-autopilot-agentic-labor-market.md` (the
    "agentic npm" / module-registry direction)

**Status: gated (concept / roadmap).** The LLM-computer *paradigm* is real and
Percepta-credited; the *live PoC* is exact-trace executor work
(`compute.tassadar_executor_poc.v1`, green) and must not be presented as general
LLM-computer capability, model-capability parity, or transformers-as-a-served-
product. "Learning by construction" and "the flywheel" are **strategy /
roadmap** narrative, not live capabilities: the agentic-npm module registry is
`marketplace.agentic_npm_module_registry.v1` (**planned** — the transcript's own
"upcoming video"), and the flywheel's demand leg has no external demand proof
(`proof.demand_provenance.v1`, planned). Present these as where we are headed,
not as shipped.

---

## 8. One-screen verification recipe (for a skeptic / agent)

```
# 1. Is the run live?
curl -s https://openagents.com/api/public/tassadar-run-summary | grep -o '"runState":"[^"]*"'
#   -> "runState":"active"

# 2. Real Bitcoin settled total + receipt count (reconciled, sim excluded)
curl -s https://openagents.com/api/public/tassadar-run-summary \
  | python3 -c 'import sys,json;d=json.load(sys.stdin);s=d["settlement"];print(s["settledPayoutSats"],"sats /",s["settledReceiptCount"],"receipts /",s["reconciledState"])'
#   -> 1005 sats / 2 receipts / settling

# 3. Per-settlement real-vs-sim breakdown
curl -s https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements \
  | python3 -c 'import sys,json;[print(r["amountSats"],r["movementMode"],r["realBitcoinMoved"]) for r in json.load(sys.stdin)["settlementRows"]]'

# 4. Direct verification challenge dereference
curl -s https://openagents.com/api/public/training/verification-challenges/training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c \
  | grep -o '"state":"[^"]*"'
#   -> "state":"Verified"

# 5. A real-Bitcoin receipt (5-sat self-serve)
curl -s https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1 | grep -o '"realBitcoinMoved":[a-z]*'

# 6. Proof replay bundle
curl -s 'https://openagents.com/api/public/proof-replays?ref=first-real-settlement' \
  | grep -o '"schemaVersion":"[^"]*"'
#   -> "schemaVersion":"proof_replay_bundle.v1"

# 7. Honest promise states (world-firsts are RED, not green)
curl -s https://openagents.com/api/public/product-promises | grep -o '"registryVersion":"[^"]*"'

# 8. Install the node
npx @openagentsinc/pylon   # latest = stable 1.0.0
```

---

## 9. Route normalization notes (curl-checked 2026-06-18)

- **Settlements feed public alias is the canonical agent URL.**
  `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements`
  resolves and returns `openagents.training_run_settlements.v1`. The legacy
  non-`/public/` route still resolves for compatibility, but launch evidence
  should cite the public alias.
- **Verification challenges are directly dereferenceable.**
  `https://openagents.com/api/public/training/verification-challenges/<ref>`
  resolves and returns public-safe challenge, run, window, verifier, state,
  digest/verdict refs, and staleness metadata. Raw traces, prompts, payment
  material, wallet material, and provider payloads stay out of the projection.
- **Simulation rows remain excluded from real Bitcoin totals.** The reconciled
  per-run real truth is `1,005` sats (settlement feed +
  `summary.settlement`). The settlement feed returns the historical 5-sat
  simulation row with `realBitcoinMoved:false`; the run summary and receipt-
  backed public aggregate code count only settled real-Bitcoin receipts for
  real movement totals.

---

## 10. Verdict — is the pack complete enough to back the video?

**Yes, for the honest-scoped video.** Every on-camera claim maps to a
dereferenceable public ref, with status marked:

- **settled-live:** run is live (§1); real Bitcoin settled to two independent
  contributors with per-receipt evidence and real-vs-sim distinguished (§2);
  install works via stable `npx @openagentsinc/pylon` v1.0 (§5); the registry
  honestly carries the strong claims as RED/PLANNED (§6).
- **visibility-captured / still gated for broad copy:** the money loop ran
  end-to-end with distinct worker/validator identities, real rejections, a real
  payout, and one auto-stream timeline/replay/manifest capture (§4). The broad
  "anybody installs on any platform and is automatically paid" promise remains
  red until scale, Windows/WSL, and Spark-helper auto-start/readiness evidence
  exists.
- **gated:** the two world-firsts (defensible only with full qualifiers,
  Percepta credited, held RED) (§3); LLM-computer general capability,
  learning-by-construction, and the flywheel (concept/roadmap) (§7).

The pack does **not** back: "anybody plugs in today on any platform and is
automatically paid at scale," bare unqualified world-first wording, "first
decentralized training run," "we invented the LLM-computer," or the agentic-npm
marketplace as live. Keep the on-camera copy to "the run is live, the loop is
proven end-to-end, and we've paid real Bitcoin to independent contributors —
come help us prove it at scale," with the world-firsts stated only in their full
qualified form.
