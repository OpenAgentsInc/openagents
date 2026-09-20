# Reproducing the week's claims from rows

Every optimization result this repository published in the week to
2026-09-19 was produced by hand, printed to a terminal, and pasted into a
document. The numbers were real and the method was not recorded. Now that
rows are durable, gates carry digests, and the suite has a locked partition,
this record asks the six load-bearing claims again from the record rather
than carrying them forward on trust.

Every table below is a view over
`crates/gym/results/support-v2-three-way.jsonl`, produced by `gym compare`,
`gym fit`, or `gym permute --store`. Each of those verifies the receipt chain
before it prints anything, so a table that printed is a table over rows that
nobody removed and nobody inserted.

## The short answer

| Claim | Asserted in | Verdict | Where it stands now |
| --- | --- | --- | --- |
| Base 0.77 to choice adapter 0.90 accuracy | `lev/measurements/2026-09-19-adapter-v1.md` | reproduced, moved | 0.772 to 0.886 on the 79 items the adapter never trained on, *p* = 0.012 paired |
| The band goes from anti-informative to monotone | `lev/measurements/2026-09-19-adapter-band.md` | reproduced, weaker | 0.73 / 0.79 / 0.94 against a published 0.64 / 0.82 / 0.95; the top band against the rest is *p* = 0.06 |
| Band-conditioned calibration cuts log loss 2.601 to 0.388 | same | reproduced | 2.847 to 0.372, passing every criterion of `probability-v1`, and still written to no record |
| Permutation augmentation cuts flip rate 0.120 to 0.040 | `lev/measurements/2026-09-19-adapter-v1.md` | withdrawn | withdrawn by [#9375](https://github.com/OpenAgentsInc/openagents/issues/9375) hours before this run, on its own measurement |
| Hosted Jev 0.94, `kev-0.5b` 0.72, Lev 0.77 | `lev/measurements/2026-09-19-suite-v2-scores.md` | reproduced | 0.949, 0.722, 0.772 on the 79 of those items this suite leaves open |
| `routing` is the only admitted family on the base model | `lev/disposition.md` | reproduced, and stronger | it is the only family any of the seven doors in the record is admitted for |

Three findings outside the six claims matter more than four of them.

**Half the locked partition is training data for every adapted door.**
`training/lev-adapter/convert.py` reads `crates/lev/suites/support-v2.json`
and trains on its calibration split. The three-way partitioning was drawn
later over the same 196 items, and it put 20 of those 98 training items in
the locked partition. So the partition being kept back to confirm a door has
already been learned by three of the four doors it would confirm. The base
model is unaffected. Fixed for future runs in the same change; it cannot be
undone for the adapters that exist.

**No comparison crosses the question-set boundary.** [#9386](https://github.com/OpenAgentsInc/openagents/issues/9386)
made the question text a third digest, and a row recorded before it does not
say which question set it served. `gym compare` refuses to call two such
sides a comparison, which is the correct rule — an unrecorded question set is
not the authored one — and it means every one of the 785 rows recorded
earlier that day can be read but not judged against anything recorded after.
Every claim here crosses that line, because every claim's baseline predates
it.

**The band claim rests on a map that was never written down.** `lev-band`
prints its fitted map and records nothing, so the 2.601-to-0.388 result
exists in a paragraph and in no file. The release that would serve it,
`lev-adapted@2`, carries `evalRef: []` and admits nothing.

## What the chain holds

One file, `crates/gym/results/support-v2-three-way.jsonl`: **1,099 rows over
seven doors**, one row per item per door, receipt-chained, and the chain
verifies. Suite `support-v2-three-way` at digest `54fbf4137c3de538`, question
set `support-v2-three-way-v1` at digest `9745b1d9a0f38288`, judged by
`probability-v1` at `gate:368cefd18f30`. Every door answered all 157 open
items; none refused, and none was lost to the harness.

| Side | Identity | Accuracy | ECE | Brier | Log loss | Confident errors |
| --- | --- | --- | --- | --- | --- | --- |
| `jev (hosted)` | not verifiable | **0.936** | 0.049 | 0.048 | 0.165 | **0** |
| `lev-adapted@1` | base `9799725f`, adapter `lev-adapted@1` | 0.943 | 0.065 | 0.062 | 1.443 | 8 |
| `kev-8b` | base `49e3418f` | 0.879 | **0.044** | 0.086 | 0.279 | 1 |
| `lev-base` | base `9799725` | 0.783 | 0.107 | 0.161 | 2.353 | 12 |
| `kev-4b` | base `906bfd4b` | 0.745 | 0.081 | 0.148 | 0.592 | 5 |
| `kev-0.5b` | not verifiable | 0.713 | 0.074 | 0.186 | 0.548 | 3 |
| `kev-0.6b` | base `da87bfb6` | 0.675 | 0.141 | 0.181 | 0.522 | 4 |

**Read `lev-adapted@1`'s row with the training overlap in mind**: 78 of those
157 items are its own training records. On the 79 it never saw it scores
0.886. Every other door is clean on every item.

Three of the seven sides were added by this run: hosted Jev, which had been
recorded as unreachable and was reachable all along; `lev-adapted@1`; and the
rows behind the fitted maps. The other four came from
[#9384](https://github.com/OpenAgentsInc/openagents/issues/9384) and
[#9369](https://github.com/OpenAgentsInc/openagents/issues/9369) earlier the
same day, into the same chain.

The latency column is deliberately absent. The machine carried up to eleven
on-device doors across nine agents while this ran, and `lev-adapted@1`'s
median of 20,016 ms against the base door's 1,494 ms is a measurement of the
queue rather than of the adapter.

### What it cost

The chain holds 4,105 seconds of recorded door time — about 68 minutes of
answers, spread over a wall clock several times that. Hosted Jev's 157 items
cost 31 seconds of that and a fraction of a cent. One Lev pass at eight
samples an item cost between 50 and 90 minutes of wall clock depending on
what else the device was serving.

That ratio is the reason this reconciliation is partial. Re-deriving a
published Lev number costs an hour of a shared device per door, and there
were seven doors' worth of claims.

## Claim by claim

### Base 0.77 to choice adapter 0.90 accuracy

**Reproduced, and the thirteen points are eleven.** `lev-adapted@1` — the
choice adapter, served from its release manifest — answered all 157 open
items of the three-way suite through the same L2 estimator at the same seed
block as the base door.

| Items | `lev-base` | `lev-adapted@1` | Difference | Paired |
| --- | --- | --- | --- | --- |
| The 79 no adapter trained on | 0.772 | **0.886** | +0.114 | 10 to 1, *p* = 0.012 |
| All 157 open | 0.783 | 0.943 | +0.159 | 26 to 1, *p* < 0.001 |
| `development` | 0.782 | 0.936 | +0.154 | |

**The first row is the only held-out one, and it is the one to quote.** The
adapter was trained on the 98 calibration items of the two-way suite, and the
three-way partitioning was drawn later over the same 196 items, so 78 of the
157 open items are its own training records — 39 of them in the partition
named `development`. The +0.159 is half a memory test. The +0.114 is the
claim, and at two floors with one discordant item against it, it holds.

Per family, on the 79:

| Family | `lev-base` | `lev-adapted@1` | Published |
| --- | --- | --- | --- |
| `routing` | 0.825 | 0.900 | 0.82 to 0.92 |
| `severity` | 0.733 | 0.800 | 0.67 to 0.78 |
| `urgency` | 0.708 | 0.917 | 0.73 to 0.93 |

**The overconfidence finding reproduces, and only on the clean items.** The
original run's sharpest observation was that the adapter got better at being
right and worse at being sure: log loss 1.952 to 2.323, confident errors six
to eight. On the 79 held-out items that is exactly what happened again — log
loss 2.024 to 2.861, confident errors five to eight, ECE 0.106 to 0.127. On
all 157 items it reverses: log loss 2.353 to 1.443 and confident errors
twelve to eight, because a model answering its own training records
confidently and correctly improves every measure at once. Contamination does
not merely inflate accuracy here; it flips the sign of the conclusion.

One thing changed that the claim did not cover. The adapted door's `routing`
map passes `probability-v1` — log loss 1.397 to 0.200 on 40 fitted items —
which makes it the first admitted calibration map for an adapted Lev door.
It is written to `crates/lev/calibration/lev-adapted@1/` and named in
`crates/lev/manifests/lev-adapted-v1.json`, whose `evalRef` was empty until
this run.

**The Gym refuses to call this comparison a comparison, and it is right
to.** The base rows were recorded before [#9386](https://github.com/OpenAgentsInc/openagents/issues/9386)
and do not name the question set they served. Every number above is read off
the per-side table with the paired test computed beside it, not off a gate
verdict. An hour of device time re-scoring `lev-base` under the current build
would settle it.

### The band goes from anti-informative to monotone

**Reproduced in direction, and weaker than it was stated.** `lev-band` asked
both doors for a certainty band on the 79 evaluation items the three-way
suite leaves open, greedy, one call per item.

| Band | `lev-base` | `lev-adapted@2` |
| --- | --- | --- |
| `almost certainly not` | 11 of 12, 0.92 | — |
| `unlikely` | 2 of 2, 1.00 | 8 of 11, **0.73** |
| `likely` | 52 of 64, 0.81 | 27 of 34, **0.79** |
| `almost certain` | 1 of 1, 1.00 | 32 of 34, **0.94** |

The base model's shape reproduces exactly as described: its two lowest bands
score 0.92 and 1.00 while the bulk of its items pile into `likely` at 0.81.
Reading it would mislead you, and that is what anti-informative means. The
adapter's three bands are still in the right order.

What does not survive is "with real spread". The published table read
0.64 / 0.82 / 0.95; this one reads 0.73 / 0.79 / 0.94, and the bottom two
bands are 0.06 apart on 11 and 34 items. The separation worth anything is the
top band against everything else — 0.94 against 0.78, Fisher exact
*p* = 0.060 — and the bottom band against the middle one is *p* = 0.69. The
base model's inversion is not established either: its low bands against
`likely` is *p* = 0.44.

So the categorical difference holds and the size of it does not. The honest
statement is that the adapter's band orders correctly on every pair and
separates its top band from the rest at about one part in twenty, where the
base model's band orders incorrectly and separates nothing. The acceptance
criterion in [#9363](https://github.com/OpenAgentsInc/openagents/issues/9363)
— the band varies, and it separates correct from incorrect better than chance
— is met on the direction and is not met at a conventional significance on 79
items.

### Band-conditioned calibration cuts log loss 2.601 to 0.388

**Reproduced, and the record it rests on still does not exist.** `lev-band
--calibrate` fitted the map again against the band adapter, on the 78
calibration items and 79 evaluation items the three-way suite leaves open:

| Map | ECE | Brier | Log loss | Confident errors | Published log loss |
| --- | --- | --- | --- | --- | --- |
| raw, no map | 0.112 | 0.118 | 2.847 | 8 | 2.601 |
| pooled | 0.123 | 0.127 | 0.588 | 8 | 0.499 |
| **band-conditioned** | **0.066** | **0.101** | **0.372** | 8 | 0.388 |

Every part of the claim holds. The pooled map leaves ECE worse than the raw
signal, 0.123 against 0.112, exactly as it did before. The band-conditioned
map passes `probability-v1` on all eight criteria — log loss 2.847 to 0.372,
ECE down 41% against the 10% a candidate has to earn, Brier inside its
ceiling — and the same two bands earn their own table, `almost certain` and
`likely`, with `unlikely` falling back to the pool. This is the largest
result claimed this week and it is the one that reproduces most exactly.

One number moved in the right direction and is worth naming. The page reports
confident errors rising nine to eleven under the map and explains it as the
measure behaving correctly. Here they are 8 before and 8 after, so the
explanation still stands and the effect it explains did not recur.

**What has not changed is that none of this is in a file.** `lev-band` prints
the map and writes nothing — no record, no rows, no digest — so the claim is
a paragraph both times. The release that would serve it, `lev-adapted@2`,
carries `evalRef: []` and admits nothing, which means the best-calibrated Lev
probability this repository has measured is one no door can serve. Fitting it
through `gym fit --records`, the way `lev-adapted@1`'s maps were fitted in
this run, needs the band in the row, and the row has no band field.

### Permutation augmentation cuts flip rate 0.120 to 0.040

**Withdrawn, hours before this run, on a measurement of its own.**
[`../../lev/measurements/2026-09-19-flip-rate-variance.md`](../../lev/measurements/2026-09-19-flip-rate-variance.md)
asked all six orders a three-option Choice admits rather than the one pair
every published flip rate used, and found that a single door's number carries
about plus or minus 0.13 at two sigma. Band against permutation is 0.075 at
the reversed pair against a floor of 0.145, with a paired exact test at
*p* = 0.45.

This reconciliation adds nothing to that and would have measured it worse:
`gym permute` asks the door through its own estimator, so it reads the argmax
of an eight-sample L2 distribution where the published number was one greedy
call. Those are two statistics, and the greedy one is the right one for an
order probe, because it has no sampling noise to confound the order axis.

### Hosted Jev 0.94, `kev-0.5b` 0.72, Lev 0.77

**All three reproduced to three places.** The claim was made on the 98-item
evaluation split of `support-v2`; 79 of those items are open under the
three-way partitioning, and all six doors have rows on them.

| Side | Published | On the same 79 items | On all 157 open items |
| --- | --- | --- | --- |
| `jev (hosted)` | 0.94 | **0.949** | 0.936 |
| `kev-0.5b` | 0.72 | **0.722** | 0.713 |
| `lev-base` | 0.77 | **0.772** | 0.783 |

Read the Lev and Kev agreement for what it is worth: both doors reproduce
exactly within a seed block, and both rows sets were produced at seed block 0
by the same estimator, so the agreement shows the number was transcribed
correctly rather than that it is robust. Hosted Jev is the one genuinely
independent reproduction here — a fresh pass against a closed hosted model,
landing at 0.949 where 0.94 was published.

**What does not reproduce is the conclusion drawn from those three numbers.**
`2026-09-19-suite-v2-scores.md` reads them as "the harder suite separated the
doors properly". It separated hosted Jev from the other two — 0.177 over Lev
at *p* = 0.003 paired — and it did not separate Lev from `kev-0.5b`: 0.050 on
79 items against a floor of 0.056, 12 discordant items to 8, *p* = 0.50. That
correction is now on that page.

### `routing` is the only admitted family on the base model

**Reproduced, and it is a stronger statement than the claim.** `gym fit
--store` re-judges every recorded row under `probability-v1` without asking a
door:

| Door | `routing` | `urgency` | `severity` |
| --- | --- | --- | --- |
| `lev-base` | **passed** | unverifiable | unverifiable |
| `lev-adapted@1` | **passed** | unverifiable | unverifiable |
| `kev-0.5b` | failed | unverifiable | unverifiable |
| `kev-0.6b` | failed | unverifiable | unverifiable |
| `kev-4b` | failed | unverifiable | unverifiable |
| `kev-8b` | failed | unverifiable | unverifiable |
| `jev (hosted)` | failed | unverifiable | unverifiable |

On the base model `routing` is the only admitted family, as claimed. Across
the whole record it is the only family any door is admitted for, and the two
doors admitted for it are the two Lev doors. The `urgency` and `severity`
rows are all the same refusal — the floor of 30 fitted items against 24 and
15 — which is a property of the partitioning rather than of any door. Every
`routing` failure is a door whose raw signal was already good enough that a
five-bin table could not rescale it without costing log loss, or in
`kev-4b`'s case Brier, which is the compliment the gate pays a sharp door.
The reason the two Lev doors pass is the opposite of a compliment: their raw
log loss on that family is 3.493 and 1.397, so there was a great deal for a
table to fix.

`lev-adapted@1`'s row is new, and it is what closes the third confound
below.

## The three confounds

The issue named three. Each one turns out to be in a different state, and
only one of them is closed.

### The gate and the suite changed in the same commit

**Closed going forward, and it cannot be repaired backward.** Every row now
pins `suite_digest`, `question_digest`, `gate_id`, and `gate_digest`, and
`gym fit --store --gate <id>` re-judges recorded rows under any rule without
asking a door, so the effect of a rule change is a query. What no tool can
recover is the run that caused the complaint: it produced no rows, so there
is nothing to re-judge. The separation starts with the record, not before it.

### The evaluation split has been read at least six times

**Still true, and this run read it again.** Everything in the reconciliation
that carries a number rests on it:

- The 79 items narrowed by `unseen-by-lev-adapters.txt` are that split, minus
  the 19 of its items the three-way suite locked.
- The `development` partition is half that split: 39 of its 78 items.
- The band tables are measured on the same 79 items, because `lev-band` reads
  the two-way file.

There is one more read than the count says. `convert.py` writes `valid.jsonl`
from the evaluation split, and the training runs kept `adapter-epoch2.pt`,
`adapter-epoch3.pt`, and `adapter-epoch4.pt` — so the split also chose which
epoch to export. A split that selects a checkpoint is a development set,
whatever it is called.

The only unread evidence in the repository is the locked partition, and this
run did not spend it. For the base model that partition is clean. For the
three adapted doors it is not, and the reason is the finding at the top of
this record.

### The calibration records were fitted on the base model

**Now enforced rather than merely true.** [#9388](https://github.com/OpenAgentsInc/openagents/issues/9388)
gave each release a manifest and keyed serving off it, and all three adapted
releases carried `evalRef: []`. A family with no measured reference does not
admit, so no adapted Lev door could serve a probability at all. That is
stricter than the state the confound describes, and it is the right
direction: the previous failure was a base-fitted map sitting on disk through
two adapter runs.

This run closed part of it. `gym fit --store --records` re-fits and re-judges
from the recorded rows without asking a door, so a record for an adapted door
costs nothing once its rows exist. `crates/lev/calibration/lev-adapted@1/`
holds three of them, `crates/lev/manifests/lev-adapted-v1.json` names them in
its `evalRef`, and `lev-adapter-check` on that manifest reports `admits
routing`. That is the first adapted Lev release with a measured reference,
and it is measured on the partition the release was fitted on rather than
asserted.

`lev-adapted@2` and `lev-adapted@3` still admit nothing, because this run did
not score them. The work is one command each once a quiet device is
available.

The first adapted record also turned a test into a tautology and back.
`an_adapted_release_admits_nothing_until_a_map_is_fitted_against_it` asserted
that an adapted release names no measurement — which was the rule standing in
for itself, because no adapted map existed to name. What the rule protects is
narrower and now testable: a release may name only maps fitted against the
door it describes. `Manifest::check_eval_refs` compares the record's
`door_identity.adapter` against the release id, a record fitted against
another door is refused by field, and the test asserts that instead. The
comparison lives in the check rather than beside it, because a rule a caller
has to remember to apply is a rule that gets skipped — which is the sentence
that describes how the stale maps survived in the first place.

## What this run could not score

A door that was not reached is a gap in the reconciliation rather than a
result, so each one is named with what it would cost to close.

**`lev-adapted@2` and `lev-adapted@3` have no rows on this suite.** The band
and permutation adapters were served and reachable; they were not scored,
because the machine was running up to eleven on-device doors across nine
agents and one 157-item Lev pass cost between 50 and 90 minutes of wall
clock. The accuracy column of the three-adapter table in
[`../../lev/measurements/2026-09-19-adapter-band.md`](../../lev/measurements/2026-09-19-adapter-band.md)
is therefore not re-derived. That table's comparisons were already withdrawn
as inside the noise floor, so nothing rests on it, but the numbers themselves
remain terminal output. Two passes on a quiet machine would fix it.

**No comparison involving a door scored before #9386 is gate-judged.** That
is five of the seven sides: `lev-base` and the four Kev checkpoints. Their
rows can be read, and every number quoted from them here comes from the
per-side table with a paired test computed beside it, but `gym compare` marks
the verdict `refused` and names the reason. Re-scoring is the only remedy:
about fifteen minutes of CPU for the four Kev doors, and about an hour of
device time for `lev-base`.

**The locked partition is untouched.** Thirty-nine items, unread by this run
and by every run in the record. What this reconciliation found is that it is
not clean for an adapted door, which is a worse problem than not having spent
it.

**One number in the band run is worth a measurement of its own.** The base
door answered those 79 items correctly 66 times under one greedy call —
0.835 — where the same door on the same items scores 0.772 under the
eight-sample L2 estimator every published Lev number uses. The two passes
are not identical, because the band probe asks for a `certainty` field and
that changes what is generated, so this is not yet a finding. If it holds
under a fair comparison it says the ensemble costs six points of accuracy to
buy a resolution of one in eight, which is a trade nobody has priced.

**`lev-band` writes no rows.** The two band tables in this record are
terminal output, like the claims they check. The band is Apple's constrained
enum, it is not on the System One wire, and `gym::row::Row` has no field for
it, so the Gym cannot record it today. Either `lev-serve` publishes the band
in an extension and the row carries it, or `lev-band` writes its own store.
Until one of those lands, the two band claims can be re-measured and cannot
be re-derived.

## What the reproduction changed in the tools

Six faults, each found by trying to re-derive a published number.

**`gym compare` pooled permuted trials and `gym fit` did not.** A permuted
trial measures order sensitivity; it is not a second reading of the item. Any
store that had been through `gym permute` scored its Choice items twice in
the comparison table and everything else once. `fit` had always dropped them,
with the reasoning written beside the filter. `compare` now shares it, and
says in the line under its heading how many rows it left out.

**The flip rate was the one table that was not a view.** `gym permute` wrote
both passes as rows and then printed a number that could only be had by
running the doors again. `gym permute --store` reads it back, which is what
lets a flip rate be narrowed to a partition or to the items a door was not
trained on, the way an accuracy already could.

**A view could not be narrowed at all.** `--items` takes a file of item ids,
skipping blank lines and lines that begin with `#`, so the file states the
rule that chose them. That is the point of the format rather than a
convenience: a subset picked after the numbers are in is how a result gets
talked into existence, and
`crates/gym/results/unseen-by-lev-adapters.txt` is derived from the two
committed suites and from no result.

**Two runs cannot append to one chain, and two branches cannot merge one as
text.** A receipt names the row before it, so rows written in parallel are
two chains from a shared root, and a concatenation verifies as neither. This
run hit it twice: once recording four doors at once, and once rebasing onto a
branch where another agent had appended 628 rows to the same file. `gym
merge --store <destination> --from <source>` folds one store into another,
re-sealing each row onto the destination chain and leaving behind any
perturbation the destination already holds.

**A client timeout shorter than the door's latency manufactures harness
failures.** A harness failure leaves the record set entirely, which is the
right rule and the wrong outcome when the harness invented the failure. This
machine was running several on-device doors at once, so a Lev call that
answers in five seconds queued for twenty-five, and the client's ten-second
default turned a 157-item run into an empty file with 157 timeouts. `gym eval
--timeout <seconds>` sets it. The default is untouched, because a door that
is genuinely hung should not be waited on forever.

**`lev-band` read the locked partition on every run it ever made.**
`support-v2` and `support-v2-three-way` are the same 196 items under two
partitionings, and a lock is a property of the item rather than of the file
that names it. The probe reads the older file, so it spent the newer file's
locked partition without ever passing a flag that would have been refused. It
now skips those items and prints how many it skipped. The same fault, with
worse consequences, is the training corpus finding above.
