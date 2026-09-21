# Fixed waves against completion-driven refill and separate resource lanes

This is a deterministic simulation measurement, not live evidence. Every
number below comes from `crates/coder-scheduler`'s discrete-event
simulation over declared tick estimates — no executor ran, no wall clock
was read, and no delegate session was opened. The results compare
scheduling policies against each other on the same pinned inputs. They
say nothing about live Devin speed, provider latency, or real host
contention; the live evidence for those stays in the
[project supervisor verification](../verification/2026-09-21-project-supervisor.md)
and the [observed fan-out episode](2026-09-20-observed-fanout.md).

## What was simulated

`simulate::run_lanes` drives a sealed catalog through the real
`plan::select` on two independent axes:

- **Fill** — `waves` admits a wave and replans only when the wave's
  slowest task finishes; `refill` replans at every completion.
- **Lanes** — `separate` bounds admission on every declared resource
  dimension (executor slots, CPU units, memory, the integration lane);
  `session-count` bounds only executor slots, the shape of a scheduler
  that counts sessions and nothing else. The quiet-host rule stays in
  force under both — it is a scheduling rule, not a quantity.

Review is not modeled: a completion counts as `completed`, so the
report's `accepted` field is the scheduled count restated in the model's
own terms. Fields the simulation cannot observe — `gate_ticks`,
`failures`, `retries`, `spend` — record `unknown`, never zero. The host
declares 64 CPU units, 65,536 MiB of memory, and one integration lane;
the run sweeps executor slots 2, 4, and 8.

## The pinned workloads

Three fixture catalogs in `simulate::fixtures`, each sealed so the report
names its workload by digest:

| Workload | Digest | Shape |
| --- | --- | --- |
| `chain` | `sha256:5554ed8cd76d7e93db32ec7c570636d6b6157596c277861548a06b7db4c6d636` | Six tasks in one dependency chain, ticks 2+3+1+4+2+3 = 15. |
| `fan-out` | `sha256:e7bc1993cb56287d94e234f389043ee48bf443fd16d562e0a76975fbb18aa47c` | A 2-tick root, eight skewed leaves (one 8-tick, seven 1-tick), a 1-tick join. |
| `mixed-dag` | `sha256:2d375c22d29b55ee68167a4aad6326856b989e484ec2322584aaa88e881b79ff` | Twelve tasks: a dependency diamond, a shared-read task against live writes, a write-write conflict over `docs/roll-up.md`, two integration-lane tasks, an 8-CPU/8-GiB build, a quiet-host measurement gated on the diamond, and `upstream-fix` on the externally completed `external:vendor-1`. |

The digests are pinned by a test, so a fixture edit repins the workload
and must re-measure rather than inherit these numbers.

## Results

`scheduled` was every task in every row — nothing was refused, no task
was left unscheduled. `rounds` counts plan rounds.

### `chain` — serial by construction

| Slots | Fill | Lanes | Makespan | Rounds | Mean wait | Max wait |
| ---: | --- | --- | ---: | ---: | ---: | ---: |
| 2, 4, 8 | waves | separate | 15 | 7 | 0.00 | 0 |
| 2, 4, 8 | refill | separate | 15 | 7 | 0.00 | 0 |

Session-count rows are identical — the chain holds at most one slot and
never touches a resource lane. On fully serial work the policies tie, as
they must.

### `fan-out` — where a stranded wave slot costs

| Slots | Fill | Makespan | Rounds | Mean wait | Max wait | Peak slots |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 2 | waves | 14 | 7 | 5.40 | 10 | 2 |
| 2 | refill | 11 | 11 | 2.10 | 6 | 2 |
| 4 | waves | 12 | 5 | 3.20 | 8 | 4 |
| 4 | refill | 11 | 7 | 0.50 | 2 | 4 |
| 8 | waves | 11 | 4 | 0.00 | 0 | 8 |
| 8 | refill | 11 | 5 | 0.00 | 0 | 8 |

Lane-model rows are identical — every leaf declares only an executor
slot. Refill wins at widths 2 and 4 because a freed slot takes the next
leaf instead of waiting for the 8-tick task. At width 8 the wave holds
all eight leaves at once and the policies tie. Waves' only win is
fewer plan rounds — 7 against refill's 11 at width 2.

### `mixed-dag` — where the lanes are the story

| Slots | Fill | Lanes | Makespan | Rounds | Mean wait | Max wait | Peak slots | Peak int |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | waves | separate | 29 | 8 | 8.83 | 27 | 2 | 1 |
| 2 | waves | session-count | 27 | 8 | 8.33 | 25 | 2 | 2 |
| 2 | refill | separate | 25 | 11 | 7.33 | 21 | 2 | 1 |
| 2 | refill | session-count | 25 | 11 | 7.33 | 21 | 2 | 1 |
| 4 | waves | separate | 20 | 5 | 2.00 | 9 | 4 | 1 |
| 4 | waves | session-count | 20 | 5 | 2.00 | 9 | 4 | 1 |
| 4 | refill | separate | 16 | 9 | 1.50 | 7 | 4 | 1 |
| 4 | refill | session-count | 15 | 9 | 1.33 | 7 | 4 | 2 |
| 8 | waves | separate | 19 | 5 | 1.17 | 7 | 6 | 1 |
| 8 | waves | session-count | 19 | 5 | 0.58 | 7 | 7 | 2 |
| 8 | refill | separate | 15 | 8 | 0.42 | 3 | 6 | 1 |
| 8 | refill | session-count | 15 | 7 | 0.17 | 2 | 7 | 2 |

CPU and memory peaks reached 8 units and 8,192 MiB — the heavy build —
under every policy, and the quiet-host lane was held in every row.

## What the numbers say

Refill never lost to waves on makespan or queue wait in any run of any
fixture. It tied on serial work (`chain`, every width) and on
uncontended work (`fan-out` at 8 slots, where the wave already holds
everything), and it won wherever skewed durations strand a slot behind a
wave's slowest task — 3 ticks on `fan-out` at width 2, 4 on `mixed-dag`
at every width under separate lanes. Waves' only edge is structural:
fewer plan rounds (5 against 9 on `mixed-dag` at width 4), which matters
only if replanning itself has a cost the simulation does not model.

The lane comparison cuts the other way, and honestly so. Session-count
never lost makespan either, and on `mixed-dag` it *won* — 27 against 29
under waves at width 2, 15 against 16 under refill at width 4 — by doing
exactly what separate lanes exist to refuse: it ran two integration
tasks through a one-lane host (peak `integration_lanes` 2 against a
declared 1). On a dedicated check — three 4-GiB tasks on an 8-GiB host —
the same policy peaked at 12,288 MiB held against 8,192 declared, and
"won" its makespan by half. That speed is the contention. Separate lanes
cost at most 2 ticks on these workloads and bounded every declared
dimension at its declared value.

## Limits

The ticks are declared estimates, not durations; a policy that wins here
wins under the model, not under a wall clock. The simulation has no
executor, so `failures` and `retries` are `unknown` rather than zero; it
has no reviewer, so `gate_ticks` is `unknown`; it has no price list, so
`spend` is `unknown`. `accepted` restates completions because review is
not modeled. No row shows live throughput, real contention cost, or a
safe live concurrency bound — those need the bounded live runs the
supervisor verification records separately.

## Reproduce

```sh
cargo test -p coder-scheduler print_fixture_benchmark -- --ignored --nocapture
```

The table is deterministic: same fixtures, same host declaration, same
bytes. `cargo test -p coder-scheduler` covers the fixture digests, the
determinism, the hand-computed small graph, the lane-oversubscription
checks, and the `unknown` fields.
