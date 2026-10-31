# What Psionic takes from llama.cpp

Read-only survey, 2026-10-29, of
[ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp/tree/0893ec98f5eafad36a43a72cd1c3723d72b0d03a)
at `0893ec98f5eafad36a43a72cd1c3723d72b0d03a` (MIT, "Copyright (c) 2023-2026
The ggml authors"). The source was cloned and read; nothing from it was built
or run. Psionic reimplements designs in Coax with its own checks and treats
llama.cpp as a comparator and a source of independent fixtures, never as a
dependency. Paths below are relative to that revision.

## Why it matters now

Elephant's BF16 weights are 18.8 GB. The RTX 4080 has 16 GB, so a decision
either streams part of the weights per request (chapter 44) or uses a smaller
derived format that fits. llama.cpp's own measurements put Q8_0 close to the
original: mean KL divergence about 0.0014 and the same top token about 97.7% of
the time on Llama-3-8B (`tools/perplexity/README.md`). On this GPU class, the
pre-reset Rust Psionic and llama.cpp b11538 both ran a 9B Qwen3.5-style hybrid
decision at about 0.2 s for 1,000 tokens (`docs/inference/clef-native.md` at
`0b401561a0^`). That is the bar for chapter 44.

## Designs to reimplement, by chapter

| Design | Where in llama.cpp | Chapter |
| --- | --- | --- |
| Benchmark method: prompt processing and generation at fixed lengths, depth with a prefilled cache, warm-up and repetitions, mean and standard deviation | `tools/llama-bench/` | 34 |
| Grouped-query attention that loads each key/value head once per query-head group; vector and tensor-core variants; split-K decode | `ggml/src/ggml-cuda/fattn*.cu`, Vulkan `flash_attn_decode_*` | 41 |
| Chunked Gated DeltaNet (WY form, chunk 64, f32 state, rollback snapshots) | `src/models/delta-net-base.cpp`, `ggml/src/ggml-cuda/gated-delta-net*.cu` | 42 |
| Fusion with overlap guards (norm and scale, gate/up with activation, top-k routing, a recurrent step with its cache write) and captured device graphs | `ggml/src/ggml-cuda/ggml-cuda.cu` | 43 |
| Metal tuning tables by GPU family; parallel command-buffer encoding | `ggml/src/ggml-metal/` | 47 |
| 8-bit activation blocks with sums; integer dot products; row kernels for few tokens and int8 tensor-core tiles for whole prompts; row interleaving for SIMD | `ggml/src/ggml-cuda/mmvq.cu`, `mmq.cuh`, `ggml/src/ggml-cpu/repack.cpp` | 48 |
| The GGUF container with its overflow, alignment and version refusals | `ggml/src/gguf.cpp` | 49 |
| Q8_0, then K-quants with importance-weighted quantization and per-tensor mixes; the quality card (perplexity with error, KL divergence, same-top-token rate, Δp spread) | `ggml/src/ggml-common.h`, `ggml-quants.c`, `src/llama-quant.cpp`, `tools/imatrix/` | 50 |
| K-quant, IQ4 non-uniform and IQ2/IQ3 lattice decoders | `ggml/src/ggml-common.h`, `ggml-quants.c` | 51 |
| Tokenizer families (SentencePiece, WordPiece, Unigram) and a bounded Jinja subset for chat templates | `src/llama-vocab.cpp`, `common/jinja/` | 52 |
| Grammar and JSON-schema constrained decoding; sampling chain | `common/json-schema-to-grammar.cpp`, `src/llama-sampler.cpp` | 53 |
| Speculative decoding: draft models, n-gram drafts, rejection-sampling tests | `common/speculative.cpp`, `tests/test-speculative-rejection-sampling.cpp` | 75, 87, 102 |
| Continuous batching; a unified cache pool; host-memory prompt cache; context checkpoints for recurrent models; slot save and restore | `tools/server/` | 173, 174 |
| Content-addressed weight upload between peers (`SET_TENSOR_HASH`), without its trust model | `ggml/src/ggml-rpc/` | 176, 185 |

llama-server also answers `/v1/systemone` for its own decision-model types.
That is an interoperation target for chapter 171, not an oracle for the Base
Labs slot readout.

## Fixtures it provides

These are independent oracles when their provenance is independent:

- **Tokenizer vectors.** `models/ggml-vocab-*.gguf` with `.inp` and `.out`,
  23 families including `qwen35`. The expected ids were produced by the upstream
  Hugging Face tokenizers (`convert_hf_to_gguf_update.py`), not by llama.cpp.
  Before use, confirm the vocabulary and merges match the pinned model's.
- **Chat templates.** 75 real templates in `models/templates/`, including
  Qwen3.5, gpt-oss, DeepSeek V3.1–V4, Ministral 3, Devstral and Nemotron.
- **Grammars and schemas.** `grammars/*.gbnf` and the expected cases in
  `tests/test-json-schema-to-grammar.cpp`.
- **Numerical tolerances.** `tests/test-quantize-fns.cpp` (round-trip and dot
  tolerances per format family), `tests/test-backend-ops.cpp` (each operator
  on each backend against the CPU), `tests/test-rope.cpp`, `tests/test-sampling.cpp`.

llama.cpp's sampler uses `std::mt19937`, so seeded runs cannot be matched with
Psionic's counter-based generator; compare greedy outputs and distributions.

## Format notes

ggml's MXFP4 stores element j in the low nibble and j+16 in the high nibble of
byte j; the original gpt-oss checkpoint stores elements 2i and 2i+1. Chapter 60
reads the original layout; a GGUF reader must not assume one for the other.
GGUF's default alignment is 32 bytes and its current version is 3.

## Benchmarking against it fairly

- Run only an owner-approved packaged build, never anything built from the
  clone. Record its version and build commit.
- Use the same weights: the same GGUF file for quantized runs, or a GGUF whose
  tensor bytes Psionic's reader proves equal to the pinned Safetensors.
- Match threads, device offload, attention mode, cache type and batch sizes.
- Report prompt processing at fixed lengths, decision-shaped runs (a shared
  prefix plus suffixes) and long prompts, with warm-up, at least five
  repetitions, mean and standard deviation, machine, driver, clocks and load.
- Treat the earlier claim that Psionic beat llama.cpp as unverified until it is
  reproduced this way.

## Licensing

Porting algorithms needs no notice. Copying tables (IQ grids, the IQ4
non-uniform values, K-quant scale packing) or fixture files carries the MIT
notice in a Psionic `NOTICE` and the chapter that uses it.
`ggml/src/ggml-cpu/llamafile/sgemm.cpp` has its own Mozilla notice. Tokenizer
data in the vocabulary files is under each model's own licence.
