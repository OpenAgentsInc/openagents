# CS336 A4 Data-Refinery: Real Workload + Admission Seam (issue #4680)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4680` (CS336 distributed-homework epic,
plan step 8 — the A4 data-refinery homework: filtering, dedup, and
eval-delta payment design).

Registry version (live worker): `2026-06-11.7` (worker `fc722a14`). This
commit ships **no** registry edit and records **no** promise transition:
the named promise `training.data_refinery_corpus.v1` stays `planned`
behind `blocker.product_promises.crawl_scale_corpus_missing`,
`blocker.product_promises.corpus_provenance_receipts_missing`, and
`blocker.product_promises.eval_delta_payment_missing`. None of those
blockers is honestly cleared by a synthetic-corpus deterministic-stage
foundation, so the registry is left untouched.

Operator approval reference for bounded paid closeouts:
`approval.operator.20260611.focus_cs336_issue4680` (spend cap 300 sats).
**Spend tonight: 0 sats.** No live paid dispatch was run in this pass.

## What this commit adds

Before this commit the A4 lane was contract-only (`b7561da25`): job
kinds, dispatch payloads, the verification-challenge binding, the
public-safety guard, and the payment-policy doc. There was **no real
refinery work** behind the deterministic-recompute class and **no
admission seam** for refinery output. This commit adds both.

