# DeepSeek model support and infrastructure plan

Reviewed on 2026-10-20 against OpenAgents
`818354f0e5317f40a2d6adf0b7a3228f10b8b20d`. This extends the
[DeepInfer review](deepinfer-lessons.md) and
[implementation roadmap](implementation-roadmap.md).

The survey enumerated 45 public repositories in the DeepSeek GitHub
organization, shallow-cloned 18 selected repositories, and reused the existing
DeepInfer clone. Review depth ranges from documentation screening to the
specific source implementations linked below. Four official checkpoint
configurations and tokenizer configurations were downloaded at immutable
revisions. No model weights, dependencies, vendor submodules, or device runs
were needed. All support targets below are proposed; this change implements
no decoder and qualifies no new model.

## Recommended support sequence

1. Qualify an R1 distill through existing Qwen/Llama execution and add its
   actual DeepSeek prompt and reasoning contract. Use the existing explicit
   llama.cpp option for a GPU comparison where native GPU support is absent.
2. Integrate DeepSeek's Rust recipe components for V4/V4.1 protocol behavior,
   with token-level fixtures and a mock backend first. Keep this capability
   separate from executing those models.
3. Build native MLA and DeepSeek MoE correctness around V2-Lite-Chat, then
   accelerate that implementation on a supported local backend.
4. Add V3/R1 routing and weight-format variants, then V3.2 sparse indexing.
   Treat V4.1's attention, memory, and residual graph as a further architecture
   project with its own reference execution and hardware qualification.

This gives OpenAgents a practical first DeepSeek offering while creating the
primitives required for native DeepSeek models. Kernel integration follows
reference correctness and measured hardware compatibility.

## What Psionic currently supports

| Area | Finding at the source baseline | Consequence |
| --- | --- | --- |
| Decoder families | `GgufDecoderFamily` and architecture classification include Llama, Qwen variants, Mistral, Gemma4, and GPT-OSS; they have no DeepSeek decoder case. [P1] | A model alias or an expert-count field does not enable DeepSeek execution. |
| Prompts | `GgufPromptTemplateFamily` and the digest allowlist have no DeepSeek template family. [P1] | Reusing Qwen/Llama math still requires checkpoint-specific prompt and tokenizer qualification. |
| Runtime paths | Generic dense CPU execution exists; the native CUDA/Metal text-family coverage is narrower. Pylon already selects an explicit llama.cpp engine. [P2] [P4] | Begin a distill with CPU qualification and an explicitly labeled external-engine GPU path; do not label it native CUDA by inheritance. |
| MoE and weights | GPT-OSS/Gemma expert execution and cluster expert-placement contracts exist. `QuantizationMode` includes GGUF block formats, including MXFP4. [P2] [P3] | Reuse primitives, but implement DeepSeek routing, projections, and exact scale layouts. An FP4 name does not establish compatible bytes. |
| API awareness | Conformance code recognizes an external parser named `deepseek3`. [P5] | This is parser capability recognition, not a native DeepSeek decoder or proof of V4.1 protocol compatibility. |
| Qualification | The pinned Pylon catalog and runner already exercise native and llama.cpp model paths. Public prompt-template golden fixtures exist. [P4] [P6] | Extend those assets rather than adding another model catalog, server, or qualification framework. |

## Concrete model targets

These are official source checkpoints, not ready-to-load GGUF entries. Each
qualification must additionally pin the actual converted/quantized artifact,
converter revision, ordered files, hashes, tokenizer, and runtime build.

| Candidate and inspected revision | Architecture evidence | Initial support target |
| --- | --- | --- |
| [R1-Distill-Qwen-1.5B config][H1], `ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562` | `Qwen2ForCausalLM`, `model_type=qwen2` | First small distill: native CPU correctness and separately qualified llama.cpp GPU serving. Native Qwen3.5 CUDA support is not a Qwen2 implementation. |
| [R1-Distill-Llama-8B config][H2], `6a6f4aa4197940add57724a7707d069478df56b1` | `LlamaForCausalLM`, Llama3 RoPE scaling | Second distill after tokenizer, template, and long-context behavior pass; preserve its underlying Llama license requirements. |
| [V2-Lite-Chat config][H3], `85864749cd611b4353ce1decdb286193298f64c7` | MLA, 27 layers, 64 routed experts, two shared experts, six active experts | First native DeepSeek architecture reference. The repository lists 16B total and 2.4B active parameters; all required weights still need storage/residency or an explicit offload strategy. [V2] |
| [V4.1-Flash-1017 config][H4], `1bae0e1088993b9b82de08f59de5be5a34e42170` | `deepseek_v41`, separate text/vision configs, compressed/shared attention, 384 routed experts, mixed FP8/FP4 weights | Protocol fixtures now; native execution only after its additional graph/state components and a suitable device plan are qualified. |

