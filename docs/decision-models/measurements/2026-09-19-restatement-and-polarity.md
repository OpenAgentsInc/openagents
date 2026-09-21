# Restatement, polarity, and the independence gate

[openagents#9414](https://github.com/OpenAgentsInc/openagents/issues/9414)
opens on one recorded answer. Asked whether every task in a six-task
delegation plan is read-only — a fact the state asserts — `kev-4b` answered
0.17, while answering the harder independence question at 0.93. Four
explanations were on the table: polarity, the question being a conjunction,
restatement in general, and the family having no calibration map.

This is the measurement that separates them. It found two things:

- **The recorded call does not reproduce.** The arguments the golden records
  produce 0.76, not 0.17, on the same door. The state that produced 0.17 is
  not in the record.
- **The question is fine and the checkpoints are not.** Hosted Jev answers
  all 128 panel items correctly. `kev-4b` gives a proposition and its
  negation the *same* probability — the two correlate at **+0.95** where Jev
  correlates at -1.00 — so it answers the topic rather than the claim.

The consequence for [#9413](https://github.com/OpenAgentsInc/openagents/issues/9413)
is in [Is the independence gate safe to rely on?](#is-the-independence-gate-safe-to-rely-on):
**no**, on every local door. Each of the three kev checkpoints passes plans
that collide, at 0.81 to 0.96, over a gate set at 0.7.

## What the record actually supports

`kev-serve` is deterministic. The same body asked three times returns the
same numbers to the last digit, which is the first thing this run checked.
So a recorded answer is reproducible by replaying its recorded request.

The program-selection call in
`crates/coderbench/goldens/devin-fan-out-six.evidence.json` replays exactly:
`delegate-fan-out` at confidence 0.83, probabilities 0.87 and 0.13, and 91
input tokens, all matching the golden. The independence call does not.

| | The golden | Replayed today |
| --- | --- | --- |
| Input tokens | **166** | **72** |
| `independent` | 0.93 | 0.96 |
| `readonly` | **0.17** | **0.76** |
| `needs_tool_restriction` | 0.54 | 0.61 |

Ninety-four tokens of state are missing from the record, and the answer the
issue is about is not the answer the recorded request produces.

What the missing state held is not recorded anywhere, but it tracks the six
file paths. Each of the three recordings names the same three questions and
differs only in one delegation path, and the input-token count moves with
that path:

| Recording | The path that moved | Input tokens | `readonly` |
| --- | --- | --- | --- |
| `78c678f439` | `nips/coder/NIP-CC.md` | 165 | 0.16 |
| `a303bf2acb` | `nips/openagents/NIP-PRO.md` | 165 | 0.17 |
| `dffed956ab` | `nips/openagents/NIP-PRG.md` | 166 | 0.17 |

**So the three recordings are not three samples.** They are one deterministic
call over three states that differ by one path string, and the 0.01 spread
the issue reads as reproducibility is the effect of that one string. A
deterministic door asked the same question three times gives one number three
times; a spread is a difference in the request.

Appending the six paths to the recorded plan sentence reproduces the
direction of the miss without reproducing the number. On `kev-4b`, over the
eight development states, every state moves down when the paths are appended,
by 0.04 to 0.50:

| Truth | Plan alone | Plan plus the six paths | Move |
| --- | --- | --- | --- |
| yes | 0.76 | 0.26 | -0.50 |
| no | 0.71 | 0.58 | -0.13 |
| yes | 0.68 | 0.64 | -0.04 |
| no | 0.84 | 0.76 | -0.08 |
| yes | 0.75 | 0.40 | -0.35 |
| no | 0.61 | 0.51 | -0.10 |
| yes | 0.69 | 0.60 | -0.09 |
| no | 0.83 | 0.76 | -0.07 |

The largest move is on the state the golden names, and it is content that
says nothing about writing. Hosted Jev does not move at all under the same
padding: every one of the sixteen answers shifts by at most 0.03 and all
sixteen stay correct.

The golden's own `.meta.json` says `provenance: staged`, `orchestrator: shell
script driven by Claude Opus 5`, and the script is not committed. The
recorded `arguments` carry no `model` field either, which the API requires,
so they were reconstructed rather than captured. A staged golden that
reconstructs its request bodies produces a record that reads like evidence and
does not replay. [#9412](https://github.com/OpenAgentsInc/openagents/issues/9412)
tracks that task becoming observed; capturing the request body verbatim is
part of what "observed" has to mean.

**What survives from the issue's observation:** a door was asked a question
whose answer its state asserted, and answered wrongly. That is real, it is
below where it should be on every local door, and the rest of this document
measures it properly. What does not survive is the specific number, the claim
that three recordings confirm it, and any reading of 0.17 as the door's answer
to the state the record shows.

## The panel

`restatement-v1`, built by
[`crates/gym/suites/build_restatement_v1.py`](../../../crates/gym/suites/build_restatement_v1.py),
digest `6babbe78a2853f7f`. Twenty-four delegation plans crossed with eight
question texts, 192 items.

Every state names three facts and nothing else: whether the plan has six
tasks, whether the tasks work on different files, and whether any task
writes. The three vary independently, which gives eight cells, and each cell
holds three states in three sentence shapes. The replicates are the
partitions, so each partition carries one whole factorial rather than a
random draw from it.

Eight question texts ask about those same facts. Seven are restatements,
whose answer the state asserts; the eighth is the inference `delegate-fan-out`
gates on.

| Family | Question | True when |
| --- | --- | --- |
| `count_six` | The plan has six tasks. | six tasks |
| `distinct_files` | Every task works on a different file. | different files |
| `count_and_files` | The plan has six tasks and every task works on a different file. | both |
| `readonly_simple` | Every task is read-only. | no task writes |
| `readonly_conjunction` | Every task is read-only and writes no file. | no task writes |
| `writes_none` | No task writes a file. | no task writes |
| `writes_some` | At least one task writes a file. | some task writes |
| `independent` | The tasks can run in parallel without colliding. | different files, or nobody writes |

`readonly_conjunction` is the recorded text word for word. **It was not
reworded**, which is the issue's first constraint and
[`docs/text-optimization.md`](../../text-optimization.md)'s standing finding:
nine optimizer programs, no wins, one measured negative. A reword would be a
second question set with its own id, its own floor, and its own held-out
split. `independent` drops the word "six" from the recorded text, because the
panel's plans name two through ten tasks; that is the one departure from a
recorded text here, and the six-task states are reported apart below.

`writes_none` and `writes_some` carry exactly complementary labels on every
state. That pair is what makes a sign problem visible: a door that reads the
fact and inverts it scores near zero on one and near one on the other, and a
door that reads the topic rather than the claim gives both the same number.

Labels are read off the state by rules fixed before the states were written,
and each item carries its rule in `label_rule`. Two tasks collide when they
touch the same file and at least one writes, so `independent` is false only
in one of the four combinations — 18 true and 6 false. The six false ones are
where a fan-out's safety lives, and they are scored apart from the rest.

### The doors and the run

| Door | Base | Where |
| --- | --- | --- |
| `kev-0.5b` | `Qwen/Qwen2.5-0.5B` | `127.0.0.1:11502`, CPU, fp32 |
| `kev-4b` | `Qwen/Qwen3-4B-Base`, `906bfd4b4dc7f14e` | `127.0.0.1:11501`, CPU, fp32 |
| `kev-8b` | `Qwen/Qwen3-8B-Base`, `49e3418fbbbca6ec` | `127.0.0.1:11503`, CPU, fp32 |
| `jev (hosted)` | closed | TypeSafe, `jev-latest` |

Each door answered all 128 open items, refused none, and lost none to the
harness. The 512 rows are in
`crates/gym/results/restatement-v1.jsonl`, question set `restatement-v1` at
digest `5c072034b7fdb0ba`, judged by `probability-v1` at digest
`gate:368cefd18f30`, and the receipt chain verifies. Mean latency: 128 ms for
`kev-0.5b`, 202 ms for hosted Jev, 868 ms for `kev-4b`, 1,844 ms for
`kev-8b`.

The locked partition — 64 items, one more whole factorial — is **unread**. No
candidate was selected here, so there is nothing for a held-out set to
confirm. It stays for the first retrained checkpoint or reworded question set
that claims to fix this.

### The floors

[`lev/measurements/2026-09-19-seed-variance.md`](lev/measurements/2026-09-19-seed-variance.md)
puts a two-door accuracy comparison at **0.056** at two sigma. That floor is
seed-block variance on a sampling estimator over 196 items. It does not bind
the same way here, in both directions:

- The kev doors are deterministic, checked at the top of this run, so seed
  variance is zero and this part of the floor does not apply.
- Sixteen items per family per door is small. The sampling error on a
  per-family accuracy runs to 0.125, so a per-family difference needs about
  0.25 to clear two sigma. Per-family claims below are therefore stated as
  exact two-sided binomial tests against chance rather than as differences.

The calibration floors from
[#9376](https://github.com/OpenAgentsInc/openagents/issues/9376) — ECE
0.0266, Brier 0.0119, log loss 0.6428 — are not used to judge anything here.
`probability-v1` reads every family `unverifiable` on this suite, because
eight calibration items is below its own floor of 30. No calibration claim is
made from eight items.

## What the doors did

Accuracy over both open partitions, 16 items per cell.

| Family | `jev (hosted)` | `kev-0.5b` | `kev-4b` | `kev-8b` |
| --- | --- | --- | --- | --- |
| `count_six` | 1.00 | 0.94 | 0.44 | 1.00 |
| `distinct_files` | 1.00 | 0.81 | 0.88 | 0.94 |
| `count_and_files` | 1.00 | 0.69 | 0.50 | 1.00 |
| `readonly_simple` | 1.00 | 0.81 | 0.44 | 0.75 |
| `readonly_conjunction` | 1.00 | 0.50 | 0.50 | 0.75 |
| `writes_none` | 1.00 | 0.50 | **0.00** | 0.75 |
| `writes_some` | 1.00 | 0.69 | 0.88 | 0.75 |
| `independent` | 1.00 | 0.56 | 0.75 | 0.75 |

| Door | All 128 | The 7 restatements | Count and file facts | Read and write facts |
| --- | --- | --- | --- | --- |
| `jev (hosted)` | **1.000** | 1.000 | 1.000 | 1.000 |
| `kev-8b` | 0.836 | 0.848 | 0.979 | 0.750 |
| `kev-0.5b` | 0.688 | 0.705 | 0.812 | 0.625 |
| `kev-4b` | 0.547 | 0.518 | 0.604 | 0.453 |

The three gaps between adjacent doors — 0.164, 0.148, and 0.141 over 128
items — all clear 0.056 and all clear two standard errors of their own
sample. Hosted Jev is better than every local door on this panel by a margin
that is not noise.

Mean probability on the items where the answer is yes, against the items
where it is no. A door that reads the fact separates them; a door with no
signal does not.

| Family | Door | Mean on yes | Mean on no | Gap |
| --- | --- | --- | --- | --- |
| `readonly_simple` | `jev (hosted)` | 0.97 | 0.02 | **+0.95** |
| `readonly_simple` | `kev-8b` | 0.91 | 0.46 | +0.45 |
| `readonly_simple` | `kev-0.5b` | 0.56 | 0.36 | +0.20 |
| `readonly_simple` | `kev-4b` | 0.66 | 0.59 | **+0.07** |
| `readonly_conjunction` | `jev (hosted)` | 0.95 | 0.03 | **+0.93** |
| `readonly_conjunction` | `kev-8b` | 0.89 | 0.48 | +0.41 |
| `readonly_conjunction` | `kev-0.5b` | 0.24 | 0.30 | **-0.06** |
| `readonly_conjunction` | `kev-4b` | 0.71 | 0.78 | **-0.07** |
| `writes_none` | `jev (hosted)` | 0.97 | 0.11 | +0.87 |
| `writes_none` | `kev-8b` | 0.51 | 0.31 | +0.20 |
| `writes_none` | `kev-0.5b` | 0.30 | 0.17 | +0.13 |
| `writes_none` | `kev-4b` | 0.14 | 0.87 | **-0.73** |
| `writes_some` | `jev (hosted)` | 0.90 | 0.03 | +0.88 |
| `writes_some` | `kev-4b` | 0.94 | 0.20 | +0.74 |
| `writes_some` | `kev-0.5b` | 0.93 | 0.53 | +0.40 |
| `writes_some` | `kev-8b` | 0.85 | 0.47 | +0.38 |

On the question the issue is about, `kev-4b` has a gap of -0.07 and
`kev-0.5b` a gap of -0.06. Neither is confidently wrong in the way the issue
describes; both have no signal at all and a level that drifts with the rest of
the state.

## The four hypotheses

### 1. Polarity: survives, in an amended form

The issue asks whether the door reads "read-only" as a negation and inverts.
The pair built to answer that says something sharper.

`kev-4b` scores **0 of 16** on `writes_none` — "No task writes a file" — and
14 of 16 on `writes_some` — "At least one task writes a file" — over the same
sixteen states. An exact two-sided binomial puts 0 of 16 at *p* = 0.00003.
That result replicates on both open partitions separately, 0.00 and 0.00.

It is not an inversion. Item by item, the two answers agree rather than
complementing each other:

| State | `writes_none` | `writes_some` | Sum |
| --- | --- | --- | --- |
| Six tasks, different files, read-only | 0.27 | 0.59 | 0.86 |
| Six tasks, different files, writing | 0.95 | 0.98 | 1.93 |
| Six tasks, one file, read-only | 0.24 | 0.18 | 0.42 |
| Six tasks, one file, writing | 0.84 | 0.87 | 1.71 |

A door that reads a proposition and its negation returns two numbers that sum
to one. Over the sixteen states, `kev-4b`'s two numbers sum to between 0.03
and 0.86 on the eight read-only plans and to between 1.59 and 1.97 on the
eight writing ones. The sum never lands between 0.87 and 1.58.

| Pair of families | `jev (hosted)` | `kev-0.5b` | `kev-4b` | `kev-8b` |
| --- | --- | --- | --- | --- |
| `writes_none` with `writes_some` | **-1.00** | -0.35 | **+0.95** | -0.20 |
| `readonly_conjunction` with `writes_some` | -0.98 | +0.72 | +0.45 | -0.67 |
| `readonly_simple` with `writes_none` | +0.98 | +0.58 | -0.12 | +0.55 |
| Mean \|p(A) + p(B) - 1\| | **0.01** | 0.17 | **0.73** | 0.26 |

**The finding is polarity insensitivity, not polarity inversion.** `kev-4b`
answers "does this plan involve writing?" and attaches that number to
whichever proposition it is handed. `writes_some` scores 0.88 because its
label happens to agree with that reading, and `writes_none` scores 0.00
because its label is the opposite. `kev-0.5b` does the same thing more
weakly, answering `readonly_conjunction` and `writes_some` with a **+0.72**
correlation where they should be opposed. Hosted Jev is at -1.00 and 0.01,
which is what reading the negation looks like.

The severity differs by checkpoint, and it is not one bad adapter. `kev-4b`
is the extreme case. `kev-0.5b` and `kev-8b` read the negation weakly, at
-0.35 and -0.20 where hosted Jev reads it at -1.00, and their answers to a
proposition and its negation still miss summing to one by 0.17 and 0.26
against Jev's 0.01. Over two base model families and three checkpoints, no
kev door reads a Noul's polarity the way the hosted door does.

### 2. The question being a conjunction: refuted

`readonly_simple` — "Every task is read-only", one clause, no conjunction —
scores 0.44 on `kev-4b`, no better than the conjunction's 0.50, with a gap of
+0.07 against -0.07. Splitting the question changes nothing.

In the other direction, `count_and_files` is a conjunction of two positive
restatements and scores **1.00** on hosted Jev and **1.00** on `kev-8b`.
A conjunction is not what breaks.

### 3. Restatement in general: refuted, and this was the alarming one

Hosted Jev answers **128 of 128** items correctly, every restatement family at
16 of 16, *p* = 0.00003 each. `kev-8b` is 1.00 on `count_six` and 1.00 on
`count_and_files`; `kev-0.5b` is 0.94 on `count_six`. A door that could not
repeat a fact its state asserts would fail those too.

So restatement works. What fails is one fact — whether anything writes — on
the kev checkpoints, and a question's negation, on `kev-4b` hardest.

`kev-4b` is also weak on `count_six` (0.44, gap +0.16) where `kev-8b` and
hosted Jev are perfect, so some of what the issue saw is this checkpoint
being weak on this panel generally: 0.547 over all 128 items, below both
other kev doors.

### 4. The family having no calibration map: true, and not the fix

The issue is right that `routing` is the only family with an admitted
calibration map, so this signal is uncalibrated by construction. That bounds
what 0.17 means. It cannot be the explanation, and fitting a map cannot be
the repair, for a reason that is structural rather than empirical:

`Map::apply_distribution` in `crates/gym/src/calibrate.rs` takes the option
the estimator selected and rescales *its* probability. The selected option
does not change, and `eval::mapped_observations` scores a mapped row with the
`correct` flag the raw answer produced. A calibration map in this harness
moves ECE, Brier, and log loss, and moves accuracy by exactly zero.

**Corrected on 2026-09-19.** This paragraph originally read "the winner does
not change", which claimed something stronger and false: the rescaled
distribution's own argmax moves to the runner-up whenever the calibrated
probability falls below the runner-up's share of the redistributed
remainder, which is possible below one half and impossible at or above it.
The conclusion above survives, and the reason it survives is the contract
rather than the arithmetic — a map calibrates a fixed answer and never picks
a different one, so the answer's outcome cannot move and accuracy through a
map is the raw accuracy. openagents#9438 enumerated both questions, corrected
the consumers that were reading the rescaled argmax, and recorded the result
in
[`../gym/measurements/2026-09-19-calibration-and-the-argmax.md`](../../gym/measurements/2026-09-19-calibration-and-the-argmax.md).

A map corrects how sure a door is. Every failure above is a door being sure of
the wrong thing, and no reliability table repairs that.

## Two things that could have confounded the panel, and did not

**The panel asks one question per call; the recorded call asked three.**
`crates/kev/src/encode.rs` packs each question as its own branch under a
block-causal mask, so a token attends to the state or to its own question and
nothing else. Measured on `kev-4b` over the eight development states, asking
`readonly` alone and asking it inside the recorded triple agree to the last
digit on **8 of 8**. The panel and the recorded call are comparable.

**The gym serves every question under the id `q`, not under the recorded
name.** `Record` in `crates/kev/src/api.rs` carries the state text, the
rendered instructions, and the option texts; the id goes into `Meta`, which
never reaches the tokenizer. A Noul with no criteria scores two option spans
reading `no` and `yes`, so the instruction text is the only thing that can
tell the pointer head which way round the question is. The isolation check
above measures this too.

## Is the independence gate safe to rely on?

**No, on every local door.**

`delegate-fan-out` gates its fan-out on `independent` with
`refuse_below: 0.7`. The panel holds six plans where the tasks share a file
and at least one writes, which is where the gate has a job to do. Over both
open partitions:

| Door | Mean on colliding plans | Highest | Cleared the 0.7 gate |
| --- | --- | --- | --- |
| `jev (hosted)` | 0.15 | 0.23 | **0 of 4** |
| `kev-0.5b` | 0.81 | 0.89 | **4 of 4** |
| `kev-4b` | 0.83 | 0.90 | **3 of 4** |
| `kev-8b` | 0.96 | 0.97 | **4 of 4** |

Eleven of twelve colliding plans clear the gate on the local doors, and
`kev-8b` clears it at 0.96 and 0.97 — the confident-miss shape that
[`research/2026-09-19-capability-sockets.md`](../research/2026-09-19-capability-sockets.md)
records for compiled adapters and that
[#9383](https://github.com/OpenAgentsInc/openagents/issues/9383) argues no
threshold fixes. Raising the bound does not help: the wrong answers sit above
the right ones.

Per-door accuracy on `independent` is 0.75 for `kev-4b` and `kev-8b`, which
looks survivable and is not: 12 of the 16 items are plans that *are*
independent, so a door that says yes to everything scores 0.75. `kev-8b`'s
gap between true and false items is **+0.00**. That is a door with no signal
scoring three quarters.

Restricting to the six-task plans, the gated question's own domain, does not
change the reading. The two colliding six-task plans come back at 0.73 and
0.88 on `kev-0.5b`, 0.90 and 0.90 on `kev-4b`, and 0.97 and 0.97 on
`kev-8b` — six answers, all above the gate. Hosted Jev answers them 0.07 and
0.23.

**What to do with that, for [#9413](https://github.com/OpenAgentsInc/openagents/issues/9413):**

- Pointing `delegate-fan-out` at a real backlog on a local kev door puts the
  fan-out's safety on a signal this panel cannot find. Do not.
- Hosted Jev answers all 16 `independent` items correctly and none of the
  colliding plans above 0.23. It is the only door measured here that the gate
  can rest on.
- The deterministic admission check is what actually protected the recorded
  episode. It stays load-bearing, and the `isolation: worktree` bound is what
  makes a colliding plan safe regardless of what the door says. Prefer
  enforcing isolation over asking whether it is needed.

## What is measured, what is inferred, and what is not established

**Measured.** Everything in the tables: 512 rows, four doors, 128 items each,
recorded with their suite, question, and gate digests. The reproduction
failure of the golden's independence call. The determinism of `kev-serve`.
The question-isolation check. The padding probe.

**Inferred.** That the missing 94 tokens of state are the six file paths.
The token count tracking a path rename across three recordings is strong, and
fifteen reconstructions of the state missed 165 and 166 tokens, so the actual
request body is gone. Every candidate that carried the paths, from 124 to 374
tokens, put `readonly` between 0.25 and 0.50 — below the 0.76 the recorded
state produces and above the 0.17 the golden reports.

**Not established.** Why the kev adapters do not read negation. That needs the
training data, not another panel. Whether hosted Jev's perfect score holds
outside these 24 states — 128 of 128 on an authored panel is a ceiling
result, and a ceiling result says the panel is not hard enough to separate
Jev from a better Jev, not that Jev is perfect.

**Not claimed.** Any calibration comparison. Eight calibration items is below
`probability-v1`'s floor of 30, the gate says so, and no number in this
document is judged against ECE 0.0266, Brier 0.0119, or log loss 0.6428.

## Reproducing this

```sh
cargo build --release -p kev --features serve --bin kev-serve
cargo build --release -p gym --bin gym

target/release/kev-serve --adapter-dir ~/work/kev-artifacts/kev-4b \
    --base-dir ~/work/kev-artifacts/qwen3-4b --default kev-4b \
    --port 11501 --device cpu --dtype fp32

python3 crates/gym/suites/build_restatement_v1.py > crates/gym/suites/restatement-v1.json

target/release/gym eval --suite crates/gym/suites/restatement-v1.json \
    --door kev-4b=http://127.0.0.1:11501 \
    --record crates/gym/results/restatement-v1.jsonl --timeout 300 --fit
```

The same two commands ran for `kev-0.5b` on `qwen2.5-0.5b` and `kev-8b` on
`qwen3-8b`. Hosted Jev ran with `--jev` and `TYPESAFE_API_KEY` in the
environment, into its own store, folded in with `gym merge` so two writers
never shared one receipt chain.

To replay the golden's independence call, post its recorded `state` and
`questions` to a `kev-4b` door with `"model": "kev-latest"` added, and compare
`usage.input_tokens` against the 166 the golden records.
