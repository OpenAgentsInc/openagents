# Pylon V1.0 Launch-Readiness Audit — "The Tassadar Run is Live" (Episode 238)

Date: 2026-06-18
Scope: launch-readiness review for the public "anybody plugs in consumer
compute → gets paid Bitcoin" video in `docs/transcripts/238.md`.
Registry baseline: `/api/public/product-promises` version `2026-06-18.4`
(served live; matches source at
`apps/openagents.com/workers/api/src/product-promises.ts`).

This audit maps every claim in the 238 transcript to its product-promise
backing, gives a clear READY vs NOT-READY verdict for the video, defines V1.0
Pylon, and states the launch gate. It then verifies three concrete launch
gates with live evidence and reports the findings plainly.

> Honest-scope note: this document is public-safe and contains no secrets,
> wallet material, raw addresses, or private data. It links only to public
> projections and source refs.

---

## 1. Claim-by-claim mapping (238.md → promise backing)

State key: green = live for the scoped claim with current evidence; yellow =
partial / gated / caveated; red = blocked for affirmative copy; planned =
roadmap language only.

| # | Transcript claim (238.md) | Backing promise(s) | State | Verdict for on-camera use |
|---|---|---|---|---|
| 1 | "The Tassadar run is live … it's paying people for contributing compute" | `training.decentralized_training_launch.v1` | **green (bounded)** | OK only as scoped: launched, two distinct independent contributors paid real Bitcoin (1,000-sat canary + 5-sat self-serve), 1,005 sats real total. NOT network-scale, NOT "paying people" at scale. |
| 2 | "Share your compute and earn Bitcoin" / "Anybody can plug in a computer and get paid Bitcoin" (the core CTA) | NEW `pylon.consumer_compute_earns_bitcoin_self_serve.v1` (added this audit), `training.decentralized_training_launch.v1`, `pylon.install_without_wallet_knowledge.v1` | **RED/YELLOW** | NOT READY as an unqualified "anybody → gets paid" promise. The self-serve door exists (one contributor came through it) but the payout was operator-retro-settled, and the default `npx` install is the wrong package (gate #1). |
| 3 | World first: "first AI model training run paid in Bitcoin … to consumer compute" | NEW `claims.world_first_ai_training_paid_bitcoin.v1` (added this audit) | **RED (gated-pending-verification)** | An independent prior-art search exists (`world-firsts-verification.md`) and finds it defensible ONLY with full qualifiers ("Bitcoin + replay-verified training compute + own consumer devices"). NOT READY for bare on-camera use; still needs an evidence pack + owner-signed receipt-first upgrade. State only the qualified wording. |
| 4 | World first: "the first public LLM-computer training run" | NEW `claims.world_first_public_llm_computer_training_run.v1` (added this audit) | **RED (gated-pending-verification)** | Independent search exists and finds it defensible as "first public/open-contributor LLM-computer training run," crediting Percepta. NOT READY bare; needs evidence pack + owner-signed upgrade. Also: the live run is exact-trace executor PoC work, not gradient-descent model training (see `compute.tassadar_executor_poc.v1` and the transcript's own "no gradient descent" line). |
| 5 | "What is LLM-computer? … out of a lab called Percepta … programs compiled into transformer weights … no gradient descent" | `compute.tassadar_executor_poc.v1` (green, bounded), `models.tassadar_percepta_executor.v1` (red) | green (PoC) / red (model) | The PoC framing is backed (bounded exact-trace replay). Do NOT claim general LLM-computer capability, performance parity, or transformers-as-a-served-product — `compute.tassadar_executor_poc.v1` unsafeCopy forbids it. |
| 6 | Live Money Loop: go to openagents.com/agents.md → join run → download Pylon → claim → run → validator replays → verified → pay 5k to W+V | `agents.one_instruction_sheet.v1` (green), `pylon.v03_release_candidate.v1` (yellow), `pylon.first_real_model_training_run.v1` (yellow), `training.verification_classes.v1` (yellow), `training.decentralized_training_launch.v1` (green, bounded) | mixed | The loop mechanism is real and proven once end-to-end. The "download Pylon" step is the blocker: default `npx` install is broken (gate #1). The "5k sats each" rate is live per the run rails but the autonomous auto-pay-at-verdict is unproven (gate #2). |
| 7 | "Right now it's five Bitcoin sats each … prices could change" | `training.decentralized_training_launch.v1` | green (with caveat) | OK to state as a current run parameter that may change. The 5-sat self-serve settlement is real but was operator-retro-settled, not auto-paid. |
| 8 | Learning by Construction: verified program → module → library grows → compose modules → more capability ("like an agentic npm") | NEW `marketplace.agentic_npm_module_registry.v1` (added this audit, PLANNED), `marketplace.wasm_plugins.v1` (planned) | **PLANNED** | Roadmap language only. The transcript itself says "more on that in an upcoming video as we reboot our plugin marketplace." Do NOT present the agentic-npm registry as live. |
| 9 | The Flywheel: verified work → better model → lower cost → more demand | `proof.demand_provenance.v1` (planned), `compute.agentic_kernel_optimization_at_scale.v1` (red) | planned/red | Strategy narrative, not a live capability. No external demand proof exists; internal demand is plumbing proof, not market proof (`proof.demand_provenance.v1`). |
| 10 | "We're not going to be making any big claims that aren't sourced by evidence … evidence pack built into all this" | `promises.registry.v1` (green), `proof.claim_upgrade_receipts.v1` (yellow) | green | This is the honest north star and is backed. The world-first and self-serve-earning claims must follow it: no green without dereferenceable receipts. |

---

## 2. READY vs NOT-READY for the public launch

The video's headline promise is: **"anybody plugs in consumer compute → gets
paid Bitcoin."**

### NOT READY as an unqualified public promise.

Three independent blockers, any one of which is sufficient to hold the
unqualified claim:

1. **Install is broken at the default door (gate #1).** `npx @openagentsinc/pylon`
   resolves the `latest` dist-tag = `0.2.5`, a bootstrap stub
   ("Bootstrap the standalone OpenAgents Pylon release asset and run first-run
   smoke checks"), published 2026-06-08. The working contributor node is
   `1.0.0-rc.33` under the `rc` dist-tag (published 2026-06-18); the in-repo
   working build is `rc.37`, not yet published to npm at all. A fresh, naive
   `npx @openagentsinc/pylon` does NOT give a node that can join the run and
   earn. (Details in §4.1.)

2. **No fully-autonomous self-serve settlement has landed (gate #2).** Exactly
   two real Bitcoin settlements exist on the run (1,000-sat canary + 5-sat
   self-serve, 1,005 sats real total). The 5-sat self-serve payout was
   **operator-retro-settled** via the admin endpoint because the auto-stream
   skipped at verdict time. The first fully-autonomous settlement — gate firing
   at verdict, contributor auto-paid, no operator action — has NOT happened.
   (Details in §4.2.)

3. **The two world-firsts are unverified (gate #3).** Both world-first claims
   need an independent search + evidence verification before on-camera use.
   (Details in §4.3.)

### READY (the honest, scoped version that the registry already backs):

The following is true today and can be stated on camera with the existing
green/bounded evidence:

- The decentralized training run `run.tassadar.executor.20260615` is **live and
  active**.
- The end-to-end loop is **proven once in the open**: an independent contributor
  installs Pylon, claims a window, submits an executor trace; an independent
  validator on a separate machine/identity replays the pinned fixture; the
  challenge finalizes `Verified`; and **two distinct independent contributors
  have been paid real Bitcoin** (1,005 sats real total), native over Spark,
  each with `realBitcoinMoved:true`.
- This is **exactly two bounded canary-scale settlements**, not network-scale
  paid training. Copy must stay scoped to those two settlements.

The honest launch message is: *"The run is live, the loop is proven
end-to-end, and we've paid real Bitcoin to independent contributors — come help
us prove it at scale,"* NOT *"anybody can plug in today and automatically get
paid."*

---

## 3. V1.0 Pylon definition

**V1.0 Pylon is the contributor earning path, proven self-serve and
autonomous:**

```
install Pylon  →  join the Tassadar run  →  claim work  →  run the job
   →  independent validation (replay on a separate device/identity)
   →  auto-paid Bitcoin at verdict  →  dereferenceable evidence pack
```

The single bar for "V1.0 Pylon is done" is: **a brand-new independent
contributor can run one documented install command, join the run, do work, get
independently validated, and be automatically paid real Bitcoin — with a public
evidence pack and zero operator assistance at any step.**

### Explicitly IN V1.0
- One documented, working install command that yields an earning-capable node.
- Join `run.tassadar.executor.20260615`, claim a window, run the executor-trace
  job.
- Independent validator replay (exact-trace) producing a `Verified` challenge.
- **Auto-paid** real Bitcoin at verdict (auto-stream, no operator action).
- A public, dereferenceable evidence pack (run summary, settlements feed,
  verification challenge, settlement receipt).

### Explicitly OUT of V1.0 (later)
- The **five Bitcoin revenue streams** (`pylon.five_bitcoin_revenue_streams.v1`,
  planned; `pylon.v0_3_multi_earning_node.v1`, red). V1.0 is the single
  training-run earning path, not multi-stream stacking.
- The **module marketplace / agentic-npm** (`marketplace.agentic_npm_module_registry.v1`,
  planned; `marketplace.wasm_plugins.v1`, planned). This is the transcript's own
  "upcoming video."
- The **W-* headless coding workflow** / cloud coding sessions
  (`autopilot.cloud_coding_sessions.v1`, red; the coding-agent labor lane). V1.0
  is training-run compute, not coding-work-for-hire.

---

## 4. Launch gate — verified findings

These are the must-be-true items before the video ships. Each was verified
live; findings are stated plainly.

### 4.1 GATE #1 — What does `npx @openagentsinc/pylon` actually install? (BLOCKER)

**Finding: BLOCKED.** The default install gives the wrong package.

Verified via `npm view @openagentsinc/pylon dist-tags`:

```
{ latest: '0.2.5', rc: '1.0.0-rc.33' }
```

- `latest` = `0.2.5` (published 2026-06-08). Package description:
  *"Bootstrap the standalone OpenAgents Pylon release asset and run first-run
  smoke checks."* This is a bootstrap/smoke stub, **not** the working
  contributor node.
- `rc` = `1.0.0-rc.33` (published 2026-06-18). Description:
  *"Headless, CLI-only Pylon contributor node built on Bun and Effect."* This is
  the working node, but only reachable via `npx @openagentsinc/pylon@rc`.
- The in-repo working build is `apps/pylon/package.json` version
  `1.0.0-rc.37` — i.e. the repo is four RCs ahead of what is published to the
  `rc` dist-tag, and the working build is not yet on npm at all.

**Consequence:** anyone who runs the natural, undecorated
`npx @openagentsinc/pylon` (the most likely thing a new contributor or their
agent will type) gets `0.2.5`, which cannot join the run and earn. This
directly contradicts the video's "anybody can plug in a computer and get paid"
CTA.

Mitigating context: the documented front door is NOT `npx`. Live
`https://openagents.com/AGENTS.md` and `https://openagents.com/INSTALL.md`
route contributors to a **signed binary** from
`updates.openagents.com/pylon/rc/<platform>/feed.json` (and Autopilot Desktop
for normal testers). So the official path does not depend on the npm `latest`
tag. But the npm CLI RC is an advertised surface
(`pylon.v03_release_candidate.v1`), and the video's "download Pylon" framing
invites the naive `npx` attempt. Either the `latest` tag must point at a node
that can earn (or at least a stub that hard-stops and redirects to the signed
binary / `@rc`), or the launch copy must never imply `npx @openagentsinc/pylon`
is the install command.

**Must-be-true before ship:** either (a) publish the working build to a
dist-tag the default install resolves, or make `latest` redirect/instruct
clearly; and (b) ensure the one documented install command the video points to
actually yields an earning-capable node end to end.

### 4.2 GATE #2 — Has a fully-autonomous self-serve settlement landed? (BLOCKER)

**Finding: NOT MET.** No fully-autonomous (operator-free) settlement exists yet.

Verified via the live per-run settlements feed
`GET /api/public/training/runs/run.tassadar.executor.20260615/settlements` (3 rows):

| amountSats | realBitcoinMoved | movementMode | contributor | nature |
|---|---|---|---|---|
| 5 | false | simulation | pylon.448ba824… (Orrery) | simulation record (excluded from real total) |
| 1000 | true | real_bitcoin | pylon.448ba824… (Orrery) | owner-armed **canary** |
| 5 | true | real_bitcoin | pylon.81f0facfe… (Trigger) | self-serve, **operator-retro-settled** |

- Real settled total = **1,005 sats** to **two distinct independent
  contributors**.
- The 1,000-sat row is an owner-armed canary (operator-assisted by design).
- The 5-sat row came through the rc.32 self-serve public
  install→register→claim→submit→independent-validation path (the first
  independent contributor through the self-serve door) — but the payout was
  **operator-retro-settled** via the admin settlement endpoint because the
  auto-stream skipped at verdict time (a payout-target resolution bug, since
  fixed).
- Per the registry's own `2026-06-18.3` caveat and the
  `training.decentralized_training_launch.v1` record: *"the first
  fully-autonomous auto-stream settlement (gate firing at verdict with no
  operator action) has NOT happened yet."*

So the precise truth: a new independent contributor CAN install → claim →
verify, and HAS been paid — but the final payout step still required an
operator. The "install → … → auto-paid with NO operator assist" loop is
**unproven**.

Reconciliation note: the run-summary `settledPayoutSats` field and the
`/api/public/pylon-stats` 24h aggregate still read `1,010` (they have not yet
excluded the simulation row); the `/settlements` feed is the reconciled per-run
real truth (`1,005`).

**Must-be-true before ship:** at least one settlement where the auto-stream
fires at verdict and pays a fresh independent contributor with zero operator
action, captured in a dereferenceable receipt.

### 4.3 GATE #3 — The two world-firsts need verification + an owner-signed upgrade (BLOCKER for bare on-camera use)

**Finding: independent prior-art search now exists; still RED pending an
evidence pack + owner-signed receipt-first upgrade. Use full qualifiers only.**

The transcript asserts two world-firsts:
1. "first AI model training run paid in Bitcoin … to consumer compute"
2. "first public LLM-computer training run"

An independent web-research prior-art review landed on `origin/main`
concurrently with this audit:
`docs/launch/2026-06-18-world-firsts-verification.md` (commit `d354361ff`,
"verify the two world-first claims (L-3, #5395)"). It checks prior art (Spirit
of Satoshi, Bittensor/Templar, Gensyn, Prime Intellect, Nous/Psyche, Salad,
LightPhon, Percepta, Tracr) and concludes both claims hold **only with their
full qualifiers**: claim 1 is first as "Bitcoin + replay-verified training
compute + own consumer devices" together (token-paid networks and
data-bounty/inference precedents do not defeat the qualified phrasing); claim 2
is first as "public/open-contributor LLM-computer training run," crediting
Percepta as the paradigm originator.

That review satisfies the independent-search part of the gate. What remains
before either claim can go green (or be stated bare on camera) is a
dereferenceable evidence pack tying the qualified claim to the live run
receipts, plus an owner-signed receipt-first upgrade per
`proof.claim_upgrade_receipts.v1`. Per the project's own evidence-first
standard (the transcript's closing line and `promises.registry.v1`), the
world-first is a strong public claim and must stay scoped to the verified
qualified wording.

Additional accuracy caveat on claim #2: the live run is **exact-trace executor
PoC** work (`compute.tassadar_executor_poc.v1`), and the transcript itself says
the LLM-computer core has **"no gradient descent."** Calling it a "training run"
is defensible only in the broad executor-construction sense; it is not
gradient-descent model training. Copy must not conflate the two.

**Must-be-true before ship:** an independent search (prior art / competing
claims) plus an evidence pack for each world-first, recorded as the verification
gate on the two new RED promises added this audit. Until then, drop or
heavily qualify the world-first language on camera.

---

## 5. Launch gate summary (the must-be-true list)

Before the "anybody plugs in → gets paid Bitcoin" video ships:

- [ ] **Gate #1 — install:** one documented install command yields an
      earning-capable node; the default `npx @openagentsinc/pylon` does not
      silently hand a new contributor the `0.2.5` bootstrap stub.
- [ ] **Gate #2 — autonomous settlement:** at least one fully-autonomous
      auto-stream settlement (verdict → auto-paid, no operator) to a fresh
      independent contributor, with a dereferenceable receipt.
- [ ] **Gate #3 — world-firsts:** independent-search verification now exists
      (`docs/launch/2026-06-18-world-firsts-verification.md`); still need an
      evidence pack + owner-signed receipt-first upgrade, or the claims are
      stated only with their full verified qualifiers (claim 1: "Bitcoin +
      replay-verified training compute + own consumer devices"; claim 2:
      "public/open-contributor LLM-computer training run," crediting Percepta).
- [ ] **Copy discipline:** the video stays scoped to "run is live, loop proven,
      two contributors paid real Bitcoin" and does NOT extrapolate to
      network-scale earning, the five revenue streams, the agentic-npm
      marketplace, or general LLM-computer capability.
- [ ] **Registry alignment:** the new world-first, self-serve-earning, and
      agentic-npm promises are served live at `/api/public/product-promises`
      with honest states (this audit adds them at registry version
      `2026-06-18.5`; a Worker redeploy is required to publish the bump).

When Gate #1 and Gate #2 close and Gate #3 verifies, V1.0 Pylon (as defined in
§3) is launch-ready and the new RED/YELLOW promises can be upgraded
receipt-first per `proof.claim_upgrade_receipts.v1`.

---

## 6. Promise registry changes made by this audit

Added to `apps/openagents.com/workers/api/src/product-promises.ts` with honest
states (no green; no flips of existing promises):

- `claims.world_first_ai_training_paid_bitcoin.v1` — **RED**
  (gated-pending-verification): the independent prior-art search
  (`docs/launch/2026-06-18-world-firsts-verification.md`) is cited; still gated
  on an evidence pack + owner-signed receipt-first upgrade, with copy scoped to
  the verified qualified wording.
- `claims.world_first_public_llm_computer_training_run.v1` — **RED**
  (gated-pending-verification): same independent search cited (crediting
  Percepta); still gated on an evidence pack + owner-signed upgrade, plus the
  "no gradient descent" / executor-PoC accuracy boundary.
- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` — **RED**: the video's
  core promise, gated on a proven fully-autonomous self-serve settlement
  (gate #2) AND a working default install (gate #1).
- `marketplace.agentic_npm_module_registry.v1` — **PLANNED**: the transcript's
  "upcoming video" agentic-npm / module-marketplace reboot.

Registry version bumped `2026-06-18.4` → `2026-06-18.5`; the product-promises
test (`product-promises.test.ts`) is aligned to the new version and asserts the
new records' honest states.
