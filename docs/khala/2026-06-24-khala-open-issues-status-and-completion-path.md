# Khala — open-issue status & completion path (2026-06-24)

> Internal status audit. **Not a product promise or public-claim copy.** Khala is
> OpenAgents' **orchestrating-model brand** — one OpenAI-compatible inference endpoint
> (`POST https://openagents.com/api/v1/chat/completions`, virtual models
> `openagents/khala-mini` · `khala-pro` · `khala-code`) that presents as a single model
> but is an **agent network underneath**, routing across a pool of models, tools,
> validators, and (eventually) Pylon edge-compute workers — with work **verified, not
> trusted**, settled **natively in Bitcoin**, and watchable in the Verse. (The older
> `khala-*` websocket *sync/replay engine* is deprecated/superseded; ignore those labels.)

## Scope: what's actually open

As of 2026-06-24 the **only** open issues in the repo are **4 Khala-program issues**.
The shareable-agent-trace program (EPIC #6206) is fully shipped + closed and is **not**
part of this audit.

| # | Issue | Kind | One-line state |
|---|---|---|---|
| #6017 | EPIC: Khala buildout to the head-to-head | umbrella tracker | M0–M6 landed (mostly inert/owner-gated); **M7 + M8 are the real remaining build** |
| #6016 | M8: the head-to-head demo (north-star) | milestone | scaffolds + self-gating audit done; blocked on a **live** verified+paid+watchable run |
| #6015 | M7: Conductor lane | milestone | readiness preflight landed; needs a **real trained NL-planner composition** (long pole) |
| #6049 | EPIC: Khala on Machine Payments (MPP) + Stripe Directory | umbrella | discovery live; crypto+card **armed on prod**; Lightning rail + directory listing + e2e paid fetch remain |

Two are umbrella epics (#6017, #6049); two are concrete milestones (#6015 M7, #6016 M8).
**#6015/#6016 are children of #6017** — closing them (plus the live gates on M0–M6) closes #6017.

---

## #6017 — EPIC: Khala buildout to the head-to-head

The umbrella. North-star: publish our own verified, Bitcoin-settled, in-Verse,
playable-artifact version of the public *"build a high-quality single-file three.js
crossy-road game"* head-to-head. Source of record:
`docs/inference/khala-buildout-roadmap.md`.

### Milestone state (M0–M8)

| M | Issue | State | What remains |
|---|---|---|---|
| M0 serve metered + receipt | #6008 | code landed (priced catalog alias + `openagents` disclosure block) | live SDK proof is **owner-gated** (enable a funded account) |
| M1 Autopilot → Khala | #6009 | landed (call path + receipt projection + RPC + smoke, PR #6021) | live proof keyed on M0 enable; optional chat-pane button binding |
| M2 verified coding outcomes | #6010 | **CLOSED** — `khala-code` + crossy-road rubric + Playwright headless verifier; on prod returns `verified:true`/`reward:1` in ~106s | — (the verification harness exists + works) |
| M3 Bitcoin settlement | #6011 | landed **dormant** (Spark verified-work settlement leg reusing the Tassadar gate, PR #6023) | **arming is NEEDS-OWNER** (funded payout to the guinea-pig Pylon) |
| M4 Pylon workers in pool | #6012 | landed **inert** (parity-gated fabric supply adapter, PR #6022; no transport bound → lane skipped) | activate (one line) once a **Psionic serving transport** exists |
| M5 Verse serving view | #6013 | in progress (world-contract gateway projection + desktop Verse projection landed) | the new render primitives (crackling energy → Pylons, settlement beams) + live events |
| M6 learned coordinator (TRINITY) | #6014 | **CLOSED** — P1 `forward_with_hidden` + P2 `CoordinatorHead` (psionic#1133) + the M6 buy-mode shadow run landed | (was the long pole; shadow A/B is done) |
| M7 Conductor lane | #6015 | **OPEN** — see below | real trained NL-planner composition |
| M8 head-to-head demo | #6016 | **OPEN** — see below | a live verified+paid+watchable manifest |

### Honest read

The request surface, auth, balance gate, adapter registry, metering/receipts,
settlement leg, Pylon adapter, and Verse projection **already exist** — most of M0–M6 is
wiring that is deliberately **inert or owner-gated**, not missing. The genuinely
unbuilt capability is the **learned-composition story (M7)** and its **live north-star
proof (M8)**. Everything else on #6017 is a *flip-the-switch* (owner arms funding/payout)
or a *render-primitive* task, not new architecture.

### To close #6017

Close #6015 (M7) and #6016 (M8) **and** convert the inert/owner-gated M0/M3/M4/M5 legs
to live evidence:
1. Owner enables a funded account → M0/M1 live SDK proof.
2. Owner arms M3 settlement → one real Bitcoin/Spark payout to a Pylon worker, public receipt.
3. A Psionic serving transport exists → flip M4's adapter on (one line) → ≥1 Pylon serves in the pool.
4. Land M5's remaining render primitives + live serve events.
5. M7 + M8 done (below).

---

## #6015 — M7: Conductor lane (compose to win the benchmark)

**Goal.** A natural-language planning coordinator that decomposes the crossy-road task
across the pool (plan → implement → verify → refine) — the composition that lets
`openagents/khala` match frontier quality at lower cost.

### State
- **Landed (read-only):** the M7 *readiness preflight projection*
  `apps/pylon/src/khala-m7-conductor-preflight.ts` (schema
  `openagents.khala.m7.conductor_preflight.v0.1`, commit `ef320626ff`). It mirrors the
  Psionic `ConductorReadiness` close gates and **separates `canStartConductorTraining`
  from `canPublishM7Claim`** — i.e. an owner-capped training run can be *start-ready*
  before the public M7 claim can go green.
- **Substrate:** M6 (P1 hidden-state + P2 `CoordinatorHead`) is landed; the M6 shadow
  A/B run is done. P3–P5 (sep-CMA-ES, reward adapter, pool binding) were honest stubs
  as of the last status — confirm their current state in Psionic before training.

### What's necessary to complete M7 (the long pole)
1. **Train the Conductor NL planner (GRPO).** Emit subtasks + worker ids + access-list
   topology; adopt the TMAX stability recipe (**DPPO + FP32 LM head**, filter zero-std
   samples). Per `docs/sakana/conductor.md` + `docs/research/tmax/synthesis.md` §5.
   → **GPU compute + owner-armed training run** (the gating cost).
2. **Compose the benchmark:** plan (frontier via gateway) → implement (best coding
   worker) → verify (M2 rubric/replay) → refine, and **win** — solve crossy-road by
   composition at comparable quality, lower cost than single-model.
3. **Pool:** add open Pylons and (where applicable) verified Tassadar modules; expose
   `openagents/khala`.
4. **Verse:** the multi-worker fan-out view (compose-across-the-map).
5. **Publication evidence:** a publishable **M7 Conductor preflight** ref
   (`preflight.khala.m7.conductor.publishable.v0_1`) — M8 already hard-requires this.

**Done when** `openagents/khala` solves crossy-road by composition, beating single-model
cost at comparable quality, verified by the M2 rubric. **Blockers:** GPU compute +
owner-armed training; Psionic P3–P5 finished; not closeable as a docs/code-only slice.

---

## #6016 — M8: the head-to-head demo (north-star)

**Goal.** Publish our own verified, Bitcoin-settled, watchable head-to-head with the
built game playable in our own three-effect world.

### State — scaffolds done, self-gating, honest
The whole *measurement + closure* harness is built and merged:
- Evidence schema + runbook (`docs/inference/khala-head-to-head-demo.md`).
- Metric reducer + closure audit (`scripts/khala-demo/reduce-head-to-head.mjs`) and
  publication renderer (`render-publication.mjs`), with focused tests.
- A `livePromotionAudit` that derives `closureAudit.canClose` from real checks; it
  **stays `canClose:false`** until a live manifest satisfies every check (including a
  publishable **M7 conductor preflight** ref, added in `dd5cdfdeb1`).
- Unblocked already: `khala-code` streams a full crossy-road game on prod in ~106s and
  returns `verified:true`/`reward:1` — so a **real verified head-to-head outcome can be
  driven end-to-end** by the M8 runner (#6031).

### Remaining blockers (the reducer's own list)
`fixture_scaffold_not_live` · `live_khala_run_missing` · `m7_live_conductor_missing` ·
`settlement_receipts_missing` · `artifact_not_playable_in_world` · `energy_telemetry_missing` ·
`frontier_baseline_not_live` · `publication_missing` (+ the M7 preflight ref).

### What's necessary to complete M8
1. Run **`openagents/khala` vs a live frontier baseline** on the prompt, side by side
   (live Khala run + live frontier run — not fixtures).
2. The run must carry an **accepted-outcome verdict** (M2 rubric) and a **settlement
   receipt** (M3 armed) — verified + paid.
3. The built game must be **playable inside the three-effect Verse**, with **Verse
   playback refs** and **energy telemetry**.
4. Cite the **M7 conductor preflight** publishable ref.
5. Produce **#6016-D** final publication (replace scaffold prose with live refs, raw
   inputs, accepted-outcome verdict, payment/settlement refs, playback refs, honest-loss
   notes) → reducer returns `canClose:true` → **#6016-E** live-promotion gate passes with
   owner sign-off (world-first / AO-per-kWh / product-promise upgrades stay blocked on
   DE-10 evidence).

**Done when** the reducer's closure audit returns `canClose:true` for a live manifest.
**Depends on** M3 (settlement armed), M5 (Verse playback), M7 (live conductor) — and M2
(already done). So **M8 is gated on M7 + the owner-armed settlement + the Verse playback
primitives**, not on new measurement code.

---

## #6049 — EPIC: Khala on Machine Payments (MPP) + Stripe Directory

**Decisions (owner):** accept MPP; **USDC + card settle into the Stripe balance**;
**Bitcoin/Spark stays the contributor-payout rail**; do both discovery **and** payments.
Plan: `docs/stripe/2026-06-22-khala-mpp-integration-plan.md`.

### Phase state
| Phase | State |
|---|---|
| **0 — Stripe profile** (owner/dashboard) | ✅ **approved** on the live account ("OpenAgents, Inc."); the directory `profile_…` id is wired into the Worker as a public `var` (`STRIPE_MPP_NETWORK_PROFILE_ID`, PR #6131) |
| **1 — discovery surfaces** | ✅ **live + crawler-readable**: `/llms.txt`, `/agents.md`, `/ai.md`, `/skill.md` return proper `text/plain`/`text/markdown` (PR #6051/#6060); `/v1/models` lists the virtual models; `/openapi.json` service-discovery doc with `x-payment-info` offers served (PR #6139) |
| **2 — paid MPP endpoint** | 🟡 **crypto (Tempo/Base/Solana USDC) + card/SPT ARMED on prod**: `POST /mpp/v1/chat/completions` → `402` + deposit address (~2.6s); Worker-native Payment-Auth verify (`draft-httpauth-payment-00`, PR #6138); full crypto pay-loop **proven on staging** (402 → `simulate_crypto_deposit` → 200 + `Payment-Receipt` + `mpp:<pi>` credit, PR #6141). **Lightning rail still not surfacing — see below.** |
| **3 — unify with the loop** | partially: settled USDC/card mint USD-origin credits; the Lightning rail mints **Bitcoin-origin** `balance_msat` credits (RL-3) |
| **4 — other primitives** (fine-tune, sandboxes…) | not started (deliberately) |

### Remaining blockers to close #6049
**Done-when:** `stripe directory search "llm inference api" --mpp-supported` shows **Khala**
(first inference result) **and** `mppx fetch <endpoint>` pays + returns a real completion.

1. **Stripe Directory listing visibility.** Profile approved + wired, `/openapi.json`
   advertises offers, but the **directory crawl/badge is pending** — confirm Khala
   actually appears in directory search (owner/Stripe-side crawl timing).
2. **Lightning rail (Spark-primary) is not live.** It has been hardened repeatedly
   (per-rail isolation + bounded mint so it can never hang the 402 — PR #6149; Spark as
   primary issuer via the `MDK_TREASURY` container with MDK fallback — PR #6152; raised
   mint budgets — #6153/#6157) but prod `wrangler tail` shows the 402 **still leads with
   `[base, stripe]`, never Lightning**: the Spark `/spark/funding-invoice` container
   subrequest returns 200 (mint succeeds ~3.7–5.7s) yet the leg drops as
   `provider_unavailable`, and the paired MDK fallback subrequest comes back
   `canceled` (one hit a `/api/mdk/api/mdk` 404 double-path). Latest hardening
   `9f8089e218` (Spark payload parsing accepts raw SDK shapes; MDK route normalized to
   `/api/mdk`). **Still needs:** confirm the Spark mint result actually parses to a
   `paymentHash` in the Worker (so it stops fail-closing into the canceled fallback),
   and/or a **pre-minted Spark invoice pool** to remove the cold-container latency from
   the 402 path entirely.
3. **End-to-end paid fetch.** No `mppx` client is installed; the done-condition needs a
   real `mppx fetch` (or equivalent paid client) that pays a live offer and gets a
   completion back. The crypto loop is proven on **staging**; a **prod** paid completion
   (ideally over Lightning, owner's Bitcoin-first preference) is the final proof.

Note: **MPP is optional for the non-MPP launch path** — the endpoint is fail-safe inert
(`503 mpp_not_configured`) when unarmed, so #6049 does not block the broader launch.

---

## Cross-cutting: how each blocker is gated

| Blocker | Gate type | Unblock |
|---|---|---|
| M0/M1 live SDK proof | **owner** | enable a funded account |
| M3 settlement arming | **owner** | arm funded payout to the guinea-pig Pylon (NEEDS_OWNER) |
| M4 Pylon-in-pool activation | **dependency** | a Psionic serving transport exists → flip the inert adapter |
| M5 render primitives + live events | **codeable** | build crackling-energy/settlement-beam primitives + wire live serve events |
| **M7 Conductor training** | **compute + owner** | GPU + owner-armed GRPO run; finish Psionic P3–P5 |
| M8 live manifest | **dependency** | needs M3 armed + M5 playback + M7 live + a real frontier run, then publish |
| MPP directory listing | **owner / Stripe** | confirm the directory crawl lights the badge |
| MPP Lightning rail | **codeable** | fix the Spark `paymentHash` parse / MDK fallback cancel; consider a pre-minted invoice pool |
| MPP e2e paid fetch | **codeable + owner** | install/run a paid client against armed prod (Bitcoin-first) |

## The shortest path to zero open issues

1. **MPP (#6049)** is closest to done and the most **codeable**: fix the Lightning rail
   (Spark `paymentHash` parse + remove cold-container latency), confirm the Stripe
   Directory listing, and run one real prod paid fetch. No new ML/compute.
2. **M7 (#6015)** is the genuine **long pole**: it needs GPU compute + an owner-armed
   GRPO Conductor training run (and Psionic P3–P5). Nothing closes it without real
   training + a verified composition win.
3. **M8 (#6016)** then becomes a **driving** task once M7 is live + M3 is armed + M5
   playback exists: run the live head-to-head, publish, flip the reducer to
   `canClose:true`.
4. **#6017** closes when M7 + M8 close and the owner-gated M0/M3 legs have live evidence.

**Net:** one issue (#6049) is mostly an engineering finish + an owner/Stripe confirmation;
the other three collapse onto a single dependency — **an owner-armed, GPU-backed Conductor
(M7) training run that wins the benchmark by composition** — after which the north-star
demo (M8) and the epic (#6017) follow.
