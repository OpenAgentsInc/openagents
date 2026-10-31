# 54. Search the Coax book

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter is the first NVIDIA application in the plan: semantic search over
the Coax book with Nemotron-3-Embed-1B-BF16. It also builds the two references
that every faster index is measured against.

## What this chapter covers

- The model: Nemotron-3-Embed-1B-BF16 at a pinned revision.
- Exact query and document formatting, as the model card specifies.
- Bidirectional attention with padding masks.
- Padding-aware average pooling and normalization of the embedding.
- Stable identities for documents and chunks, so a result names exactly what
  it came from.
- Two references:
  - a BM25 lexical baseline;
  - exhaustive vector search over every chunk.
- Held-out retrieval measured on a fixed query set.

## What it builds

- A planned embedding path for the model, a chunker with stable identities,
  and a BM25 index.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): pinned original weights.
- [Chapter 10](ch010-attention-without-training.md): attention, here without a
  causal mask.
- [Chapter 52](ch052-tokenizers-and-chat-templates.md): the model's tokenizer.

## Done means

- Token ids, selected intermediate tensors and embeddings match independent
  references for recorded inputs.
- Held-out ranking is reported for BM25 and exhaustive vector search on the
  same queries.
- Every result names its document and chunk identity.
- The model's terms are recorded; the card and config identify OpenMDW-1.1.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md): Nemotron-3-Embed-1B-BF16.
- [Roadmap](../docs/roadmap.md): retrieval as the first NVIDIA application.
