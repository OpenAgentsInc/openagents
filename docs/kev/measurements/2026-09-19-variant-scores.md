# Every kev checkpoint on one suite

**Checkpoint scope, added 2026-09-20:** this record measures the historical
adapters pinned in `crates/kev/fixtures/`. Upstream has replaced all three
Qwen3 adapters under the same names. The scores below remain evidence for
the measured artifacts, not for the new Hub contents. See the
[release review](../2026-09-20-upstream-review.md).

Every Kev score this repository published on the suite it currently measures
against came from `kev-0.5b` — the checkpoint kev's own card says is not the
one to serve. The older 52-item table carried `kev-4b` as well, on 26
evaluation items, which is far too few to separate two doors. So the working
reading of Kev rested on its smallest checkpoint, which made Kev look like
the weakest door here. That is our error and not kev's.

This record scores all four published checkpoints on the same items, in one
store, and names the checkpoint on every row.

The machine was heavily contended throughout, which leaves the accuracy
panel intact and the latency comparison undecided. Both are treated that way
below.

## What ran

Four doors, one variant each, on this machine on 2026-09-19. Each door served
on its own port so that `GET /v1/models` names one checkpoint and every
recorded row carries it:

```text
cargo build -p kev --features serve --release --bin kev-serve
cargo build -p gym --release --bin gym

target/release/kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.5b \
    --base-dir ~/work/kev-artifacts/qwen2.5-0.5b --default kev-0.5b --port 8101
target/release/gym eval --door kev-0.5b=http://127.0.0.1:8101 \
    --record crates/gym/results/support-v2-three-way.jsonl --fit
```

The same two commands ran for `kev-0.6b` on `qwen3-0.6b`, `kev-4b` on
`qwen3-4b`, and `kev-8b` on `qwen3-8b`. Every door ran on CPU at fp32, the
`kev-serve` default and the precision the conformance fixtures pin, so
device and precision are not a difference between the rows.

Suite `support-v2-three-way` at digest `54fbf4137c3de538`: 196 items, 79
calibration, 78 development, 39 locked and unread. Judged by
`probability-v1`, digest `gate:368cefd18f30`. Every door answered all 157
open items, refused none, and lost none to the harness.

The rows land in `crates/gym/results/support-v2-three-way.jsonl` beside the
`lev-base` rows already there, so the comparison below is a query over one
receipt-chained file rather than four pasted tables. The store now holds 785
rows over five doors and the chain verifies.