The inspected [Qwen distill tokenizer config][H1T] and
[Llama distill tokenizer config][H2T] contain the same chat-template text,
SHA-256 `56a1447ad31926fdc21fb07e56e5642bd9c850c4f52d8c8af7bbe5f079a84f5f`.
That hash is absent from Psionic's current allowlist. Their BOS/EOS spellings
are DeepSeek-specific, and the generation prompt starts a thinking segment.
The shared template does not make their token IDs or numerical architectures
interchangeable. [P1]

Context metadata also needs qualification: V2-Lite-Chat's config, tokenizer
config, and repository model table report different context limits. Do not
select the largest number as an operator claim. Record an explicit tested cap
and enforce prompt plus output budgets. The same principle applies to the
V4.1 checkpoint's million-token allocation and recipe output defaults. [H3] [H3T] [V2]

## Infrastructure worth learning from

Every repository link below is pinned to the reviewed commit. The intended
use separates immediate Psionic work from larger deployment prerequisites.

| Repository | What to take from it | Scope and prerequisite |
| --- | --- | --- |
| [deepseek-recipe][Recipe] | Official Rust conversation conversion, V4/V4.1 encoding, token-aware output parsing | Integrate behind current model/API boundaries; it supplies neither inference nor tool execution. |
| [DeepSeek-V2][V2] | Small MLA-MoE target and architecture reference | Use the Lite checkpoint to establish native correctness. |
| [DeepSeek-V3][V3] | Readable MLA, shared/routed experts, grouped routing, FP8 reference math | Implement exact semantics before fusing kernels; full-model memory differs greatly from active-parameter count. |
| [DeepSeek-R1][R1] | Distill lineage, model-specific generation behavior, licensing | Distills exercise existing base architectures; full R1 is a separate MoE target. |
| [DeepSeek-V3.2-Exp][V32] | Sparse indexer and attention reference | Its indexer and MLA use different RoPE layouts; the repository records a correction to this detail. |
| [FlashMLA][FlashMLA] | Attention/cache layout, variable-length kernel tests, fused operations | Current release targets V4.1 on SM100/SM103 or Ascend 950. Earlier models require another pinned generation. |
| [DeepGEMM][DeepGEMM] | Contiguous versus masked grouped expert GEMMs, MQA index scoring, fused Mega MoE | Main optimized CUDA path requires SM90/SM100; scale/layout and communication assumptions are part of the contract. |
| [DeepSelect][DeepSelect] | Separate top-k algorithms for index scores and vocabulary sampling | Qualify dtype, shape, strides, ties, sorting, and invalid-value behavior for each call site. |
| [DeepEP][DeepEP] | Reusable symmetric buffers, dispatch/compute/combine overlap, expert replica movement | Current path requires Hopper-or-newer, modern NCCL, and NVLink/RDMA. It is not transport for ordinary Pylon links. |
| [EPLB][EPLB] | Hot-expert replication and logical/physical expert maps | Adapt policy to existing sparse expert placement after measuring load and migration cost. |
| [LPLB][LPLB] | Communication-aware expert scheduling experiments | Research alternative; solver overhead and omitted compute costs can outweigh benefits. |
| [DeepSpec][DeepSpec] | Draft/target pairing, training recipes, acceptance and confidence calibration | Released drafts target specific Qwen3/Gemma4 checkpoints; none is automatically a draft for Psionic's Qwen3.8 or DeepSeek targets. |
| [TileKernels][TileKernels] | Reference and fused routing, quantization, mHC, and Engram operations | Reimplement required semantics in Rust first; TileLang/PyTorch is not a new Psionic product runtime. |
| [DeepJIT][DeepJIT] | Source/include/compiler/options-aware compilation caching and lazy loading | Apply cache identity and packaging lessons through current backends; it is C++ infrastructure, not a Rust inference engine. |
| [Engram][Engram] | Static lookup memory is separate from dynamic KV and MoE compute | Account for table placement, token history, gating, and memory traffic when required by a checkpoint. |
| [3FS][ThreeFS] | Asynchronous batched I/O and disaggregated storage design | RDMA storage infrastructure, not the missing DeepSeek decoder or a complete inference KV manager. |
| [DualPipe][DualPipe] | Measurement of compute/communication overlap | Primarily forward/backward training schedules; do not transplant a backward schedule into autoregressive decode. |
| [open-infra-index][InfraIndex] | Historical production context and links between components | Starting index, not a current compatibility matrix. DeepEP and FlashMLA have changed since its 2025 descriptions. |

