# 202. Keep the prefix stable

> **Status: unimplemented — design draft.** Nothing here runs yet.

A server with a prefix cache reuses work only when each request starts with
the same bytes as the last one. This chapter fixes the order in which the
harness assembles a prompt, so that each turn only appends to the
model-visible bytes. It then measures what that saves on Psionic's own
server.

## What this chapter covers

- A fixed assembly order: system sections, then tool schemas, then history.
- Metadata outside the model input. Identity headers, plugin inventory and
  session metadata travel beside the request, never in messages, system
  prompts or tool schemas. DeepSeek Harness follows the same rule, and its
  serving reports a 96% input cache hit rate on 77K-token average inputs (an
  upstream figure, not a Psionic result).
- A prefix check. Before each request, the harness compares the new
  model-visible bytes with the previous request's and records where they first
  differ.
- The measurement: cache hit rate, prefill tokens saved and time to first token
  across a long session.

## Dependencies

- [Chapter 201](ch201-model-visible-means-logged.md): the log that the prompt is assembled from.
- [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): the paged state and tiered prefix cache that this chapter
  measures against.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): the service benchmark and its percentile reporting.

## Done means

- Across a scripted long session, every request that adds no condensation
  starts with the previous request's model-visible bytes.
- A change that would place metadata inside the model input fails a check.
- The chapter reports hit rate, prefill saved and P50 and P95 time to first
  token, with and without the fixed order, on one named host and model.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
