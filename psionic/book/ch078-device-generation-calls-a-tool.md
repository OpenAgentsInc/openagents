# 78. Device generation calls a tool

> **Status: unimplemented — design draft.** Nothing here runs yet.

Chapter 77 runs a schema-constrained tool call on the CPU reference. This
chapter moves the same contract onto the resident device engine, in the
streamed path.

## What this chapter covers

- The resident GPU engine from chapters 65–70.
- Bounded JSON-schema masks applied before sampling, on the device path.
- Host grant checks before any tool runs.
- Cancellation during a tool call.
- Final-only display: reasoning and commentary stay out of the visible
  answer.
- No hidden CPU fallback inside a throughput measurement.

## Dependencies

- [Chapter 77](ch077-call-a-tool.md): the tool-call contract.
- [Chapter 63](ch063-the-harmony-format.md): commentary handoffs and stops.
- [Chapter 66](ch066-a-complete-cuda-decoder.md) and
  [chapter 72](ch072-gpt-oss-on-metal.md): the device engines.
- [Chapter 53](ch053-choosing-the-next-token.md): grammar masks.

## Done means

- The forced named-tool round trip from chapter 77 passes on the device, with
  the same call and result as the CPU reference for the fixture prompt.
- A call outside the grant is refused before any effect.
- A cancelled request stops the model and leaves no partial tool effect.
- Timing records show no CPU execution of model layers in the device runs.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 11.
- [gpt-oss target](../docs/targets/gpt-oss.md).