Harness/UI repositories and the separate vision/OCR model families were
screened out of the first text-serving implementation. Ascend variants are
relevant to a future NPU backend, not a prerequisite for CPU/CUDA/Metal support.

## Reuse official Rust protocol components

`deepseek-recipe` is the most direct new integration opportunity. Pin the
reviewed source/release and keep its core, encoding, and parsing components
behind a DeepSeek adapter in `psionic-models`/`psionic-serve`. Keep OpenAgents'
request validation, transport, authorization, model selection, and tool
execution in their existing owners.

The implementation has two useful extension points:

- `TokenizerEncoder::encode_ids` receives explicit special-token ranges.
  `PromptTokenizer` encodes ordinary text without the added vocabulary and
  inserts only intentional special-token IDs. A Psionic adapter must preserve
  that distinction; tokenizing the rendered string generically loses it.
  [Recipe encoding]
- `StreamTokenizer` and `InferenceChunk::Tokens` retain token boundaries.
  Special tokens become marked pieces, and incomplete UTF-8 remains buffered.
  This lets the output parser distinguish a real reasoning/tool marker from
  ordinary text with the same spelling. A plain text chunk cannot provide
  that distinction. [Recipe decoder] [Recipe inference]

Keep rendering and parsing keyed by exact model version and tokenizer.
Recipe 0.2's encoders target V4/V4.1; do not use them as R1/V2/V3 templates.
Add the older models' pinned renderers and independent golden token fixtures
through the existing template mechanism. For V4/V4.1, prefer a small adapter
over copying the entire protocol library into Psionic. Audit dependency and
tokenizer feature choices; the demo servers and image package are not needed
for a text-only fixture integration. [Recipe] [Recipe manifest]

Resolve thinking mode, reasoning effort, and generation options for the selected
model version before dispatch. Recipe applies version-specific sampling
defaults; enforce the actual runtime's context/output budget and keep those
defaults scoped to the DeepSeek adapter. [Recipe options] [Recipe request]

Conversion can produce an `OutputConstraint`, but the inference caller must
enforce it. Connect it to R2's constrained decoder before advertising strict
JSON/schema/tool constraints. Prompting with a schema is not enforcement.
Map stream completion, errors, usage, and cancellation deliberately into
Psionic's existing API behavior, including reasoning/final separation and
incremental tool arguments. Test fragmented markers, UTF-8, literal marker
spellings, multiple tool results, long arguments, truncation, and no final
finish event. [Recipe request] [Recipe parser]

Keep vision disabled until its model execution and preprocessing are qualified.
Reject unsupported image requests explicitly. Do not inherit a permissive URL
fetcher or silently replace images with placeholder text.

## Build native execution in architecture order

### MLA and V2-Lite first

Extend the model loader with a distinct DeepSeek family and explicit tensor
mapping; do not route `deepseek2` through ordinary Llama attention. Preserve
immutable source/converted-artifact identity and reject unsupported variants.
Use a CPU reference with tiny deterministic fixtures and retained independent
reference outputs before running the complete Lite checkpoint.

The reference needs direct or low-rank query projection, compressed KV
projection and normalization, decoupled RoPE, output projection, causal masking,
and the checkpoint's YaRN/softmax scaling. Lite has no query LoRA rank and a
512-dimensional KV latent plus a 64-dimensional RoPE component. Test an
expanded attention reference against the compressed/absorbed computation;
then make latent-plus-position cache a distinct state format rather than
pretending it is conventional per-head K/V. [H3] [V2 implementation] [V3 model]

