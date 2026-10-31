# 187. Stage packages

> **Status: unimplemented — design draft.** Nothing here runs yet.

A stage should fetch and load only what it runs. This chapter defines stage packages: the tensors for one layer range plus the shared assets that range needs.

Each chunk is verified against [chapter 7](ch007-reading-original-weights.md)'s identities as it arrives, so a stage never runs on unverified bytes.

## What this chapter covers

- **Per-stage tensor sets.** A package lists the tensors for its layer range and the shared assets it needs, such as embeddings, the tokenizer or a shared expert.
- **Chunk-by-chunk verification.** Each fetched chunk is checked against its digest before it is used.
- **Four separate measurements.** Fetched bytes, materialized bytes, peak memory and time to readiness.
- **Licence boundary.** The redistribution licence of the V4.1 checkpoint is an open owner question, so distributing its stage packages waits for that answer.

## What it builds

- A planned stage package manifest derived from [chapter 186](ch186-split-a-model-into-stages.md)'s boundary contract.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): original-weight identities and shard indexes.
- [Chapter 178](ch178-start-fast.md): content-addressed weight delivery.
- [Chapter 186](ch186-split-a-model-into-stages.md): the stage boundary contract.

## Done means

- A stage loads only the tensors in its package, checked by an inventory of what it read.
- A chunk with one changed byte is refused, and the stage does not report ready.
- Fetched bytes, materialized bytes, peak memory and time to readiness are reported for each stage of the [chapter 186](ch186-split-a-model-into-stages.md) split.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
