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
a clean run, `1` for faults, `2` for a refusal, and `3` when there is no
trace to judge. [`../../docs/coderbench.md`](../../docs/coderbench.md) has
the flags and the reasoning.

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
