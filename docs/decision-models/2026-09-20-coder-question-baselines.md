# Each production question against the constant that would replace it

[`2026-09-19-coder-turns.md`](2026-09-19-coder-turns.md) scored
`crates/coder/src/classify.rs` on real turns and found that six of its seven
questions do no better than a constant. openagents#9395 asked for the
decision that finding implies: which questions to retire, which to
re-specify, and which to keep, each with its number and the command that
produced it.

This record is that decision. It measures the majority-class baseline of
every family on every partition of `coder-turns-v1`, scores hosted Jev on
the 130 calibration items the earlier run left unscored so that each
question is judged on 260 open items rather than 130, and publishes the
result as a new question set, `coder-turns-v2`, with a suite and 76
recorded rows behind it.

The short version:

- **Five questions are retired.** `damage` is `no` on 55 of 55 rounds.
  `needs_code`, `risk`, `progress`, and `useful` are read by no decision
  and, on 32 to 44 real items each, score between 0.5 below their constant
  and level with it. Nothing this suite can measure is lost by not asking
  them.
- **`shell_outcome` is kept.** It beats its constant by +0.227 on both
  partitions independently, 0.773 against 0.545, and it is the one input
  the shell loop reads.
- **`action` is kept and does not yet pay for itself.** Hosted Jev scores
  0.875 on 32 turns against a constant of 0.969. All four of its errors
  are the routing changes it made, and it missed the one turn the session
  shows was clarified. It stays because it is the router's only input, and
  removing it is removing the router, which is a product decision this
  record does not make.
- **`risk` is re-specified on paper and not asked.** The distinction a
  useful version has to name is written below with the label rule it
  needs. It is not in `coder-turns-v2`, because no item here carries a label
  for it, and a question shipped before it is scored is how v1 was chosen.

Every number below is from a run on 2026-09-20 against hosted Jev from a
Linux machine, with the command beside it. No Kev weights and no Apple
hardware were available, so no local door was scored; the retire decisions
rest on the label counts, which no door can move, and on the door `coder`
ships with.

## The baselines

The majority class of a family is what a router scores by answering every
item with the family's most common label. It is the floor a question has to
clear to be worth asking.

```text
python3 crates/gym/suites/baseline_coder_turns.py \
    crates/gym/suites/coder-turns-v1.json
python3 crates/gym/suites/baseline_coder_turns.py \
    crates/gym/suites/coder-turns-v1.json \
    --partition calibration --partition development
```

Counting labels does not read the locked partition in the sense the ledger
guards. No state is shown to a door and no answer is scored; the counts say
how the labels fall, as the suite's `sampling` field already does for the
shell rounds. The locked items remain unscored.

All 95 states, 325 items:

| Family | Items | Labels | Majority | Wilson 95% |
| --- | --- | --- | --- | --- |
| `damage` | 55 | 55 `no` | 1.000 | 0.935 to 1.000 |
| `action` | 40 | 39 `respond`, 1 `clarify` | 0.975 | 0.871 to 0.996 |
| `needs_code` | 40 | 38 `yes`, 2 `no` | 0.950 | 0.835 to 0.986 |
| `useful` | 55 | 49 `yes`, 6 `no` | 0.891 | 0.782 to 0.949 |
| `risk` | 40 | 35 `2`, 4 `1`, 1 `0` | 0.875 | 0.739 to 0.945 |
| `shell_outcome` | 55 | 29 `pass`, 25 `retry`, 1 `stop` | 0.527 | 0.398 to 0.653 |
| `progress` | 40 | 17 `0`, 16 `1`, 7 `2` | 0.425 | 0.285 to 0.578 |

The two open partitions, 260 items, which is what a door can be scored on:

| Family | Items | Labels | Majority | Wilson 95% |
| --- | --- | --- | --- | --- |
| `damage` | 44 | 44 `no` | 1.000 | 0.920 to 1.000 |
| `action` | 32 | 31 `respond`, 1 `clarify` | 0.969 | 0.843 to 0.994 |
| `needs_code` | 32 | 30 `yes`, 2 `no` | 0.938 | 0.799 to 0.983 |
| `useful` | 44 | 39 `yes`, 5 `no` | 0.886 | 0.760 to 0.950 |
| `risk` | 32 | 28 `2`, 3 `1`, 1 `0` | 0.875 | 0.719 to 0.950 |
| `shell_outcome` | 44 | 24 `pass`, 20 `retry` | 0.545 | 0.401 to 0.683 |
| `progress` | 32 | 15 `0`, 12 `1`, 5 `2` | 0.469 | 0.309 to 0.636 |

