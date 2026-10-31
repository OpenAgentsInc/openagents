# 186. Split a model into stages

> **Status: unimplemented — design draft.** Nothing here runs yet.

Part XX splits a model that no single machine holds. The first step is a contract for the boundary between two stages: a typed list of everything that crosses it.

A placement plan is not evidence of a correct boundary. Two stages must match the whole model on identical tokens before any later chapter depends on the split.

## What this chapter covers

- **A boundary derived from the model.** The contract lists each part that crosses: hidden state, residual streams, shared-KV sidebands and recurrent sidebands.
- **Bound identities.** Each part is bound to the artifact digest, the stage's layer range and the protocol version.
- **Whole-model parity.** Two stages must match the whole model on identical tokens for logits, greedy output and cached continuation.
- **Dense Ministral 3 first.** The first split uses Ministral-3-3B (26 layers, width 3072) from [chapter 81](ch081-execute-ministral-4.md), with raw activations.
- **Larger boundaries later.** The DeepSeek V4.1 plan lists four residual streams, KV and indexer source layers, chosen candidate blocks and Engram output. Mistral Large 4's layer-sharing map has the same character. The contract must express both.

## What it builds

- A planned boundary contract schema in `proto/` and a stage runner that executes a layer range.

## Dependencies

- [Chapter 81](ch081-execute-ministral-4.md): Ministral 3 executed on one machine.
- [Chapter 7](ch007-reading-original-weights.md): artifact identities.
- [Chapter 17](ch017-qualify-honestly.md): comparison rules when arithmetic differs.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): memory measurements per layer.

## Done means

- A two-stage split of Ministral 3 produces the same logits, greedy output and cached continuation as the whole model on the fixture prompts, on the machines that ran it.
- A boundary message with a wrong artifact digest, layer range or protocol version is refused.
- The bytes that cross the boundary per token are measured.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
