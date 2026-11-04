# TwIL-8B target assessment

webAI released TwIL-8B on 2026-11-04 as open weights
([model card](https://huggingface.co/webAI-Official/TwIL-8B)). This note
says what the model is, what its claims mean, whether we can use it, and
what Psionic needs to run it. The owner asked about it as a local model for
Aethyrforge.

## Summary

- **The model:** a post-train of IBM's `granite-4.2-8b`, a plain dense
  decoder. Psionic already has most of the pieces: GGUF, K-quants, a dense
  decoder block with grouped-query attention and SwiGLU, and Metal hosts.
- **The license:** the weights are under the **webAI Non-Commercial
  License 1.0**, "non-commercial research and educational purposes only".
  We can't ship it in Aethyrforge, serve it on Pylon, or use it in paid
  work without a license from webAI, and even a product evaluation may
  need their okay (see License notes).
- **The plan:** build the Granite path in Psionic. The base model is
  Apache 2.0 and has the same architecture and tokenizer. Evaluate TwIL-8B on
  it once the license question is settled. Ship TwIL-8B only if webAI grants a commercial license, and
  otherwise ship Granite 4.2 8B or our own post-train.

## What it is

From `config.json` and the model card (pinned 2026-11-04):

| Property | Value |
| --- | --- |
| Architecture | `GraniteForCausalLM`, dense, untied embeddings |
| Size | 8.79B parameters; bf16 16.38 GiB; Q4_K_M GGUF 4.98 GiB |
| Layers | 40; hidden 4096; FFN 12,800 (SwiGLU, `silu`) |
| Attention | 32 query heads, 8 KV heads (GQA), head dimension 128, no bias |
| Position | RoPE, theta 10,000,000; 131,072-token context |
| Norm | RMSNorm, eps 1e-5 |
| Granite scales | `attention_multiplier` 0.0078125 (1/128, not 1/√128); `embedding_multiplier`, `residual_multiplier` and `logits_scaling` are all 1.0 |
| Tokenizer | GPT-2-style byte-level BPE, vocabulary 100,352; BOS 100283, EOS `<\|im_end\|>` 100257 |
| Chat format | ChatML-style roles; a `<think> … </think>` block before the answer, with `enable_thinking` and `low_effort` switches in the template |
| Variants | `balanced` (repository root, default) and `verbose` (`verbose/`), same architecture |
| Files | Safetensors per variant; GGUF: Q4_K_M (both), Q8_0 and F16 (verbose) |

## The claims

The tweet's "93% on coding, 92% on instruction following, 90% on contest
math" is the share of the best frontier score on the model's three best rows:
LiveCodeBench, IFBench and HMMT Nov 2025. The card itself is more careful:

- **Strong for its size on math and code.** MATH-500 98.2, AIME 2025 88.3,
  LiveCodeBench 81.6. It's ahead of Qwen3.5-9B on code (+16.0) and IFBench
  (+9.9), and level on contest math.
- **Weak on knowledge.** GPQA Diamond 70.2 (Qwen3.5-9B 81.7), HLE 12.6. Its
  frontier share there is 75% and 26%.
- **Caveats in the card:** competitor scores are published figures that
  webAI didn't rerun; several TwIL rows were measured on the verbose
  variant only; there's no safety or preference tuning beyond Granite's.
- **Long reasoning:** about 19,500 output tokens per AIME 2026 problem.
  That's fewer than Qwen3.5-9B, but far too many for an interactive edit.
  For Aethyrforge it would run with thinking off or `low_effort`, and the
  benchmarks don't measure that.

None of this is our measurement. Treat it as a claim until our own eval
runs.

## Fit for Aethyrforge

Aethyrforge's planned local interpreter is gpt-oss-20b
(`apps/aethyrforge-desktop/README.md`). The interpreter turns a spoken
request into a typed edit: world data, tuning or code
(`docs/verse/2026-11-04-aethyrforge-conductor.md`).

| | gpt-oss-20b | TwIL-8B / Granite 4.2 8B |
| --- | --- | --- |
| Weights in memory | about 13 GB (MXFP4) | 5 GB (Q4_K_M), 8.7 GB (Q8_0) |
| Active parameters per token | 3.6B (mixture of experts) | 8.8B (dense) |
| Decode speed | faster per token | about 2–2.5× more work per token |
| Psionic today | Metal host exists (`psionic/platform/gptoss-metal.c`) | not yet |
| License | Apache 2.0 | TwIL: non-commercial. Granite: Apache 2.0 |

The 8B model uses less than half the memory, which matters on a Mac that
already runs the world, a room and Parakeet. It decodes slower per token. A
typed edit is a short answer, so the deciding numbers are accuracy on our
edit tasks and time to the first useful token, not contest scores.

KV cache: 40 layers × 8 heads × 128 × 2 (K and V) × 2 bytes = 160 KiB per
token in f16. A 32k context is 5 GiB; the full 131k is 20 GiB. Aethyrforge
should cap its context at 16k–32k.

## What Psionic needs

What exists:

- GGUF v3 reading (chapter 49), Q8_0 (chapter 50) and K-quants including
  Q4_K and Q6_K (chapter 51, `psionic/lib/kquant.coax`).
- A dense decoder block with RMS scaling, grouped-query attention, rotary
  tables, SwiGLU and an incremental KV cache (roadmap, "dense decoder block").
- Bounded Jinja chat templates (chapter 52) and byte-BPE (chapter 123).
- Metal hosts for gpt-oss, Elephant and Parakeet (`psionic/platform/`).

What's missing, in order:

1. **The Granite binding.** Map the GGUF `granite` architecture's tensor
   names and metadata to the dense block. Apply `attention_multiplier` as
   the attention scale. Read the other three multipliers and refuse any
   value Psionic doesn't apply, instead of ignoring it.
2. **Granite's tokenizer.** GPT-2-style byte-level BPE with its pinned
   pre-tokenizer pattern and 100,352 entries, checked against
   `tokenizer.json`'s merges and Hugging Face's own encodings of a fixed
   text set.
3. **The chat template.** Its Jinja, including `enable_thinking`,
   `low_effort` and the tool-call macros, checked within the chapter 52
   bounds, or a refusal naming the unsupported construct.
4. **A dense 8B decode path on Metal.** The gpt-oss kernels are MXFP4
   expert kernels. A dense model needs Q4_K and Q8_0 matrix-vector kernels
   for decode, a batched prefill path, and GQA attention over a paged
   f16 KV cache.
5. **Parity.** Match llama.cpp on the same GGUF file, which supports
   `granite`, as `psionic/docs/llama-cpp.md` describes: token IDs, per-layer
   activations within tolerance, full-vocabulary logits and greedy
   continuation, then decode speed on the same Mac.
6. **The Aethyrforge eval.** Our typed-edit set, with thinking off and with
   `low_effort`, against gpt-oss-20b: accuracy, time to first token and
   tokens per second.

Steps 1–5 apply unchanged to Granite 4.2 8B, so the work isn't lost if
TwIL-8B's license never opens.

## Status

Work on `ibm-granite/granite-4.2-8b` (issue #12870). Run
`psionic/check.sh --only granite`. The pinned files live in
`~/models/psionic/granite-4.2-8b-a7042144125c/`: the base revision
`a7042144125c`, and IBM's GGUF revision `93f3f6a8938e` with `SHA256SUMS`.
Without them the gate runs only the owned tiny model.

- **Steps 1–3 landed.** `psionic/lib/granite/model.coax` binds the GGUF
  and refuses any embedding, residual or logit multiplier other than 1.
  `psionic/lib/granite/tokenizer.coax` encodes and decodes the byte-level
  BPE. `psionic/lib/granite/chat.coax` renders plain conversations with
  `enable_thinking` and `low_effort`. Chapter 52 can't compile the
  published template (its first refusal is whitespace control), and tool
  calls are refused, not approximated. On both GGUF files, the 48
  tokenizer texts and 11 chat renders equal Hugging Face's
  `tokenizers` 0.22.1 and `transformers` 5.15.1 token for token, and the
  GGUF vocabulary, merges and added tokens equal `tokenizer.json`.
  llama.cpp b10330 tokenizes all 48 texts the same way.

## License notes

- **TwIL-8B:** webAI Non-Commercial License 1.0. Section 3.3 limits use of
  the work "and any derivative works thereof" to non-commercial research
  and education. Derivatives include our quantizations and fine-tunes.
  Section 3.5 bars using webAI's names to promote a derivative. Keep the
  weights out of git, out of release bundles and out of Pylon's served
  models.
- **Granite 4.2 8B:** Apache 2.0 (IBM), with the same architecture,
  tokenizer and context.
- **Our eval:** an evaluation by OpenAgents to inform a product decision is
  arguably commercial use. Ask the owner or counsel before running it,
  or ask webAI for an evaluation or commercial license.
