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