MoE tests must compare expert IDs, combine weights, shared-expert output, and
final block output. The Lite config selects softmax/greedy routing without
top-k renormalization. V3 adds grouped/bias-corrected selection variants whose
combine weights come from the original scores. V4.1's gate uses
`sqrtsoftplus` and separate image routing bias. These are versioned semantics,
not interchangeable implementations of a generic top-k function.
[H3] [V2 implementation] [V3 model] [V41 gate]

Add a single-node CUDA implementation after CPU parity, preserving a clear
fallback on SM89. Qualify Metal separately. Quantized Lite is a later measured
artifact choice; 2.4B active parameters do not make all 16B weights disappear
from the memory plan. Extend existing expert and matrix primitives only where
their semantics match.

### V3/R1, then V3.2 sparse indexing

Add the exact projection/router and weight-format variants required by a
pinned V3 or full R1 artifact. Validate scale blocks, packing, gate precision,
and expert identity before optimizing grouped GEMMs. FP8 storage can be
decoded to a reference precision without making the reference an FP8-fast-path
claim. Preserve checkpoint MTP tensors as separate optional functionality.
[V3 model] [V3 kernel]

V3.2 requires a separate learned indexer: score production, causal/valid
bounds, top-k selection, and indexed attention. Its reference config uses
`index_topk=2048`; V4.1 uses 512. The indexer RoPE is non-interleaved while MLA
RoPE is interleaved in the reviewed V3.2 implementation. Make these explicit
fixtures, including short rows, equal scores, invalid indices, and empty
attention selections. [V32 model] [V32 config] [H4]

DeepSelect's BF16 kernel warns that ties do not necessarily prefer the earlier
index. Its default NaN policy can trap the device kernel. An adapter must
qualify tie behavior for model/sample reproducibility and preserve a controlled
error path; do not import a process-aborting default into the native serving
contract. Sampling top-k and DSA indexing need separate tests. [Select kernel] [Select interface]

### V4.1 is a distinct graph

The pinned checkpoint and DeepInfer builder require more than V3.2 MLA plus
another weight file:

- Compressed and sliding-window attention with cross-layer KV/index sharing,
  including source-layer dependencies and cache recovery policy.
- Multi-stream manifold hyper-connections (`hc_mult=4`) and their mixing/
  normalization operations; a stage boundary can carry wider residual state.
- Engram lookup tables, history/addressing, gating, and their memory placement.
- Version-specific MoE scoring, shared/routed expert layouts, and SwiGLU limits.
- Exact FP8/FP4 block/scaling formats, and separately optional DSpark/MTP and vision.

The [checkpoint config][H4], [DeepInfer builder][V41 builder],
[attention parameters][V41 attention], and [mHC implementation][V41 mhc]
provide concrete starting points. Create a reference and contract for each
component before composing them. Generic R4/R5 stage boundaries/packages must
include source-layer shared state and Engram/mHC dependencies; an equal layer
split is not sufficient. Keep full, exact replay as the initial reference.
Bounded SWA reconstruction remains an explicitly approximate follow-up.

## Hardware and version combinations must be explicit

The documented RTX 4080 is SM89, outside the primary optimized hardware targets
below. Use it for distill qualification and native reference/portable-kernel
work, not as proof that these datacenter libraries will load or run.
[NVIDIA GPU list]

| Component generation | Reviewed boundary | Psionic decision |
| --- | --- | --- |
| FlashMLA `0f11e178` | Drops Hopper and earlier V3/V3.2/V4 models; changes FP8/FP4 KV layout; current CUDA targets SM100/SM103. [FlashMLA] | V4.1-specific optional backend, with its cache format pinned. |
| Older FlashMLA `ba89a346` | Separate dense/sparse kernel support matrix for SM90/SM100 and older models. [Legacy FlashMLA] | Evaluate exact kernel/model combinations for V3/V3.2. Do not pair its cache buffers with the new interface. |
| DeepGEMM `5081f851` | Main path targets SM90/SM100; SM90 and SM100 use different scaling-factor formats. [DeepGEMM] | Bind kernel/device/scale layout in the execution profile; keep a portable implementation. |
| DeepEP `b5a33eef` | V2.5 uses NCCL Gin; V1/NVSHMEM is removed. Hopper-or-newer, NVLink/RDMA, and NCCL/CUDA prerequisites apply. [DeepEP] | A future scale-up/fast-fabric backend. Existing ordinary-network pipeline transport remains separate. |

