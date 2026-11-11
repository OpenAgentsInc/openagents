# 28. Cases and graders

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

A claim needs a measurement, and a measurement needs tests. This chapter
builds the test set: cases where a member should act, cases where it should
not, and graders that decide each one.

## Draft

A test set for a member is a list of cases. Each case has an input, an
environment to run in, and a grader that says pass or fail.

Cases come in two kinds. In a should-fire case, the member ought to act and
the work ought to come out better. In a should-not-fire case, the member ought
to stay out of the way. The first kind measures reach. The second measures
restraint. A member with reach and no restraint acts everywhere and breaks
work it should have left alone, so every test set has both kinds.

Graders are deterministic where they can be: a test that runs, a file that
must exist, a value that must match. Where judgment is needed, a typed
decision makes it, with the question pinned by digest. A grader does not ask a
chat model for its opinion in prose.

The old tree kept a list of ways tests go wrong, written as knowledge notes.
The tests come from the same belief as the code. The checks are looser than
the grader. The checks pass only on the given example. Leftover state makes a
check pass. Each of these is a way to get a green result that means nothing,
and the test-set tooling checks for the ones it can.

A test set is a contribution in its own right. It is packaged and signed like
a plugin, it has an author, and it earns credit when others use it.

The project has written tests first since [Episode
6](../../docs/transcripts/006.md). [Episode
252](../../docs/transcripts/252.md) separates the design of a proof from the
run that produces it.

## What we build

A plan for what this chapter ships once the draft is approved:

- A test-set format: cases marked should-fire or should-not-fire, each with an
  environment and a grader.
- Graders: deterministic checks first, typed decisions where judgment is
  needed.
- A linter for the known ways a test set can fool its author.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coder/tui/filter_eval.py`](../../coder/tui/filter_eval.py)

Planned paths: `openagents/eval/suite.coax`.

## Sources

- **Episodes:**
  - [Episode 6, *Implementing Agent Data Models via TDD*](../../docs/transcripts/006.md): test-first from the start
  - [Episode 29, *Automating Tests*](../../docs/transcripts/029.md)
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): studying a benchmark
  - [Episode 121, *SWE-bench Planning*](../../docs/transcripts/121.md): benchmark planning
  - [Episode 252, *Observer, Automated QA*](../../docs/transcripts/252.md): verification obligations and exact receipts
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`, `coderbench`, `computer-conformance`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Reach and restraint"
  - `knowledge/slip.*.md` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P15