The Wilson interval is the honest width of a small sample. `damage` at
55 of 55 has a lower bound of 0.935: a `yes` rate above 6.5% is excluded,
and nothing below it is. The one `stop` and the one `clarify` in the
archive both sit in the locked partition, so on the items a door can be
scored on, `shell_outcome` has two classes and `action` has one `clarify`
against 31 `respond`.

## Hosted Jev against each constant, on 260 items

The earlier record scored the development partition. The calibration
partition had never been scored and no map was ever fitted on it, so it was
spent here as a second, independent reading of the same question.

```text
gym eval --suite crates/gym/suites/coder-turns-v1.json \
    --partition calibration --family <family> --jev \
    --record crates/gym/results/coder-turns-v1.jsonl
```

Once per family, seven runs, 130 rows appended to the chain. `gym compare
--store crates/gym/results/coder-turns-v1.jsonl` now reads 520 rows, chain
verified, and hosted Jev's pooled accuracy over 260 items is 0.67.

The comparison that matters, per family, per partition, from the rows:

```text
python3 crates/gym/suites/baseline_coder_turns.py \
    crates/gym/suites/coder-turns-v1.json --partition <partition> \
    --store crates/gym/results/coder-turns-v1.jsonl --door "jev (hosted)"
```

| Family | Items | Constant, dev | Jev, dev | Constant, cal | Jev, cal | Jev minus constant, both | Read by |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `shell_outcome` | 22 + 22 | 0.545 | 0.773 | 0.545 | 0.773 | **+0.227** | `ShellVerdict::route` |
| `damage` | 22 + 22 | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | `ShellVerdict::damage_gate` |
| `progress` | 16 + 16 | 0.438 | 0.438 | 0.500 | 0.500 | 0.000 | nothing |
| `action` | 16 + 16 | 1.000 | 0.875 | 0.938 | 0.875 | −0.094 | `classify::route` |
| `needs_code` | 16 + 16 | 1.000 | 0.625 | 0.875 | 0.812 | −0.219 | nothing |
| `useful` | 22 + 22 | 0.864 | 0.409 | 0.909 | 0.364 | −0.500 | nothing |
| `risk` | 16 + 16 | 0.875 | 0.312 | 0.875 | 0.438 | −0.500 | nothing |

The second partition confirmed the first on every family. `shell_outcome`
beat its constant by the same 0.227 on both, `damage` and `progress` tied
theirs on both, and the four losing families lost on both. The
[0.056 two-sigma floor](../lev/measurements/2026-09-19-seed-variance.md)
is the smallest difference worth reading, and every difference in the
last column except `progress` and `damage` clears it.

Two sets of 16 items are still two small samples. What the second
partition adds is not precision on any one number; it is that no family
changed sign, which is what a decision needs.

## What each question does, and the decision

### `damage`: retire

Fifty-five rounds, fifty-five `no`. The label rule looks for a later undo,
restore, or report of harm, and finds none in the 540 rounds the archive
holds. Hosted Jev agreed on all 44 open rounds, with a `yes` probability
that never rose above 0.13 on the calibration partition or 0.10 on the
development partition, against a stop threshold of 0.7.

The question cannot score above its constant on this suite, and its route
is already inert on the door `coder` ships with: since #9397,
`ShellVerdict::damage_gate` reads `damage` only from a door that says the
number is calibrated, and hosted Jev says nothing about calibration, so the
gate records the number and does not act on it. A question whose answer is
constant and whose consumer is switched off is a request cost and a
privacy cost, and on `lev-base` it was a 3-in-21 false-stop risk.

Retiring it does not say shell commands never do harm. It says this archive
holds no example, so nothing here can measure whether any wording catches
one. The right instrument for that is a suite with harmful rounds in it,
and until one exists, a threshold nobody can test protects nothing.