Each profile must bind model/config revision, weight and tokenizer digests,
prompt/parser version, backend, kernel commits, device capability, scale/packing
layout, KV encoding, and output mode. Validate runtime compatibility before
loading weights. Changing a kernel generation can invalidate cache and stage
state even when the model name is unchanged.

DeepJIT's cache includes source, tracked includes, compiler identity/options,
and an application dependency signature. Reuse those identity principles
through Psionic's backend build/cache paths. A compilation-cache fingerprint
is not artifact authentication. For any optional upstream kernel binding,
inventory PyTorch/C++ dependencies and buffer/stream ownership explicitly;
the Python wrapper is not automatically a standalone Rust-callable ABI.
[JIT runtime] [GEMM binding]

## Concrete implementation queue

These steps extend R0–R9 rather than replace them. They are independent of
another DeepSeek organization survey and do not require creating GitHub issues
or starting a new service during this documentation task.

| Step | Priority and dependencies | Existing owner to extend | Completion evidence |
| --- | --- | --- | --- |
| DS0: Pin and qualify the first R1 distill | P0; R0 measurements | `psionic-models` tokenizer/templates, `psionic-serve`, existing Pylon model catalog/runner | Exact artifact and prompt-token fixtures, CPU answer/continuation results, reasoning/final separation, EOS/truncation tests, and separately labeled external-engine GPU results. Tool support stays disabled until tested. |
| DS1: Add the official Rust V4/V4.1 protocol adapter | P0; independent of a full native model | `psionic-models`, `psionic-serve`; R2 for strict constraints | Versioned rendering/token fixtures and mock streamed tool/reasoning/API tests. Report this as protocol-qualified, not native inference support. |
| DS2: Implement a V2-Lite MLA/MoE CPU reference | P1; DS0 artifact/token contract | `psionic-models`, `psionic-runtime`, `psionic-serve`, CPU backend | Independent layer logits, compressed-versus-expanded attention, router/combine parity, cache continuation/reset, and a complete pinned-model result. |
| DS3: Accelerate single-node MLA/MoE and add V3/R1 variants | P1; DS2; R1/R3 for batched/overlap claims | Existing CUDA backend, expert kernels, runtime memory plans | Exact layout validation, quantized/reference parity, measured total resident memory and latency/committed throughput. Metal is separately qualified. |
| DS4: Add V3.2 DSA and then V4.1 reference components | P2; DS2/DS3; DS1 for V4.1 serving | Model graph, cache formats, indexed attention, mHC/Engram modules | Per-component and full-model parity. Current FlashMLA/DeepGEMM acceleration additionally needs a compatible pinned device/kernel profile. |
| DS5: Scale expert execution on a qualified fast fabric | P2; DS3 or DS4, R1, R4–R7 | `psionic-cluster`, backend collectives, existing package/delivery contracts | Measured routing skew, per-rank critical path, migration/replica costs, correct expert maps, and complete requests under failure. No token-level WAN-EP claim. |
| DS6: Qualify matched DSpark drafts | P2 experiment; R9 and an exact target/draft pair | Existing MTP reference, sampler, batched verifier | Fixed-length baseline first; acceptance plus ECE/Brier/AUROC calibration, rejection-state parity, and measured committed throughput in the intended thinking/tool mode. |

First land DS0's exact token/template fixtures and DS1's adapter fixtures as
small changes, then DS2's attention/router reference. Avoid enabling a family
enum or public capability before its executor passes. Keep protocol-qualified,
external-engine-qualified, native-reference, native-device, and distributed
qualification results distinct and attach them to concrete artifacts.

## Further infrastructure lessons and what to defer

DeepGEMM separates prefill's contiguous grouped layout from decode's masked
layout when device-side expert counts are not available to the CPU. Its
Mega MoE overlaps dispatch, compute, and combine within a symmetric-memory
domain. This reinforces R1/R3: an efficient MoE backend needs a batch and
buffer-lifetime contract, not an isolated faster matrix multiply. [DeepGEMM]

EPLB separates logical experts from physical replicas; LPLB investigates
communication cost but documents solver overhead and missing matrix cost.
Add observed demand and replica-migration accounting to the existing sparse
expert scheduler before considering either policy. Keep original routing
semantics intact when moving weights. [EPLB source] [LPLB]

