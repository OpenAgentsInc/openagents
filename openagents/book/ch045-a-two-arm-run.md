# 45. A two-arm run

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

With a test set in hand, this chapter runs it twice: once with the member
admitted and once without, on the same baseline agent. The difference is the
first piece of evidence.

## Draft

The question is never "is this plugin good?". The question is what admitting
it changed. To answer, you need two runs that differ in exactly one thing.

The baseline arm runs the agent as it is, on every case. The subject arm runs
the same agent, on the same cases, with the member admitted. Everything else
is locked: the model and its version, the instructions, the defaults already
in place, the environment and the grant. Both arms write full records (chapter
8), so anyone can read what happened in each case.

The report gives, for each arm, how many cases passed, the score, the cost and
the time. It gives the change between arms with its uncertainty. Six cases
can't support a strong statement, and the report says so and does not hide it.

A written gate reads the report and gives a verdict. The gate names one
primary outcome ahead of time, so a result can't be rescued after the fact by
picking a better-looking number. A member with no good effect on its primary
outcome is rejected, however safe it is.

Cost is part of the result. In the old tree's hosted runs, the repository map
passed more cases and also finished faster: 10.7 seconds against 24.9. A
member can stand in for compute.

The run happens in the evaluation sandbox from chapter 40, on your computer or
on a hosted runner.

## What we build

A plan for what this chapter ships once the draft is approved:

- A two-arm runner: the same cases with and without the subject, everything
  else locked.
- A report with both arms, the change, its uncertainty, cost and time, and the
  gate's verdict.
- A gate file with a primary outcome named ahead of time, pinned by digest in
  the report.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/run`](../../coax/lib/nostr/run)
- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`coax/remote`](../../coax/remote)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Report | `openagents.eval.v1` `Report`, `Measurement`, `RunCoverage` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | Nostr kind 3189, public | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** the two-arm runner in `openagents/eval/run.coax` and a gate
  file that names one primary outcome.
- **Trace:** the six-case fixture set runs twice on a fake model with
  recorded answers and the controlled clock. The report prints the baseline
  arm `2 of 6`, the subject arm `5 of 6`, the change `+3`, its uncertainty,
  the time for each arm, the gate's digest and the verdict.
- **Refusal:** a gate edited after the run no longer matches the digest in
  the report, and the verdict is refused. Arms that differ in more than the
  subject, such as the model version, are refused before either arm runs.
- **Independent check:** the counts are hand-derived from the recorded
  answers. The report reads through
  [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax),
  whose coverage counts must sum per arm.

Planned paths: `openagents/eval/run.coax`.

## Sources

- **Episodes:**
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): comparing configurations on a benchmark
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:30, "Run it"
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): with-and-without thinking
- **Old code at `coder-v1.0.0`, crates:** `eval-runner`, `ext-eval`, `gym`, `workflow-run`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-ATIF](../../nostr/nips/openagents/NIP-ATIF.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Capability delta"
  - [Coder book, chapter 21: Measure the harness](../../coder/book/ch021-measure-the-harness.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P15

## Open questions for the owner

1. How many cases does a test set need before its result may be published? The
   first adoption used six.
