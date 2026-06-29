# CS336 A4 Data-Refinery Payment Policy

Date: 2026-06-10

Issue: [#4680](https://github.com/OpenAgentsInc/openagents/issues/4680)

This document records the OpenAgents monorepo side of the CS336 A4
data-refinery homework lane.

## Dispatchable Stages

The Worker job kind is `cs336_a4_data_refinery`, targeting Psionic lane
`psion_cs336_a4_data_refinery_reference_v1`.

No-spend dispatch contracts are defined for:

- `pii_masking`;
- `gopher_rules`;
- `exact_line_dedup`;
- `minhash_dedup`.

All four stages use `deterministic_recompute` verification: workers commit a
public output digest, validators rerun bounded samples, and only matching
public digest refs are accepted. Raw Common Crawl shards, private contributor
data, local paths, wallet material, and provider credentials are never public
payload fields.

## Psionic Conformance Status

Psionic `OpenAgentsInc/psionic#1102` landed the first tranche:

- PII masking: partial, with heuristic scanners and assignment replacement
  tokens; still needs conformance testing against Stanford fixtures before
  paid grading.
- Gopher rules: landed with per-rule verdicts.
- Exact-line dedup: landed.
- MinHash dedup: landed with seeded hash families, LSH banding, exact-Jaccard
  verification, and union-find.

Still external to this monorepo:

- HTML/WARC extraction;
- langid;
- NSFW/toxic/quality classifiers;
- full adapter matrix conformance against `assignment4-data/tests/adapters.py`.

## Payment Policy

Base payments are per verified shard:

- policy ref: `policy.cs336_a4.pay_per_verified_shard_processed`;
- pays only after deterministic recompute accepts the stage output;
- pays for public-safe processing work, not raw data volume.

Quality bonuses remain blocked until:

- the fixed-trainer eval loop exists;
- operator funding is approved;
- Psionic classifier adapters and conformance status are complete.

Bonus policy ref: `policy.cs336_a4.eval_delta_quality_bonus_pending`.

Quality measurement ref:
`measurement.cs336_a4.downstream_eval_delta_fixed_reference_model`.

Boundary ref:
`boundary.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data`.

The intended quality bonus compares downstream eval delta after training a
fixed reference model on accepted filtered data. This pays for data quality
under a held-constant trainer, not for larger datasets or private data resale.

## Smoke

Run from `apps/openagents.com/workers/api`:

```sh
bun run smoke:cs336-a4:data-refinery
```

The smoke proves at least three A4 stages are dispatch-contract ready,
public-safe, deterministic-recompute bound, and quality bonuses remain blocked
until the fixed-trainer eval and funding steps exist.

## Real Refinery Workload And Admission Seam (added 2026-06-11)

The contract slice (`b7561da25`) defined the dispatch payloads and the
payment policy. This addition provides the real deterministic work the
deterministic-recompute verification grades, plus the admission seam that was
previously missing for refinery output.

- `src/cs336-a4-refinery-workload.ts` runs the four stages over a bounded,
  public-safe **synthetic** corpus seeded from the A1 tokenizer shard digest
  (same provenance binding as the #4675/#4679 runs). The corpus is synthetic
  by construction: the only "PII" present are template tokens over the reserved
  `example.invalid` namespace and documentation-range IPv4s, so no real Common
  Crawl payload or contributor-sourced sensitive material is ever materialized
  or published. Each stage commits a SHA-256 digest over its exact output plus
  public counts; re-running the stage on the same shard reproduces the digest,
  any input perturbation changes it.
- `scripts/cs336-a4-data-refinery-run.ts` is the contributor-side executor:
  no network, no secrets, no spend, public-safe counts and digests only.
- `src/training-data-refinery.ts` adds the admission seam that did not exist
  before: `admitCs336A4DataRefineryEvidence` writes receipted refinery shards
  into a run's public projection (`a4DataRefinery`), and
  `publicDataRefineryProjection` builds the public feed. Unreceipted shards are
  not admissible; a public-safety guard rejects wallet, payment, raw-shard, and
  private-path material at admission time; `stages_verified` requires at least
  three distinct stages with a verified `deterministic_recompute` challenge
  (acceptance criterion #1).
- Routes: `POST /api/training/runs/{trainingRunRef}/data-refinery-evidence`
  (admin, OpenAPI `admitTrainingA4DataRefineryEvidence`) and
  `GET /api/training/refinery/a4` (public, OpenAPI
  `readTrainingA4DataRefineryDashboard`).

## Eval-Delta Bonus Design (parameters; no fabricated numbers)

The eval-delta quality bonus is a **design** until a fixed-trainer eval loop
and operator funding exist. It is recorded here with explicit parameters and
modeled basis labels so it can be implemented honestly later; no eval-delta
score is admitted, projected, or paid by the surfaces above, and the
`a4_eval_delta` leaderboard lane stays empty behind
`blocker.training_leaderboard.a4_eval_delta.requires_verified_receipts`.

- Measurement basis (`measurement.cs336_a4.downstream_eval_delta_fixed_reference_model`):
  train the **fixed** reference trainer (plan step 3 config, held constant) on
  a contributor's accepted filtered/deduped output and on an unfiltered
  baseline of the same source, then measure the downstream eval metric delta.
  Data quality is the dependent variable because training is held constant —
  the A4 leaderboard insight stolen from Stanford's staff-model design.
- Bonus formula (modeled, not yet live): `bonus_sats = round(clamp(delta, 0,
  delta_cap) * bonus_rate_sats_per_unit)`, paid **in addition to** the
  per-verified-shard base, and only when `delta > 0` (no penalty for neutral
  filtering, no bonus for quality regressions). `delta_cap` and
  `bonus_rate_sats_per_unit` are operator-set funding parameters, unset until
  funding is approved.
- Anti-gaming boundaries: the bonus pays measured downstream delta, never raw
  volume (`boundary.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data`);
  the eval uses a held-out eval set the contributor does not control; and the
  filtered output that earns a bonus must first pass deterministic-recompute
  verification of the stage that produced it.
- Honest status tonight: **no real eval-delta measurement was run.** The
  deterministic refinery stages are the live deliverable; the eval-delta bonus
  is design + typed policy + blockers only. Blockers remain
  `blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus`,
  `blocker.cs336_a4.operator_funding_required_for_bonus_settlement`, and
  `blocker.cs336_a4.psionic_classifier_adapters_partial`.
