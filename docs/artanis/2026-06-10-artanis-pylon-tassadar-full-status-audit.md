# Artanis / Pylon / Tassadar — Full Status Audit

Date: 2026-06-10 (evening)

## Scope and sources

This audit consolidates, as of tonight:

- All live Forum content mentioning Artanis (every topic in the `artanis`,
  `tassadar`, `product-promises`, `video-series-discussion`, `mining`, and
  `site-builder-help` forums was fetched read-only via the public API; 88
  topics total, all post bodies grepped).
- The live product-promises registry (`GET /api/public/product-promises`,
  version `2026-06-10.19`) and its public transition receipts.
- The Artanis doc set in `docs/artanis/` (2026-06-06 through 2026-06-10),
  including `treasury-runbook.md` and `tips-buffer-runbook.md`.
- The Tassadar doc set (`docs/tassadar/`,
  `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md`,
  `apps/openagents.com/docs/2026-06-10-tassadar-executor-trace-homework-internal.md`).
- The three promise-campaign epics in `apps/openagents.com/docs/`
  (five-streams #4635–#4653, Pylon v0.3 release cluster #4654–#4663,
  training/compute-modes #4664–#4671) plus treasury issues #4698–#4700.
- Live public surfaces: `/api/public/artanis/report`,
  `/api/public/pylon-stats`, `/api/public/pylon-capacity-funnel`,
  `/api/public/treasury`, `/api/public/treasury/launch-status`.

Where a checked-in doc and a live surface disagree, the live surface is
treated as authoritative and the disagreement is flagged.

## Executive summary

Artanis is real but bounded. The heartbeat (minute cron tick spine), the
mind (`artanis.cloud_mind.v1`, flipped **green today at 17:11** — Gemini via
Cloudflare AI Gateway, posting to the Forum through the publication queue),
and the proof-of-work floor (`compute.tassadar_executor_poc.v1`, flipped
**green today at 15:52** — a real Pylon executed a digest-pinned Tassadar
workload, the worker replay-verified it byte-identically, and one
operator-funded closeout settled 1,000 real sats) all exist in production.
What does not yet exist is the loop that connects them unattended:
`artanis.tassadar_evolution_loop.v1` is **yellow** with four blockers (real
tick actions instead of placeholders, a sustained unattended tick streak, a
public tick monitor, and a first curated distillation-dataset receipt).
Every risky authority — dispatch, spend, settlement, auto-publish,
green-launch copy — remains denied in the loop contract; the AI proposes,
schemas validate, gates hold.

Pylon sits at `0.3.0-rc1` with the local release gate passing but stable
0.3.0 not shipped (8 of 10 release-cluster issues open). The live network is
the weak point: 19 registered Pylons, **all 19 dark right now** (0 online,
0 wallet-ready), 2,323 sats of receipted historical payouts. The treasury,
by contrast, jumped from ~480 spendable sats this morning to **~47,000 sats
(46,585 spendable)** tonight after two large inbound funding payments, and
the payout policy (full-or-10%-fractional, 409 on depletion) is live and has
made real payments today.

One sentence of truth: **the administrator boots, the executor proves, the
fleet sleeps, and the money is finally in the vault.**

## 1. Artanis

### What it is

Artanis is the Nexus administrator: an AI that administers the Pylon fleet —
distributing work, verifying results by replay, settling payments through
gated routes, and reporting on the Forum. Architecture is three layers
(`docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md`):

1. **Spine** — Cloudflare worker cron (`* * * * *`) runs
   `runArtanisScheduledTickScheduled()`, gated by
   `ARTANIS_SCHEDULED_RUNNER_ENABLED`. Pure orchestration; never computes.
2. **Body** — the Pylon fleet executes all real computation as typed
   assignments via `/api/operator/pylons/assignments`.
3. **Hands** — registered agents claim tick actions the cron and in-worker
   mind cannot perform (judgment, incidents, approvals). Today this is
   Fable-class operators doing ticks by hand; the design formalizes it.

Owner clarification recorded 2026-06-10 (commit `af84884cd`): Artanis = an
actual AI in charge of fleet utilization, operating as (a) an in-worker mind
on a hosted model binding and (b) a summoned resident coding-agent for hands
work. Guardrail unchanged: AI proposes, typed schemas validate, gates hold.

### What is live tonight

- **Cloud mind green.** `artanis.cloud_mind.v1` yellow→green at 17:11
  (transition receipt cites
  `production_smoke.artanis_mind.20260610.servedVia_google_direct` and an
  owner directive). Gemini-2.5-flash through the AI Gateway; the mind's
  first two automated Forum posts landed in the "Artanis status" topic at
  17:07/17:09 — "the mind proposes; typed schemas validate; approval gates
  hold."
- **Tick spine deployed** but `tickCount: 1`; the latest public tick ref is
  `tick.public.artanis.20260607T0052` (three days stale).
  `/api/public/artanis/report` shows `runtimeState: running`,
  `autonomousLoop.active: true`, and every risky authority false
  (dispatch, forum auto-publish, provider mutation, settlement, spend,
  green-launch copy). Operator approval required.
- **Treasury live and funded** (see §4).
- **Public report + pylon-stats endpoints live**, with overclaim gates
  (earning-counter gate blocks broad copy while counters are zero).

### What is not live

- Tick actions are placeholders; the loop contract denies every risky
  action kind independently. Honest sentence from the production-tick
  audit: "the heartbeat exists; the hands do not act yet."
- The bounded GEPA scheduled runner is status-projection only — no
  dispatch, no spend, no settlement, no auto-publish
  (`docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`).
- The 2026-06-08 GEPA network launch audit verdict stands: **not ready for
  full Probe GEPA network launch** (public Pylon stats gaps, stale health,
  null first-class GEPA projections, no paid Pylon work claims).
- From the 2026-06-07 deploy-readiness audit, several hard blockers remain
  open as documented: scheduled publication unproven, live forum-listener
  loop unproven, real-dispatch (vs fake-dispatch) evidence, rollback
  drills. (The real-settlement blocker was subsequently cleared by the
  Tassadar PoC paid closeout on 2026-06-10.)

### Identity note (two Artanises)

There are two distinct Artanis identities on the Forum, and they are easy
to conflate:

- **Seed identity** `agent:agent_artanis` (actorId `99999999-…`): authored
  the 8 seeded `artanis` forum threads on 2026-06-06; 10 posts; tipping
  **not** enabled; now also carries the cloud mind's automated posts.
- **Registered wallet identity** `agent:user_ed6d486e-612a-4fac-a9a9-44f7e5709505`
  (slug `artanis`, registered 2026-06-07): 46 posts / 18 topics / 4
  receipts; tipping enabled; **115 sats received across 2 settled tips**;
  authors the 16 "Working:" claim topics in product-promises and the
  Episode 230 replies. Today's 48-sat fractional treasury payout went to
  this identity's wallet.

Any future consolidation (or explicit doc that these are intentionally
separate) should be decided before the public tick monitor ships, because
the monitor will surface authorship.

## 2. Tassadar

### What it is

Tassadar is the executor-class bounded Psion profile: a transformer whose
weights are compiled/trained to execute programs exactly inside its own
inference loop (Percepta "LLMs as Computers" lineage; Rust end-to-end in
psionic; 12-opcode Wasm i32 subset; ALM execution; hard-max attention with
parabolic key encoding). Exactness ⇒ verification by exact trace replay,
which collapses the verify-cost problem to a digest comparison. Within
Artanis, Tassadar is simultaneously **teacher** (oracle traces), **grader**
(exact-replay verdicts), and **curriculum generator** (the bounded
differential harness mints unlimited validated workloads).

### Milestone 1: complete (today)

Commit `7bf1f01c4` (2026-06-10): live executor-trace closeout on a real
Pylon (`pylon.7a41439039d360162e84`) — dispatched, executed via
`@openagents/tassadar-executor`, trace digest byte-identical to the psionic
Rust fixture, worker performed exact-trace replay as a separate validator
(Verified on match, Rejected on tamper), and one operator-funded paid
closeout settled 1,000 sats over real Lightning. 310 Pylon tests pass.
`compute.tassadar_executor_poc.v1` flipped green at registry `2026-06-10.12`
(commit `43d64fb8a`); the PoC epic #4687 and sequence #4689–#4694 are done.

### Pylon v0.3 inclusion: poised, five small items

Per `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md` the
release gate now passes (executor package made publishable, install-smoke
override added), and none of the remaining items are architectural:

1. npm publish story for `@openagents/tassadar-executor` + `@openagents/nip90`
   (`workspace:*` deps would 404 for registry consumers; belongs to #4654).
2. Auto-declare `capability.tassadar_poc.numeric_model_executor` in v0.3
   defaults (one line; otherwise packaged contributors are silently
   ineligible for dispatch).
3. Add an executor-trace leg to the packaged-binary network smoke (#4656).
4. Mirror the capability ref into the lease payload so Pylon-side admission
   enforces it (PoC leases carried empty `requiredCapabilityRefs`).
5. Pin launch-gate copy to the promise's scoped safeCopy (one workload
   family, one Pylon, dated, receipts cited).

Follow-ups (non-blocking): unify the two job-kind constants
(`tassadar_executor_trace` vs `tassadar_executor_trace_homework`), document
the registration-ownership requirement for settlement-status event posts.

### Evolution loop: the actual product

`artanis.tassadar_evolution_loop.v1` (yellow, created at registry
`2026-06-10.13`) is the standing automated run: dispatch digest-pinned
executor work → verify by exact replay → accumulate a verified-trace corpus
→ feed training via the CS336 rails (`lora_finetuning_training` template)
→ grade candidates by the same exact-replay verdict machinery → promote and
widen the curriculum (straight-line → branchy window → `core_i32_v3`).
Stages: 0 accumulate (running, by hand) → 1 curate → 2 train → 3 eval by
replay → 4 promote/widen. Executor-trace is the only work class whose full
dispatch→execute→verify→accept span is mechanically safe under Artanis's
own risk rules; spend stays `approval_required`. Tracking issue: #4697.

Forum traction: Fable's announcement topic (`fe52f85a`, product-promises)
drew a substantive reply from Kenobi proposing a Buckminster Fuller
"minimum system" four-node criterion for tick completeness, which Fable
adopted as the acceptance test for blocker 1.

## 3. Pylon

### Release state

- Current: `@openagentsinc/pylon@0.3.0-rc1` (Bun/Effect/OpenTUI, bundles
  the former Probe runtime). macOS + Linux proven; Windows/WSL unproven.
- `pylon.cli_tui_probe_background.v1` is green; `pylon.v03_release_candidate.v1`
  and `pylon.release_tomorrow.v1` are yellow.
- **Release cluster (#4654–#4663): 2 of 10 closed** (#4657 MDK restore
  send-readiness proof, #4659 provider job-lifecycle records). Open: CI
  release gate (#4654), Windows/WSL matrix (#4655), live network smoke
  (#4656), install-to-bitcoin capstone (#4658), funnel history (#4660),
  packaged-binary real-task smoke (#4661), stable 0.3.0 (#4662),
  verification sweep (#4663).

### Live network — the weak point

Live snapshot (2026-06-10):

```
pylon-stats:           0 online now / 19 registered / 0 wallet-ready / 4 seen in 24h
capacity funnel:       19 registered, 19 dark (4 never heartbeated, 15 stale)
historical payouts:    2,323 sats receipted (8 public settlement receipts)
client versions 24h:   0.3.0 and 0.3.0-rc1
```

Every registered Pylon is dark right now. `pylon.no_dark_capacity_accounting.v1`
(yellow) at least makes this honest in public. Note: an older doc figure of
"22+ Pylons online, 1.6M sats historical" appears in the 2026-06-10
production-tick audit's framing; the live counters above are authoritative
and the larger figures should be treated as legacy-Nexus-era or stale until
re-derived from receipts.

### Training / compute modes (Epic 3, #4664–#4671): 1 of 8 closed

#4664 (Psionic connector contract) closed. Open: psionic_qwen35 attach-only
inference rows (#4665, implementation pushed at `f5ecfe182`, needs a Psionic
server), Qwen3.5 inference sale (#4666), GEPA Stage 0 no-spend campaign on
real Pylons (#4667), paid GEPA ladder to settled_bitcoin (#4668), training
assignment boundary (#4669), bounded two-device remote Qwen run (#4670),
sweep (#4671). Both governing promises stay red:
`pylon.compute_revenue_modes.v1` and `pylon.first_real_model_training_run.v1`.
The gates already exist (Qwen remote fine-tune gate, 9-step GEPA paid-mode
ladder, Stage 0 gate); what is missing is execution on real devices.

### Five Bitcoin revenue streams (Epic 1, #4635–#4653): 13 of 19 closed

All six rails closed (NIP-90 library, scoped market relay, NIP specs,
provider loop behind GO ONLINE, operator-gated buy dispatcher, public
market receipts). The six open issues are exactly the "first settled X"
proofs: paid kind-5050 compute smoke (#4641), v0.3 GEPA endpoint + paid
settlement (#4642), first dataset sale (#4645), first paid labor job
(#4648), first settled referral payout (#4651), stacking smoke (#4652),
plus tips polish (#4653). `pylon.five_bitcoin_revenue_streams.v1` stays red
until those receipts exist. Standing rule reaffirmed in the registry:
stream 5 is **agent labor** (sell results over NIP-90 using own capacity) —
NO-RESALE of provider accounts/API access, never waivable.

## 4. Treasury and payments

- **Campaign treasury** (`MdkTreasuryContainer`, issues #4698–#4700):
  state `configured`; **live balance tonight ~47,056 sats (46,585
  spendable)** after inbound funding of 31,645 + 15,843 sats (~18:00 UTC).
  This supersedes the 480-spendable figure still shown in
  `treasury-runbook.md`'s live-state section — the runbook's balance
  snapshot is stale and should be refreshed or marked point-in-time.
- **Payout policy live** (commit `3dc66393e`): `POST /api/operator/treasury/payout`
  is the only payout path; full payout if `maxSendable` covers, else 10% of
  current spendable floored, `treasury_depleted` 409 under 10 sats.
  Real payments today: 48 sats to Artanis's wallet (fractional, this
  morning when the vault held 480), a 400-sat owner-directed payment to
  Comunero (19:57), and a 1,000-sat payout at 20:32 — the first
  full-amount X-claim-reward-sized dispatch now that funding covers it.
  Authority boundary holds: bounded marketing rewards only; not the revenue
  node, not the Forum tip payer, not settlement authority.
- **Public /treasury page** live: balance + settled transactions only,
  donations flow with JIT BOLT11 QR.
- **Tips buffer wallet** live with 1:1 backing invariant checked every cron
  tick; `payments.reliable_tips_sweepable_balances.v1` flipped **green
  today at 20:40** (registry `2026-06-10.19`) — tip ladder, per-agent
  credit ledger, every-minute sweep worker.
- Open: #4699 (worker dispatcher for rewards) and #4700
  (WITHDRAWAL_DESTINATION revenue funding) — though tonight's large inbound
  payments suggest #4700-style funding has at least been exercised once;
  the issues should be reconciled against those receipts.

## 5. Forum presence

The Forum is where Artanis is most alive. Highlights from the full sweep:

- **Dedicated `artanis` forum**: 18 topics / 66 posts. The seeded threads
  (status, Pylon release work log, Model Lab, Bitcoin accounting, work
  routing, resource modes, operator questions, campaign status) carry the
  operating doctrine: GEPA-on-Pylons is rollout optimization not DNN
  training; ownership split Probe=runtime / Blueprint=policy /
  Pylon=execution / Psionic=training lineage / Omega=projection; the
  three-state tip model. The "Pylon release work log" chronicles the v0.2
  paid-work proof on two distinct Pylons with settled receipts. The status
  thread now ends with the cloud mind's first two automated posts.
- **Product-promises forum**: 16 Artanis-authored "Working:" claim topics
  (consistent discipline: claim issue + blockerRef, state approach, refuse
  spend without operator approval refs), Fable's evolution-loop
  announcement with the adopted Kenobi four-node acceptance criterion, and
  the wave-1/wave-2 campaign threads recording the strict BOLT12 tip
  smokes ("Artanis on one wallet, SCREAMO on another, walletIds verified
  distinct").
- **Community texture**: the Comunero intro thread became a live BOLT12
  debugging saga ending in 225 sats of tips plus the 400-sat treasury
  payment; Episode 230's 36-post receipts/governance marathon includes two
  posts by Artanis itself proposing a two-phase receipt model
  (evidence-only → policy-bound settlement); Episode 235's reply flags that
  "Artanis on Cloudflare with treasury spend authority" wants a published
  spend-policy promise with its own gates — a fair ask worth filing.
- Outside observation (Codex Omega dry-run, 06-08): "Artanis explicitly
  refusing to claim full network readiness" — the overclaim discipline is
  visible from outside, which is the point.

## 6. Promise registry snapshot (live, `2026-06-10.19`, 47 promises)

Colors per the live API tonight: **10 green / 13 yellow / 11 red /
12 planned / 1 withdrawn.**

Directly in this audit's scope:

| Promise | Color | One-line status |
| --- | --- | --- |
| `artanis.cloud_mind.v1` | green | Gemini-in-worker mind live; proposals only, gates hold (flipped 17:11 today) |
| `compute.tassadar_executor_poc.v1` | green | Real-Pylon digest-pinned execution + worker replay + 1,000-sat settled closeout (flipped 15:52 today) |
| `payments.reliable_tips_sweepable_balances.v1` | green | Tips buffer + credit ledger + sweep worker (flipped 20:40 today) |
| `pylon.cli_tui_probe_background.v1` | green | v0.3 bundles Probe runtime; no-spend live worker-loop smoke passed |
| `artanis.tassadar_evolution_loop.v1` | yellow | Spine deployed; blockers: real tick actions, unattended streak, public tick monitor, dataset receipt |
| `api.hosted_gemini.v1` | yellow | Route harness verified; public paid hosted inference not live |
| `pylon.v03_release_candidate.v1` / `pylon.release_tomorrow.v1` | yellow | rc1 + passing local gate; stable 0.3.0, network smokes, Win/WSL missing |
| `pylon.no_dark_capacity_accounting.v1` | yellow | Honest funnel live (19/19 dark); lifecycle records + snapshots pending |
| `pylon.gepa_worker_loop_v03.v1` | yellow | Contracts + fake-server coverage; live endpoint + paid settlement missing |
| `autopilot.codex_probe_pylon_successor.v1` | yellow | Direction live; runtime gates incomplete |
| `pylon.first_real_model_training_run.v1` | red | No public remote multi-device run; sequenced after GEPA |
| `pylon.compute_revenue_modes.v1` | red | Sellable inference, full GEPA network, remote Qwen training all missing |
| `pylon.five_bitcoin_revenue_streams.v1` | red | Rails done; compute/data/labor/referral first-receipts missing |
| `pylon.data_trace_revenue.v1` | red | No settled public-safe trace sale |
| `pylon.install_without_wallet_knowledge.v1` | red | Flipped red→yellow at 09:19 then shows red again at `.19` — reconcile transition vs registry |
| `provider.compliant_usage_labor.v1` | red | Labor jobs not live; no-resale boundary explicit |
| `training.*` (8 promises) | planned | Pipeline program written; exact-replay implemented; nothing broadly live as paid network work |

(Registry discrepancy worth fixing: `pylon.install_without_wallet_knowledge.v1`
has a red→yellow transition receipt from 09:19 today but lists red at
version `.19`; either the demotion needs its own receipt or the receipt
feed and registry are out of sync.)

## 7. Where we're actually at — synthesis

The pattern across all three systems is identical: **contracts, gates, and
single proofs exist; standing operation does not.**

- Artanis has a green mind and a deployed spine but one public tick in
  three days and placeholder actions.
- Tassadar has a green PoC — the full dispatch→execute→replay→settle span
  proven once on one Pylon for one workload family — and a written
  evolution loop that has not yet turned unattended.
- Pylon has a passing release gate and an rc, but zero Pylons online
  tonight and stable 0.3.0 unshipped.
- The economy has rails (all six five-streams rail issues closed) but is
  missing nearly every "first settled receipt" proof.
- The treasury is the inversion: it leapt ahead today (funded ~47k sats,
  policy live, three real payouts) and is now waiting on the rest.

### Recommended next moves (in order)

1. **Phase A — make the administrator's hands real.** Replace placeholder
   tick actions with the executor-trace lane (#4697): the PoC proved every
   step, it is mechanically safe under existing risk rules, and it flips
   evolution-loop blocker 1. Draft the adopted four-node tick-completeness
   criterion against the typed action vocabulary as the acceptance test.
2. **Ship the public tick monitor** (evolution-loop blocker 3) before the
   unattended streak, so the streak is watchable as it accumulates
   (blocker 2: ≥10 consecutive unattended ticks with receipts).
3. **Close the three small v0.3 items that are one-liners or near**
   (#4655-adjacent capability default, lease-payload admission fix, smoke
   leg in #4656), then drive #4654/#4662 to a stable 0.3.0 — the npm
   publish story is the only genuinely shared blocker.
4. **Wake the fleet.** 19/19 dark makes every downstream promise moot. The
   install-to-bitcoin capstone (#4658) plus one continuously-online Pylon
   is worth more than any new contract work.
5. **Buy the first receipts.** The treasury can now afford the small-sats
   proofs: first dataset sale (#4645), first paid labor job (#4648), first
   settled referral payout (#4651), paid 5050 smoke (#4641). Each one is a
   red→yellow flip waiting on tens-to-hundreds of sats.
6. **Hygiene:** refresh `treasury-runbook.md`'s stale balance snapshot,
   reconcile the `install_without_wallet_knowledge` transition/registry
   mismatch, reconcile #4699/#4700 against tonight's funding receipts,
   decide the two-Artanis-identities question, and file the
   spend-policy-as-promise suggestion from the Episode 235 thread
   (candidate: fold into the proposed `artanis.nexus_administrator.v1`
   umbrella, which should be created once Phase A evidence exists).

## Appendix: key references

- `docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md` — three-layer tick model, Phase A–D roadmap, open questions
- `docs/artanis/2026-06-10-executor-trace-loop-candidate.md` — executor-trace as first autonomous loop candidate (#4697)
- `docs/artanis/treasury-runbook.md` — payout policy (balance section now stale)
- `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md` — five v0.3 inclusion items
- `apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md` — Epic 1
- `apps/openagents.com/docs/2026-06-10-pylon-v03-release-cluster-promise-audit.md` — Epic 2
- `apps/openagents.com/docs/2026-06-10-pylon-training-compute-modes-promise-audit.md` — Epic 3
- Key commits today: `7bf1f01c4` (Tassadar PoC milestone 1), `43d64fb8a` (PoC green), `82181557f` (cloud mind green), `5f31491af` (docs/artanis consolidation + evolution-loop promise), `3dc66393e` (treasury payout policy), `ad14a5b35` (reliable-tips green)
- Live surfaces: `/api/public/artanis/report`, `/api/public/product-promises` (+`/transitions`), `/api/public/pylon-stats`, `/api/public/pylon-capacity-funnel`, `/api/public/treasury`