**Correction, 2026-09-20 (openagents#9421).** When this ran, `kev-serve`
answered refusals with only the FastAPI `{"detail": …}` body — no typed code
— so the `gym::eval::classify` rule then in force would have filed any Kev
refusal as a harness loss rather than a refusal. The "refused none, lost
none" claim above therefore does not rest on the classifier: a refused item
would have left no scored row, and the committed store shows no missing
pair. Each Kev door holds all 157 open item/split pairs, every one scored,
the receipt chain verifying;
[`crates/gym/tests/kev_variant_rows.rs`](../../../crates/gym/tests/kev_variant_rows.rs)
reconciles the four row sets against the pinned suite, including gate,
model, and run provenance. The classifier implementation establishes how a detail-only refusal would
have been classified. Complete final-row coverage does not establish that
no failed attempt or retry occurred. The committed run record contains no
raw failed-response or refusal log that could resolve that question. New
refusal-path tests establish the corrected behavior; they do not reconstruct
historical responses.

## The panel

All 157 open items, both partitions pooled, from
`gym compare --store crates/gym/results/support-v2-three-way.jsonl`:

| Door | Base signature | Accuracy | ECE | Brier | NLL | Confident errors | Median latency |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `kev-0.5b` | not published | 0.713 | 0.074 | 0.186 | 0.548 | 3 | 204 ms |
| `kev-0.6b` | `da87bfb6` | 0.675 | 0.141 | 0.181 | 0.522 | 4 | 203 ms |
| `kev-4b` | `906bfd4b` | 0.745 | 0.081 | 0.148 | 0.592 | 5 | 1,302 ms |
| `kev-8b` | `49e3418f` | **0.879** | **0.044** | **0.086** | **0.279** | **1** | 2,675 ms |
| `lev-base` | `9799725` | 0.783 | 0.107 | 0.161 | 2.353 | 12 | 1,494 ms |

The latency column is what `gym compare` prints from the rows, and it was
measured on a heavily contended machine. Read it as recorded rather than as
a result; [the latency section](#latency-recorded-under-contention-and-settling-nothing)
says why, and no conclusion here rests on it.

By partition:

| Door | Calibration accuracy | Development accuracy | Calibration ECE | Development ECE |
| --- | --- | --- | --- | --- |
| `kev-0.5b` | 0.71 | 0.72 | 0.056 | 0.138 |
| `kev-0.6b` | 0.65 | 0.71 | 0.155 | 0.126 |
| `kev-4b` | 0.71 | 0.78 | 0.122 | 0.050 |
| `kev-8b` | 0.89 | 0.87 | 0.051 | 0.062 |
| `lev-base` | 0.78 | 0.78 | 0.149 | 0.127 |

The base signature is the base checkpoint revision the artifact's
`head_meta.json` names, which `kev-serve` now publishes as
`base_model_signature` and every row records. `kev-0.5b`'s artifact names no
revision, so its rows say so rather than carrying a signature nobody checked.

## Latency, recorded under contention and settling nothing

The harness times every call, so the store carries a latency for all 785
rows. Read the numbers as what the door returned on a busy machine, not as a
property of the checkpoints.

| Door | Median | Mean | p90 | Fastest | Slowest |
| --- | --- | --- | --- | --- | --- |
| `kev-0.5b` | 204 ms | 213 ms | 232 ms | 165 ms | 843 ms |
| `kev-0.6b` | 203 ms | 209 ms | 240 ms | 173 ms | 433 ms |
| `kev-4b` | 1,302 ms | 1,380 ms | 1,865 ms | 1,036 ms | 3,939 ms |
| `kev-8b` | 2,675 ms | 2,930 ms | 3,504 ms | 2,113 ms | 8,873 ms |
| `lev-base` | 1,494 ms | 1,501 ms | 1,537 ms | 1,414 ms | 2,875 ms |

**The machine was heavily contended for the whole of this run, so the
latency comparison is inconclusive and no ranking is drawn from it.** Nine
agents were working in this repository at once. While these four passes ran,
a second `kev-serve` in bundle mode held all four checkpoints resident and
ran at about 600% CPU, three `lev-serve` doors were up, a dozen `lev-bridge`
helpers belonging to four other worktrees were resident, and the load average
sat between 13 and 19 on a machine with no idle period during the run. Other
agents saw the same weather from the other side: a live-device conformance
test timed out twice at a 10-second client timeout, and a bridge helper was
killed mid-request during an unrelated smoke test.

The p90 and the slowest column carry the evidence of it. `kev-8b` ran to
8,873 ms on one item against a 2,675 ms median, and `kev-0.5b` to 843 ms
against 204 ms — a factor of four and a factor of three over a workload whose
items vary far less than that.

What the numbers are good for is the order of magnitude, which is not in
dispute and matches the conformance record: the two small checkpoints answer
in hundreds of milliseconds and the two large ones in seconds. They are not
good enough to decide anything at the margin, and the margin is where the
decision lives. Latency here is not a footnote: the reason to record it is
that
[#9382](https://github.com/OpenAgentsInc/openagents/issues/9382) puts time in
the gate, and a contended measurement could invent a reversal or hide a real
one, then be quoted later as a property of a checkpoint. **Before latency
decides anything between these variants, re-run all four on a quiet
machine.** The `lev-base` column makes the point twice over, because those
rows came from a different pass on a different day and were never comparable
with today's in the first place.

One thing the contention did not touch is the accuracy panel. Decoding is
greedy and deterministic, a slow answer is the same answer, and no call
failed: 157 of 157 items produced a scored row for every door, with no typed
refusal from any door and nothing lost to the harness. That distinction is
the one `gym::eval::classify` draws — a typed refusal code in the body is the
door's own answer and stays in the denominator, while a failure carrying no
code is the harness and produces no row at all. For Kev it could not draw
that distinction yet — see the correction above — but under this much load
the second kind is what you would expect to see, and none appeared.

The medians are also longer than the figures in
[`../jev-comparison.md`](../jev-comparison.md), which came from a nine-case
battery of much shorter requests. Suite items carry a full support message
and a full option set, so part of that gap is the request and part of it is
the load, and this run cannot separate the two.

## What clears the floor, and what does not

[`../../lev/measurements/2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)
measures this suite's resampling standard deviation at 0.0197 on accuracy,
so a difference between two doors measured once each needs **0.056**, 7.2%
relative, to clear two sigma. Every difference below is stated against that
floor first.

The doors answer the same items, so a paired test is also available and is
tighter: the column on the right is an exact McNemar test over the items
where exactly one of the two doors was right. It is reported because
`decision-v1` says in its own text that its unpaired standard error
"overstates the noise between two doors answering the same items" — not to
rescue a claim the floor rejects. Where the two disagree, the headline takes
the conservative reading.

| Comparison | Difference | Floors of 0.056 | Split of the items only one got right | Exact *p* | Reading |
| --- | --- | --- | --- | --- | --- |
| `kev-8b` over `kev-0.5b` | +0.166 | 3.0 | 38 to 12 | 0.0003 | real |
| `kev-8b` over `kev-4b` | +0.134 | 2.4 | 25 to 4 | 0.0001 | real |
| `kev-8b` over `lev-base` | +0.096 | 1.7 | 24 to 9 | 0.014 | not established |
| `kev-4b` over `kev-0.6b` | +0.070 | 1.3 | 23 to 12 | 0.090 | not established |
| `kev-4b` over `kev-0.5b` | +0.032 | 0.6 | 25 to 20 | 0.55 | not established |
| `kev-0.5b` over `kev-0.6b` | +0.038 | 0.7 | 18 to 12 | 0.36 | not established |
| `lev-base` over `kev-4b` | +0.038 | 0.7 | 21 to 15 | 0.41 | not established |

Three things follow.

**`kev-8b` leads every column of the panel**, and against the other Kev
checkpoints the lead is established: three floors over `kev-0.5b`, 2.4 over
`kev-4b`, and the same direction on ECE, Brier, log loss, and confident
errors. Winning accuracy and calibration at once is the combination this
directory usually has to choose between. Its lead over `lev-base` is the one
to state carefully: 0.096 on accuracy is 1.7 floors, so the suite cannot
resolve it, and the panel columns beside it have no floor measured at all.
The honest reading is that `kev-8b` is at least as good as the best door
here and probably better, not that it has been shown to win.

**`kev-4b` is not distinguishable from `kev-0.5b` here**, which is the
result the issue that asked for this run did not expect in either direction.
Upstream's own research log has capacity dominating out of domain — 0.6B to
4B worth 14 to 19 points — and on our support suite the 0.6B-to-4B step is
0.070, below the floor, while the 4B-to-8B step is 0.134, well above it. On
these items the jump is at 8B, not at 4B.

**The step from `kev-0.5b` to `kev-0.6b` is not a regression that this suite
can see.** `decision-v1` fails `kev-0.6b` on `accuracy_does_not_fall`, which
compares two point estimates with no noise model at all, while the gain
criterion beside it scales a standard error. A 0.038 drop is 0.7 floors and
the paired test puts it at *p* = 0.36. The verdict is the gate's asymmetry
showing, not a measured loss, and correcting it means a `decision-v2` rather
than an edit to a rule whose digest is already in 785 rows.

## Where the checkpoints differ by family

**Agreement ceiling, added 2026-09-20.** A second reader, labelling blind, agreed with the stored labels on `routing` 50/51 (0.980, kappa 0.971), `urgency` 29/31 (0.935, kappa 0.870), and `severity` 18/18 (1.000, kappa 1.000), on a 100-item sample of these same 196 items. Read each family accuracy against that ceiling, not against 1.0; the intervals and the three disputed items are in [`../decision-models/2026-09-20-instrument-validity.md`](../../decision-models/measurements/2026-09-20-instrument-validity.md).

| Door | `routing`, 80 items | `urgency`, 48 items | `severity`, 29 items |
| --- | --- | --- | --- |
| `kev-0.5b` | 0.775 | 0.667 | 0.621 |
| `kev-0.6b` | 0.775 | 0.688 | 0.379 |
| `kev-4b` | 0.713 | 0.771 | 0.793 |
| `kev-8b` | 0.887 | 0.854 | 0.897 |
| `lev-base` | 0.825 | 0.729 | 0.759 |

`kev-4b` is the interesting row. Its flat suite score hides a trade: it is
worse than `kev-0.5b` on `routing`, the Choice family, and much better on
`severity`, the ordered Score family, and on `urgency`, the Noul family. Its
`severity` accuracy is 0.793 against 0.621, and its `routing` accuracy is
0.713 against 0.775. A reader who only wants routing gets nothing from 4B,
and pays for it in a latency this run cannot measure honestly. A reader who
wants Score gets a lot. None of these family differences clears the floor on
its own at these item counts, so treat the shape as a lead rather than a
result.

`kev-0.6b` scoring 0.379 on `severity` is the one number here large enough
to read on its own: 11 of 29 ordered judgments right, below the 0.5b
checkpoint it is meant to supersede, on the family whose answers this suite
finds hardest.

The `severity` column is an argmax column. `gym::eval::read_answer` scores a
Score item by its highest-probability level rather than by the `score` field,
so these numbers say which level the model put its mass on and say nothing
about whether the weighted mean it reports lands there too. Those are
different questions, and the second one belongs to
[#9378](https://github.com/OpenAgentsInc/openagents/issues/9378).

## No kev door earns a calibration map here

`gym eval --fit` fitted one map per family per door on the calibration
partition and scored it on development. Nothing was admitted:

| Door | `routing`, 40 fitted | `urgency`, 24 fitted | `severity`, 15 fitted |
| --- | --- | --- | --- |
| `kev-0.5b` | failed: log loss 0.511 to 0.544 | unverifiable | unverifiable |
| `kev-0.6b` | failed: log loss 0.329 to 0.510 | unverifiable | unverifiable |
| `kev-4b` | failed: Brier 0.143 to 0.191 | unverifiable | unverifiable |
| `kev-8b` | failed: log loss 0.289 to 0.338 | unverifiable | unverifiable |

The `urgency` and `severity` verdicts are the floor of 30 fitted items, the
same shortfall the Lev run hit on the same partitions. The `routing`
verdicts are the more interesting ones, and for `kev-8b` the refusal is a
compliment of the kind hosted Jev already earns here: at a raw ECE of 0.079
on that family there is little left for a five-bin table to find, and the
binning costs log loss to reach it.

So choosing `kev-8b` buys accuracy and a sharper raw probability. It does
not buy an admitted map, and a Kev probability still gates nothing until a
map passes on a family with enough items to fit one.

## What this run does not say

**Hosted Jev had no rows on this suite when this ran. It has them now, and
the gap it left open is closed the other way.** The key was present on the
machine all along, in a file outside the repository that nothing exported
into the process environment — `crates/jev` reads the environment and loads
no dotenv of its own, so the door reported no API key rather than being
down. Hosted Jev answered the same 157 items a few hours later, recorded to
this same store; the run is in
[`../../gym/measurements/2026-09-19-reproducing-the-week.md`](../../gym/measurements/2026-09-19-reproducing-the-week.md).

| Side | All 157 | The 79 items no adapter trained on |
| --- | --- | --- |
| `jev (hosted)` | 0.936 | 0.949 |
| `kev-8b` | 0.879 | 0.848 |

**The gap is real, and it is smaller than the published numbers suggested.**
On all 157 items it is 0.057, which is one floor, and the doors disagree on
15 items, 12 of them in Jev's favor: a paired exact test puts that at
*p* = 0.035. On the 79 items this record read indirectly it is 0.101, 8
discordant items to 0, *p* = 0.008. The indirect reading above guessed 0.09
and guessed that the gap was no longer established; the first half was close
and the second was wrong, because an unpaired floor is the wrong test for two
doors answering the same items.

What the store still refuses is the *verdict*. These Kev rows predate
[#9386](https://github.com/OpenAgentsInc/openagents/issues/9386) and do not
name the question set they served; the Jev rows do. `gym compare` will not
call two such sides a comparison, so the numbers above are quoted from the
per-side table and the paired test is computed beside it rather than read off
a gate. Re-scoring the four Kev doors under the current build would produce a
judged comparison and costs about fifteen minutes of CPU.

**No latency question is settled.** The section above says why: the four
passes ran on a machine carrying nine concurrent agents, and the numbers are
recorded as observed rather than as a measurement of the checkpoints. The
accuracy panel stands; the time column waits for a quiet machine. A quiet
Linux box without the Kev weights re-timed the one door it could reach:
hosted Jev at a 98.5 ms median over eight blocks, 3.4% relative spread, and
a second pass on which the paired test against `kev-8b` reads *p* = 0.077
rather than 0.035 because one near-tie item moved. The run, its conditions,
and what it leaves open are in
[`../../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md`](../../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md).
The same kind of box, with the two small Kev doors on it, re-timed them
later the same day: `kev-0.5b` at a 329 ms median and 407 ms p95,
`kev-0.6b` at 337 and 448, CPU only, with the same accuracies to the item;
[`../../gym/measurements/2026-09-20-kev-quiet-latency.md`](../../gym/measurements/2026-09-20-kev-quiet-latency.md)
has the sweeps. `kev-4b` and `kev-8b` are still timed only here, under
load.

The rest of the limits are the ones every run here carries. The locked
partition is unread. The labels are the author's. Every door ran once on one
machine; Kev is deterministic, so repeating a run reproduces it exactly and
there is no per-door spread to average, which also means the 0.056 floor
borrowed from Lev's seed resampling is a conservative stand-in rather than a
measurement of Kev's own variance. Nothing here measures order sensitivity:
`gym permute` was not run for these doors.

## What this changes in the documents

- [`../../lev/disposition.md`](../../lev/disposition.md) reported a four-way
  table on the 52-item `support-v1` suite and concluded that "Lev beats
  kev-4b outright." On 157 items that difference is 0.038, below the floor,
  and the two doors are indistinguishable. The claim is withdrawn on the page.
- Every published Kev suite score named `kev-0.5b` or named nothing. Where it
  named nothing, it now names the checkpoint.
- The best Kev on our suite is `kev-8b` at 0.879, not `kev-0.5b` at 0.713.
  Any sentence that reads "Kev scores 0.72" is a sentence about the smallest
  checkpoint, which no one recommends serving.
- Pages quoting a per-variant latency now point here, and this page says the
  number is not yet decidable. A quiet-machine re-run of all four doors is
  the work that makes it decidable, and
  [#9382](https://github.com/OpenAgentsInc/openagents/issues/9382) is landing
  its gate with the latency floor recorded as pending for the same reason.