### `needs_code`, `useful`: retire

Neither is read by a decision. `needs_code` is `yes` on 38 of 40 turns
because a coding agent's turns need code, and hosted Jev is 0.219 below
that constant. `useful` is `yes` on 49 of 55 rounds and Jev is 0.500 below
its constant: it answers `no` on rounds the session shows the agent went
on to use. Both are displayed in the terminal and nowhere else.

### `progress`: stop computing it

`progress` is the one question with real variety, 17, 16, and 7 across its
three levels, and it ties its constant on both partitions at 0.438 and
0.500. It is asked on every turn, and read by nothing.

The issue offered two ways out: wire it to something, or stop asking. This
record takes the second. Wiring a question that scores at chance to a
behavior would put an unmeasured coin flip in the path of every turn, and
no consumer for it was proposed with evidence. If one appears, the
question goes back in as a new set, scored against this suite first.

### `risk`: retire, and re-specify before it is asked again

`risk` is the worst family on the board, 0.500 below its constant, and the
constant is not a risk signal. The truth is `2`, "writes files or could
break a build", on 35 of 40 turns, because writing files is the workload.
A rubric that grades an activity nearly every turn performs cannot
separate the turns that matter.

The distinction a replacement has to name is reversibility and reach, not
activity:

| Level | Names |
| --- | --- |
| 0 | Nothing outside the conversation changes: the step reads, or answers in prose. |
| 1 | Changes inside the repository that version control can restore: edits tracked files, runs the build or the tests. |
| 2 | Changes version control cannot restore: deletes untracked or ignored files, rewrites history, touches files outside the repository, sends something over the network, or spends money. |

And the label rule it needs is an outcome rule, read from what the agent
did next in the same session, exactly as v1's `risk` was: the highest
level any command or edit before the next human turn reaches.

That wording is **not in `coder-turns-v2`**, and this is deliberate. No
item in the archive carries a label under it, so a set that included it
would be asking a question with no way to score the answer. The `damage`
rule found no harm in 540 rounds, which is a reason to expect level 2 to
be rare on this archive too, and a rare class is exactly the case where a
question has to be scored before it is trusted. The rewording is a
proposal for the next harvest, not a candidate this record measured.

### `shell_outcome`: keep

The one question that pays. 0.773 against 0.545 on each partition, +0.227,
four times the noise floor. It gates the shell loop, and since #9396 its
`retry` answer changes the next generation rather than being read as
`pass`.

Its errors are worth naming: over 44 open rounds Jev answered `retry` on
7 rounds whose truth was `pass`, and `pass` on 3 rounds whose truth was
`retry`, where the session shows the agent corrected and reran. A false
`retry` costs a suffix on the next instruction; a false `pass` costs
nothing the old loop did not already cost.

### `action`: keep, with the number beside it

On 32 open turns, hosted Jev answered `respond` on 28 turns whose truth was
`respond`, `clarify` on 3 whose truth was `respond`, and `respond` at 0.98
on the one turn whose truth was `clarify`. That is 0.875 against a constant
of 0.969: every routing change the question produced was wrong, and the
one turn where a change was right went unrouted.

It stays in `coder-turns-v2` because it is the router. `classify::route`
reads nothing else, and a turn question set with no `action` is a `coder`
that does not classify turns, which changes the product and is not this
record's call. What this record can say is that on this workload the
router's only input has not beaten the constant on 32 turns, that
`end_conversation` and `none` are never the truth so half its options are
unexercised, and that the `Clarify` route has cost three turns and saved
none. A decision to remove or replace it should be made on more turns
than this, and this suite is where they should be scored.

## The set, the suite, and the rows

`crates/gym/questions/coder-turns-v2.json`, digest `9bdee0dbe14862fd`,
holds `action` and `shell_outcome` word for word from v1. A question set's
digest is over its text, so the v2 digest differs from v1's
`4d0aaf39558294e7` only because five questions are absent.

