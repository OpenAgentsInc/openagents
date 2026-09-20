# What 98 labels bought, against a zero-label compile

[`research/2026-09-19-compiled-functions.md`](research/2026-09-19-compiled-functions.md)
settled what ProgramAsWeights is and left one question open, the one that
could move the root of [`choosing.md`](choosing.md): how much of our `routing`
accuracy did our 98 real labels buy, against an adapter a 4B hypernetwork
writes from the question text alone? This record answers it with runs from one
Linux CPU box (openagents#9387). The harness and every raw answer are in
[`training/compiled-functions/`](../../training/compiled-functions/).

## The four numbers

All on the `support-v2` `evaluation` split, `routing` family, 50 items, suite
digest `6877c24bf261d5bdcb0550824c20f017c7bb5095aac22dbf5f4c6ef47b789368`.
Read the differences against the measured seed-variance floor of **0.056**
(7.2% relative), not against zero.

| System | Labels it saw | Accuracy | Wilson 95% | Record |
| --- | --- | --- | --- | --- |
| **A: compiled from prose only** | 0 | **0.90** (45/50) | 0.786 to 0.957 | `training/compiled-functions/runs/prose.json` |
| **B: compiled from prose plus four example pairs** | 4 | **0.90** (45/50) | 0.786 to 0.957 | `training/compiled-functions/runs/with-examples.json` |
| Lev, LoRA `lev-adapted@1` | 98 | 0.92 | — | [`../lev/measurements/2026-09-19-adapter-v1.md`](../lev/measurements/2026-09-19-adapter-v1.md) |
| Lev, base, prompted zero-shot | 0 | 0.82 | — | same |

Two more rows that the issue did not ask for and the reading needs:

| System | Labels it saw | Accuracy | Wilson 95% | Record |
| --- | --- | --- | --- | --- |
| `Qwen/Qwen3-0.6B`, the compiler's interpreter, base, prompted with spec A | 0 | 0.30 (15/50) | 0.191 to 0.438 | `runs/base-prompt.json` |
| The same, prompted with spec B as four-shot | 4 | 0.36 (18/50) | 0.241 to 0.499 | `runs/base-prompt-examples.json` |
| Most common label | — | 0.28 | — | [`choosing.md`](choosing.md) |

The Lev rows are the published numbers from the adapter record, not runs from
this box: Lev needs Apple hardware, which this machine does not have. Both
came from the same 50 items under the same digest, so the comparison is on
the same items, but it is unpaired.

### Paired, on the 40 items every door has answered

`support-v2-three-way`, digest
`54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9`,
`development` partition, `routing`, 40 items. The doors' per-item rows are in
`crates/gym/results/support-v2-three-way.jsonl`; the pairing is
`training/compiled-functions/compare.py`.

| Door | Door right | A right | Door only | A only | McNemar p |
| --- | --- | --- | --- | --- | --- |
| `lev-adapted@1` (98 labels) | 38/40 | 36/40 | 4 | 2 | 0.69 |
| `baseline-bge` (frozen embeddings, fitted on calibration) | 38/40 | 36/40 | 4 | 2 | 0.69 |
| `lev-base` (zero-shot) | 35/40 | 36/40 | 3 | 4 | 1.00 |
| `jev (hosted)` | 35/40 | 36/40 | 2 | 3 | 1.00 |

B against the same doors: 35/40, with `lev-adapted@1` at 4 door-only, 1
B-only, p = 0.38. The compiled programs disagree with the LoRA on six of
forty items and the disagreements split four to two. No pair here is
separable.

## Which of the three outcomes happened

**A lands within 0.056 of our LoRA.** The gap on the 50 evaluation items is
0.02, one item. On the 40 paired items it is 0.05, two items, p = 0.69. Both
are under the floor. B did not narrow anything: it lost one item A had and
gained nothing, which on 50 items is noise, and the reading is that four
inline examples are worth nothing measurable when the compiler is already
writing its own. See the next section.

So the finding the issue named comes true: **our 98 labels bought nothing on
`routing` that a hypernetwork could not infer from the question text.** The
base model the compiler writes into is at the majority-label floor under the
same prose, so the 0.60 between 0.30 and 0.90 is what the compile added, and
it added it from zero labels of ours.

`choosing.md` has its third branch as of this record. What that branch says,
and what it does not, is below.

## What the compiler actually did with zero labels

The pseudo-program the compiler wrote from spec A, stored beside the adapter
in `~/.cache/programasweights/programs/96943e9b8bdfc3eb7c9f/`, contains six
labelled examples the compiler generated itself:

```text
Input: "The invoice for March was not received."
Output: billing
Input: "The app crashed during login."
Output: technical
Input: "Can I upgrade to the Pro plan?"
Output: sales
...
```

This is the research record's prediction, observed: *labels come back as a
frontier model's guess at what your labels would be*. Zero labels from us
became six labels from the compiler's own model before the adapter was
mapped, and then the adapter was mapped to a prompt that carries them. The
0.90 is not a label-free number. It is a number whose labels we did not write
and cannot see the distribution of.

That is also why B bought nothing. Four examples of ours went into a prompt
that already had six of theirs on the same three-way split, and a
Qwen3-0.6B adapter trained by a hypernetwork on a 100-item task has nowhere
left to go on `routing`.

## What the guide's root should say now

The root question stays *do you have labelled outcomes?* What changes is the
NO branch. Before this record it had one leaf: a general model, and your
first task is to get labels. It now has two:

- A general model, with every number treated as unverified until you have
  labels to check it against.
- A compiled specialist, if you will accept a frontier model's guess at what
  your labels would be, and the task is Choice, and no probability gates an
  action.

The second leaf is the one this record earned. Its three conditions are not
hedges; each one is a measurement here:

- **A frontier model's guess.** The six examples above are the labels. On
  `routing`, where the criteria are three short phrases a support desk would
  write the same way, the guess was as good as ours. On a family whose
  labels encode an outcome only the operator can see — which door actually
  handled the ticket, whether the delegation actually collided — nothing in
  the spec tells the compiler what the label is, and the measurement here
  says nothing about that case.
- **Choice only.** The compiled function returns a bare string. Noul and
  Score have no leaf here, and the contract has three primitives.
- **No probability.** ECE, Brier, and log loss are undefined for a bare
  string. The 0.90 cannot gate anything; it can only be taken. Every door in
  the store, including the base Lev at 0.82, returns a distribution that can
  be measured and refused. This one cannot be refused for hedging because it
  cannot hedge.

The LoRA's 98 labels bought the same accuracy and a calibrated distribution
that passes `probability-v1` on this family (log loss 1.397 to 0.200 under the
admitted map, in the adapter record). That is what the labels were for.
Accuracy was never the thing they bought.

## What this box could not measure

- **Lev.** Both Lev rows are the published records, not reruns. The suite
  digest pins them to the same items.
- **Kev.** No weights here. Its rows are in the paired table's source file
  but were not needed for this question.
- **The teacher-synthesis tier.** Out of scope by the issue, and by the
  floor: the one independent replication puts it 0.03 above the cheap tier
  on n = 100 with McNemar p = 0.51.
- **Through the Gym.** The compiled function is not a door: it does not
  answer `POST /v1/systemone` and has no distribution to score. It was scored
  by a harness that reads the same suite file under the same digest and
  scores exact label match, which is what `probability-v1` does for
  accuracy. The receipt chain does not hold it and the store does not know
  it. Wrapping it as a door would give it a fabricated probability, which is
  the thing the guide says not to do.
- **Hosted Jev was not rerun.** Its paired rows were already in the store
  under the same digest.

## Reproducing it

```sh
cd training/compiled-functions
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python -r requirements.lock
.venv/bin/python measure.py --spec prose --out runs/prose.json
.venv/bin/python measure.py --spec with-examples --out runs/with-examples.json
.venv/bin/python measure.py --spec base-prompt --out runs/base-prompt.json
.venv/bin/python measure.py --spec base-prompt-examples --out runs/base-prompt-examples.json
.venv/bin/python compare.py runs/prose.json runs/with-examples.json
```

The compile is deterministic on the spec: compiling spec A again returned the
same program ID, `96943e9b8bdfc3eb7c9f`, from the compiler snapshot
`paw-4b-qwen3-0.6b-20260407`. Spec B is `1230b5bb2556f2cc8f04`. Inference is
greedy over a Q6_K quantization of the 0.6B interpreter on the CPU, and every
raw string is in the record, so a rerun that differs will show where.

The compile is anonymous and therefore public: the spec is the question text
from `crates/gym/suites/support-v2.json` and, for B, four calibration states,
`routing/000`, `routing/020`, `routing/040`, and `routing/006`. No scored item
was in any spec.

### The base-prompt rows, and why there are two prompt forms

The first prompt form tried, `Input: {state}\nOutput:`, made the base model
answer `sales`, the last option listed, on all 70 items. That form is kept as
`runs/base-prompt-input-output.json` and
`runs/base-prompt-examples-input-output.json`. The form reported above,
`Message: {state}\nTeam:`, was chosen after seeing that, and it moved the base
to 0.30 and the four-shot to 0.36, still at the majority-label floor. Two
forms were tried and both are published; no third was tried. The base rows
exist to say what the compile added, and both forms say the same thing: the
0.6B interpreter cannot do this task from the prose, and the adapter can.
