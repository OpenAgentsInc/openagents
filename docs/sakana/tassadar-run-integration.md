# Tassadar Run × Sakana Coordinator: Integration

*Analysis — 2026-06-22. Whether and how a Sakana-style learned coordinator
combines with the live Tassadar run. State grounded in the authoritative promise
sources as of 2026-06-22 (live registry `/api/public/product-promises`,
`docs/promises/2026-06-20-verification-class-sampling-policy.md`,
`docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md`) — not the
older `docs/launch/` audits, which lag.*

Companion to [`adapting-sakana-coordination.md`](adapting-sakana-coordination.md),
[`coordinator-as-verified-work.md`](coordinator-as-verified-work.md),
[`psionic-coordinator-roadmap.md`](psionic-coordinator-roadmap.md).

## Current run state (authoritative, dated)

The run is `run.tassadar.executor.20260615`, GREEN under
`training.decentralized_training_launch.v1`. Two facts the older launch audits
miss:

1. **The run is a multi-class verification registry, not a single kernel.**
   `training.verification_classes.v1` (per-contribution sampling owner-approved
   **2026-06-20**) defines five verification classes, each with a non-zero
   per-contribution sample-rate floor (aggregate-only is deprecated as a
   sufficient grade):

   | Class | Default sample rate | Maps to |
   |---|---|---|
   | `exact_trace_replay` | **1.0** | deterministic execution (the `loop_sum` kernel today) |
   | `deterministic_recompute` | 0.25 of shards | tokenizer/filters/reward grading |
   | `freivalds_merkle` | per-step + 0.20 | matrix work |
   | `statistical_cross_check` | 0.15 | **benchmarks** |
   | `seeded_replication` | 0.15 | **rollouts** |

   Per that doc: `exact_trace_replay` is fully exercised — **5 distinct paid
   contributors, 5 Verified challenges, 5 settlements** on this run (at sample
   rate 1.0); `freivalds_merkle` has had a weak-device validator paid for a
   recheck (#4676); **`seeded_replication` and `statistical_cross_check` are
   defined but not yet on real dispatched work** ("adopt this policy the first
   time they run"). *(The high-level registry summary still says "two
   contributors" — that counter is scoped to the launch-promise canary +
   self-serve pair and lags the verification-class evidence; cite the
   2026-06-20 doc for the current per-class count.)*

2. **Settlement autonomy: the blocking bug is fixed; the first hands-off fire is
   unobserved.** The earlier operator intervention on a settlement was a
   payout-target resolution bug, **since fixed** (per the live registry).
   `docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md` proves
   one 1,000-sat real Spark settlement to an independent contributor (Orrery)
   with a public receipt and an independent validator challenge. What has **not**
   yet happened: a fully-autonomous auto-stream settlement firing at verdict with
   zero operator action. So the gap is *observational* now, not a broken path.

I previously cited a 4-day-old `docs/launch/` audit for the autonomy claim; the
authoritative current state above supersedes it.

## The key realization

The run already has verification classes for **stochastic, variable-quality
work** — `seeded_replication` (rollouts) and `statistical_cross_check`
(benchmarks). Those are exactly the work types a coordinator produces, and they
are **defined-but-undispatched**. So the integration is not "graft model
selection onto the deterministic `loop_sum` loop" (there's no choice to optimize
there). It is: **a learned coordinator is the natural first real dispatched work
for the rollout/benchmark verification classes the run already specifies.**

That reframes all three combinations below around the run's *verification-class
registry* as the reward substrate.

## Three integrations

### 1. Reward oracle = the run's verification verdict (this is roadmap P4)

The coordinator's terminal reward is "did the assembled trajectory produce a
`Verified` verdict?" The run already produces exactly that verdict, per
contribution, on real dispatched work, under a typed class registry. Wire the
coordinator's atomic-evaluation harness (roadmap P4) to emit a contribution into
the appropriate class and read the verdict back:

- **Deterministic / kernel-parity coordinator work** → `exact_trace_replay`
  (rate 1.0): a dense reward on every trajectory — ideal for sep-CMA-ES, which
  wants clean per-eval Bernoulli rewards.
- **Rollout-style coordinator work** (the general case: coordinate LLMs to solve
  a task, grade the answer) → `seeded_replication` (rate 0.15) or
  `statistical_cross_check` for benchmark-graded tasks.

**Decouple training from settlement.** The training reward is the **verdict**,
which is proven, per-contribution, and live. Settlement (the sats) is a
downstream economic layer gated on the autonomous-fire observation. So we can
train a coordinator against verdicts **today** without waiting on the
autonomous-settlement gate — settlement only matters when the coordinator
graduates to a *paid* labor lane (integration 3).

### 2. Kernel-optimization parity = first coordination target on the exact lane

`docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`
defines work where acceptance = throughput gain **and** output-parity verified by
replay — running on the same `exact_trace_replay` machinery the run already
exercises at rate 1.0. This is the cleanest near-term coordination target with a
dense, non-gameable reward: a Conductor-style coordinator orchestrates
planner → kernel-writer → checker; the parity+throughput verdict is the reward;
and the deliverable serves Psionic's own kernel work directly. No new
verification class needed — it rides the fully-proven one.

### 3. Coordinator as a new dispatched workload family / labor participant

Per [`coordinator-as-verified-work.md`](coordinator-as-verified-work.md),
*"coordinate models to solve task X for ≤ B sats, accepted iff the result
Verifies"* is a well-formed work definition. The run is the venue and settlement
rail. A coordinator lane would be the **first real dispatched work for
`seeded_replication`/`statistical_cross_check`**, lighting up classes the
registry already defines, and would render natively on the existing run
visualization (assigned → submitted → replay → verified → settled;
`docs/launch/2026-06-17-tassadar-training-run-visual-language.md`) — worker
selection and role assignment are legible on that grammar with no new visual
language. This is also where the labor-market promises that are already GREEN
(`labor.forum_work_requests.v1`, `labor.nostr_negotiation_market.v1`) provide a
proven settlement path to attach to.

## Reward-oracle wiring spec (ties to roadmap P4)

```
coordinator trajectory τ
  → emit contribution under verification class C
       C = exact_trace_replay        (deterministic / kernel-parity work)
       C = seeded_replication        (LLM rollout work)
       C = statistical_cross_check   (benchmark-graded work)
  → run samples a per-contribution challenge at class rate (floor > 0)
  → independent validator replays / recomputes / cross-checks
  → verdict ∈ {Verified, Rejected}
  → R(τ) = 1.0 if Verified else 0.0     ← coordinator terminal reward (P4)
  → (optional, downstream) auto-stream settlement → sats   ← economic layer only
```

Two design consequences from the sampling policy:

- **Training wants a dense reward; sampled classes give a sparse one.** At rate
  0.15, only ~15% of `seeded_replication` contributions get a challenge, so a
  naive reward would be undefined on the rest. Options: (a) for the *training
  lane*, raise the sample rate toward 1.0 — rates are tunable config, not
  contract, with a non-zero floor; (b) accept a probabilistic but unbiased
  reward (challenge-or-skip) — fine for ES/RL in expectation, just noisier, so
  budget more evals. Start with (a) for fitness stability, fall to (b) at scale.
- **LLM nondeterminism breaks naive replay.** `seeded_replication` only
  re-verifies if the worker output is reproducible — decode at temperature 0 /
  fixed seed, or the replay won't match and everything Rejects. Pin decode
  params per worker as part of the contribution, exactly as `exact_trace_replay`
  pins the program.

## Honest caveats / what blocks the live lane

- **Autonomous settle-at-verdict is unobserved** (bug fixed, not yet fired with
  an independent contributor). Not on the *training* critical path (we train on
  verdicts), but it is on the *paid-lane* critical path (integration 3).
- **`seeded_replication`/`statistical_cross_check` are undispatched.** A
  coordinator lane is their first real exercise, so expect class bring-up cost
  (validator implementation, sample-rate tuning) before the reward oracle is
  trustworthy.
- **Capability gating still applies** — the coordinator selects within the
  receipted capability-eligible worker set; it never overrides the dispatch
  gate.
- **Copy discipline.** Per the settlement-gate doc, public claims stay scoped to
  proven facts (the canary settlement; the per-class verdict counts). A
  coordinator lane does not get to claim "the network coordinates frontier models
  at scale" until there are accepted-work receipts proving it.

## Build path (folds into the roadmap phases)

- **Roadmap P0 (offline reward)** can validate the oracle against
  `exact_trace_replay` verdicts on a deterministic sub-task — proven, dense,
  rate-1.0 — before any LLM nondeterminism enters.
- **Then** move the reward to `seeded_replication` for real LLM coordination
  work, raising the training-lane sample rate for fitness stability.
- **Kernel-parity (integration 2)** is the strongest first *live* target: dense
  exact-lane reward, serves Psionic, no new verification class.
- **Shadow-candidate ship** (roadmap P4/governance) is unchanged: the coordinator
  rides the existing promoted/candidate contract and confidence bands.

**Bottom line:** the run integrates not through its current kernel workload but
through its **verification-class registry**. `exact_trace_replay` (proven, dense)
is the right reward oracle to bootstrap the coordinator offline and for
kernel-parity work; `seeded_replication`/`statistical_cross_check` (defined,
undispatched) are where a general LLM coordinator becomes the run's first
stochastic dispatched work. Train on verdicts now; attach settlement when the
autonomous-fire gate is observed.