`crates/gym/suites/coder-turns-v2.json`, digest `e406aa54be745bd6`, is the
95 v1 items of those two families, unchanged: same states, same outcome
labels, same partitions, 38 calibration, 38 development, 19 locked and
unread. `crates/gym/suites/build_coder_turns_v2.py` derives it from the
committed v1 file, because the session archive v1 was harvested from is
not in the repository.

```text
python3 crates/gym/suites/build_coder_turns_v2.py
gym eval --suite crates/gym/suites/coder-turns-v2.json --jev \
    --record crates/gym/results/coder-turns-v2.jsonl
```

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| calibration, raw | 0.82 | 0.173 | 0.131 | 0.445 | 1 | 38 |
| development, raw | 0.84 | 0.094 | 0.118 | 0.363 | 0 | 38 |

Per family, from the rows:

| Family | Partition | Items | Constant | Jev, v2 | Jev minus constant |
| --- | --- | --- | --- | --- | --- |
| `action` | calibration | 16 | 0.938 | 0.875 | −0.062 |
| `action` | development | 16 | 1.000 | 0.938 | −0.062 |
| `shell_outcome` | calibration | 22 | 0.545 | 0.773 | +0.227 |
| `shell_outcome` | development | 22 | 0.545 | 0.773 | +0.227 |

The v2 pooled accuracy of 0.83 against v1's 0.67 is not a better door and
not a better question. It is the same door on the same two families, with
the five families that scored below their constants no longer in the
denominator. The number to read is the last column, which did not move.

### The same text, asked twice

The v2 rows are the v1 text, asked again the next day. Of 76 argmax
answers, 73 matched the v1 row on the same item and 3 did not: one shell
round moved from `pass` to `retry`, one from `retry` to `pass`, and one
turn moved from `clarify` to `respond`. The families' accuracies moved by
one item each on the development partition, `action` up and
`shell_outcome` level, and 3 of 76 is 0.039, under the 0.056 floor. That
is what a single hosted door's day-to-day variance looks like on this
suite, and it is the reason no difference under that floor is read
anywhere on this page.

## What this changes in `coder`, and what it does not yet

Nothing in `crates/coder` changed in this record. `classify::questions`
still sends `needs_code`, `risk`, and `progress` beside `action`, and
`classify::shell_questions` still sends `useful` and `damage` beside the
round's choice. `crates/coder/tests/suite_questions.rs` pins that text to
`crates/gym/questions/coder-turns-v1.json`, which is the right test doing
its job: production and the file it is scored against cannot drift apart
without a test saying so.

Retiring the five in production is therefore one change in two files that
belong together: drop them from `classify.rs`, and repoint
`suite_questions.rs` at `coder-turns-v2.json` and the v2 suite. The
`damage` gate, its calibration reading, and the `DAMAGE_STOP` threshold
go with the question, and the terminal's `useful`, `risk`, and `progress`
lines go with theirs. That change is openagents#9395's remaining box and
is left to a commit that owns those files.

The request itself gets smaller when it lands. A turn request goes from
four questions to one and a shell-round request from three to one. This
record does not publish the latency or token saving, because the Gym asks
one question per request and production asks them together, so the
saving has to be measured on `coder`'s own traces rather than here.

## What this does not say

- **The locked partition is still unread.** Sixty-five v1 items and the
  same 19 v2 items have never been shown to a door. Both open partitions
  are now spent on hosted Jev, so the next question-text comparison on
  these families has only the locked partition left for a clean read, and
  the ledger will record it.
- **One door.** Every door number here is hosted Jev. No Lev or Kev door
  ran, because this machine has neither Apple hardware nor Kev weights.
  The retire decisions do not depend on a door: a family whose labels are
  constant cannot be beaten by any door, and a family read by nothing costs
  the same whatever answers it.
- **Retired is not disproved.** A question that loses to its constant on
  this workload might carry signal on another. This archive is one
  operator, one repository, four sessions; the decision is for the
  workload that exists.
- **`action`'s number rests on one positive.** One `clarify` in 32 open
  turns. The question's miss on it is one item, and its three false
  `clarify` answers are three. That is enough to say the question has not
  paid for itself yet and not enough to say it cannot.
- **The re-specified `risk` is unscored.** It is a proposal with a label
  rule, and it becomes a question when a harvest labels it and this suite
  scores it.