DeepSpec provides a path to obtaining confidence heads rather than inventing
one from logits. Its released checkpoints are matched to specific non-thinking
targets, and the code tracks per-position calibration metrics. Re-evaluate
thinking/tool workloads and target revisions. Do not start its default large
target-cache preparation as part of a serving integration. [DeepSpec] [Spec calibration]

3FS's useful near-term lesson is batched asynchronous I/O with explicit
buffers. Its storage cluster, metadata services, and RDMA clients are not a
drop-in persistent KV service. Keep Psionic's current content-verified
datastream and cache ownership; revisit shared remote KV only after R8 shows
that restore/transfer cost beats recomputation on a real workload. DualPipe's
training schedule and smallpond-style data processing do not block model
serving. [Storage design] [DualPipe]

Retain each repository's applicable license and third-party notices if code is
adapted. Check checkpoint terms separately from code licenses: the original
V2/V3 model agreements and R1 distills' base-model requirements are not one
organization-wide license. The existing artifact governance fields are the
place to carry those facts. [V2] [V3] [R1]

## Source revisions

Repository links resolve to the inspected commit and file. Official model
metadata links include immutable Hugging Face revisions. Psionic paths refer
to the source baseline above. The survey is evidence for the plan, not a claim
that any unrun hardware configuration passed.

