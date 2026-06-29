# Focus-Sweep Status Audit: Six Open Issues, Infrastructure, and the Night's Ledger

Date: 2026-06-11 (~10:00 UTC)
Scope: full status after the blocker attack-order sweep
(`docs/2026-06-10-all-open-issues-blocker-attack-order.md`) and the owner's
two mid-sweep refocus directives (no inference / no Qwen; then: GEPA to
planned-wontfix, implementation focus = Tassadar/Psion + CS336 pipeline
prep). All issue comment threads reviewed; all live surfaces and delegated
infrastructure checked at audit time.

## Where the board stands

The repo went from **32 open issues** at the attack-order audit to **6 open
now** — 26 closed in one overnight session, every closure with evidence or
an explicit owner-directive rationale on the thread, plus 4 new issues
arriving mid-session (3 of them the new Tassadar research workstreams, all
still open; 1 closed in-flight). The six that remain are exactly the focus
lane: one live capstone in progress on delegated infrastructure (#4678),
three freshly-filed Tassadar research workstreams (#4748–#4750), and two
verification sweeps (#4663, #4671) that are now actually runnable because
their prerequisites stopped being open questions.

Registry: **`2026-06-11.7`**, 53 promises — **12 green / 20 yellow / 6 red /
14 planned / 1 withdrawn**. The red column was cut from 10-11 to 6 tonight,
not by overclaiming but by a real red→yellow flip
(`pylon.install_without_wallet_knowledge.v1`, live smoke), two
planned→yellow flips (`training.verification_classes.v1`,
`training.device_capability_dataset.v1`, both with passing receipt-first
transitions), one yellow→green (`forum.content_tipping.v1`, passing
transition), and four owner-directed demotions to planned (GEPA worker
loop, compute revenue modes, five streams, data trace revenue — all with
passing transitions citing `directive.owner.20260611.focus_tassadar_psion_cs336`).

## The six open issues, in detail

### #4678 — training: A1 leaderboard-class run, real gradients across contributor devices

**The capstone. In progress RIGHT NOW on the SHC box by a parallel lane —
do not double-dispatch.**

Comment-thread state (10 comments reviewed):
- The monorepo/public-projection side landed on 2026-06-10 (commit
  `5f776fb6c`): `summary.realGradient` on public run summaries, the
  `/api/training/leaderboards/a1` route, run-page UI status, OpenAPI/AGENTS
  rows, and copy gates that structurally require two devices, Freivalds
  commitments, merge/eval refs, and loss-under-budget evidence before any
  observed real-gradient status can publish.
- **The Psionic external ask landed** (`OpenAgentsInc/psionic#1114`, lane
  `psion_cs336_a1_real_gradient_reference_v1`): hand-derived analytic
  backprop for the full A1 architecture (embedding → RMSNorm → causal
  attention → RMSNorm → SwiGLU → unembedding → CE), with every parameter
  tensor's analytic gradient matching central differences at 1e-5 relative
  tolerance. The "finite-difference-only" blocker that the issue flags is
  resolved upstream.
- What remains is exactly the Lane B live leg: the run on **≥2 real
  contributor devices** with paid settled closeouts.

Delegated-infrastructure evidence found at audit time on `oa-shc-katy-01`
(23.182.128.195): staging directories `/home/ubuntu/oa-pylon-4678`,
`/home/ubuntu/oa-cs336-a1`, `/home/ubuntu/oa-bench-runs`, and a **running
wallet daemon** under `/home/ubuntu/oa-wallet-4678` plus an in-flight
`npm exec @moneydevkit/agent-wallet@latest`. This session's own #4678
delegation was withdrawn before launch when this parallel activity became
apparent; the SHC machine gives the run its second physical device and a
second device class (x86_64 Linux vs the local apple-silicon Pylons).

Promise linkage: this run is the rehearsal evidence toward
`blocker.product_promises.remote_multi_device_training_missing` on
`pylon.first_real_model_training_run.v1` (red; now its *only* blocker after
the Qwen rescope). Note the issue body's formal-clear pointer to #4670 is
stale — #4670 was closed not-planned under the no-Qwen directive, so this
run *is* the clear path now; the transition should say so.

**Next step:** let the parallel lane finish; if it stalls, the remaining
work is mechanical against tonight's playbook (shard assignments to the SHC
Pylon + a local Pylon, cross-device recompute verification, paid closeouts,
a1_loss leaderboard admission, receipt-first red→yellow transition,
registry `.8`).

### #4748 — W2: verified trace factory, day-0 contract freeze (filed 05:22 UTC, no comments)

First of the three new Tassadar research workstreams from
`docs/tassadar/RESEARCH_PLAN.md`. Freezes the corpus-machine contracts
BEFORE scale: `trace_record` artifact schema v0.1 (profile_version,
program_hash, input_seed, compiler/executor hashes, compact uint16/uint32
trace tokens, step offsets, digests, validator receipts — binary hot path,
~2 GB per 1B tokens vs tens of GB as JSON), validator verdict schema v0.1
with the four-tier ladder (schema/hash → full replay for new
workers/profiles → reputation spot-checks → random adversarial replay),
quarantine-before-admission, the iron rule (never train from unverified
artifacts; expected digests never ship in generation assignments), and the
training split policy. Builds on rails that all have receipts as of
tonight: the assignment route, the worker-as-validator verdict flow
(`exact_trace_replay` from #4674 plus the four more classes exercised
tonight), and the registry. Clearing target: the evolution-loop promise's
fourth blocker ("no distillation dataset curated").

**Next step:** pure contract/spec + schema/test work — no fleet, no spend,
no owner action. Highest-leverage of the three because W3 is explicitly
blocked on W2's first 100M verified tokens.

### #4749 — W3: the student program (filed 05:22 UTC, no comments)

The Psion side: a four-baseline sweep on verified traces — (a) next-token
distillation, (b) + auxiliary state losses, (c) 2D-head/hard-max-regularized
variant with analytic parabolic init and max-margin lookup loss, (d) frozen
analytic executor + learned interface — evaluated by **first divergence
behind replay, never perplexity**. Explicitly **blocked on W2**; filed now
so the experiment design is frozen before anyone improvises it. Naming rule
binding: students carry Psion's claim vocabulary (bounded statistics),
never Tassadar's (proofs).

**Next step:** nothing executable until W2 produces verified tokens; the
design-freeze content itself lives in the issue + RESEARCH_PLAN.md. Park
behind #4748.

### #4750 — W4.1: TassadarCapabilityEnvelope consumer in Pylon (filed 05:22 UTC, no comments)

Pylon advertises executor-class capacity with the GEPA-style no-overclaim
posture: profile (window version, legs, replay class) declared only when
backed by a **self-test receipt**, refusal outside it, assignment-route
filtering on the declaration. Why now: the evolution loop needs
capability-driven eligible-device discovery instead of operator curation,
and W2's Plane A routing keys on this declaration. Serving/pricing
explicitly out of scope.

**Next step:** self-contained engineering in apps/pylon + workers/api;
no spend; pairs naturally with the admin tick's eligibility pre-filtering
(`ec2e8b281`). Good first pick after #4748's contract freeze.

### #4663 — release-cluster verification sweep

Nine comments, almost all same-day recheck churn from when the cluster was
open ("registry 2026-06-10.6 … 2026-06-10.25, prerequisites still open").
The prerequisite set has since fully resolved: #4655/#4656/#4657/#4659/
#4660/#4661 closed done, #4658 closed done (red→yellow live), #4654 closed
by owner decision (no hosted CI; manual script gate), #4662 closed deferred
(npm credential is the one owner action; runbook at
`apps/pylon/docs/2026-06-11-v030-release-preparation-record.md`).

The sweep is therefore runnable now, and its honest outcome is
predetermined: `pylon.v03_release_candidate.v1` and
`pylon.release_tomorrow.v1` stay yellow on
`pylon_v03_stable_release_not_green` (npm publish pending the credential);
`pylon.install_without_wallet_knowledge.v1` verifies at yellow with its
self-serve remainder; `pylon.no_dark_capacity_accounting.v1` is green
(verify the exception receipt's follow-up landed before its 2026-06-12
expiry); `autopilot.codex_probe_pylon_successor.v1` stays yellow (its
yellow→green attempt failed verification on 2026-06-10 at 21:41 — the gate
working as designed). Deliverable: fresh `lastVerifiedAt` receipts, the
Forum wrap-up post, honest remainders named.

### #4671 — training/compute-modes verification sweep

Twelve comments of the same recheck shape, plus two substantive ones: the
2026-06-10 14:23 note adding `compute.tassadar_executor_poc.v1` to the
sweep's coverage (since verified green), and earlier Forum wrap-ups. The
prerequisite epic was rescoped twice by owner directive: #4664/#4669 closed
done; #4665/#4666/#4670 closed not-planned (Qwen); #4667/#4668 closed
not-planned (GEPA). The sweep's original green targets are moot —
`pylon.compute_revenue_modes.v1` is now **planned** by directive.

What the sweep should now verify is the *replacement* training surface
built tonight: `compute.tassadar_executor_poc.v1` green;
`training.verification_classes.v1` yellow (five classes exercised on real
dispatched production work as of tonight: exact_trace_replay,
deterministic_recompute, freivalds_merkle, statistical_cross_check,
seeded_replication); `training.device_capability_dataset.v1` yellow;
`pylon.first_real_model_training_run.v1` red pending #4678. Deliverable:
the wrap-up post against the Tassadar-focused promise set, fresh
verification receipts, and closure — the per-issue rescope comment from
this session (2026-06-11) is already on the thread.

## Delegated infrastructure status

**SHC box `oa-shc-katy-01` (23.182.128.195, ubuntu, key-auth verified):**
healthy — up 10 days, load ~0.02, 16 vCPU / 62 GB (57 free), Docker 29.1.3,
bun + node present. Active tenants at audit time:
- **Parallel #4678 lane (live):** `oa-pylon-4678`, `oa-cs336-a1`,
  `oa-bench-runs` staging dirs; `oa-wallet-4678` MDK daemon running; an
  active `npm exec @moneydevkit/agent-wallet@latest`.
- **Leftover from the 2026-06-08 v0.2 proof:** a wallet daemon still running
  out of `/tmp/pylon-v02-linux-home.SYyREJ` (plus `/tmp/pylon-v02-shc-proof*`
  artifacts). Candidate for cleanup — but not while the #4678 lane is mid-run
  on the same host, and not by this session (not ours).
- No Docker containers running; the control API (8787) and Codex broker
  (8788) installs from the runbook eras remain per
  `autopilot-omega/docs/2026-06-02-shc-agent-deployment-runbook.md`.

**Local machine:** 16 Pylon-related processes (the rung-0 launchd Pylon and
the supervised loops for the two cs336 device Pylons). Six long-lived MDK
wallet daemons from *older* lanes (artanis-gepa 2026-06-08 payer/receiver,
fresh-recipient-b, opencode-tippingbot, autopilot-omega issue-503 recipient,
and one global npx daemon) — all pre-dating this session and left untouched;
every daemon this session's subagents started (ports 3473–3490 range) was
verified stopped. The pylon-4658 contributor home (`/tmp/pylon-4658-home`)
and validator home remain in place and registered: `pylon.24819249b4634a4c9d5e`
(~334 sats) and `pylon.4f4ef3d029e57674be98` (~313 sats) — these are now
standing test devices for the training lane.

**Live fleet (audit time):** 3 online / 45 registered / 3 wallet-ready /
3 assignment-ready. The registered count keeps climbing (42 → 45 tonight)
as fresh smoke identities accumulate — worth an eventual janitorial pass on
never-heartbeated registrations, but the funnel labels them honestly.

**Treasury:** 44,956 sats, untouched by this session (all paid closeouts
were funded from the operator edge wallet, exactly as the campaign-rewards-
only authority boundary requires). Edge payer wallet: ~1,615 sats remaining
of the night's ~2,476 starting balance.

## The night's ledger (what this session closed and proved)

26 issues closed, each with an evidence or directive comment:

- **Fixed and deployed:** #4735 (pylon-stats self-describing counter
  windows — live-verified counters reconcile against rows), #4653
  (forum.content_tipping.v1 → GREEN: live webhook callback smoke, real
  21-sat refund projected publicly, all transitions receipt-first), #4658
  (live install-to-bitcoin smoke on a real machine: nine-stage chain, 21
  sats settled, promise red→yellow), #4675 (A1 homework job kind: paid
  verified closeout, run.cs336.a1.demo live), #4676 (paid validator lane:
  independent cross-Pylon re-verification, self-validation structurally
  blocked, verification_classes planned→yellow), #4681 (A2
  device-capability dataset live: statistical_cross_check's first
  production runs, device_capability_dataset planned→yellow), #4679 (A3:
  24-cell paid IsoFLOP sweep, fit from the committed Psionic reference,
  public curve live), #4680 (A4: four refinery stages computed,
  cross-verified, paid, publicly admitted; eval-delta as honest design),
  #4682 (A5: rollout+grading splits cross-graded between Pylons,
  seeded_replication's first production runs, public eval suite live),
  #4683 (receipt-backed leaderboards: a3_isoflop lane + settled-sats
  linkage with pending-never-as-paid proven by test), #4677 (run pages —
  closed once a real CS336 run existed to render).
