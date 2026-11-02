# 9. Microcoder

> **Status: unimplemented — design draft.** The chapter text and its checks aren't written yet. The
> loop it describes exists in `coax/lib/microcoder/` and is specified in
> [docs/coder/microcoder.md](../../docs/coder/microcoder.md).

Microcoder is Coder's own engine: a small loop that runs on any model we can
reach and needs no other agent installed. It is the engine for plain questions,
small jobs and machines without Claude Code or Codex.

## What this chapter covers

- The state: environment facts, the task, the routing decision, the last 12
  actions with output heads and tails, the files asked for and the loop's own
  notes.
- One structured call per step. The model returns a rationale, commands,
  files to view, `finished`, a reply and whether to ask a question.
- Commands run in the working directory with a timeout and capped output;
  `view` reads capped files under it. Writes go through commands, so they show
  in the transcript.
- The endings it reports: `finished`, `asked`, `bad_replies`, `no_capacity`,
  `stopped` and `idle`. The person sets limits; the loop adds none beyond
  output caps.
- Gateways: one interface and one file per model provider, with fallback before
  the first token.

## Dependencies

- [Chapter 1](PLAN.md#i-the-loop): the simplest loop, which Microcoder is an
  instance of.
- [Chapter 5](PLAN.md#i-the-loop): confinement for its commands.

## Done means

- `openagents coder run --engine micro "TASK"` runs the loop in the current
  directory and prints each step and the ending.
- Recorded gateway streams replay through the loop with byte-identical logs.
- Each ending is reached by a scripted fixture.

## Sources

- `coax/lib/microcoder/` (`loop.coax`, `state.coax`, `action.coax`,
  `tools.coax`, `run.coax`).
- [Microcoder in Coax, and the inference gateways](../../docs/coder/microcoder.md).
