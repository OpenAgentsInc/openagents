# Inkling 2 Mini as an inference target

Target specification, 2026-11-20. Based on Thinking Machines Lab's
[Introducing Inkling 2 Mini](https://thinkingmachines.ai/news/inkling-2-mini/)
announcement. For live Apple Silicon measurements of the native Psionic Metal
GGUF runtime (`55bf398569`) against `llama-server`, see
[Inkling 2 Mini Benchmark: Psionic Metal vs llama.cpp Metal](inkling-benchmark.md).
Part XXVII of [the Psionic book](../../book/SUMMARY.md)
(chapters 266–271) builds native Psionic support from original NVFP4/MXFP8
weights through Metal execution, `/v1/responses` tool calling, and
OpenAgents Desktop integration.

## Why Inkling 2 Mini

Inkling 2 Mini is an open-weights **looped transformer** designed for local
agentic coding, search and tool use:

- **Looped Mixture-of-Experts architecture.** 26.4B total parameters, 3.4B
  active parameters, and 5.4B weight activations per token. Its middle layers
  run **three times per token** (a 3-pass recurrence), trading extra internal
  compute per token for stronger reasoning and shorter chains of thought.
- **Native NVFP4 and MXFP8 weights.** Most weights are trained and deployed
  natively in **NVFP4**, with the **last four layers in MXFP8**, giving a
  ~20 GB memory footprint without post-hoc GGUF re-quantization.
- **Local hardware fit.** Designed to run locally on Macs with ≥32 GB unified
  memory and on NVIDIA DGX Spark.
- **Agentic coding and tool-use focus.** Leads open-weights models under 100B
  total parameters on DeepSWE v1.1 (48.2%), Vals Index v2.2 (46.5%),
  SWE-bench Pro v2 Hard (77.9%), SWE-bench Verified (79.2%), BrowseComp
  (79.0%), DeepSearchQA (85.3%), and MCP Atlas (82.7%).
- **Variable-loop research companion.** Released alongside
  `TML-d24-varloop-research`, a variable-loop research checkpoint used to study
  how loop count affects no-CoT reasoning horizon, serial reasoning, CoT
  legibility and monitorability.

## Architecture and weight formats

Admission pins the exact upstream repository revision, full SHA-256 file
hashes, `config.json`, tokenizer files and complete tensor inventory before
any execution claim is recorded. The target contract covers:

1. **Weight formats.** Native NVFP4 (microscaled 4-bit floating point with
   block scales and per-tensor/global scales as specified by the checkpoint)
   for the majority of layers, and MXFP8 (microscaled 8-bit floating point with
   shared E8M0 block exponents) for the final four layers, plus unconverted
   embeddings, norms, router gates and LM head as configured.
2. **Looped middle-layer recurrence.** A prelude of non-looped layers, a middle
   recurrent block executed `L = 3` times per token (and configurable `L >= 1`
   for `TML-d24-varloop-research`), and a postlude of non-looped layers.
   Recurrence state and KV-cache indexing are explicit per `(layer, loop_pass)`
   so prefill and single-token decode match bit-for-bit within declared
   floating-point tolerances.
3. **MoE routing.** Top-k expert routing per token at each pass through the
   looped middle layers, reusing Psionic's MoE router foundation from
   [chapter 61](../../book/ch061-a-mixture-of-experts-layer.md).
4. **Chat template, thinking and tool calls.** Exact tokenization of system,
   user, assistant, reasoning/CoT and tool-call / tool-result turns, wired into
   Psionic's `/v1/responses` contract.

## Known limitations

Looping increases effective depth for reasoning and long-context work without
increasing factual knowledge stored in weights. Factual recall benchmarks
(such as AA-Omniscience) remain in line with similarly sized models; in
OpenAgents Desktop, Inkling 2 Mini should be paired with search, file and tool
access rather than treated as a closed-book encyclopedia.

## What Part XXVII must establish

Each chapter in Part XXVII earns its own evidence before the next depends on it:

- **Chapter 266 (NVFP4 and MXFP8 weights):** pinned file hashes, tensor
  inventory, exact CPU reference dequantization for both NVFP4 and MXFP8, and
  refusal cases for corrupt scales, mismatched shapes or unknown dtypes.
- **Chapter 267 (Looped MoE transformer):** a small hand-checked looped MoE
  reference with explicit per-pass KV-cache layout, variable loop count
  (`L = 1..10`), and exact prefill/decode equivalence.
- **Chapter 268 (CPU execution):** full ~20 GB original checkpoint loading,
  tokenizer/chat-template fixtures, and logit + greedy-decode parity against
  an independent reference.
- **Chapter 269 (Metal execution):** Coax-generated Metal kernels for NVFP4
  and MXFP8 matvec/matmul, looped MoE prefill and decode, and ≥32 GB unified
  memory residency planning.
- **Chapter 270 (Tool calls and Responses):** streaming reasoning/CoT and
  structured tool-call parsing and emission over Psionic's `/v1/responses` API.
- **Chapter 271 (OpenAgents Desktop):** resident local Psionic Metal runner
  integration in `apps/openagents-desktop/` with model discovery/download,
  memory guardrails, model picker selection, and end-to-end streaming tool-use
  sessions.
