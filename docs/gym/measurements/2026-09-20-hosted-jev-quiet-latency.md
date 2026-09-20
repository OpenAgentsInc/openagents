# Hosted Jev on a quiet machine: latency, and a second pass on the same items

[openagents#9393](https://github.com/OpenAgentsInc/openagents/issues/9393)
asked for two things on `support-v2-three-way`: hosted Jev rows beside the
four Kev checkpoints, and a re-timing of those four checkpoints on a machine
running nothing else. The first was done before this record by
[`2026-09-19-reproducing-the-week.md`](2026-09-19-reproducing-the-week.md).
The second needs the Kev weights, which the machine this ran on does not
have. This record is what that machine could measure: the one door it can
reach, hosted Jev, timed on a quiet box with the conditions written down,
plus a second full pass on the 157 open items into the separate store the
issue asked for. The Kev half stays open, and the section at the end says
so.

## The machine

A Linux box with nothing else of ours running. Recorded before the runs
began:

```text
cpu: INTEL(R) XEON(R) PLATINUM 8559C x8
mem: 31 GiB
kernel: 5.15.200
loadavg before the eval pass:    0.58 0.54 0.23
loadavg before the latency sweep: 0.44 0.51 0.23
loadavg after the latency sweep:  0.05 0.32 0.19
```

No `kev-serve`, `lev-serve`, or `lev-bridge` was resident, and no other
agent had a worktree on the machine. The busiest processes were the desktop
compositor and an idle browser, at under 3% CPU each. Every number below is
network time to `https://api.typesafe.ai` plus the service's own time; the
local CPU contributes almost nothing to a hosted call, so the quiet machine
removes one confound rather than measuring the door's hardware.

## What ran

```text
cargo build -p gym --release --bin gym
target/release/gym eval --jev \
    --record crates/gym/results/support-v2-three-way-quiet.jsonl --fit
target/release/gym latency --jev --blocks 8
```

The key came from `TYPESAFE_API_KEY` in the environment. Suite
`support-v2-three-way` at digest `54fbf4137c3de538`, asked as
`support-v2-three-way-v1` at digest `9745b1d9a0f38288`, judged by
`probability-v2` at `gate:5bdfd1423c6c`. The locked partition was not read:
`gym eval` serves the calibration and development partitions, and a hosted
door reading the locked 39 would spend a read this issue did not ask for.
The suite's `exposure` record does not bear on a hosted door, which trained
on nothing here; the ledger admits it, and the read was still not taken.

## Latency, eight blocks

`gym latency` makes eight passes over the same 157 items in the same order
and reports nearest-rank percentiles per pass. 1,256 calls, 130.5 s wall.
Every call was answered; none refused, none lost.

| Block | Calls | Refused | Lost | p50 | p95 | Mean |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 157 | 0 | 0 | 97.8 ms | 159.8 ms | 103.4 ms |
| 1 | 157 | 0 | 0 | 96.2 ms | 154.8 ms | 101.1 ms |
| 2 | 157 | 0 | 0 | 98.7 ms | 159.7 ms | 103.9 ms |
| 3 | 157 | 0 | 0 | 98.2 ms | 153.5 ms | 102.7 ms |
| 4 | 157 | 0 | 0 | 106.2 ms | 168.5 ms | 111.1 ms |
| 5 | 157 | 0 | 0 | 98.3 ms | 169.1 ms | 105.1 ms |
| 6 | 157 | 0 | 0 | 95.1 ms | 143.9 ms | 99.2 ms |
| 7 | 157 | 0 | 0 | 97.4 ms | 164.0 ms | 104.7 ms |

| Statistic | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- |
| p50 | 98.5 ms | 3.3 ms | 3.4% | 95.1 to 106.2 ms |
| p95 | 159.2 ms | 8.4 ms | 5.3% | 143.9 to 169.1 ms |

[`2026-09-19-latency-noise-floor.md`](2026-09-19-latency-noise-floor.md)
named three things a quiet run has to establish. Read against them:

1. **A spread rather than a trend.** The medians run 97.8, 96.2, 98.7,
   98.2, 106.2, 98.3, 95.1, 97.4. Seven of eight sit inside a 3.6 ms band
   and the eighth is 8 ms above it, with no direction over the sweep.
   That is a spread.
2. **A spread that does not rest on one block.** Dropping the widest block
   moves the p50 figure from 3.4% to 1.3% relative and the p95 figure from
   5.3% to 5.1%. The p95 spread stands without its widest block; the p50
   spread is mostly one block, so 3.4% is the conservative figure and 1.3%
   the one seven blocks agree on. Neither is a factor of six.
3. **Whether one relative spread transfers between a fast door and a slow
   one.** Not answerable here. This sweep timed one door, and a hosted
   door's spread is network jitter plus service load, which a local
   checkpoint does not share. A relative floor for `deployment-v1` still
   needs the local doors measured under the same discipline; this record
   gives the hosted column only.

For comparison, the same door's median on the same items the day before
was 186 ms, recorded from a run whose machine was not quiet and whose
route to the service was a different box. The 98 ms here is not evidence
that the service got faster; it is evidence that where a hosted call is
measured from matters as much as when.

## The second pass, and one item that moved

The eval pass wrote 157 rows to
`crates/gym/results/support-v2-three-way-quiet.jsonl`, a store of its own so
the rows sit beside yesterday's rather than replacing them. 157 asked, 157
scored, 0 refused, 0 lost, 16.7 s wall. `gym compare` on the store verifies
the chain and prints:

| Side | Accuracy | ECE | Brier | NLL | Confident errors | Scored | Median latency |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `jev (hosted)` asked as `support-v2-three-way-v1` | 0.930 | 0.051 | 0.048 | 0.164 | 0 | 157 | 100 ms |

Yesterday's pass in `support-v2-three-way.jsonl` scored 0.936 on the same
157 items. The two passes agree on 156 of them. The one they do not is
`severity/031`, a Score item whose truth is level `2`:

| Pass | Distribution | Argmax | Correct |
| --- | --- | --- | --- |
| 2026-09-19 | `{0: 0.02, 1: 0.47, 2: 0.51}` | `2` | yes |
| 2026-09-20 | `{0: 0.02, 1: 0.50, 2: 0.48}` | `1` | no |

The door moved 0.03 of mass between two adjacent levels on an item it was
already near a coin flip on, and the argmax followed. That is the only
pass-to-pass difference in 157 items, and it says two things worth keeping:
hosted Jev is not bit-for-bit deterministic across days, and a 0.006 change
in a suite accuracy can be one near-tie item, which is well inside the
0.056 floor
[`../../lev/measurements/2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)
measured.

`--fit` admitted no map, as it did not yesterday: `routing` fails
`log_loss_does_not_rise` (0.154 to 0.562), and `urgency` and `severity`
are `unverifiable` at 24 and 15 fitted items against a floor of 30.

## What this does to the Kev-against-Jev gap

[`../../kev/measurements/2026-09-19-variant-scores.md`](../../kev/measurements/2026-09-19-variant-scores.md)
states `kev-8b`'s gap to hosted Jev as 0.057, one floor, 12 discordant items
to 3, paired exact *p* = 0.035. Against today's pass the same arithmetic,
computed over the two stores by item ID and split, reads:

| Jev pass | Jev | `kev-8b` | Difference | Floors of 0.056 | Split of the items only one got right | Exact *p* |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-09-19 | 0.936 | 0.879 | +0.057 | 1.0 | 12 to 3 | 0.035 |
| 2026-09-20 | 0.930 | 0.879 | +0.051 | 0.9 | 12 to 4 | 0.077 |

One near-tie item moved the paired test from one side of 0.05 to the other.
The honest reading is the one the floor gives on both days: the gap is about
one floor, in Jev's favor on every pass, and a single 157-item pass cannot
resolve it. The paired test is the tighter instrument, but two passes of the
same door disagree about whether it clears 0.05, so neither pass alone
should be quoted as settling it. The `kev-8b` rows are unchanged and predate
the question-set digest, so `gym compare` still refuses to call the pair a
judged comparison; the numbers above are computed beside the store, not
read off a gate.

## What stays open

- **The four Kev doors are not re-timed.** They need `~/work/kev-artifacts/`,
  which this machine does not have. The contended medians in the variant
  record — 204, 203, 1,302, and 2,675 ms — remain the only Kev latencies on
  this suite, and the "settles nothing" reading of them stands. The command
  to run, on a quiet machine with the weights, is the one the issue gives:
  `gym eval --door kev-0.5b=http://127.0.0.1:8101 --record
  crates/gym/results/support-v2-three-way-quiet.jsonl --fit`, once per
  checkpoint, into the store this record started.
- **`deployment-v1`'s `latency_block_sigma_relative` is still
  `unmeasured`.** A hosted door's 3.4% is one column of the floor, not the
  floor; the rule applies one relative figure to every door, and this run
  cannot say whether a local checkpoint's spread on a quiet machine looks
  anything like it.
- **The locked partition is unread** by any door in the quiet store.
