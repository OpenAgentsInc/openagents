# Kev on a quiet machine: the latency floor, a p95 per door, and the ranking with time in it

Three issues left latency boxes open for want of Kev weights on a quiet
machine.
[openagents#9393](https://github.com/OpenAgentsInc/openagents/issues/9393)
asked for the Kev doors re-timed with the load written down;
[openagents#9382](https://github.com/OpenAgentsInc/openagents/issues/9382)
asked for latency's own noise floor before anything is gated on it, a p95
per door, and an answer to whether the ranking changes when time is a
criterion; and
[`2026-09-20-hosted-jev-quiet-latency.md`](2026-09-20-hosted-jev-quiet-latency.md)
measured the hosted column of that floor and said the local column still
needed doing. This record is the local column: `kev-0.5b` and `kev-0.6b`
served by `kev-serve` on CPU, on a machine running nothing else of ours, in
two sweeps of eight blocks each, plus one recorded eval pass per door into
the quiet store so each has a p95.

`kev-4b` and `kev-8b` are not here. The `kev-4b` base fetch stalled at
5.4 GB of its second shard for an hour and was stopped, and `kev-8b` was
not attempted; their only
latencies remain the contended rows of
[`../../kev/measurements/2026-09-19-variant-scores.md`](../../kev/measurements/2026-09-19-variant-scores.md).
No Lev door is here either: Lev needs Apple hardware, which this machine is
not.

## The machine

CPU only. No GPU, no Metal, no accelerator of any kind; `kev-serve` ran fp32
on the host's cores.

```text
cpu: INTEL(R) XEON(R) PLATINUM 8559C, 8 cores, 1 thread per core, KVM guest
mem: 31 GiB
kernel: 5.15.200 (Linux)
loadavg before sweep 1: 0.05 0.13 0.34   (2026-09-20T15:00:28Z)
loadavg after sweep 1:  3.69 3.39 2.14   (2026-09-20T15:12:34Z)
loadavg before sweep 2: 1.27 2.61 2.25   (2026-09-20T15:21:28Z)
loadavg after sweep 2:  3.20 3.11 2.76   (2026-09-20T15:35:05Z)
```

The load averages after each sweep are the sweep: a `kev-serve` answering
one request at a time on the host's cores, and nothing else of ours. No
`lev-serve`, `lev-bridge`, or other agent's worktree was on the machine.
The eval passes recorded below ran between the two sweeps, from 15:15Z to
15:20Z, and the second sweep started with the one-minute load average still
carrying them. Two things a quiet load average does not show, both recorded
here because they bear on the spread:

- **Two doors were resident.** Both servers were up for the whole sweep,
  each holding its weights in memory. `gym latency` asks one door at a
  time, so only one was computing at any moment, but both were present and
  block order alternated between them.
- **The host is a virtual machine.** Under KVM the hypervisor can take
  cycles from a guest at times of its choosing. Steal time was not read
  during the sweeps, so how much of the spread below is the hypervisor is
  not known; it is the confound a local CPU door has that a hosted door does
  not, and the figure includes it.

## What ran

The tree was at `04c4b4e6b`. Weights came from
`./scripts/fetch-kev-artifacts.sh kev-0.5b kev-0.6b` into
`~/repos/kev-artifacts/`, outside git.

```text
cargo build -p kev --features serve --release --bin kev-serve
cargo build -p gym --release --bin gym

kev-serve --adapter-dir $A/kev-0.5b --base-dir $A/qwen2.5-0.5b \
    --default kev-0.5b --port 8101
kev-serve --adapter-dir $A/kev-0.6b --base-dir $A/qwen3-0.6b \
    --default kev-0.6b --port 8102

gym latency --door kev-0.5b=http://127.0.0.1:8101 \
    --door kev-0.6b=http://127.0.0.1:8102 --blocks 8          # twice

gym eval --door kev-0.5b=http://127.0.0.1:8101 \
    --record crates/gym/results/support-v2-three-way-quiet.jsonl --fit --timeout 60
gym eval --door kev-0.6b=http://127.0.0.1:8102 \
    --record crates/gym/results/support-v2-three-way-quiet.jsonl --fit --timeout 60
```

`--default` matters: without it a server advertises itself as `kev-latest`
and refuses a request that names the checkpoint, which is how the first
attempt at this sweep ended. Suite `support-v2-three-way` at digest
`54fbf4137c3de538`, asked as `support-v2-three-way-v1` at digest
`9745b1d9a0f38288`, 157 calibration and development items in the same order
every block. The locked partition was not read.

## The sweeps

Each sweep is eight passes over the 157 items per door, door order
alternating by block, nearest-rank percentiles per pass. 2,512 calls per
sweep, 5,024 in all; every one answered, none refused, none lost.

### `kev-0.5b`

| Sweep | Block | p50 | p95 | Mean |
| --- | --- | --- | --- | --- |
| 1 | 0 | 199.7 ms | 262.4 ms | 197.6 ms |
| 1 | 1 | 312.6 ms | 368.1 ms | 312.6 ms |
| 1 | 2 | 317.5 ms | 379.0 ms | 313.3 ms |
| 1 | 3 | 265.1 ms | 332.4 ms | 263.4 ms |
| 1 | 4 | 286.7 ms | 349.5 ms | 285.2 ms |
| 1 | 5 | 244.7 ms | 301.6 ms | 249.6 ms |
| 1 | 6 | 269.1 ms | 329.1 ms | 262.9 ms |
| 1 | 7 | 280.5 ms | 327.7 ms | 275.2 ms |
| 2 | 0 | 275.7 ms | 347.1 ms | 276.0 ms |
| 2 | 1 | 298.6 ms | 443.4 ms | 309.9 ms |
| 2 | 2 | 344.0 ms | 538.0 ms | 375.9 ms |
| 2 | 3 | 291.6 ms | 370.3 ms | 293.5 ms |
| 2 | 4 | 266.9 ms | 356.3 ms | 267.1 ms |
| 2 | 5 | 301.1 ms | 367.1 ms | 301.6 ms |
| 2 | 6 | 304.8 ms | 361.8 ms | 302.0 ms |
| 2 | 7 | 265.9 ms | 352.6 ms | 263.7 ms |

| Statistic | Blocks | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- | --- |
| p50, sweep 1 | 8 | 272.0 ms | 37.9 ms | 13.9% | 199.7 to 317.5 ms |
| p95, sweep 1 | 8 | 331.2 ms | 37.0 ms | 11.2% | 262.4 to 379.0 ms |
| p50, sweep 2 | 8 | 293.6 ms | 25.5 ms | 8.7% | 265.9 to 344.0 ms |
| p95, sweep 2 | 8 | 392.1 ms | 66.3 ms | 16.9% | 347.1 to 538.0 ms |
| p50, both | 16 | 282.8 ms | 33.1 ms | 11.7% | 199.7 to 344.0 ms |
| p95, both | 16 | 361.6 ms | 60.7 ms | **16.8%** | 262.4 to 538.0 ms |

### `kev-0.6b`

| Sweep | Block | p50 | p95 | Mean |
| --- | --- | --- | --- | --- |
| 1 | 0 | 268.6 ms | 361.1 ms | 270.9 ms |
| 1 | 1 | 343.2 ms | 430.1 ms | 340.0 ms |
| 1 | 2 | 290.0 ms | 401.8 ms | 302.5 ms |
| 1 | 3 | 320.3 ms | 378.6 ms | 310.4 ms |
| 1 | 4 | 317.7 ms | 393.7 ms | 316.7 ms |
| 1 | 5 | 312.5 ms | 416.0 ms | 303.8 ms |
| 1 | 6 | 321.7 ms | 378.1 ms | 314.9 ms |
| 1 | 7 | 307.0 ms | 381.2 ms | 302.7 ms |
| 2 | 0 | 377.7 ms | 492.9 ms | 375.5 ms |
| 2 | 1 | 369.9 ms | 535.7 ms | 370.8 ms |
| 2 | 2 | 377.1 ms | 592.7 ms | 386.4 ms |
| 2 | 3 | 328.9 ms | 421.1 ms | 336.1 ms |
| 2 | 4 | 341.6 ms | 427.6 ms | 338.1 ms |
| 2 | 5 | 331.8 ms | 430.1 ms | 332.6 ms |
| 2 | 6 | 343.7 ms | 430.4 ms | 343.6 ms |
| 2 | 7 | 330.9 ms | 416.5 ms | 334.5 ms |

| Statistic | Blocks | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- | --- |
| p50, sweep 1 | 8 | 310.1 ms | 22.5 ms | 7.2% | 268.6 to 343.2 ms |
| p95, sweep 1 | 8 | 392.6 ms | 22.6 ms | 5.7% | 361.1 to 430.1 ms |
| p50, sweep 2 | 8 | 350.2 ms | 21.2 ms | 6.1% | 328.9 to 377.7 ms |
| p95, sweep 2 | 8 | 468.4 ms | 65.6 ms | 14.0% | 416.5 to 592.7 ms |
| p50, both | 16 | 330.2 ms | 29.6 ms | 9.0% | 268.6 to 377.7 ms |
| p95, both | 16 | 430.5 ms | 61.4 ms | **14.3%** | 361.1 to 592.7 ms |

## Read against the three tests

[`2026-09-19-latency-noise-floor.md`](2026-09-19-latency-noise-floor.md)
named three things a quiet sweep has to establish before its spread can be a
floor.

1. **A spread rather than a trend.** Neither door drifts in one direction.
   `kev-0.5b`'s medians in sweep 1 go up, down, up, down, up, and its first
   block is its fastest by 45 ms, which is the first pass through a cold
   server and not a trend. `kev-0.6b`'s sweep 2 is its slowest three blocks
   followed by five within 15 ms of each other. Sweep 2 as a whole sits
   above sweep 1 for both doors, by 22 ms at the median for `kev-0.5b` and
   40 ms for `kev-0.6b`, and this record cannot say why: the eval passes
   in between had finished, and steal was not read. It is a level shift
   between sweeps, not a slope within one, and it is inside the 16-block
   figure.
2. **A spread that does not rest on one block.** Dropping the widest block
   from each door's 16 moves `kev-0.5b`'s relative p95 spread from 16.8% to
   11.3% and `kev-0.6b`'s from 14.3% to 10.8%. Both stand above the hosted
   door's 5.3% without their widest block. The widest blocks are real
   blocks of a real quiet machine, so they stay in the figure; the
   without-them numbers say how much of the figure is one bad half-minute.
3. **Whether one relative spread transfers between a fast door and a slow
   one.** Between the two local doors, yes within 2.5 points: 16.8% and
   14.3% for doors whose p95s differ by 70 ms. Between hosted and local, no:
   hosted Jev's p95 moved 5.3% on the same items from the same host, a third
   of the local figure, because a hosted call's wall clock is network plus
   service and a local call's is this machine's cores and whatever the
   hypervisor takes from them. Between these doors and a door that answers
   in seconds — `kev-4b`, `kev-8b`, Lev — not measured, and this record
   does not claim it.

## The floor, and the gate that carries it

`deployment-v2` at `crates/gym/gates/deployment-v2.json` carries
`latency_block_sigma_relative` as `0.17`, basis `derived`, evidence this
record and the hosted-Jev record: the widest of the three measured relative
p95 spreads, rounded up. Its digest is
`gate:c55849811d8cf6fd3aaf9bd2363d325bedf4e272d44b45b982c7e856f0664dc6`.
`deployment-v1` is unchanged and still `unmeasured`, because a rule whose
floor moved from nothing to a number is a different rule, and its `$comment`
still says why the first attempt did not count.

What the floor does: under `regression_sigmas` of 2, a candidate's p95 has
to move 34% against the baseline's before the gate calls the move the door
rather than the machine. Why the widest rather than the mean: a floor too
narrow refuses doors for the weather, which is the failure the noise-floor
record warned about, and a floor too wide only fails to separate doors a
longer sweep could. The gate keeps a `pending_measurement` for the doors
that answer in seconds, naming #9382, so that a verdict involving one of
them says its floor was measured on faster doors.

## A p95 per door

Nearest-rank over each door's 157 recorded rows on the 157 open items. The
quiet store is `crates/gym/results/support-v2-three-way-quiet.jsonl`; the
contended store is `support-v2-three-way.jsonl`, and its rows are the
published numbers, kept beside the quiet ones rather than replaced.

| Door | Store | Conditions | Accuracy | p50 | p95 | Refused | Cost |
| --- | --- | --- | --- | --- | --- | --- | --- |
| hosted Jev | quiet | quiet host, network to the service | 0.930 | 100 ms | 177 ms | 0 | metered |
| `kev-0.5b` | quiet | this record, CPU only | 0.713 | 329 ms | 407 ms | 0 | `unmetered_local_lane` |
| `kev-0.6b` | quiet | this record, CPU only | 0.675 | 337 ms | 448 ms | 0 | `unmetered_local_lane` |
| `kev-4b` | contended | load 13 to 19, nine agents | 0.745 | 1,302 ms | 2,256 ms | 0 | `unmetered_local_lane` |
| `kev-8b` | contended | load 13 to 19, nine agents | 0.879 | 2,675 ms | 3,971 ms | 0 | `unmetered_local_lane` |
| `lev-base` | contended | Apple hardware, another day | 0.783 | 1,494 ms | 1,612 ms | 0 | `unmetered_local_lane` |
| `lev-adapted@1` | contended | Apple hardware, 8 samples across 4 helpers | 0.943 | 20,016 ms | 30,993 ms | 0 | `unmetered_local_lane` |

The Kev accuracies in the quiet store are identical to the contended
store's, item for item: greedy decoding on a fixed input gives the same
answer at any load. Their medians are not: 329 ms here against 204 ms on
2026-09-19 for `kev-0.5b`. The 2026-09-19 host was a different machine, and
a latency is a property of a door on a host, not of a checkpoint. That is
the reason the table names the conditions in every row.

## Does the ranking change when time is a criterion?

**Yes, at the top, and this record can now show one side of it.**

On accuracy alone, against the 0.056 floor
[`../../lev/measurements/2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)
measured for a two-door comparison: `lev-adapted@1` at 0.943 and hosted
Jev at 0.930 are 0.013 apart, a quarter of a floor, and tied. `kev-8b` at
0.879 is 0.051 behind Jev, about one floor, which
[`2026-09-20-hosted-jev-quiet-latency.md`](2026-09-20-hosted-jev-quiet-latency.md)
found the paired test could not settle across two Jev passes. `kev-4b`,
`kev-0.5b`, and `kev-0.6b` follow; the last two are 0.038 apart, 18
discordant items to 12, paired exact *p* = 0.36, and tied.

With time as a criterion:

- **Jev separates from `lev-adapted@1`.** Jev's p95 is 177 ms on a quiet
  host; Lev's is 30,993 ms on its published, contended rows. Under
  `deployment-v2` the band around Jev's p95 is 60 ms, and no contention
  correction turns 31 seconds into 237 ms. The two doors that tie on
  accuracy do not tie on time, so the door at the top changes from "either"
  to Jev. This is the pair the ranking question was asked about, and it is
  the one place the answer is yes on evidence rather than on a guess.
  Lev's side is still the published number: nobody has swept a Lev door on
  a quiet Apple machine, and this host cannot.
- **`kev-0.5b` and `kev-0.6b` stay tied.** 407 and 448 ms, 41 ms apart
  against a band of 138 ms around the smaller. Time does not separate the
  two doors that accuracy could not.
- **Jev separates from both small Kev doors on time as well as accuracy.**
  229 ms and 271 ms above Jev's p95, four bands. On this host a local
  0.5B door is not faster than the hosted service; the table
  `deployment-v1`'s `$comment` quotes, with `kev-0.5b` at 180 ms and Jev at
  250, was measured on other machines and does not hold here.
- **`kev-8b` against `kev-4b` is a budget question, not a ranking one.**
  `kev-8b` wins 0.134 of accuracy, more than two floors; it loses 1.7 s of
  p95 on contended numbers that this record could not re-time. Whether the
  order flips depends on the ceiling a workload states, and the gate keeps
  that ceiling out of the rule on purpose: without a `Budget`, the criterion
  is `unverifiable`, and that is the right verdict here.

The record that first asked this,
[`2026-09-19-deployment-ranking.md`](2026-09-19-deployment-ranking.md), said
yes by medians and could certify nothing, because there was no p95 and no
floor. There is now a floor, a p95 for every door in the store, and one
pair — Jev against Lev-adapted — where the ranking demonstrably changes.
`gym compare` does not yet read `deployment-v2` against store rows: the
gate judges a `Deployment` profile that no command builds from a store, so
the bands above were computed from the gate's rule beside the store, not
read off a verdict.

## What stays open

- **Lev's p95 on a quiet machine.** Apple hardware, which this host is not.
  Until it exists, the Jev-over-Lev result rests on Jev's quiet p95 and
  Lev's published contended one, and the gap is two orders of magnitude.
- **`kev-4b` and `kev-8b` on a quiet machine.** The weights did not finish
  fetching here. The first is the door the model card recommends serving;
  its quiet p95 is the number a router budget would actually be read
  against.
- **The floor for doors that answer in seconds.** `deployment-v2`'s pending
  measurement. The figure it carries was measured on doors between 100 and
  450 ms.
- **The hypervisor's share of the spread.** A run on bare metal, or with
  steal time read across the sweep, would say how much of the 17% is the
  machine rather than the door.
