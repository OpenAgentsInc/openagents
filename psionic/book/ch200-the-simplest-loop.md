# 200. The simplest loop

> **Status: unimplemented — design draft.** Nothing here runs yet.

A harness turns a served model into an agent. It assembles each request,
streams the answer, runs the tools the model calls and decides when a turn is
done. This chapter builds the smallest loop that does that over Psionic's own
API. Every later chapter in Part XXI adds one thing to this loop, and keeps it
only if it measures a gain over this baseline.

## What this chapter covers

- The loop: assemble the request, stream the answer, run each tool call, append
  the results and repeat until the model owes no more tool calls.
- Exact boundaries. A turn is zero or more steps. A step is one model request
  plus the tools that request calls. Each boundary is a typed event.
- A deadline on every turn. The loop refuses to start a step that cannot finish
  before the deadline.
- Cancellation. One cancel stops the model stream and every running tool
  together, and the turn records that it was cancelled.
- The model behind the loop: a Psionic-served model through Part III's API and
  chapter 79's Responses server, with no other provider in the path.

## What it builds

- A planned `psionic/lib/harness/` module with the loop, the turn and step
  types and a deadline and cancellation token.
- A loopback example that runs one task with one tool against a resident
  gpt-oss-20b.

## Dependencies

- [Chapter 20](ch020-serve-systemone.md) and [Chapter 22](ch022-run-it-live.md): the served API and its health contract.
- [Chapter 79](ch079-serve-responses.md): Responses-native streaming and function handoffs.
- [Chapter 28](ch028-a-decision-arrives-while-play-continues.md): deadlines, cancellation and late-response fencing.
- [Chapter 53](ch053-choosing-the-next-token.md): constrained decoding, used for tool arguments from [Chapter 203](ch203-tools-as-typed-capabilities.md) on.

## Done means

- A scripted model fixture drives the loop through zero, one and several tool
  calls, and the loop emits the expected turn and step events in order.
- A turn that exceeds its deadline stops, and no tool runs after the deadline.
- Cancellation stops the model stream and all tools; no tool result is appended
  after the cancel event.
- One real task runs end to end against a pinned Psionic-served model, with the
  model identity and build recorded.
- This loop is the baseline that [Chapter 211](ch211-measure-the-harness.md) measures every later addition
  against.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
