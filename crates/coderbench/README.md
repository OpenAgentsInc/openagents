# coderbench

Task manifests and recorded goldens for whole Coder episodes.

`crates/gym` scores a door on one item. This crate scores an **episode**:
everything from the operator's sentence to the final summary, recorded as
one [ATIF](../atif) trace.

## Running an episode

```sh
coderbench run devin-fan-out-six            # run Coder, capture, judge
coderbench diff devin-fan-out-six trace.jsonl   # judge a trace you have
```

`run` refuses before it starts Coder when the machine does not hold what
the task requires, and names the requirement that failed. It exits `0` for
a clean run, `1` for faults, `2` for a refusal, `3` when there is no trace
to judge, and `4` when the evidence a judgment needs is missing.
[`../../docs/coderbench.md`](../../docs/coderbench.md) has the flags and
the reasoning.

## A grade answers with three values

`failed` beats `unverifiable` beats `passed`, which is `crates/gym`'s
`gate::Verdict` and the same type rather than a second word for it. A
delegation the trace records as wrong is a failure. A delegation that
recorded no correctness either way is unverifiable: nobody checked it, and
six silences are not six correct answers.

Missing evidence never passes. The grade reads each call's outcome, the
answers a decision returned, the order the steps came in, whether the trace
closed itself and read back whole, the exit code the turn ended with, and
the checkout as it was before and after the run. A task that forbids writes
is judged against the workspace, because an absent `wrote` field is a run
that said nothing rather than a run that wrote nothing.

## A task can own the expected answers

A manifest may carry `grade.expects`: one `{prompt, answer}` entry per
delegation, in the order the request asks the questions. The runtime never
receives them — they are the manifest's own copy of the truth, so the grade
checks what the run recorded against something the run did not write.

The check is positional. The run's first delegation is compared to the first
entry: the recorded `arguments.prompt` must be the pinned prompt exactly —
case, spacing, and wording are the question — and the recorded `output` must
be the pinned answer, whitespace at the edges aside, with case intact. A
missing, duplicated, reordered, or substituted delegation fails rather than
matching wherever it lands, a self-asserted `correct` flag adds nothing, and
a trace that calls its own answer wrong contradicts the manifest and fails.

A manifest that cannot check cannot pass: `expects` pins one entry per
delegation (`expects.len()` is `delegations`, and `delegations_correct` is
the same count because every pinned answer must verify), each prompt and
answer is nonblank, and no two prompts repeat. `Task::load` refuses such a
manifest, and `judge` faults it on a task built by hand.

A task without `expects` keeps the trace-reported evidence rule: the trace's own
`correct` flag is the only correctness evidence, so only a delegation the
trace itself records as checked counts.

`tests/negative.rs` holds the runs that must not grade clean, starting with
the one from audit finding A04
([#9418](https://github.com/OpenAgentsInc/openagents/issues/9418)): six
empty ungraded delegations, null decision answers, and the required check
names, which drew no faults at all from the grader as it stood.

## What a golden is

A golden is not a transcript to match character for character. It is the
**path** a correct run takes — which decisions were asked, which way they
went, which capabilities were reached, and what came back. Two runs of one
task differ in wording and agree on the path.

`Task::judge` compares the path and ignores the prose. It returns every
fault rather than the first, because a run that picked the wrong program
usually gets several later things wrong and the first fault is rarely the
informative one.

## Decisions and checks are graded apart

A task lists `decisions` and `checks` separately. A check is code and
answers the same way every time. A decision is a model and does not. A task
that listed them together would accept a run that asked a model what a check
should have settled.

## Provenance

Every golden has a `.meta.json` sidecar saying what it rests on. A golden
without one is an error rather than a default, because the default a reader
assumes is the strongest one.

| Provenance | Meaning |
| --- | --- |
| `observed` | Coder ran and this is what it did. |
| `staged` | Every call is real and something else drove them. |
| `authored` | Nobody ran it. |

Three states rather than two, because "recorded" was covering two different
things. A recording of the program under test and a recording of something
else doing what that program should do are not the same evidence, and the
second is the one a reader over-trusts.

The sidecar is beside the trace rather than inside it because ATIF describes
a session, and a session cannot say who was driving it.

## Tasks

| Task | What it tests |
| --- | --- |
| `devin-fan-out-six` | The delegation path: probe, program lookup, program selection, independence, admission, six parallel delegations, summary. **Staged** on 2026-09-20 — every call real, orchestrated by a shell script rather than by Coder. It becomes observed when a real run reproduces it ([#9412](https://github.com/OpenAgentsInc/openagents/issues/9412)). |

Read [`../../docs/coderbench.md`](../../docs/coderbench.md) for what the
first recording found.
