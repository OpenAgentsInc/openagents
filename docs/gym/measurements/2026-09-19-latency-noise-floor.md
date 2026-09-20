# Latency's noise floor, and why this machine could not measure it

`deployment-v1` gates a door on its 95th-percentile latency. Before it can
refuse anything on a clock, somebody has to measure how much that clock moves
when nothing about the door changed — the same question
[`2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)
answered for accuracy, asked about wall time.

This is the record of that attempt. **It does not produce a floor.** The
machine was running nine agents against several copies of the same on-device
model, and what came back is a measurement of the contention rather than of
the doors. The gate therefore carries
`latency_block_sigma_relative` as `unmeasured`, every latency criterion
reports `unverifiable`, and no door is refused on a clock yet.

## What a block is here

A *block* is one pass over the same items against the same door. It is the
wall-clock analogue of a seed block: the door, the items, and the item order
are held fixed, so anything that moves between blocks is the machine. The
sweep alternates which door runs first in each block, so a machine that
drifts over the run does not hand one door the quiet half of it.

The tool is `gym latency`, added with this record:

```text
cargo build --release -p gym --bin gym
target/release/kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.5b \
    --base-dir ~/work/kev-artifacts/qwen2.5-0.5b --port 8009
target/release/gym latency --door kev-0.5b=http://127.0.0.1:8009 --blocks 8
target/release/gym latency --door kev-4b=http://127.0.0.1:8009 \
    --blocks 2 --partition development
```

Percentiles are nearest rank: the `p`th percentile of `n` sorted calls is the
call at position `ceil(p * n / 100)`. Nearest rank never interpolates, so
every number below is a call somebody waited through. A call the door
declined is timed and counted as a refusal; a call the harness never got an
answer out of is dropped from both, which is the line
[`gym::eval::classify`](../../../crates/gym/src/eval.rs) draws and
openagents#9369 landed.

## `kev-0.5b`, 157 items, eight blocks

| Block | Calls | Refused | Lost | p50 | p95 | Mean |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 157 | 0 | 0 | 202.8 ms | 280.2 ms | 222.0 ms |
| 1 | 157 | 0 | 0 | 230.5 ms | 373.9 ms | 246.1 ms |
| 2 | 157 | 0 | 0 | 237.7 ms | 262.6 ms | 231.4 ms |
| 3 | 157 | 0 | 0 | 240.6 ms | 268.8 ms | 233.9 ms |
| 4 | 157 | 0 | 0 | 243.8 ms | 269.9 ms | 238.8 ms |
| 5 | 157 | 0 | 0 | 244.7 ms | 293.4 ms | 246.6 ms |
| 6 | 157 | 0 | 0 | 249.6 ms | 296.3 ms | 248.0 ms |
| 7 | 157 | 0 | 0 | 263.8 ms | 1219.1 ms | 392.0 ms |

| Statistic | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- |
| p50 | 239.2 ms | 17.6 ms | 7.4% | 202.8 to 263.8 ms |
| p95 | 408.0 ms | 329.6 ms | 80.8% | 262.6 to 1219.1 ms |

## `kev-4b`, 78 items, two blocks

| Block | Calls | Refused | Lost | p50 | p95 | Mean |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 78 | 0 | 0 | 1412.7 ms | 2343.7 ms | 1526.0 ms |
| 1 | 78 | 0 | 0 | 1775.8 ms | 4080.3 ms | 1997.3 ms |

## Why none of that is a floor

Three things in those tables say the machine moved, and a floor derived from
a moving machine is a record of one afternoon wearing a digest.

**The median rose in every block.** 202.8, 230.5, 237.7, 240.6, 243.8, 244.7,
249.6, 263.8. Eight blocks, one direction, a 30% rise from first to last over
about five minutes. A noise floor is a spread around a value; this is a trend,
and a trend means the thing being held fixed was not.

**One block's p95 is four times its neighbours'.** Block 7 reports 1,219 ms
against a run where every other block sits between 262 and 374. One call that
waits behind something else on a shared machine lands in the tail by
definition, and the tail is exactly what the percentile reads. That single
block carries almost the whole 80.8% standard deviation: drop it and the
remaining seven have a relative spread near 13%. Choosing which of those two
numbers to publish is not a measurement, it is a preference.

**Both doors were slower than their own record.** The four-way table in
[`docs/lev/disposition.md`](../../lev/disposition.md) puts `kev-0.5b` at about
180 ms and `kev-4b` at about a second. This sweep found 203 to 264 ms and
1,413 to 1,776 ms for the same doors on the same machine — 30% and 60% above
what the quiet machine recorded. The doors did not change.

## What else was running

Nine agents were working in this repository at the same time, several of them
driving the same Apple on-device model through `lev-serve` and its Swift
bridge helpers, and others driving `kev-serve`. The independent evidence:

- Another agent reported
  `crates/lev/tests/conformance.rs::an_unmodified_jev_client_round_trips_all_three_question_types`
  timing out against a 10-second client timeout on two consecutive runs, and a
  single-helper bridge killed mid-request during a smoke test, with nothing in
  its own change touching that path.
- The port this sweep used was itself contended. Partway through, another
  agent's `kev-serve` was running on port 8009, started by a command that
  begins `lsof -ti :8009 | xargs kill`. A sweep that points at a port is not
  guaranteed to be pointing at the door it started against.

The ~1,600 ms figure the issue quotes for Lev was measured on a quiet machine.
Nothing measured over these hours is comparable with it, and a floor derived
here would be wrong by construction rather than by optimism.

## What `deployment-v1` does instead

`latency_block_sigma_relative` is `unmeasured`, so:

- Every latency criterion reports `unverifiable`, including for a door seven
  times over a stated ceiling. The gate does not know how wide its own band
  is, and a band it guessed at would refuse doors for whatever else the
  machine was running.
- The gap travels with the rule. `pending_measurement` is inside the digest,
  so filling the floor produces `deployment-v2` and leaves every verdict
  already recorded under `deployment-v1` readable as what it was.
- The cost and refusal criteria are unaffected and decide normally. Metering
  is a count rather than a clock, and a contended machine does not change what
  a decision is billed at.

## What would fill it

Run the same sweep on a machine running nothing else:

```text
target/release/gym latency --door <name>=<url> --blocks 8
```

Three things the run has to establish, none of which this one did:

1. **A spread rather than a trend.** Blocks in a random order of magnitude
   around one value, not a staircase. If the median still climbs, the machine
   is still moving and the sweep is still measuring it.
2. **A spread that does not rest on one block.** Report the figure with and
   without the widest block. If they disagree by a factor of six, as they do
   above, eight blocks are not enough.
3. **Whether one relative spread transfers between a fast door and a slow
   one.** `deployment-v1` applies a single relative figure to a 180 ms door
   and a 1,600 ms door. If their spreads differ, the rule is substituting, and
   it would have to say so in every detail line the way `ab.rs` does for its
   family guard — or carry a floor per door.

Until then, the honest reading of any latency number in this repository is
that it is one draw on one machine on one afternoon, and that nobody has said
how wide the interval around it is.

## Filled, 2026-09-20

[`2026-09-20-hosted-jev-quiet-latency.md`](2026-09-20-hosted-jev-quiet-latency.md)
ran the sweep for hosted Jev and
[`2026-09-20-kev-quiet-latency.md`](2026-09-20-kev-quiet-latency.md) for
`kev-0.5b` and `kev-0.6b`, 16 blocks each on a quiet CPU-only host. The
p95 spreads were 5.3%, 16.8%, and 14.3%; `deployment-v2` carries 0.17. The
third test above is answered between the two local doors and between hosted
and local, and still open for a door that answers in seconds.
