# 50. Quantized weights and caches

> **Status: unimplemented — design draft.** Nothing here runs yet.

Elephant's BF16 weights are 18.8 GB, and the RTX 4080 has 16 GB. A derived
format with fewer bits per weight lets the whole model stay resident. This
chapter adds quantized weight formats and an optional quantized KV cache, and
it measures what each one costs in quality before you use it.

## What this chapter covers

- Q8_0 first: about 8.5 bits per weight, small enough to keep Elephant
  resident on a 16 GB GPU.
- Then the 4- and 6-bit K-quants, with importance-weighted quantization and
  per-tensor format mixes.
- Chapter 60's MXFP4 decoder as another candidate format.
- An optional quantized KV cache.
- Independent decode vectors for every format, and refusal of partial rows.
- A separate model profile for each derived checkpoint.
- Quality against memory, each compared with the original weights:
  - perplexity on fixed text;
  - chapter 16's decision metrics;
  - a product evaluation.
- A quality loss declared before measuring, with an uncertainty estimate over
  the workload.
- Aggressive 2–3-bit consumer profiles carry a quality card (top-1 agreement
  and KL divergence against the original, and task scores).

## What it builds

- Planned format decoders, a quantizer with an importance matrix, and a quality
  card generator.

## Dependencies

- [Chapter 16](ch016-are-the-probabilities-any-good.md): decision quality
  metrics.
- [Chapter 48](ch048-integer-matrix-products.md): integer products for the
  quantized formats.
- [Chapter 49](ch049-read-a-gguf-file.md): reading published quantized files.
- [Chapter 60](ch060-original-mxfp4-weights.md): the MXFP4 decoder.

## Done means

- Every format decodes its independent vectors exactly and refuses partial
  rows.
- Each derived profile reports measured memory and the three quality
  measurements against the original, with the declared loss bound and its
  uncertainty.
- A profile whose measured loss exceeds the declared bound is not promoted.
- No quantized profile inherits the original weights' evidence.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): Q8_0, K-quants,
  `imatrix` and the quality card. The survey cites an upstream Q8_0 figure of
  mean KL about 0.0014 on Llama-3-8B; that is an upstream claim.
