# Asking every turn whether it is a program request

`crates/coder/src/turn.rs` now asks one question before every turn: which
program does this request ask this machine to run, from the ones this host
would run, or **none**. A program answer runs the program. `none` — nearly
every turn — proceeds exactly as the turn did before the question existed.

[`docs/programs.md`](../../programs.md) warned against this question in as many
words: *do not ask "is this a delegation?" every turn*, because on the
real-turn suite six of seven production questions score no better than a
constant and this one would join them. It does join them. This is the
measurement that says so, and what it says is more useful than the warning.

The short version, in three numbers:

- **On 32 real turns the baseline is 0.969 and the headroom is 0.031** —
  below the 0.056 two-sigma floor. No accuracy win was available on real
  traffic before the door was asked, whatever it answered.
- **Hosted Jev scores 0.938 there, below the constant.** It costs two false
  positives in 31 negatives, and buys the one real program request.
- **It missed no program request: 0 false negatives in 9.**

The two errors are not the same size, which is why they are counted apart.

## What was asked

One Choice question, `openagents.program.v1`, over one field: the
operator's sentence. The wording is `questions/program.json` and the
options are the programs in `programs/` **this host would admit** — a
program it would refuse at admission is not a route, so it is not an
option. On this repository that is two of the four:

| Option | Offered | Why |
| --- | --- | --- |
| `none` | yes | Declared by the question set, so its wording is digested with the rest. |
| `delegate-fan-out` | yes | Every step's bounds are ones this host keeps. |
| `answer-question` | yes | One `delegate` step, bounded the same way. |
| `review-changes` | no | Names `openagents.review-finding.v1`, which this host has no wording for. |
| `run-suite` | no | Names a `check` this host does not run. |

The committed copy is `crates/gym/questions/program-selection-v1.json`.
On 2026-09-20 the repository gained a fifth program, `burn-down`, which
this host admits, so the question in production became
`program-selection-v2.json`: the same wording with a fourth offered option.
`cargo test -p coder --test suite_questions` fails when v2 drifts from the
repository's own files. The numbers in this report are about v1, the
three-option question; v2 has not been scored, and a row that pins it
would be the first measurement of the four-option one.

## What it was asked about

`crates/gym/suites/program-selection-v1.json`, 52 items, digest
`7b66bc4ce8336de2`, built by `build_program_selection_v1.py` from two
sources that are never pooled silently:

| Source | Items | What it can say |
| --- | --- | --- |
| `turn/…` | 40, of which 8 are locked | How often the question fires when nobody asked for a program. |
| `authored/…` | 12 | Whether it recognises a program request when it sees one. |

The real turns are the `action` family of
[`coder-turns-v1`](2026-09-19-coder-turns.md), reused rather than
re-harvested: one copy of those states in the repository, and the filters
`AGENTS.md` requires already applied to it. Each item keeps the id and the
partition that suite gave it, its locked partition included. The locked
eight were labelled and asked of nothing.

The written items exist because real traffic holds almost no program
requests — one in thirty-two — so nothing harvested could say whether the
question recognises one. **They are evidence about recognition and not
about prevalence**, and mixing the two would produce a number that means
neither.

Every label is a reading, so every item carries `label_source: author` and
the rule that produced it:

> A request is labelled with a program when it asks this machine to run
> that program's work — `delegate-fan-out` for several pieces of work handed
> to other sessions at once, `answer-question` for one question about the
> repository handed to an executor — and `none` otherwise.

