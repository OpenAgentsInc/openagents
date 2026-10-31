# 207. Programs as tool calls

> **Status: unimplemented — design draft.** Nothing here runs yet.

Some tasks need many small tool calls. Each round trip costs a model request.
This chapter lets the model write one program instead. The harness runs the
program once against host-provided functions in a bounded sandbox and returns
one structured result.

## What this chapter covers

- A bounded runtime for model-written programs, with no ambient authority.
- Host-provided functions. The program can call only the functions the
  harness binds for this step, each through [Chapter 203](ch203-tools-as-typed-capabilities.md)'s pipeline.
- One structured result per program, with typed failure kinds.
- Limits on output bytes, time and memory. A program that exceeds a limit is
  refused. Its output is not silently truncated.
- Logging. The program text, each bound call and the result are log events.

## Dependencies

- [Chapter 203](ch203-tools-as-typed-capabilities.md) and [Chapter 204](ch204-a-sandbox-and-approvals.md): tools and the sandbox.
- [Chapter 201](ch201-model-visible-means-logged.md): the log.
- Chapters [235](ch235-can-gradient-descent-learn-exact-execution.md), [236](ch236-a-frozen-exact-core-with-a-learned-interface.md) and [237](ch237-composable-weight-modules.md) use programs as
  tool calls.

## Done means

- A program that calls an unbound function is refused before it runs.
- A program that exceeds each limit is refused, and the step names the limit.
- On a fixed task set, the chapter reports model requests, tokens and
  wall-clock time with programs as tool calls against ordinary tool calls.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
