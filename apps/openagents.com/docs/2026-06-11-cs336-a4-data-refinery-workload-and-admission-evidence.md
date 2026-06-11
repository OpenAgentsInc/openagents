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
