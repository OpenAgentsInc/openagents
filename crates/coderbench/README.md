# coderbench

Task manifests and recorded goldens for whole Coder episodes.

`crates/gym` scores a door on one item. This crate scores an **episode**:
everything from the operator's sentence to the final summary, recorded as
one [ATIF](../atif) trace.

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

A golden says whether it was **recorded** or **authored**. This is the
distinction `gym::row::LabelSource` draws for items, for the same reason: a
path that was observed and a path somebody expects are different evidence,
and a file that does not say which will be read as the stronger one.

## Tasks

| Task | What it tests |
| --- | --- |
| `devin-fan-out-six` | The delegation path: probe, program lookup, program selection, independence, admission, six parallel delegations, summary. Recorded on 2026-09-20. |

Read [`../../docs/coderbench.md`](../../docs/coderbench.md) for what the
first recording found.
