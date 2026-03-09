# Inference Engine

Psionic is only inference-ready when it can honestly serve compute products rather
than just run tensor math.

## Text Generation Requirements

- model load/unload lifecycle
- request execution path
- token streaming or equivalent delivery model
- KV cache lifecycle
- deterministic execution metadata
- backend capability gating

## Embeddings Requirements

- explicit embeddings request/response contract
- deterministic vector shape metadata
- stable model identifier
- capability reporting tied to the served product
- execution receipt fields for outputs and runtime metadata

## KV Cache Requirements

The phase 0 bootstrap does not implement KV cache support. The architecture must
leave room for:

- in-memory KV cache
- paged KV cache
- tiered KV cache
- concurrency-safe session ownership

## Phase 0 Definition

Phase 0 is complete when Psionic can run a deterministic, CPU-backed
`psionic.embeddings` smoke path with truthful capability and receipt surfaces.