- `src/cs336-a4-refinery-workload.ts` — the real deterministic refinery
  over a bounded, public-safe **synthetic** corpus seeded from the A1
  tokenizer shard digest (same provenance binding as #4675/#4679). Four
  stages, each committing a SHA-256 digest over its exact output plus
  public counts:
  - `pii_masking` — masks emails/phones/IPv4 with replacement tokens,
    reports per-class counts;
  - `exact_line_dedup` — removes exact duplicate lines corpus-wide;
  - `gopher_rules` — per-document quality verdicts (too-few-words,
    low-alpha-ratio, symbol-heavy);
  - `minhash_dedup` — deterministic MinHash + LSH banding +
    exact-Jaccard confirmation + union-find near-duplicate removal.
  The corpus is synthetic by construction: the only "PII" present are
  template tokens over the reserved `example.invalid` namespace and
  documentation-range IPv4s. No real Common Crawl payload or
  contributor-sourced sensitive material is materialized or published.
- `src/training-data-refinery.ts` — the previously missing admission
  seam. `admitCs336A4DataRefineryEvidence` writes receipted refinery
  shards into a run's public projection (`a4DataRefinery`);
  `publicDataRefineryProjection` builds the public feed. Unreceipted
  shards are not admissible, a public-safety guard rejects wallet,
  payment, raw-shard, and private-path material at admission, and
  `stages_verified` requires at least three distinct stages with a
  verified `deterministic_recompute` challenge (acceptance criterion #1).
- Routes: `POST /api/training/runs/{trainingRunRef}/data-refinery-evidence`
  (admin) and `GET /api/training/refinery/a4` (public), with OpenAPI
  operations `admitTrainingA4DataRefineryEvidence` and
  `readTrainingA4DataRefineryDashboard`.
- `scripts/cs336-a4-data-refinery-run.ts` — the contributor-side
  executor (no network, no secrets, no spend, public-safe output only).
- Payment-policy doc updated with the workload/admission addition and an
  explicit eval-delta bonus design (parameters + modeled basis labels;
  no fabricated numbers).

## Local no-spend rehearsal (proven this pass)

`bun run scripts/cs336-a4-data-refinery-run.ts` executed the four stages
on this device and produced real, deterministic, public-safe output:

| Stage | Output digest (prefix) | Public counts |
| --- | --- | --- |
| `pii_masking` | `0e8f05fc4ab90be1…` | 186 masks (62 email / 62 phone / 62 IPv4) |
| `gopher_rules` | `ce6843a936165413…` | 64 in, 56 kept, 8 rejected (5 low-alpha, 3 symbol-heavy) |
| `exact_line_dedup` | `ee9cc8600064add3…` | 384 lines in, 181 removed, 203 unique |
| `minhash_dedup` | `5719cb8f68e5e5b2…` | 64 docs in, 16 removed, 16 confirmed near-dup pairs, 48 clusters |

The workload test re-runs every stage twice and asserts the digest
reproduces (the deterministic-recompute property the sampled re-runs
rely on), and asserts a different corpus shape yields a different digest
(tamper rejection).

## Verification (all green, from `apps/openagents.com/workers/api`)

- `bun run smoke:cs336-a4:data-refinery` — 6 files / 28 tests passed
  (now also covers the workload, the refinery projection, the route, and
  the leaderboard wiring, not just the contract slice).
- `bunx vitest run src/cs336-a4-refinery-workload.test.ts
  src/training-data-refinery.test.ts src/training-run-window-routes.test.ts`
  — passed.
- `bun run typecheck` — exit 0.
- `bun run check:architecture` — passed.
- `bun run check:effect-topology` — passed.

## Live state at filing time

- `GET /api/training/leaderboards/a4_eval_delta` — `rows: []`, blocker
  `blocker.training_leaderboard.a4_eval_delta.requires_verified_receipts`
  (honest empty state; this commit admits no eval-delta scores).
- `GET /api/training/refinery/a4` — **not yet live**: the deployed
  worker (`fc722a14`) predates this route. After the next deploy the
  public refinery feed serves the `a4DataRefinery` projection.

## Honest remainders (named gaps)

- `remainder.cs336_a4.live_paid_dispatch_operator_gated`: no real shard
  was dispatched, recompute-verified by the opposite Pylon, or paid this
  pass. The live paid dance (window plan/activate → lease → paid
  assignment dispatch → worker chain → sampled cross-verification →
  operator closeout → MDK Lightning settlement → settlement bridge →
  evidence admission) is the same operator-driven Lightning orchestration
  the #4675/#4679/#4681 runs used and is the remaining step for
  acceptance criterion #1 to clear *live*. The code foundation that makes
  it mechanical now exists; the deterministic stages are
  recompute-verifiable as specified and proven locally.
- `remainder.cs336_a4.synthetic_corpus_only`: the live workload runs a
  bounded synthetic corpus, not real WARC shards. Real-shard refinement
  depends on the Psionic HTML/WARC extraction and model-backed
  classifiers, still external per `psionic#1102`.
- `remainder.cs336_a4.eval_delta_design_only`: no fixed-trainer eval loop
  exists, so the eval-delta quality bonus is design + typed policy +
  blockers only. No eval-delta number is fabricated, admitted, projected,
  or paid.
- `remainder.cs336_a4.deploy_then_live_admission`: the
  `/api/training/refinery/a4` read route and the
  `data-refinery-evidence` admission route are not on the deployed worker
  yet; like the A3 pass, a pre-deploy live run would admit evidence via
  an operator-staged D1 write of route-equivalent validated JSON.

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this document or in any committed ref.

---

## Live paid leg (issue #4680, same date, worker `5a0162e9`)

Operator approval: `approval.operator.20260611.focus_cs336_issue4680`
(spend cap 300 sats). Registry version during run: `2026-06-11.7`.
**Spend this leg: 40 sats.** This leg ran the full operator-staged paid
dance on production for all four refinery stages and left the public
`GET /api/training/refinery/a4` empty state for the first time.

### Result

- `GET /api/training/refinery/a4` now serves `blockerRefs: []`,
  `observedVerifiedStages: [exact_line_dedup, gopher_rules,
  minhash_dedup, pii_masking]`, four receipted shards, all
  `verified: true`, projection status `stages_verified` (acceptance
  criterion #1: at least three distinct stages with a verified
  `deterministic_recompute` challenge — four landed).
- Every stage ran the real committed workload
  (`src/cs336-a4-refinery-workload.ts`) on the contributor device via
  `scripts/cs336-a4-data-refinery-run.ts`, was independently re-executed
  by the opposite Pylon as a cross-process recompute, and the production
  Worker's `deterministic_recompute` class finalized every challenge
  `Verified` with zero failure codes. Worker stage digests and
  opposite-Pylon recompute digests matched exactly on all four stages
  (and reproduce the local-rehearsal digests in the table above).
- Four bounded closeouts paid 10 sats each over real Lightning from the
  operator edge payer wallet with provider-confirmed balance movement on
  both sides (payer 1695 -> 1655; worker wallet 294 -> 314; validator
  wallet 274 -> 294). All four public settlement receipts return 200
  with `state: settled`, `amountSats: 10`, `movementMode: real_bitcoin`,
  `realBitcoinMoved: true`.
- Evidence was admitted through the **live deployed admission route**
  `POST /api/training/runs/run.cs336.a4.data_refinery.demo/data-refinery-evidence`
  (no D1 staging needed this time, unlike the A3 pass) and the route's
  receipted-shards-only, positive-count, and public-safety guards ran on
  the exact projected evidence.

### Live route chain (all production)

| Stage | Evidence |
| --- | --- |
| run plan (admin) | `run.cs336.a4.data_refinery.demo`, promiseRef `training.data_refinery_corpus.v1` (substrate link only; no transition claimed) |
| window plan + activate (admin) | `training.window.cs336_a4.demo.20260611.w1` (worker Pylon), `training.window.cs336_a4.demo.20260611.w2` (validator Pylon), homeworkKind `admin_dispatched_homework`, dataset `dataset.cs336_a4.bounded_synthetic_public_safe_corpus.v1` |
| window leases (contributors) | `training.lease.5f2f3ee5-bc46-4ce5-b82c-6bba15af7c09` (w1, `pylon.24819249b4634a4c9d5e`), `training.lease.b58bf0f0-63d8-4299-a0ff-96279d841e3f` (w2, `pylon.4f4ef3d029e57674be98`) |
| wallet readiness + payout targets | `pylon_event.wallet_readiness.d45b35d9-10e1-43b2-839e-99f8a701490c` / `pylon_event.payout_target_admission.32f5b7c0-bf89-4fee-b45e-fd819fc0fe4d` (`payout_target.public.cs336_a4.admitted_4680_worker`), `pylon_event.wallet_readiness.7f16ff1f-22f2-443c-b87f-f3b187a53e20` / `pylon_event.payout_target_admission.35a3fc4f-fbdb-40d7-8940-559da2621780` (`payout_target.public.cs336_a4.admitted_4680_validator`) |
| paid assignment dispatch (admin) | `assignment.cs336_a4.refinery.{pii_masking_v2,exact_line_dedup,gopher_rules,minhash_dedup}_20260611075054` (plus the superseded `assignment.cs336_a4.refinery.pii_masking_20260611075054`, see seams), paymentMode `payable_pending_settlement`, jobKind rail `claude_agent_task` with the `cs336A4` payload and `rail.job_kind.claude_agent_task_until_cs336_a4_data_refinery_deploys`; dispatch gate `ready`, one active assignment per Pylon, stages split worker/{pii_masking, exact_line_dedup} validator/{gopher_rules, minhash_dedup} |
| worker chains (agent bearer) | per-stage acceptance, progress, artifact proof metadata (`commitment.cs336_a4.pii_masking.sha256_0e8f05fc4ab90be1`, `commitment.cs336_a4.exact_line_dedup.sha256_ee9cc8600064add3`, `commitment.cs336_a4.gopher_rules.sha256_ce6843a936165413`, `commitment.cs336_a4.minhash_dedup.sha256_5719cb8f68e5e5b2` + public count result refs), worker closeouts |
| verification (admin create, open claim, admin finalize) | five `deterministic_recompute` challenges, all `Verified`, zero failure codes: `training.verification.challenge.96a6fc10-9ce0-4ef6-b89e-b599c201932f` (pii_masking_v2), `.70879882-8fc5-4952-8b91-3fba878de8ed` (exact_line_dedup), `.db851fe7-c82e-4761-91e8-467edcb92b5c` (gopher_rules), `.ba11b0fb-4d5c-4cc9-b453-736f393c9f85` (minhash_dedup), `.4ec3fa07-8507-46d4-bd5c-1f1a01336261` (superseded pii_masking v1); validator `validator.cs336_a4.deterministic_recompute_issue4680`, samplingPolicy `per_contribution`; each payload binds expected vs opposite-Pylon recomputed full-digest refs |
| operator closeouts (admin) | `accepted_work.cs336_a4.{stage}_4680_commitment_matched` x4, all paid assignments `accepted_work`; acceptance basis is the pre-registered deterministic commitment match with the cross-Pylon recompute verdict landed before payment |
| payments | 4 x 10 sats over Lightning from the operator edge payer wallet (warm channels, no JIT fees); payer 1695 -> 1655, worker wallet 294 -> 314, validator wallet 274 -> 294 (provider-confirmed both sides); redacted payment refs `payment.redacted.mdk_agent_wallet.{f1305ac783953b9a05614cc1, 7b6e3ad6e787d17c6c5a6541, e39f14df7c1c32895015a12c, 3877d3165b8d3292e0a8845c}` (sha256 derivations over private payment material, not hash prefixes) |
| pylon events | per-stage `pylon_event.payment_receipt.{d72d1910…, e4228c60…, db0b123f…, 48b66dbe…, 764ff750…}` and `pylon_event.settlement_status.{ee960bf1…, 3148b117…, e3dbebbd…, 3e5797cb…, f4594b40…}` |
| settlement bridges (admin) | `receipt.nexus_pylon.settlement.assignment_cs336_a4_refinery_{pii_masking_v2,exact_line_dedup,gopher_rules,minhash_dedup}_20260611075054`, adapter `mdk_agent_wallet`, all four public receipt routes 200 (`settled`, `amountSats: 10`, `movementMode: real_bitcoin`, `realBitcoinMoved: true`) |
| window seal + reconcile (admin) | both windows `reconciled` (w1 seal `closeout.cs336_a4.operator_accepted_pii_masking_4680`, reconcile `receipt.nexus_pylon.settlement.assignment_cs336_a4_refinery_pii_masking_v2_20260611075054`; w2 seal `closeout.cs336_a4.operator_accepted_minhash_dedup_4680`, reconcile `receipt.nexus_pylon.settlement.assignment_cs336_a4_refinery_minhash_dedup_20260611075054`) |
| evidence admission (live route) | `POST /api/training/runs/run.cs336.a4.data_refinery.demo/data-refinery-evidence` returned the projection directly: `status: stages_verified`, `observedVerifiedShardCount: 4`, `blockerRefs: []`; shards carry full output-digest refs, pylon refs, settlement + closeout receipt refs, and challenge + verdict verification refs |
| public 200s | `route:/api/training/refinery/a4` (4 verified shards, `blockerRefs: []`), `route:/api/training/runs/run.cs336.a4.data_refinery.demo` (`verifiedWorkCount=5`, `reconciledWindowCount=2`, `assignedContributorCount=2`), all four public settlement receipts |

### Spend accounting

| Movement | Sats |
| --- | --- |
| 4 refinery-stage closeouts x 10 sats (Lightning, warm channels) | 40 |
| Total operator spend (cap 300) | 40 |
| Hosted MDK treasury spend | 0 |

### Registry decision (propose only, no edit)

`training.data_refinery_corpus.v1` stays `planned` at `2026-06-11.7`.
Tonight's leg demonstrates one component of the promise's green
verification — refinery shards dispatched as paid assignments with
deterministic-recompute verification — but on a bounded **synthetic**
corpus. None of the three standing blockers is honestly cleared:
`blocker.product_promises.crawl_scale_corpus_missing` (no crawl-class
source was acquired or refined),
`blocker.product_promises.corpus_provenance_receipts_missing`
(provenance receipts exist only for the bounded synthetic shard, not a
corpus), and `blocker.product_promises.eval_delta_payment_missing` (no
fixed-trainer eval loop; no eval-delta number was computed, admitted, or
paid). Proposed for the registry-owning lane: refresh the promise
`safeCopy` to note that the first operator-staged paid + verified
refinery-stage assignments ran on a bounded synthetic corpus with public
settled receipts, without any state move.

### Seams found (worth knowing for reuse)

- `seam.cs336_a4.jobkind_literal_still_missing`: despite the A4 route
  deploy, `cs336_a4_data_refinery` is still not a
  `PylonApiAssignmentJobKind` literal (worker `5a0162e9` and current
  source both lack it), so the paid assignments rode jobKind
  `claude_agent_task` with the `cs336A4` payload under
  `rail.job_kind.claude_agent_task_until_cs336_a4_data_refinery_deploys`
  — the same documented rail as the A1/A3 pre-deploy runs.
- `seam.cs336_a4.bridge_bans_email_substring`: the settlement bridge's
  public-safe pattern rejects any ref containing the substring `email`,
  so the first pii_masking assignment's artifact proof ref
  (`…masked_emails_62`) permanently blocked that assignment's bridge
  (events are immutable). The stage was re-dispatched as
  `assignment.cs336_a4.refinery.pii_masking_v2_20260611075054` with the
  transit rename `rename.masked_emails_reports_as_masked_mailbox_in_transit`
  (same pattern as the A1 `tokenizer` -> `bpe_vocab` rename). The
  superseded v1 assignment remains `accepted_work` with a Verified
  challenge but carries no settlement receipt; exactly one 10-sat
  payment moved for the pii_masking stage and its evidence rides the v2
  assignment.
- `seam.mdk_wallet.port_flag_ignored_for_client_commands`: the MDK
  agent-wallet CLI's `--port` flag does not pin client commands; daemon
  discovery is pid-file-first, then the `MDK_WALLET_PORT` env var, then
  default 3456. With a daemon already on 3456, un-pinned client calls
  for other wallet homes silently cross-talk to the wrong wallet
  (balances read 1695 across all three homes before the fix). All
  daemons this leg were pinned with `MDK_WALLET_PORT` and verified by
  pid-file port + expected balances before any spend, and stopped after
  the run.
- The dispatch gate freshness check (`blocker.public.pylon_dispatch.pylon_stale`)
  requires a heartbeat newer than a few minutes, so each dispatch is
  preceded by a just-in-time heartbeat.

### Honest remainders (named gaps, updated)

- `remainder.cs336_a4.live_paid_dispatch_operator_gated` is **cleared
  for the bounded synthetic lane** (this leg). What remains
  operator-staged: dispatch, challenge create/finalize, closeout,
  payment execution (hosted-MDK programmatic payouts remain disabled),
  and bridge — a standing refinery market needs self-serve admission and
  automated settlement.
- `remainder.cs336_a4.synthetic_corpus_only` stands: no real WARC/crawl
  shard was refined; real-shard refinement still depends on the Psionic
  HTML/WARC extraction and model-backed classifiers (`psionic#1102`).
- `remainder.cs336_a4.eval_delta_design_only` stands: the
  `a4_eval_delta` leaderboard remains an honest empty state; nothing
  eval-delta was computed or paid.
- `caveat.cs336_a4.single_physical_host_two_pylons`: both Pylons run on
  one physical machine; the opposite-Pylon re-runs are real
  cross-process re-executions, not cross-machine replication.
- `caveat.cs336_a4.run_state_stays_planned`: training runs still have no
  state-transition route, so the run row stays `planned` with two
  reconciled windows (known #4675 seam).

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths appear in this live-leg section or in any public ref.
