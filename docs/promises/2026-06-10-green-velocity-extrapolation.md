# Green Velocity: 24-Hour Flip Rate And A Modeled Forecast

Date: 2026-06-10 (registry `2026-06-10.9`)

Provenance: **modeled**. The historical numbers below are measured from
git history and the public transitions feed; every forward-looking date is
an extrapolation, not a commitment. Per the registry's own discipline,
promises transition on receipts and operator action only — a forecast
line in this document moves nothing. If this document and the registry
disagree, the registry is right.

## 1. The measured 24 hours

The registry was born 2026-06-09 10:51 CT (`92229baf4`, version
`2026-06-09.2`) with 22 promises. As of 2026-06-10 10:22 CT (`6d170a856`,
`2026-06-10.9`) it holds 44: 6 green, 14 yellow, 11 red, 12 planned, 1
withdrawn. Every state event in that window, from the file's git history:

**Upgrades (the velocity that matters):**

| When (CT) | Promise | Transition |
|---|---|---|
| Jun 9 11:56 | `pylon.cli_tui_probe_background.v1` | yellow → **green** (no-spend live worker-loop smoke) |
| Jun 9 12:12 | `agents.cursor_forum_wallet.v1` | yellow → **green** (register → post → tip-readiness projection) |
| Jun 9 20:16 | `autopilot.mission_briefing.v1` | red → yellow (briefing route live, #4628) |
| Jun 9 20:29 | `pylon.no_dark_capacity_accounting.v1` | red → yellow (live capacity funnel, #4629) |
| Jun 10 04:19 | `pylon.install_without_wallet_knowledge.v1` | red → yellow **proposed**: transition receipt passed all checks; registry flip still pending maintainer action |

**Births, deaths, corrections (board shape, not velocity):** 4 promises
entered green at birth (already-live claims: homepage JSON, the registry
itself, the AGENTS sheet, the code map); 22 promises were added across the
day — 9 of them the training program at `.8`; 3 capacity-resale promises
were removed and replaced by `provider.compliant_usage_labor.v1` in the
no-resale sweep; 1 rename (`api.hosted_gemini.v1`); 1 claim correction
(five-streams labor stream).

**Blocker-level motion (sub-state velocity, from commits and the
transitions feed):** the orange-check smooth-purchase blocker cleared
after a clean live rerun (two live $5 purchases total); the funded
strict-smooth BOLT 12 tip smoke passed against two live recipients; the
first live no-spend Autopilot Coder run was recorded (#4633); the X-claim
reward ledger, capacity funnel, mission-briefing route, and the
transition-receipt machinery itself all shipped inside the window. The
transitions feed holds 8 receipts: 1 passed, 4 exceptions (owner-approved
backfills), 3 failed — the failed ones are the checks working as
designed.

**Measured rates, stated honestly:**

- **State upgrades: ~4–5 per day** (2 to green, 2 to yellow in-registry,
  1 passed-pending).
- **Blocker clears / evidence events: ~6–8 per day** (the leading
  indicator; states lag blockers).
- **Board growth: +22 promises per day** — claims are being written
  roughly 5× faster than they are being proven. Under claims-first
  discipline that is correct behavior, but it means *percent green* will
  fall while *count green* rises; the honest KPI is absolute upgrades
  plus blocker clears, not the green fraction.

## 2. Why naive extrapolation would lie

Three reasons a straight line through "2 greens per day" misleads, and
how the model below corrects for each:

1. **Survivorship of easy wins.** Both 24-hour greens were
   already-built-mostly claims needing one smoke. The remaining board is
   harder by construction. Correction: classify by gate type, not by
   count.
2. **Operator gating is lumpy.** The Wave-2 audit groups the actionable
   remainder into six classes, two of which (operator spend/authority,
   real devices) do not scale with agent velocity at all — they clear in
   batches when the operator sits down or hardware arrives. Correction:
   model those as discrete sessions, not rates.
3. **Some promises are not schedulable.** Anything requiring external
   dollars, market behavior, or research outcomes (demand-side economics,
   signature monetization revenue, R3+ ladder rungs) has no honest date.
   Correction: an explicit "not schedulable" bucket.

## 3. The model

Buckets, from the blocker classes (78 unique blocker refs at `.9`; the
Wave-2 operator audit's actionable remainder maps onto the same classes):

- **A — agent-buildable** (code, tests, projections, smokes that spend
  nothing): sustained ~2–3 promotions/day observed; assume 2/day with
  decay as the easy pool drains.
- **B — operator-gated** (funded smokes, dispatch approvals, npm publish,
  registry flips): assume ~2 operator sessions/week, each clearing 3–5
  blockers.
- **C — real-device-gated** (≥2 contributor devices, separate-device
  replay, Win/WSL): one provisioning event unlocks a batch; assumed
  within week 1–2.
- **D — external-repo (Psionic)** work: parallel lane, ~days per ask at
  the observed psionic cadence (17 issues landed there on Jun 10 alone).
- **E — external receipts / market / research**: not schedulable.

## 4. The forecast (modeled, not promised)

Green count today: **6**. Pending flip already receipted: install-wallet
to yellow.

**Week 1 — by ~Jun 17.** The smoke-distance cluster. Forum tipping
(webhook callback + refund smoke, #4653), `payments.money_dev_kit.v1`
(shares the webhook blocker), `agents.x_claim_reward.v1` (one
operator-dispatched 1000-sat reward), `identity.orange_check_forum_signal.v1`
(Nostr export, the single remaining blocker),
`autopilot.mission_briefing.v1` (drill-down + cost rollups),
`pylon.no_dark_capacity_accounting.v1` (retained funnel snapshots),
`pylon.v03_release_candidate.v1` (local gates + install smokes; the
universal-release sibling waits on Win/WSL, class C).
*Modeled green count: 12–15.* `pylon.install_without_wallet_knowledge.v1`
reaches yellow-stable; green needs the funded live install-to-bitcoin
smoke (class B).

**Week 2 — by ~Jun 24.** The first-receipt cluster.
`pylon.gepa_worker_loop_v03.v1` (live endpoint smoke + one paid GEPA
settlement), `compute.tassadar_executor_poc.v1` (dispatch →
separate-device replay → one paid closeout; class B+C, runs the moment
two real devices exist), first settled dataset sale moving
`pylon.data_trace_revenue.v1` red → yellow, compute-stream paid kind-5050
and referral-payout receipts moving their streams,
`pylon.five_bitcoin_revenue_streams.v1` red → yellow. Training rails
#4673–#4677 land: `training.verification_classes.v1` and
`training.full_pipeline_program.v1` planned → yellow;
`pylon.first_real_model_training_run.v1` red → yellow via the bounded
two-device run (#4670 shape).
*Modeled green count: 16–20.*

**Week 3 — by ~Jul 1.** The training program's first real evidence.
Ablation gate zero (reproduce a published eval score) →
`training.ablation_system.v1` yellow; first paid refinery shards →
`training.data_refinery_corpus.v1` yellow; first paid benchmark
assignments → `training.device_capability_dataset.v1` yellow→green
candidate; R1 ladder rehearsal complete → `training.model_ladder.v1`
yellow; `pylon.compute_revenue_modes.v1` red → yellow;
`proof.claim_upgrade_receipts.v1` green if the audit panel lands.
*Modeled green count: 20–24.*

**Weeks 4–6 — by mid/late July.** The R2 window.
`pylon.first_real_model_training_run.v1` **green** requires the full R2
evidence: paid verified pretraining windows on real contributor devices,
weak-device validators (#4676), public run pages, settlement receipts,
and the economics gate against a rented-cluster comparator. Modeled
mid-July *if and only if* class B and C gates keep clearing at week-1–2
rates. Alongside: `training.marathon_operations.v1` toward green via the
curtailment drill (which also feeds `energy.flexible_load_proof.v1`),
`training.post_training_arc.v1` yellow, `provider.compliant_usage_labor.v1`
yellow on a first compliant labor job,
`pylon.five_bitcoin_revenue_streams.v1` green only if all five streams
hold simultaneous receipts (the stacking smoke, #4652-class).
*Modeled green count: 25–30 of the current 44.*

**Not schedulable, and saying so is the point.**
`payments.accepted_outcome_economics.v1` (needs one fully-chained real
accepted outcome with margin evidence), `marketplace.signature_monetization.v1`
and `autopilot.control_center_fanout_marketplace.v1` (live marketplace
behavior), `workrooms.source_authorized_business_objects.v1` and
`mobile.voice_approval_companion.v1` (product roadmap),
`api.hosted_gemini.v1` (production executor binding + billing),
`energy.flexible_load_proof.v1` full green (measured operator report),
`autopilot.agentic_labor_products.v1` full green (external buyers), R3/R4
ladder rungs, and anything whose green requires **external dollars** —
which the demand-provenance rule (`proof.demand_provenance.v1`) exists to
keep us from back-door scheduling.

## 5. Falsifiers for this forecast

This document is itself a claim, so it carries its own checks. The
forecast is failing if, by **Jun 17**, green count is below 10 or the
operator-gated cluster (tips/MDK/x-claim) has not cleared as a batch; it
is failing structurally if, by **Jun 24**, no second real device has
entered the system (everything in weeks 2–6 slips behind that single
fact); and the R2 date is void if the psionic real-gradient scale-up
(multi-head, batching) has not landed by ~Jul 1. Conversely, if blocker
clears keep running at 6–8/day while these dates hold, the model is
conservative. Re-measure weekly against the transitions feed and the
registry's `lastVerifiedAt` fields — the rate, like everything else
here, should be a receipt, not a vibe.

## 6. Method note

Data: `git log`/`git show` over
`apps/openagents.com/workers/api/src/product-promises.ts` (36 commits,
2026-06-09 10:51 → 2026-06-10 10:22 CT), the public transitions feed
(`GET /api/public/product-promises/transitions`, 8 receipts), the live
capacity funnel and pylon-stats endpoints, and the Wave-2 operator audit
(`docs/2026-06-10-agent-work-audit-last-12-hours.md`). State counts
derived by parsing promiseId/state pairs per commit and diffing
consecutive versions. The sample is one day of a one-day-old registry;
treat every number above accordingly.