The labels are in `program-selection-v1-judgments.json`, committed before
any door was asked so they can be argued with. Two are worth arguing with
and they are the same argument: `turn/1d845d65#23` ("task to subagent
maybe") and `turn/9b37638d#9` ("i need you to have a subagent add support
for glm 5.3 flash") each hand **one implementation task** to another
session, and this host has no program for that. `answer-question` answers a
question and writes nothing. `none` is the only truthful answer while that
is so, and a door that answers `answer-question` there is reading the
request rather than misreading it.

## The baseline, published before the accuracy

```text
python3 crates/gym/suites/score_program_selection_v1.py \
    crates/gym/results/program-selection-v1.jsonl
```

| Set | Items | Asks for a program | Constant `none` | Headroom |
| --- | --- | --- | --- | --- |
| real turns | 32 | 1 | 0.969 | 0.031 |
| written | 12 | 8 | 0.333 | 0.667 |
| both | 44 | 9 | 0.795 | 0.205 |

**The real-turn headroom is 0.031 against a two-sigma floor of 0.056.**
[#9392](https://github.com/OpenAgentsInc/openagents/issues/9392) spent a
complete experiment on a partition in exactly that position and could not
have registered a win at any strength. So an accuracy comparison on real
traffic is not available here, and saying so first is the point: the
numbers below are error counts, not a score.

## What hosted Jev did

44 items, both open partitions, 2026-09-19, `jev-latest`:

```text
gym eval --suite crates/gym/suites/program-selection-v1.json \
    --jev --timeout 300 \
    --record crates/gym/results/program-selection-v1.jsonl
```

| Set | Items | Accuracy | False positives | False negatives | Wrong program |
| --- | --- | --- | --- | --- | --- |
| real turns | 32 | 0.938 | 2 of 31 (0.065) | 0 of 1 | 0 |
| written | 12 | 0.833 | 1 of 4 (0.250) | 0 of 8 | 1 |
| both | 44 | 0.909 | 3 of 35 (0.086) | 0 of 9 | 1 |

Median latency 192 ms, slowest 517 ms, one call per turn on every turn.
`gym eval`'s own panel reads 0.88 accuracy on calibration and 0.93 on
development, ECE 0.143 and 0.109, no confident errors, nothing refused.

**On real traffic the door is 0.938 against a constant's 0.969.** Answering
`none` every time would have been right more often. What the question buys
is the one real program request, which a constant cannot ever get, at a cost
of two ordinary turns in thirty-one.

### Which error, and how much it costs

A false negative is a turn that answers normally. The operator asks again,
in more words. There were none.

A false positive runs a program nobody asked for, and that is the error
worth bounding. All four mistakes:

| Item | Answered | Labelled | Request |
| --- | --- | --- | --- |
| `turn/1d845d65#26` | `answer-question` 0.53 | `none` | "i want to see if that programs as weight arguments is similar to the plugin system we've designed, look thru ~/work/coder on that" |
| `turn/1d845d65#27` | `answer-question` 0.62 | `none` | "tell me the status of all open issues" |
| `authored/about-the-program` | `answer-question` 0.55 | `none` | "what does the delegate-fan-out program do?" |
| `authored/answer-rounds-max` | `delegate-fan-out` 0.71 | `answer-question` | "get a delegate to answer what ROUNDS_MAX is set to" |

**Every error involves `answer-question`, and none of them started a
subprocess.** Two things account for that, and the second is the one worth
keeping.

`answer-question`'s summary — "Answers a question about the repository from
its own contents" — describes what an ordinary turn does. The question is
being asked to separate two options that overlap, and nearly all the error
mass sits in that overlap. The canonical ordinary turn, "What does
ROUNDS_MAX do?", is answered `none` at 0.58 against `answer-question` at
0.42: right, and by very little. "how many delegations did the last fan-out
start?" comes back 0.55 to 0.33: also right, also narrowly.

**A program cannot fan out over work the request did not name.** The work
is the list the sentence carries, so an ordinary turn supplies none, and a
`query` step with nothing to look up refuses. A `delegate` step with nothing
to hand over now refuses too — it used to report "0 of 0 answered" and let
the program finish, which is how a wrongly selected `answer-question` would
have produced a turn that summarised nothing. Both false positives on real
turns end in a refusal the operator can read, and nothing runs.

So the cost of a false positive today is one turn that declines instead of
answering. That is a real cost and it is not six subprocesses, and the
reason is structural rather than a threshold anybody tuned.

### What a confidence floor would have done

Not a recommendation — 44 items is too few to set a threshold from — but the
separation is clean enough to record:

| Confidence | Answers |
| --- | --- |
| 0.93 – 0.99 | six `delegate-fan-out`, every one of them right |
| 0.79 | one `answer-question`, right |
| 0.71 – 0.53 | all four errors |
| 0.36 | one `answer-question`, right |

A floor at 0.75 would have removed every error at the cost of one true
`answer-question`. [#9383](https://github.com/OpenAgentsInc/openagents/issues/9383)
argues abstention belongs to the model as an option it names rather than to
a caller as a threshold applied afterwards, and this is the first place that
argument is acted on. The evidence here is mixed rather than decisive: the
named option carries the abstention (`none` was answered 32 times in 44, and
missed nothing), and a floor would still have caught what it let through.
The expensive option separates cleanly on its own — every true
`delegate-fan-out` at 0.93 or better, and no real turn was ever answered
`delegate-fan-out` wrongly.

## What this does not say

- **Nothing about a door other than hosted Jev.** kev and lev answer the
  same contract and were not asked.
- **Nothing about prevalence beyond this repository.** One program request
  in thirty-two real turns is this operator, this month, in this repository,
  and before an operator had any reason to phrase a request as one.
- **Nothing measured on the locked partition**, which is eight items and
  stays unread until there is a candidate worth spending it on.
- **The labels are readings.** They were written before the door was asked
  and they are the author's, which is what `label_source: author` says on
  every row.

## What to do next

1. **Reword `answer-question`'s summary to say it delegates**, and measure
   it as a candidate against these items. Every error here involves that
   option, and its description does not say the work leaves the machine.
2. **Ask the other doors.** A 192 ms hosted call on every turn is the
   latency this buys, and a local door may answer it as well for nothing.
3. **Re-measure when prevalence changes.** The one real program request in
   this suite predates the machine being able to run one. The rate an
   operator asks at, once asking works, is the number that decides whether
   this question earns its call.
