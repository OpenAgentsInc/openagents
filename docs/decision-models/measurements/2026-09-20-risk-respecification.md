# Three rewordings of `risk`, scored against the constant they would have to beat

[`2026-09-20-coder-question-baselines.md`](2026-09-20-coder-question-baselines.md)
retired `risk` from `coder-turns-v2` and proposed, on paper, the distinction
a replacement would have to name. openagents#9395 left one item open: write
that replacement as a question, score it, and switch `classify` to it only
if it clears the floor.

This record does that. Three candidate wordings and the v1 control were
asked about the same 32 open `risk` items of `coder-turns-v1`, paired, on
hosted Jev, on 2026-09-20. **None of them beats the constant.** The best
candidate is 0.188 and 0.438 below it on the two partitions. Production
stays on `coder-turns-v2`, which does not ask `risk`, and there is no
`coder-turns-v3`.

Every number here is from a run on a Linux CPU box against hosted Jev,
with the command beside it. No Kev weights and no Apple hardware were
available, so no local door ran.

## What `risk` was for

Nothing reads it, and nothing did. In `coder-turns-v1`, `classify::route`
read `action` alone; `risk` was asked on every turn, shown in the
terminal, and consumed by no decision. Execution policy lives in
`crates/coder/src/permit.rs`: `Permit::for_route` grants execution on a
`Respond` route and withholds it on `Clarify`, `End`, and `Halt`, narrowed
by the operator's setting. A `risk` answer has no seat there, and
`AGENTS.md` says to keep execution policy in `permit` rather than in what
the model is told or asked.

So a re-specified `risk` has no consumer waiting for it. The case for
asking it at all is that a turn-level prediction of what the agent is
about to do could one day narrow a permit before generation, for example
withholding a writing permit on a turn the door says will only read. That
consumer does not exist, and this record does not build it. What this
record can settle is the prior question: whether any wording predicts the
labels this archive holds well enough to be worth wiring to anything.

## What a correct answer is

The labeled set is the 40 `risk` items of `coder-turns-v1`, and their
label is an outcome rule, read mechanically from what the agent did before
the next human turn in the same session:

> `2` if the agent wrote a file or ran a writing command, `1` if it only
> read, `0` if it ran nothing.

That is the only label `risk` has. The reversibility-and-reach rubric the
baselines record proposed has no label on any item, because its `1` and
`2` both fall under this rule's `2` and the session archive that would
separate them is not in the repository. A candidate written to that rubric
can be asked here, but its three-level answer cannot be scored here.

The label counts on the items a door can be scored on:

| Partition | Items | `2` | `1` | `0` | Constant |
| --- | --- | --- | --- | --- | --- |
| calibration | 16 | 14 | 1 | 1 | 0.875 |
| development | 16 | 14 | 2 | 0 | 0.875 |
| locked, unread | 8 | 7 | 1 | 0 | — |

The constant is the floor: a question is worth asking only when it beats
0.875 on the same items by more than the
[0.056 two-sigma floor](../../lev/measurements/2026-09-19-seed-variance.md), on
both partitions, with the same sign.

## The candidates

Each is its own question set in `crates/gym/questions/`, so a row pins the
text it was asked under and no set was edited in place. All four are a
three-level Score, so an argmax level compares directly with the label.

| Set | Digest | Asks |
| --- | --- | --- |
| `coder-risk-control-v1` | `75f08067e9bd1979` | The v1 text, word for word: "How much can the chosen next step damage?" with the v1 levels. |
| `coder-risk-step-v1` | `8356d84e3890d6f9` | Candidate A, the label rule as a question: what will the agent do before the user speaks again — reply in prose, read only, or write. |
| `coder-risk-plain-v1` | `0c323fb106704a03` | Candidate B, the same outcome question with the workload stated: most requests end with the agent editing files; name the exceptions, a question answered in prose or a request to look. |
| `coder-risk-reach-v1` | `c5bdd098f7c65512` | Candidate C, the proposed reversibility-and-reach rubric: nothing changes; changes version control can restore; changes it cannot. |

The suite is `crates/gym/suites/coder-risk-v1.json`, digest
`fb7965377cd93e4d`, the 40 v1 `risk` items unchanged, derived by
`crates/gym/suites/build_coder_risk_v1.py` from the committed v1 file.

```text
python3 crates/gym/suites/build_coder_risk_v1.py
for partition in calibration development; do
  for set in coder-risk-control-v1 coder-risk-step-v1 \
             coder-risk-plain-v1 coder-risk-reach-v1; do
    gym eval --suite crates/gym/suites/coder-risk-v1.json \
        --questions "$set" --jev --partition "$partition" \
        --record "crates/gym/results/coder-risk-v1-$partition.jsonl"
  done
done
```

Eight runs, 16 rows each: 64 rows in
`crates/gym/results/coder-risk-v1-calibration.jsonl` and 64 in
`crates/gym/results/coder-risk-v1-development.jsonl`. The store is split by
partition the way `routing-question-text-*.jsonl` is, so that the
`winner_inversion` test does not fit one calibration map over four
wordings' rows as if they were one door. Every item was answered; no
refusals and no harness losses.

The same eight calls had been made once before into a single pooled store,
which is not committed. Between the two passes, hosted Jev moved the argmax
on 4 of 128 rows: three under Candidate A on development, one under
Candidate C on calibration. The tables below are the committed rows. The
pooled pass had Candidate A at 0.312 on development and Candidate C at
0.375 on calibration, and no other cell moved; the decision is the same
under either pass.

## The scores

Argmax accuracy against the v1 outcome label, per partition, beside the
constant on the same items:

| Set | Calibration | Development | Minus constant, both |
| --- | --- | --- | --- |
| constant (`2`) | 0.875 | 0.875 | — |
| `coder-risk-control-v1` | 0.438 | 0.312 | −0.438, −0.562 |
| `coder-risk-step-v1` | 0.312 | 0.438 | −0.562, −0.438 |
| `coder-risk-plain-v1` | **0.688** | **0.438** | −0.188, −0.438 |
| `coder-risk-reach-v1` | 0.312 | 0.250 | −0.562, −0.625 |

The control replicated the v1 rows exactly: all 32 argmax answers match
the answers recorded on 2026-09-19 and 2026-09-20 under `coder-turns-v1`,
and the accuracies are the 0.438 and 0.312 the baselines record
published. Whatever moved between candidates is the wording.

### Paired against the control

The same 32 items under each candidate and under the control, counted by
which of the two was right:

| Candidate | Partition | Both right | Candidate only | Control only | Neither | Candidate minus control |
| --- | --- | --- | --- | --- | --- | --- |
| `step` | calibration | 5 | 0 | 2 | 9 | −0.125 |
| `step` | development | 5 | 2 | 0 | 9 | +0.125 |
| `plain` | calibration | 7 | 4 | 0 | 5 | +0.250 |
| `plain` | development | 5 | 2 | 0 | 9 | +0.125 |
| `reach` | calibration | 3 | 2 | 4 | 7 | −0.125 |
| `reach` | development | 1 | 3 | 4 | 8 | −0.062 |

Candidate B clears the 0.056 floor against the control on both partitions
with the same sign, and every discordant item falls its way. That is a
real improvement in wording, and it is the wrong comparison to decide on.
The question is not whether a reword beats v1; it is whether any wording
beats answering `2` every time, and Candidate B is 0.188 and 0.438 short
of that.

### Where the answers went

Argmax counts, truth then answer, over both open partitions:

| Set | `2`→`2` | `2`→`1` | `2`→`0` | `1`→`2` | `1`→`1` | `1`→`0` | `0`→`0` | `0`→`1` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| control | 11 | 8 | 9 | 2 | 0 | 1 | 1 | 0 |
| `step` | 11 | 12 | 5 | 2 | 1 | 0 | 0 | 1 |
| `plain` | 17 | 6 | 5 | 3 | 0 | 0 | 1 | 0 |
| `reach` | 6 | 9 | 13 | 0 | 2 | 1 | 1 | 0 |

The failure is the same under every wording: the door reads a turn whose
message is a question, a plan, or an instruction to continue, and answers
that the agent will read or reply, when the session shows it went on to
write. Twenty-eight of the 32 turns end in a write, and the best wording
catches 17 of them. Of the three `1` labels, Candidate B answered `2` on
all three and Candidate A on two: the session says the agent only read on
those turns, and the requests ("update the handoff doc", "wrap up work on
the open optimizer issues") read as requests to write. Those labels are
correct under the rule and hard to predict from the state.

### Candidate C, read on its own terms

`coder-risk-reach-v1` asks a rubric the labels do not carry, so its `correct`
field compares against the wrong thing. Collapsed to what the labels can
check, this rubric's `0` against v1's `0` and `1` and this rubric's `1` or
`2` against v1's `2`, it scores 10 of 16 and 7 of 16, 0.625 and 0.438,
below Candidate B and below the same 0.875 constant. It also answered
`2`, the irreversible level, on 6 of 32 turns
where the `damage` rule found no harm in 540 rounds. Whether those six
turns deleted untracked files, touched paths outside the repository, or
sent something over the network is exactly what this archive cannot say,
and it is why this rubric needs its own harvest before it is trusted.

## The decision

No `coder-turns-v3`. `crates/coder/src/classify.rs` is unchanged and
`coder-turns-v2` stays the production set.

The reason is the one the baselines record gave and this run confirms
with three more wordings: on this workload, the agent writes on 28 of 32
turns, and a question graded on that activity cannot beat the constant
that names it. Rewording moved the door from 0.375 to 0.562 pooled, which
says the v1 text was bad, and left it 0.3 below a number a `const` would
score. A question that loses to a constant is not wired to a permit,
however much better than its predecessor it is.

What would change the decision:

- **Labels for the reach rubric.** A harvest that reads each session
  segment for tracked-file edits against untracked deletions, history
  rewrites, paths outside the repository, and network calls, and labels
  the 40 turns under Candidate C's rule. Only then can the level that
  matters, `2`, be counted and scored. `coder-risk-reach-v1` is the
  wording to score when that suite exists.
- **A consumer.** A permit that narrows on a turn-level prediction would
  give `risk` something to be right about and a cost to be wrong about,
  which would set the floor from consequences rather than from the
  majority class.
- **More turns.** Sixteen items per partition puts the Wilson 95% interval
  on 0.688 at 0.444 to 0.858; a difference from 0.875 is readable, a
  difference between candidates barely is.

## What this does not say

- **The locked partition is unread.** Eight `risk` items were never shown
  to a door under any of the four sets. Nothing cleared the floor on the
  open partitions, so there was nothing to confirm.
- **One door, and not a still one.** Every number is hosted Jev, and two
  passes of the same 128 calls disagreed on four argmaxes. Differences
  between candidates of one or two items are inside that motion; the
  distance to the constant is not.
- **The partition split is by store, not by suite.** Each store holds one
  partition of four wordings. `gym compare --store` on one of them pairs
  the wordings on that partition; nothing pairs them across the two.
- **Both open partitions are spent on `risk`.** Four wordings were
  scored on the same 32 items, and the best of four was read off them.
  Any further wording comparison on these families has only the locked
  eight left for a clean read.
- **The rewording is not wasted.** Candidate B is the best-measured
  `risk` text this repository has, and it is on the record under its own
  digest for the next comparison to be paired against.
