# Tassadar Executor-Trace Homework Internal Wiring

This note records the OpenAgents-side wiring for issue #4684. It is an
internal/contributor-facing work contract only; it does not publish a Tassadar
capability claim, registry promise, AGENTS.md capability row, marketing copy, or
public acceptance verdict projection.

## Worker Contract

- Job kind: `tassadar_executor_trace_homework`
- Verification class: `exact_trace_replay`
- Connector ref: `psionic.connector.bounded_executor_trace.v1`
- Bounded profile ref: `tassadar-article-transformer-trace-bound-trained-v0`
- Bounded route ref:
  `tassadar.article_route.direct_hull_cache_runtime.v1`
- Internal work class:
  `tassadar.internal_compute.article_closeout.v1`

The dispatch payload is built by
`src/tassadar-executor-trace-homework.ts`. It carries only public-safe refs and
explicit false flags for public capability copy and public acceptance verdict
projection.

## Closeout And Replay

Executor closeouts must provide:

- worker Pylon device ref
- distinct validator device ref
- sampled trace window ref and `{ startStep, endStep }`
- trace commitment digest ref
- replay digest ref
- worker receipt ref

`tassadarExecutorTraceVerificationChallengeRequest` refuses same-device replay.
The resulting challenge uses `exact_trace_replay`; a green verdict requires the
sampled-window replay digest to match the trace commitment digest.

## Current Status

The no-spend dispatch envelope and exact replay smoke are wired in the Worker.
Paid settlement remains blocked until an operator-funded executor-trace closeout
exists and can be referenced without exposing raw payment material.

The smoke is:

```sh
bun run smoke:tassadar:executor-trace
```

From `apps/openagents.com/workers/api`.
