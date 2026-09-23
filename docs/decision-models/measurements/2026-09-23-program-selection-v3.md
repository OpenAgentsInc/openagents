# Program selection with review-runs

On 2026-09-23 the repository gained a sixth program, `review-runs`, which
answers a question about Terminal-Bench runs from the Gym's records
([#9574](https://github.com/OpenAgentsInc/openagents/issues/9574)). This host
admits it, so the question every turn asks,
[`openagents.program.v1`](../../../questions/program.json), gained a fifth
option. A new option is a new question, so the production text is
`program-selection-v3`, and this report is its first baseline, beside
[v1](2026-09-19-program-selection.md) and
[v2](2026-09-20-program-selection-v2.md).

The short version:

- **Hosted Jev recognizes a question about runs 2 times in 6.** It misses two
  to `none` and sends two to `answer-question`.
- **It never selects `review-runs` when it shouldn't:** 0 spurious
  selections of it across the 72 open items that aren't run questions.
- **On real turns it is correct on 29 of 32, one fewer than v2's 30.** The
  extra error is a `delegate-fan-out` selection at 0.43, not a
  `review-runs` one.

So the router is a poor way in to `review-runs` today. The reliable ways in
are `?` in the Gym terminal's Runs pane and `coder-one ask`, which don't
depend on the selection question at all.

## What was asked

The option set is the six programs in `programs/` that this host admits,
plus `none`:

| Option | Summary the question offers |
| --- | --- |
| `none` | An ordinary turn the agent answers itself. |
| `answer-question` | Answers a question about the repository from its own contents. |
| `burn-down` | One delegated session per item of `.coder/work-list.json`. |
| `delegate-fan-out` | One delegated session per task, in parallel. |
| `review-runs` | Answers a question about Terminal-Bench runs from the Gym's records, with checked citations. |

`run-suite` and `review-changes` are still not offered, for the reasons v2
gives. The committed copy is
[`crates/gym/questions/program-selection-v3.json`](../../../crates/gym/questions/program-selection-v3.json),
and `cargo test -p coder --test suite_questions` fails when it drifts from
what `questions/program.json` and `programs/` produce.

## What it was asked about

[`crates/gym/suites/program-selection-v3.json`](../../../crates/gym/suites/program-selection-v3.json),
95 items, digest `7f8cdfb93b439643`, built by `build_program_selection_v3.py`:

| Source | Items | What it can say |
| --- | --- | --- |
| v2's items, unchanged | 80: 32 real turns, 36 authored open, 12 locked | Whether the new option pulls answers away from the old ones. |
| New authored items | 15: 10 open, 5 locked | Whether a question about runs is recognized, and whether run-adjacent requests stay `none`. |

The new items, in
[`program-selection-v3-judgments.json`](../../../crates/gym/suites/program-selection-v3-judgments.json),
are 9 questions about runs and 6 requests that mention runs or the Gym but
ask for no program: how to use `gym runs`, where its code is, a negated
request, a request to start a run, one to mark a run, and one to summarize a
Gym document. Every partition has both kinds. The label rule:

> `review-runs` for a question about Terminal-Bench runs themselves (their
> outcomes, failures, rankings, or Jev's judgments of them) that the Gym's
> records answer; `none` for questions about how the Gym, its commands, or
> its code work, for requests to start, mark, or change runs, and for
> negated requests.

Unlike `answer-question`, whose label needs an explicit delegation, a
question about runs is labelled `review-runs` without one, because an
ordinary turn can't read the Gym. The judgments file records that as a
dispute. Every label is one author's reading; none had independent review.
The 5 new locked items, and v2's 12, are unread.

## Hosted Jev

Hosted `jev-latest` answered all 78 open items, with no refusals and no
harness failures. The service supplies no independently verifiable weight
identity. The median HTTP latency was 1,039 ms, not a controlled latency
benchmark.

| Open set | Correct | Spurious selections | Missed requests | Wrong program | Confident errors at p ≥ 0.9 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Real turns | 29/32 | 3/31 negatives | 0/1 positive | 0 | 0 |
| Authored, v2's | 31/36 | 2/10 negatives | 1/26 positives | 2 | 0 |
| Authored, new | 5/10 | 1/4 negatives | 2/6 positives | 2 | 0 |

Raw calibration accuracy/ECE/Brier/NLL are 0.85/0.059/0.093/0.285, and
development 0.82/0.106/0.100/0.307. No map was fitted.

The six open questions about runs:

| Item | Request | Selected | p |
| --- | --- | --- | ---: |
| `cal-runs-unearned` | Which Terminal-Bench runs claimed success they didn't earn? | `review-runs` | 0.70 |
| `cal-runs-patterns` | Look through the runs Jev ranked most worth learning from and tell me what they share. | `review-runs` | 0.65 |
| `cal-runs-why-fail` | Why did Luna fail log-summary-date-ranges? | `answer-question` | 0.49 |
| `dev-runs-rank` | Why does the cargo-flight-dispatch run rank so high for learning? | `answer-question` | 0.45 |
| `dev-runs-output-paths` | Which failures come down to output paths? | `none` | 0.68 |
| `dev-runs-compare` | Compare how Coder One and Claude Code failed bun-sourcemap-leak. | `none` | 0.54 |

Jev picks `review-runs` when the request says "runs" or "Terminal-Bench"
outright, and not when it names a task or a failure the way an operator
does. The four open run-adjacent requests stay `none` except one: "Where in
the code does gym runs compute the learning order?" goes to
`answer-question` at 0.78, the overlap v2 already reports for ordinary
repository questions.

On v2's items the new option changed little: authored accuracy is the same
31 of 36, and real turns lost one item to a `delegate-fan-out` selection.
Nothing selected `review-runs` spuriously.

## What this means for the path

A spurious `review-runs` selection costs a Gym read and one ask, about a
cent on Luna and under a minute, and it answers a question about runs
nobody asked; none happened here. A missed one costs the operator a retry
with clearer words, or a trip to `?` in the Gym terminal. The error rate on
run questions is too high to call the router the way in: `review-runs` is
admitted and offered, and the selection question is not reworded here. A
reworded summary is a new question with a new id, measured on these items
before it ships.

## Reproduction

```sh
python3 crates/gym/suites/build_program_selection_v3.py > /tmp/program-selection-v3.json
cmp /tmp/program-selection-v3.json crates/gym/suites/program-selection-v3.json
cargo test -p coder --test suite_questions
cargo test -p gym --test program_selection_v3
gym eval --suite crates/gym/suites/program-selection-v3.json --jev --timeout 60 \
  --record crates/gym/results/program-selection-v3-jev.jsonl
gym compare --suite crates/gym/suites/program-selection-v3.json \
  --store crates/gym/results/program-selection-v3-jev.jsonl
python3 crates/gym/suites/score_program_selection_v3.py \
  crates/gym/results/program-selection-v3-jev.jsonl
```

[Raw rows](../../../crates/gym/results/program-selection-v3-jev.jsonl) and
[derived reports](../data/program-selection-v3/) keep every error and the
full open denominator; the store's receipt chain verifies.
