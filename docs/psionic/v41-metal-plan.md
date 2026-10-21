# DeepSeek-V4.1 stages on Apple Metal: plan and estimate

Status: plan, written 2026-10-21 for
[#12418](https://github.com/OpenAgentsInc/openagents/issues/12418). The port
itself is [#12420](https://github.com/OpenAgentsInc/openagents/issues/12420). Nothing here is built yet.

A V4.1 group can now mix GPU types (#12418): each stage is checked
against its own manifest entry, and a small GPU can keep its routed experts
in host memory. A Mac stage is the next device class. Groups run on GPUs
only (owner, 2026-10-21): CPU stages stay for unit tests and reference
diagnostics, so a Mac joins a group through Metal, never through the CPU
port. This plan scopes a Metal backend for `psionic-v41-stage` that runs a
contiguous layer range on an Apple GPU and joins a group like any other
stage.

## What the stage must do

The Metal stage implements the same contract as W7's CUDA stage
(`psionic_backend_cuda::v41_stage::V41Stage`) and W9's `StageExecutor`:

- Load a layer range from the checkpoint with FP8 and FP4 weights kept
  packed, and allocate request state for a context and step size.
- Run a step of one or more tokens with all request state on the device:
  embeddings on stage 0, then per layer the hyper-connection mixes,
  attention, Engram where the layer has it, the router and experts, and on
  the last stage the norm, head, and sampler.
- Send and take stage state on the CUDA wire (`F32` residuals and pre-mixes,
  BF16 KV and indexer-key rows, `I32` selections, `U32` candidates), so a
  Metal stage joins CUDA stages without conversion elsewhere.
- Announce runtime `psionic-v41-stage/1+metal`, device class `metal-<gpu
  family>` (for example `metal-apple9`), its profile, and a verified bundle,
  and refuse a manifest entry planned for another class.

## What Psionic's Metal backend already has

`crates/psionic/crates/psionic-backend-metal` (about 21,000 lines) serves the
GGUF families and the Clef decision lane. The parts V4.1 can reuse:

| Need | Existing piece | Reuse |
| --- | --- | --- |
| FP4 E2M1 experts with a UE8M0 scale per 32 values | `psionic_quantized_matvec_mxfp4`, `psionic_mul_mv_id_mxfp4`, `psionic_expert_matvec_f32_ids_mxfp4` (GPT-OSS) | High. MXFP4 is the same number format. V4.1 keeps codes and scales in separate tensors with its own nibble order, so the loads change; the decode and the expert-id indirection carry over. |
| FP8 E4M3 with 32 × 32 block UE8M0 scales | None. Q8_0 is int8 with an f16 scale per 32. | Low. A new kernel, modeled on the CUDA `v41_block_matvec.cu` (291 lines). |
| Activation quantization to FP8 | None | New, small. |
| Router top-k and softmax | `psionic_top_k_single_row_f32`, `psionic_top_k_softmax_single_row_f32`, expert bias and aggregation kernels | Medium. V4.1 routes with a bias, ties to the lower id, and clamped SwiGLU. |
| RMS norm, RoPE | `psionic_per_head_rms_norm_*`, `psionic_rope_neox_*`, Clef's `clef_rms_norm_f32` | Medium. V4.1 needs tail RoPE with YaRN and the CPU's summation order. |
| Dense decode attention | `psionic_decode_attention_dense_f32`, Clef flash attention (`clef_flash_attention256*`) | Low. V4.1's window, compressed, and sparse attention with a sink are different kernels. |
| Sampler | `psionic_argmax_f32`, top-k selection | Medium. The last stage samples with the front's temperature, top-p, and seed, as `v41_sample_kernel` does. |
| Buffers, submissions, command reuse | `MetalBuffer`, `MetalSubmission`, the graph-reuse runtime (`MetalAttentionGraphRuntime`) | High. Metal has no CUDA graph capture; replaying a recorded command sequence per token fills that role. |
| Hyper-connections (Sinkhorn), compressor, indexer, top-512 selection, layer-20 candidates, Engram hashing and gather | None | New. |

Apple silicon's unified memory removes two CUDA problems: Engram tables
(101 GB each at layers 1 and 14) and routed experts need no page-locked
copies or bus reads, and a stage's weights load without a host-to-device
copy.

## Work, in order

Sizes follow the completion doc: S under a day, M one to three days, L
longer. The CUDA work it ports took W4–W7 (#12391–#12394).

| Step | Work | Size |
| --- | --- | --- |
| M1 | **Packed matvecs.** Port `v41_block_matvec.cu`: FP8 32 × 32 block and FP4 E2M1 row-32 matvecs over a stack, with device-selected matrices for routed experts, and the FP8 activation quantizer. Parity with the CPU path on the committed real rows (`testdata/deepseek_v41_matvec`), as G3 checks CUDA. | M |
| M2 | **Mixes, Engram, routing.** Port `v41_mix.cu`: the joint RMS projection, pre and post mixes, 20-iteration Sinkhorn, collapse and expand; the Engram hash and gather; router logits, top-6 with lower-id ties, clamped SwiGLU, the expert sum. Parity on `testdata/deepseek_v41_mix`. | M |
| M3 | **Attention.** Port `v41_attention.cu`: tail RoPE and YaRN, the window ring and cache quantizers, window and compressed latent attention with the sink, the ratio-1 and ratio-2 compressors and row store, index scores, top-512 selection, and layer-20 candidate blocks, with consumers reading the KV sources on the device. Parity on `testdata/deepseek_v41_attention`, including the 127/128/129 cases. | L |
| M4 | **Stage and sampler.** Port `v41_stage.rs` and `v41_stage.cu` to a `V41MetalStage`: load a layer range, allocate request state, run a step as one recorded command sequence with one wait, sample on the device, and keep W9's boundary sections on the CUDA wire. The split test (stages equal the unsplit model bit for bit) and the decode-step budget from W7. | L |
| M5 | **Worker integration.** `--backend metal` in `psionic-v41-stage`, the hello's device class and runtime, a Metal build bundle (`BUILD.json` with the worker and the compiled `metallib` hashes), and Pylon's per-stage identity entry for it. A mixed group with a Mac stage answering through Pylon. | M |
| M6 | **Qualification.** Run the Metal stage's G3 and G4 checks on the owner's Mac and qualify the profile under G1 v2 before any group with a Mac stage is advertised as qualified. | M |

Estimate: **15 to 20 working days for one agent**, about three to four
weeks; M1, M2, and M3 can run in parallel, which brings it to about two weeks
with three agents. Compute cost is near zero: the kernels test on the
owner's Mac, and the group run needs only the cloud stages it already uses.

## Risks

- **No double precision on Apple GPUs.** The CUDA kernels compute `exp`,
  `log1p`, `cos`, and `sin` in double precision and round once, so the
  device equals a host model bit for bit. Metal has no `double`. The port
  needs correctly rounded float implementations (or float-float
  arithmetic) for those four functions, and its host model must follow
  them. G3's 2-ulp tolerance for these functions still holds; the
  bit-for-bit host-model checks need the new host model.
- **Reduction order.** Metal's SIMD-group reductions sum in a different
  order than CUDA's warp shuffles. Kernels that equal the CPU today do so by
  repeating its sequential order; the port must keep that order where G3
  requires equality.
- **Memory.** A layer needs about 7.4 GB of weights. A 128 GB Mac (the
  owner's M5 Max) holds about 12 layers with room for macOS; a 192 GB Mac
  Studio about 20. Placement uses `plan_devices` with a `metal-*` class and
  the unified memory the Mac reports, and Engram layers count against the
  same memory.
- **Wake and power.** A Mac stage that sleeps drops out of the group; the
  stage must hold a power assertion while it serves, as the desktop app does
  for hosts.

## Issue

[#12420](https://github.com/OpenAgentsInc/openagents/issues/12420) tracks
the port, on the OpenAgents project board.
