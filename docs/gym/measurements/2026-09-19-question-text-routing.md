# Rewording the routing question: the tenth attempt, which ran

[`docs/text-optimization.md`](../../text-optimization.md) records nine
programs for optimizing text built in this workspace since December 2025, and
no text change that ever beat a baseline. Its disposition was not a tenth
optimizer but the experiment the ninth was built for and never performed.
This is that experiment.

**It ran, and it is negative.** The best of three hand-written rewordings
gains **0.025 accuracy** on the partition it was not written against, which is
**less than half the 0.056 floor**, and it costs ECE, Brier, and log loss to
do it. Under `decision-v1` the comparison reads `unverifiable`. The other two
candidates read `failed`.

One structural finding is worth more than the verdict, and it is in
[What this partition could have shown](#what-this-partition-could-have-shown):
the routing calibration partition scores 0.975 before anything is reworded,
so the largest gain it can express is one item, 0.025 — **inside the floor by
construction.** No reworded question could have cleared the bar on this split.

## What was asked, and of what

The `routing` question of `support-v2-three-way`: one six-word sentence and
three criteria strings, written once by hand and never measured.

```json
"instructions": "Which team should handle this message?",
"criteria": {
  "billing": "Charges, invoices, refunds, and payment problems",
  "technical": "Bugs, crashes, outages, and performance",
  "sales": "Quotes, plans, upgrades, renewals, and pricing"
}
```

Three candidates were written by hand against the 40 routing items of the
`development` partition and against nothing else. Each adds the labelling rule
the items follow and the baseline text never states: a broken thing is
technical even when it is an invoice or a payment page, and a limit a larger
plan would lift is sales even when it reads as slowness.

| Set | Instructions | Criteria |
| --- | --- | --- |
| [`support-v2-three-way-v1`](../../../crates/gym/questions/support-v2-three-way-v1.json) | the baseline | the baseline |
| [`support-v2-three-way-v2`](../../../crates/gym/questions/support-v2-three-way-v2.json) | unchanged | reworded |
| [`support-v2-three-way-v3`](../../../crates/gym/questions/support-v2-three-way-v3.json) | reworded | unchanged |
| [`support-v2-three-way-v4`](../../../crates/gym/questions/support-v2-three-way-v4.json) | reworded | reworded |

Three candidates over two fields say which field carries an effect rather than
only whether one exists. Every candidate freezes the option names `billing`,
`technical`, and `sales` and the question type, because an option name is
answer-space identity: a candidate that hands back other keys is answering
another question rather than answering this one better. A test in
[`crates/gym/src/questions.rs`](../../../crates/gym/src/questions.rs) pins
that, along with the two untouched families staying untouched.

The door is hosted Jev, `jev-latest`, which is the door the lever exists for:
its weights are closed, so question text is the only thing a caller can
change. It is also the door that does not contend for the Apple on-device
model, and the machine was busy. Kev and Lev are not measured here, so the
transfer question —
[whether optimized text is a property of the task or of the model](../../decision-models/research/2026-09-19-question-text-optimization.md)
— stays open.

```text
cargo build --release -p gym --bin gym
target/release/gym eval --jev --partition development --family routing \
    --questions support-v2-three-way-v2 \
    --record crates/gym/results/routing-question-text-development.jsonl
target/release/gym compare \
    --store crates/gym/results/routing-question-text-calibration.jsonl \
    --baseline "jev (hosted) asked as support-v2-three-way-v1"
```

`gym eval --family` lands with this record. A question set holds one question
per family and a reword touches one of them, so asking the other two spends
door calls and then averages the change against items whose text could not
have moved.

## Development, the partition the candidates were written against

40 items, 160 door calls.

| Question set | Accuracy | ECE | Brier | NLL | Confident errors | Mean p(truth) |
| --- | --- | --- | --- | --- | --- | --- |
| `v1`, the baseline | 0.875 | 0.084 | 0.059 | 0.159 | 0 | 0.890 |
| `v2`, criteria | **1.000** | 0.036 | 0.009 | 0.041 | 0 | 0.964 |
| `v3`, instructions | 0.900 | 0.032 | 0.072 | 0.223 | 1 | 0.858 |
| `v4`, both | 0.925 | 0.070 | 0.060 | 0.207 | 1 | 0.880 |

The five items the baseline gets wrong are the five boundary items the rule
addresses, and `v2` gets all five with nothing lost:

| Item | Truth | The message |
| --- | --- | --- |
| `routing/052` | sales | We need a security review before we can purchase. |
| `routing/067` | billing | The trial ended early and now we are locked out. |
| `routing/068` | billing | Our admin left and nobody can access the billing portal. |
| `routing/082` | technical | The payment page throws an error when I enter a new card. |
| `routing/087` | technical | The invoice pdf is corrupted and will not open. |

`v3` gains two and loses one. `v4` gains four and loses two. **Carrying the
rule in the criteria beats carrying it in the instructions, and carrying it in
both is worse than carrying it in the criteria alone** — the one result here
that a search would have had to discover and that three hand-written
candidates discover for 160 calls.

`support-v2-three-way-v2` was selected here, on this partition, and committed
in `a50e6efe` before a single calibration row existed. The selection cannot
have been made after the fact, and the commit is the evidence.

## Calibration, the partition that decides

40 items, 160 door calls, none of which any candidate was written against.

| Question set | Accuracy | ECE | Brier | NLL | Confident errors | Mean p(truth) |
| --- | --- | --- | --- | --- | --- | --- |
| `v1`, the baseline | 0.975 | 0.061 | 0.019 | 0.074 | 0 | 0.939 |
| `v2`, criteria, **selected** | 1.000 | 0.083 | 0.031 | 0.105 | 0 | 0.917 |
| `v3`, instructions | 0.950 | 0.063 | 0.053 | 0.169 | 0 | 0.885 |
| `v4`, both | 0.875 | 0.055 | 0.074 | 0.221 | 0 | 0.846 |

`gym compare` reads all three as a **question-text** comparison, which is what
openagents#9386 made expressible, and `decision-v1` judges them:

| Candidate | Verdict | Deciding criterion |
| --- | --- | --- |
| `v2` | unverifiable | `accuracy_gain_clears_the_noise`: 40 items at 0.975 against 40 at 1.000 do not reach five expected outcomes on each side |
| `v3` | failed | `accuracy_does_not_fall`: accuracy 0.975 to 0.950 |
| `v4` | failed | `accuracy_does_not_fall`: accuracy 0.975 to 0.875 |

**The selected candidate gains one item in forty.** That is 0.025 accuracy
against a 0.056 floor, about one unpaired standard error, and one discordant
pair — McNemar's exact two-sided p of 1.0. It is not a gain. It is a tenth
consecutive negative result.

**And the rest of the panel moved the wrong way.** ECE rose from 0.061 to
0.083, Brier from 0.019 to 0.031, log loss from 0.074 to 0.105, and the mean
probability on the true class fell from 0.939 to 0.917. One calibration item
came back at 0.50 billing against 0.50 sales, where the baseline had 0.90 on
the right answer. This is the warning
[`2026-09-19-adapter-v1.md`](../../lev/measurements/2026-09-19-adapter-v1.md)
attached to the Choice adapter, repeating exactly: **a text change bought
accuracy by making the door less sure.** Reported on accuracy alone, `v2`
would read as a small win. Reported on the panel, it is a small unresolvable
accuracy move paid for with worse probabilities.

## What the development number was worth

The development gain was 0.125 and the calibration gain was 0.025. **The
optimization set overstated the effect by five times.**

`v4` is the sharper lesson, because it is the candidate a lazier run would
have shipped. It gains 0.05 on development. It loses 0.10 on calibration. A
run that optimized and reported on the same 40 items would have published a
4-point improvement that is a 10-point regression, and nothing in its own
numbers would have said so.

This is the failure mode
[`jev-align`](../../decision-models/research/2026-09-19-question-text-optimization.md)
ships as a default — `dataset=examples, valset=examples` — measured on our own
suite rather than argued about.

## What this partition could have shown

The baseline scores 0.975 on the routing calibration partition: 39 items of
40. **The largest gain any reworded question can express there is one item,
0.025, which is inside the 0.056 floor before the experiment begins.**

`decision-v1` says the same thing in its own terms and refuses to compute:
the normal approximation behind its standard error needs about five expected
outcomes on each side, and at 0.975 that needs roughly 200 items, not 40. To
resolve 0.025 at two standard errors takes about 156 items. The routing family
holds 100.

So the experiment as specified could not have produced a measurable win on the
partition it scores on, whatever text it tried. That is not a reason to
discount the negative — the candidate did not clear the bar, and the bar is
the right one. It is a reason to measure a baseline's headroom **before**
designing a text experiment, and it is the first thing the eleventh proposal
has to answer.

The two partitions are also not interchangeable. The same door under the same
text scores 0.875 on development and 0.975 on calibration, a 0.10 gap on
identical text — larger than the floor, though only 1.7 standard errors on
these counts. The repartition that produced `support-v2-three-way` put the
hard boundary items disproportionately in development. Nothing was wrong with
doing that; it means the two halves are not the same difficulty, and a result
that moves between them is partly reporting which half it landed on.

## The door does not move when the question does not

A second identical pass over the same 40 development items, recorded in
[`routing-question-text-development-repeat.jsonl`](../../../crates/gym/results/routing-question-text-development-repeat.jsonl)
because one store refuses a repeated perturbation:

| Question set | Flips | Accuracy, then | Accuracy, again | Largest probability drift |
| --- | --- | --- | --- | --- |
| `v1` | 0 of 40 | 0.875 | 0.875 | 0.080 |
| `v2` | 0 of 40 | 1.000 | 1.000 | 0.050 |

Hosted Jev is answer-stable on these items: probabilities drift by up to 0.08
and 0.004 on average, and no winner changes. So the seed-resampling spread
that
[`2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)
measured on Lev, which is where 0.056 comes from, is not the noise that binds
here — this door does not expose that axis and does not wobble on it. Item
sampling is what binds, and on 40 items at 0.975 it is wider than the effect
being looked for.

## The record

Both stores are committed, receipt-chained, and verified by every read above.

- [`crates/gym/results/routing-question-text-development.jsonl`](../../../crates/gym/results/routing-question-text-development.jsonl)
  — 160 rows, four question sets, development partition.
- [`crates/gym/results/routing-question-text-calibration.jsonl`](../../../crates/gym/results/routing-question-text-calibration.jsonl)
  — 160 rows, four question sets, calibration partition.
- [`crates/gym/results/routing-question-text-development-repeat.jsonl`](../../../crates/gym/results/routing-question-text-development-repeat.jsonl)
  — 80 rows, the repeat pass.

Every row pins the suite digest `54fbf4137c…`, the question digest of the set
it was served, and the gate digest it was judged by, so any of these tables
can be recomputed from the rows without asking a door.

**The locked partition was not read.** Nothing here is worth spending it on.

## What the eleventh proposal has to argue with

1. A hand-written reword that fixes every error on the partition it was
   written against gains **one item in forty** on the partition it was not.
2. The field that carries the effect is the **criteria**, not the
   instructions, and using both is worse than using the criteria alone.
3. Accuracy went up and **every probability metric went down**. Any future
   text result reports the panel.
4. On a partition at 0.975 the measurement cannot be made at all. Check the
   baseline's headroom before writing a candidate.
5. The cost was 400 hosted calls and a few minutes. The experiment was never
   expensive. It was never run.