- **Owner-decision closures:** #4654 (no hosted CI — workflow deleted,
  manual script gate documented), #4665/#4666/#4670 (Qwen out),
  #4642/#4667/#4668 (GEPA wontfix → planned), #4641/#4645/#4648/#4651/
  #4652/#4732/#4717/#4726 (non-focus implementation deprioritized; rails
  preserved), #4662/#4700 (deferred to single named owner actions: npm
  credential; one MDK dashboard click).
- **Closed by parallel lanes during the session:** #4734 (scanner-safe
  refs).

**Money:** ~524 sats spent across the session against per-issue caps of
200–500 (all approval refs recorded per issue), producing **37 new public
settled receipts** — 1 install-to-bitcoin, 1 A1 homework, 1 validator
recheck, 2 A2 benchmarks, 24 A3 sweep cells, 4 A4 refinery stages, 4 A5
splits — plus the tip/refund pair on #4653. Every receipt retrievable at
its public route at close time.

**Verification classes:** all five now exercised on real dispatched
production work (exact_trace_replay from the Tassadar PoC;
deterministic_recompute, freivalds_merkle, statistical_cross_check,
seeded_replication from tonight). This is the substrate W2's tier ladder
(#4748) builds on.

**Public surfaces that left their empty states tonight:**
`/api/training/device-capabilities/a2`, `/api/training/isoflop/a3`,
`/api/training/refinery/a4`, `/api/training/evals/a5`, leaderboard lanes
`a2_throughput` / `a3_isoflop` / `a5_accuracy`, and `/training/runs` now
listing two real runs.

**Standing caveats carried everywhere, honestly:** single-physical-host
(two local Pylons share one Mac — #4678's SHC device breaks this),
synthetic bounded corpora (A3 fit exponents and A4 shards are analysis
artifacts, not LLM-scaling or crawl-scale claims), operator-staged lanes
(no self-serve settlement yet), and the `a1_loss` / `a4_eval_delta` boards
empty-with-blockers rather than fabricated.

## Recommended order from here

1. **#4678** — already moving on SHC under the parallel lane; on completion
   it flips `pylon.first_real_model_training_run.v1` red→yellow and breaks
   the single-host caveat across the whole evidence base. Don't
   double-dispatch; pick it up only if the lane stalls.
2. **#4748 (W2 contract freeze)** — no dependencies, no spend, and W3 is
   blocked on it. The schema/tier/split contracts can land today.
3. **#4750 (W4.1 capability envelope)** — self-contained; makes the
   evolution loop's device discovery capability-driven and feeds W2's
   routing.
4. **#4663 + #4671 sweeps** — run once #4678 resolves (the training sweep's
   wrap reads much better with the multi-device receipt in hand); both are
   verification + Forum wrap-ups with predetermined honest outcomes.
5. **#4749 (W3)** — parked behind W2's first verified tokens by design.

Owner actions still parked (from closed-deferred issues, unchanged): npm
publish credential (#4662 chain → stable 0.3.0), one MDK dashboard payout
click (#4700), optional hosted-MDK programmatic-payouts enable (noted on
#4658).

## Cross-references

- `docs/2026-06-10-all-open-issues-blocker-attack-order.md` — the
  attack-order audit this session executed (now historical: waves 1–3
  complete, wave 4 rescoped away, wave 5 = the CS336 lane mostly done,
  wave 6 pending as #4663/#4671)
- `docs/artanis/2026-06-10-artanis-pylon-tassadar-full-status-audit.md` —
  the full system audit (its "buy the first receipts" recommendation was
  superseded by the focus directive; its Tassadar sections remain current)
- Evidence docs from tonight, all in `apps/openagents.com/docs/`:
  `2026-06-11-pylon-live-install-to-bitcoin-smoke-evidence.md`,
  `2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md`,
  `2026-06-11-cs336-a1-live-homework-paid-closeout-evidence.md`,
  `2026-06-11-training-validator-paid-closeout-evidence.md`,
  `2026-06-11-cs336-a2-device-capability-paid-closeout-evidence.md`,
  `2026-06-11-cs336-a3-isoflop-paid-sweep-evidence.md`,
  `2026-06-11-cs336-a4-data-refinery-workload-and-admission-evidence.md`,
  `2026-06-11-cs336-a5-rollout-grading-paid-evidence.md`
- `apps/pylon/docs/2026-06-11-v030-release-preparation-record.md` — the
  npm-credential runbook

---

## Addendum — end of session (~13:00 UTC 2026-06-11)

Since the main body above was written, this session closed six more issues
and the board changed shape twice.

**Closed since the main body:**
- **#4748** (W2 trace-factory contract freeze): all four contracts frozen
  and versioned (`workers/api/src/tassadar-trace-factory/`), a 314-record /
  3.48M-token six-family pilot corpus generated and validated at **100%
  schema-valid / 100% full-replay**, clean-checkout replay proven from a
  fresh /tmp worktree, and the distillation-dataset receipt
  (`receipt.dataset_curation.corpus.tassadar_trace.v0_1.local_pilot`)
  proposed to the operator against the evolution-loop's fourth blocker.
- **#4750** (capability envelope consumer): live-verified — the validator
  Pylon's public row carries `capability.tassadar_poc.numeric_model_executor`
  plus `receipt.tassadar_executor.self_test.v1.f2995c4e3c959b42`; unreceipted
  claims get typed refusals; the dispatch gate and admin-tick eligibility
  filter on the receipted capability.
- **#4663 / #4671** (both verification sweeps): 18 fresh receipts on the
  public transitions log, both Forum wrap-ups posted, no flips earned (the
  honest outcome both issues define). The capacity-funnel exception receipt
  was confirmed discharged before its 2026-06-12 expiry.
- **#4753 / #4754** (projection-staleness instances): credited-rung tips
  now fully readable (credited/swept buckets, derived receipt refs, FIFO
  sweep attribution — live-verified showing 3 previously-invisible swept
  rows / 270 sats) and x_claim_reward eligibility served at
  `GET /api/agents/claims/rewards` with the #4748-shape staleness contract
  (live: `eligible: 1`, Orrery's disclosed case).

**Session totals: 35 issues closed**, registry `.19` → `2026-06-11.7`
(plus committed `.1–.7` series), ~564 sats spent against per-issue caps,
38 public settled receipts, five verification classes live, ten production
deploys, zero gate failures left behind.

**Still open, and why:**
- **#4678** — held by an ACTIVE parallel Codex lane (SHC control API;
  uncommitted files in the shared worktree touched as recently as 05:58
  local; staging dirs on oa-shc-katy-01 current). Not taken over.
- **#4752** (OpenAPI refresh) — mechanically blocked on
  `openagents-openapi.ts`, which that same lane holds; three issues'
  worth of owed OpenAPI entries are queued in closure comments.
- **#4751** (projection-staleness epic) — two of its instances closed
  tonight (#4753, #4754); the remaining retrofit (generatedAt+maxStaleness
  across all public projections) is bounded engineering, partially blocked
  on #4752's file.
- **#4749** (W3 student program) — its own acceptance requires all four
  baselines trained on a **100M-token corpus snapshot**; the verified
  corpus is 3.48M (local pilot), and network-scale generation is gated by
  psionic W1.1. Honestly not closeable yet; the contract freeze it depends
  on (#4748) is done.

**New scope conflict requiring an owner ruling:** a parallel lane filed a
**30-issue Autopilot MVP ladder** (#4757–#4786: B1–B4 bootstrap, M1–M14
MVP, A1–A4 agent parity, P1–P9 post-MVP, epic #4786) at ~12:30 UTC. This
is an entire product program — web composer, Stripe card-on-file,
account-pool dashboards, provider connect flows, cloud-Pylon deployment,
overnight proof smokes — and it post-dates the standing owner directive
that all implementation focus is Tassadar/Psion + CS336 pipeline prep with
non-fitting work closed/deprioritized. This session deliberately did NOT
apply the old directive to the new ladder (mass-closing a freshly-filed
roadmap would destroy deliberate parallel work). The two directives cannot
both govern; the owner should say which applies to #4757–#4786.
