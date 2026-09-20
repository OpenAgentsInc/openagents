# The first run that left a record

`gym eval` against a live `lev-serve` door, 2026-09-19. The first run in this
repository whose result is a file rather than a table in a document: 157 rows
in `crates/gym/results/support-v2-three-way.jsonl`, receipt-chained, one row
per item.

That store has since grown. Four Kev doors appended 157 rows each on the same
day, so the file now holds 785 rows over five doors; see
[`../../kev/measurements/2026-09-19-variant-scores.md`](../../kev/measurements/2026-09-19-variant-scores.md).
The rows described below are unchanged, which is the property the chain
exists to give.

```text
./scripts/build-lev-bridge.sh
cargo run -p lev --features serve --release --bin lev-serve -- --port 11448
LEV_OS_BUILD=25E246 cargo run -p gym --release --bin gym -- eval \
    --door lev-base=http://127.0.0.1:11448 \
    --fit \
    --record crates/gym/results/support-v2-three-way.jsonl \
    --records crates/lev/calibration
```

Suite `support-v2-three-way` at digest `54fbf4137c3de538`: 196 items, 79
calibration, 78 development, 39 locked and unread. Judged by
`probability-v1`, digest `gate:368cefd18f30`.

Door `lev-base`, base model signature `9799725`, no adapter, `l2` over 8
samples from seed block 0, macOS build 25E246.

## What the door did

157 items asked: 157 scored, 0 refused by the door, 0 lost to the harness.

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| calibration, raw | 0.78 | 0.149 | 0.158 | 2.334 | 6 | 79 |
| development, raw | 0.78 | 0.127 | 0.164 | 2.372 | 6 | 78 |

The two partitions agree to two decimal places on accuracy, which is what a
partitioning drawn family by family is supposed to produce.

**Agreement ceiling, added 2026-09-20.** A second reader, labelling blind, agreed with the stored labels on `routing` 50/51 (0.980, kappa 0.971), `urgency` 29/31 (0.935, kappa 0.870), and `severity` 18/18 (1.000, kappa 1.000), on a 100-item sample of these same 196 items. Read each family accuracy against that ceiling, not against 1.0; the intervals and the three disputed items are in [`../decision-models/2026-09-20-instrument-validity.md`](../decision-models/2026-09-20-instrument-validity.md).

## What the maps did

One map per family, fitted on the calibration partition and scored on
development. Fitting and scoring never share an item.

| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | Mapped Brier | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 40 | 0.156 | 0.107 | 3.493 | 0.413 | 0.137 | 0.121 | passed |
| `urgency` | 24 | 0.146 | 0.032 | 1.598 | 0.606 | 0.201 | 0.208 | unverifiable |
| `severity` | 15 | 0.116 | 0.222 | 0.493 | 0.761 | 0.177 | 0.253 | unverifiable |

**`routing` is admitted**, and it is the first map in this repository that a
door can actually serve. Log loss falls from 3.493 to 0.413 and the five
confident errors on the development partition become none.

It is worth saying what the map is. All 40 calibration items landed in one
bin — the estimator reported 1.00 on 32 of them and never below 0.50 — so the
fitted table has one populated bin, and it answers 0.768 for every routing
item whatever the raw signal said. That number is the calibration partition's
own rate, 31 of 40, with Jeffreys smoothing. The map is a constant.

So it buys no resolution, because there was none in the raw signal to keep,
and the argmax is untouched: accuracy is 0.875 on development either way.
What it buys is the difference between a door that says 1.00 and one that
says 0.768 about answers that are right 87.5% of the time. The mapped ECE of
0.107 is exactly that remaining gap, and it is in the safe direction — the
map is now a little under-confident, on a family where the raw signal was
certain and wrong five times.

That is the honest reading of a large log-loss win. The raw number was not a
probability; replacing it with a base rate is an improvement mostly because
what it replaced was so far off.

**`urgency` is not refused, it is unverifiable.** Its ECE falls further than
`routing`'s — 0.146 to 0.032 — on 24 fitted items against a floor of 30.
Under the rule this gate replaced that reads as a refusal, and a reader would
conclude the map lost. It did not lose; nobody can tell yet. Twenty-four
items is six short of the smallest split that fills the coarsest table the
map is allowed to build.

**`severity` moved the wrong way**, and on 15 items that is also
unverifiable. ECE 0.116 to 0.222 on a family whose raw signal was already the
best calibrated of the three is the small-sample failure the floor exists to
catch.

## What the record says

`crates/lev/calibration/lev-base/routing.json` is the served map. It names
the door, the base model signature the runtime published, the suite and its
digest, the partition it was fitted on, the estimator and its seed block, and
the gate that judged it by id and by digest. A door checks every one of those
before serving it:

```text
lev-serve --port 11449 --calibration crates/lev/calibration/lev-base
lev-serve: serving fitted maps for routing
lev-serve: severity refused — the record was not admitted: unverifiable: …
lev-serve: urgency refused — the record was not admitted: unverifiable: …
```

A request naming `extensions.family: "routing"` with
`extensions.require_calibration: true` is answered rather than refused, which
has not happened before in this repository. The response carries the record
it rests on. The same door started against a different operating system build
refuses:

```text
a calibration record covers `routing` and does not match this door —
os_build: the map was fitted on 25E246 and this door runs 25F100.
```

## What the run found in the harness

The first attempt broke its own receipt chain after five rows, and nothing
had touched the file. `serde_json` writes an `f64` exactly and parses one
approximately: a latency of 1474.8615419999999 milliseconds was written with
every digit and read back as 1474.861542, a different double and a different
digest.

The store now seals a row over the value it will read back as, and a
regression test in `crates/gym/src/store.rs` pins that latency. It is worth
recording where the fault came from: every test the store had used integers
and nulls, so the first measured float in the repository was also the first
one to break it.

## What this does not say

The locked partition is unread. Every number here is from the calibration and
development partitions, the labels are the author's, and one door on one
machine at one seed block is one measurement. The suite's own resampling
variance, measured in
[`2026-09-19-seed-variance.md`](2026-09-19-seed-variance.md), is 0.0197 in
accuracy, so the two partitions agreeing at 0.78 is agreement within noise
rather than a reproduction.
