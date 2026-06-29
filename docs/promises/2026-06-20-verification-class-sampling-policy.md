# Verification-class validation policy: per-contribution sampling

Decision of record for the open `#4674` question
("aggregate-only vs per-contribution sampling"), the last gate on
`training.verification_classes.v1`. **Owner-approved 2026-06-20.**

## Decision

**Per-contribution sampling is the default for every verification class.** Each
individual contribution carries an independent probability of being re-executed
and challenged by a validator. The April-era **aggregate-only** compromise
(one combined challenge per batch, per-contribution sample challenges skipped —
`9a4494992`, `9eda4b045`) is **deprecated as a sufficient grade on its own**: a
single incorrect or dishonest contribution can hide inside a batch that passes
in aggregate, which violates the verify-by-replay / trust-nothing posture.

Aggregate commitments (Merkle roots, batch digests) are still used as the
*commitment* layer; the change is that **acceptance/reward is gated on
per-contribution sample challenges**, not on the aggregate check alone.

## Per-class sample rates (defaults; tunable knobs to balance rigor vs validator cost)

| Class | Default per-contribution sample rate | Notes |
|---|---|---|
| `exact_trace_replay` | **1.0** (every contribution) | Cheapest grade; already exercised per-contributor across 5 distinct paid contributors on `run.tassadar.executor.20260615` (5 Verified challenges, 5 settlements). |
| `deterministic_recompute` | 0.25 of shards | Re-run a sampled shard exactly (tokenizer/BPE, A4 filters, reward grading). |
| `freivalds_merkle` | per-step probabilistic + 0.20 per-contribution | Matrix work: row-opening Freivalds check per challenged contribution, not batch-only. |
| `statistical_cross_check` | 0.15 | Benchmarks. |
| `seeded_replication` | 0.15 | Rollouts. |

Rates are configuration, not contract — operators raise them for high-value or
low-trust lanes and lower them for cheap, high-volume work. The **floor is
non-zero** for every class (aggregate-only = sample rate 0 is not permitted).

## Why this is honest now

- The class registry is live with three classes exercised on real dispatched
  production work (`exact_trace_replay`, `deterministic_recompute`,
  `freivalds_merkle` commitment-then-challenge).
- A weak-device validator has independently re-executed and been **paid** for a
  Freivalds recheck with a settled public receipt (`#4676`).
- `exact_trace_replay` already runs per-contribution (each of the 5 live
  contributors got an independent Verified challenge + settlement) — i.e. the
  default is already in force on the cheapest class on real money.
- Classes not yet on real dispatched work (`seeded_replication`,
  `statistical_cross_check`) adopt this policy the first time they run.

## Status

This written, per-class decision clears
`blocker.product_promises.aggregate_only_policy_redecision_missing` — the last
open gate on `training.verification_classes.v1`.
