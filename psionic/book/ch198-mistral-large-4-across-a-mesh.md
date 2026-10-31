# 198. Mistral Large 4 across a mesh

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter places the first mesh-scale Mistral model across machines. The body follows the plan's target: Mistral-Large-3-675B, with 681.5 GB of original files under Apache-2.0.

It has the same latent attention and expert routing as Mistral Small 4, at 61 layers and width 7168. No single machine on hand holds it.

## What this chapter covers

- **The target.** Mistral-Large-3-675B-Instruct-2512 (KV rank 512, query rank 1536), pinned by revision and file hashes.
- **Latent attention across stages.** The latent cache from [chapter 84](ch084-attention-through-a-latent.md) crosses stage boundaries in its compressed form.
- **Expert routing across machines.** The plan states 135-expert routing placed across machines, using [chapter 191](ch191-experts-across-machines-and-from-disk.md)'s expert parallelism.
- **Parity.** The split must match a whole-model reference on selected layers and tokens.
- **Measurements.** Per-hop latency and per-stage memory.

## Dependencies

- [Chapter 84](ch084-attention-through-a-latent.md): latent attention.
- [Chapter 85](ch085-execute-mistral-small-7.md): Mistral Small 4 on one machine.
- [Chapter 186](ch186-split-a-model-into-stages.md): stage boundaries.
- [Chapter 189](ch189-place-the-model.md): placement.
- [Chapter 191](ch191-experts-across-machines-and-from-disk.md): experts across machines.
- Hardware for the mesh is an owner decision.

## Done means

- Selected layers and tokens match the whole-model reference under [chapter 17](ch017-qualify-honestly.md)'s rules.
- Per-hop latency and per-stage memory are recorded for the chosen placement.
- A stage package fetches only its own tensors, checked as in [chapter 187](ch187-stage-packages.md).

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