[Recipe]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/README.md
[Recipe encoding]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe-encoding/src/tokenizer.rs
[Recipe decoder]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/src/stream/decoder.rs
[Recipe inference]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/src/stream/inference.rs
[Recipe manifest]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/Cargo.toml
[Recipe options]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/src/request/options.rs
[Recipe request]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/src/request/mod.rs
[Recipe parser]: https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/src/stream/output_parser.rs
[V2]: https://github.com/deepseek-ai/DeepSeek-V2/blob/ec98ee3cbffc32104cd55dba8af884b3d772602a/README.md
[V3]: https://github.com/deepseek-ai/DeepSeek-V3/blob/9b4e9788e4a3a731f7567338ed15d3ec549ce03b/README.md
[V3 model]: https://github.com/deepseek-ai/DeepSeek-V3/blob/9b4e9788e4a3a731f7567338ed15d3ec549ce03b/inference/model.py
[V3 kernel]: https://github.com/deepseek-ai/DeepSeek-V3/blob/9b4e9788e4a3a731f7567338ed15d3ec549ce03b/inference/kernel.py
[R1]: https://github.com/deepseek-ai/DeepSeek-R1/blob/0cf78561f1d51c84a21b2190626b21116d5c68bb/README.md
[V32]: https://github.com/deepseek-ai/DeepSeek-V3.2-Exp/blob/87e509a2e5a100d221c97df52c6e8be7835f0057/README.md
[V32 model]: https://github.com/deepseek-ai/DeepSeek-V3.2-Exp/blob/87e509a2e5a100d221c97df52c6e8be7835f0057/inference/model.py
[V32 config]: https://github.com/deepseek-ai/DeepSeek-V3.2-Exp/blob/87e509a2e5a100d221c97df52c6e8be7835f0057/inference/config_671B_v3.2.json
[FlashMLA]: https://github.com/deepseek-ai/FlashMLA/blob/0f11e178da1f67becb087a324b400854931ec3e3/README.md
[DeepGEMM]: https://github.com/deepseek-ai/DeepGEMM/blob/5081f8513af57e61696de4f4f04b0e707ffb503e/README.md
[DeepSelect]: https://github.com/deepseek-ai/DeepSelect/blob/bfa4507d935f17ebfc3d0f00ff7d3c9a4d0e5c18/README.md
[DeepEP]: https://github.com/deepseek-ai/DeepEP/blob/b5a33eefda39485f438dc764695b71ba7b9249b6/README.md
[EPLB]: https://github.com/deepseek-ai/EPLB/blob/d52c72d5b2f2fb4c41afbf8eb21366820239913d/README.md
[LPLB]: https://github.com/deepseek-ai/LPLB/blob/0490f79452f7ef277e814449600b1b1dd4c663b3/README.md
[DeepSpec]: https://github.com/deepseek-ai/DeepSpec/blob/005e03b81cec38b7da6399833d609ee89a2587f2/README.md
[TileKernels]: https://github.com/deepseek-ai/TileKernels/blob/c1cf914a82e6901c679445f872051ce44ac2cc63/README.md
[DeepJIT]: https://github.com/deepseek-ai/DeepJIT/blob/18fe520b0473fc45d32d2f0e86923da707de19ab/README.md
[Engram]: https://github.com/deepseek-ai/Engram/blob/fb7f84a21f91223715394a33a1dc24bbfb7f788e/README.md
[ThreeFS]: https://github.com/deepseek-ai/3FS/blob/22fca04564c7cc230fd8b9523b8b92864e1dad47/README.md
[DualPipe]: https://github.com/deepseek-ai/DualPipe/blob/030ce4325f4ebeb437da4ebc6d00a70469dd58ae/README.md
[InfraIndex]: https://github.com/deepseek-ai/open-infra-index/blob/56d86855fcf6e08fdfd45ce6280bd24322c93351/README.md
[Select kernel]: https://github.com/deepseek-ai/DeepSelect/blob/bfa4507d935f17ebfc3d0f00ff7d3c9a4d0e5c18/csrc/cuda_kernels/v3/topk_select.cuh
[Select interface]: https://github.com/deepseek-ai/DeepSelect/blob/bfa4507d935f17ebfc3d0f00ff7d3c9a4d0e5c18/deep_select/interface.py
[JIT runtime]: https://github.com/deepseek-ai/DeepJIT/blob/18fe520b0473fc45d32d2f0e86923da707de19ab/include/deep_jit/runtime/runtime.hpp
[GEMM binding]: https://github.com/deepseek-ai/DeepGEMM/blob/5081f8513af57e61696de4f4f04b0e707ffb503e/csrc/jit_kernels/impls/sm90_fp8_gemm_1d1d.hpp
[EPLB source]: https://github.com/deepseek-ai/EPLB/blob/d52c72d5b2f2fb4c41afbf8eb21366820239913d/eplb.py
[Spec calibration]: https://github.com/deepseek-ai/DeepSpec/blob/005e03b81cec38b7da6399833d609ee89a2587f2/deepspec/eval/dspark/confidence_head.py
[Storage design]: https://github.com/deepseek-ai/3FS/blob/22fca04564c7cc230fd8b9523b8b92864e1dad47/docs/design_notes.md
[Legacy FlashMLA]: https://github.com/deepseek-ai/FlashMLA/blob/ba89a3466e9470ad08ab39738d4e7bb66989e1e7/README.md
[V41 gate]: https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/models/v4_modules/moe.py
[V41 builder]: https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/models/v4_modules/builder.py
[V41 attention]: https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/models/v4_modules/mla_attn.py
[V41 mhc]: https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/models/v4_modules/mhc_modeling.py
[H1]: https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/blob/ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562/config.json
[H1T]: https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/blob/ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562/tokenizer_config.json
[H2]: https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Llama-8B/blob/6a6f4aa4197940add57724a7707d069478df56b1/config.json
[H2T]: https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Llama-8B/blob/6a6f4aa4197940add57724a7707d069478df56b1/tokenizer_config.json
[H3]: https://huggingface.co/deepseek-ai/DeepSeek-V2-Lite-Chat/blob/85864749cd611b4353ce1decdb286193298f64c7/config.json
[H3T]: https://huggingface.co/deepseek-ai/DeepSeek-V2-Lite-Chat/blob/85864749cd611b4353ce1decdb286193298f64c7/tokenizer_config.json
[H4]: https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/config.json
[V2 implementation]: https://huggingface.co/deepseek-ai/DeepSeek-V2-Lite-Chat/blob/85864749cd611b4353ce1decdb286193298f64c7/modeling_deepseek.py
[NVIDIA GPU list]: https://developer.nvidia.com/cuda/gpus
[P1]: ../../crates/psionic/crates/psionic-models/src/lib.rs
[P2]: ../../crates/psionic/crates/psionic-serve/src/gguf.rs
[P3]: ../../crates/psionic/crates/psionic-core/src/lib.rs
[P4]: ../../docs/compute/pylon.md
[P5]: ../../crates/psionic/crates/psionic-serve/src/conformance.rs
[P6]: ../../crates/psionic/crates/psionic-models/src/released_chat_templates.rs
