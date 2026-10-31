# 222. A decision in milliseconds

> **Status: unimplemented — design draft.** Nothing here runs yet.

Some decisions must arrive in milliseconds. A large decoder cannot answer that
fast. This chapter executes a small encoder with fixed decision heads that
answers a typed question in one pass, and measures its latency at P50 and P99.

## What this chapter covers

- The architecture: a ModernBERT-family encoder with local and global
  attention, GeGLU feed-forward layers, quantized embedding rows and fixed
  decision heads with per-head temperatures.
- The reference target. The pre-reset work used the s1-llm-auto-router, an
  11-layer mmBERT-small encoder with 119.4M parameters (98.3M of them `uint8`
  embeddings) and seven heads, exported as ONNX. Its vendor reports 14 ms P50
  on four CPU cores. These are historical and vendor figures, not Psionic
  results.
- Loading through [Chapter 220](ch220-read-a-bounded-onnx-graph.md)'s bounded ONNX reader, or from original tensors.
- Parity: identical argmaxes and a stated distance from the reference logits.
- Latency at 512 tokens on CPU and GPU, cold and warm, interleaved.
- Identity. The model identity is separate from the engine identity, and an
  explicitly selected engine never falls back silently.

## Dependencies

- [Chapter 220](ch220-read-a-bounded-onnx-graph.md): the bounded ONNX reader.
- [Chapter 10](ch010-attention-without-training.md) and [Chapter 4](ch004-numbers-and-matrices.md): attention and numerics.
- [Chapter 48](ch048-integer-matrix-products.md) and [Chapter 50](ch050-quantized-weights-and-caches.md): integer products and quantized rows.
- [Chapter 18](ch018-time-a-decision.md): timing.
- [Chapter 44](ch044-elephant-on-cuda.md): the CUDA backend.

## Done means

- Every head's argmax equals the reference on a fixed request set, and the
  largest logit difference is reported against a bound frozen in advance.
- P50 and P99 at 512 tokens are reported on a named CPU and a named GPU, cold
  and warm. The chapter states whether a 20 ms P99 target is met; the
  pre-reset engine missed it.
- Execution parity is reported apart from decision quality.

## Sources

- [Chapter plan](PLAN.md), Part XXII.
- [Decisions in production](../docs/decisions.md), "Decisions in
  milliseconds".
