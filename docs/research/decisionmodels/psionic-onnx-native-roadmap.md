# Psionic roadmap: ONNX Runtime and native execution

Owner decision, 2026-10-15: add both ONNX Runtime (ORT) and native Psionic
execution so the same models can be benchmarked on both. Start with
[`s1-llm-auto-router`](s1-llm-auto-router.md) on CPU. Keep both engines
available after the comparison, and reuse the ORT integration for other
models. The shared contract and fixtures (#11700), optional ORT CPU
execution (#11701), native ModernBERT CPU execution (#11702), CPU
benchmarks (#11703), and shared in-process/HTTP serving (#11704) are
implemented. Embeddings and digit vision (#11705) also have both engines;
explicit CUDA/Core ML and native CUDA/Metal paths are implemented in
#11706. Device measurements and package qualification are recorded below.

## Native performance work

The seven initial milestones establish executable models and comparisons.
They do not establish native performance parity, complete native GPU execution,
or qualified Apple/mobile deployments. The owner requested native parity or
better on 2026-10-15. Keep both engines selectable and compare the same precision,
inputs, hardware, and thread budget after every optimization.

| Priority | Issue | Status | Work | Acceptance |
| --- | --- | --- | --- | --- |
| 1 | [#11722](https://github.com/OpenAgentsInc/openagents/issues/11722) | Complete | Keep the ModernBERT encoder on CUDA, including attention and elementwise operations | All reference cases pass; no intermediate host transfers; paired short and long input measurements, with remaining ORT gaps explicit |
| 2 | [#11723](https://github.com/OpenAgentsInc/openagents/issues/11723) | In progress | Improve native CPU router and digit inference | Correctness unchanged; matched ORT/native measurements; separate relative parity from the 512-token absolute target |
| 3 | [#11724](https://github.com/OpenAgentsInc/openagents/issues/11724) | Complete | Keep native digit inference on GPU | Persistent buffers and GPU convolution/activation/pooling; paired CUDA evidence |
| 4 | [#11725](https://github.com/OpenAgentsInc/openagents/issues/11725) | Implemented; Apple unqualified | Complete device-resident native Metal encoder and digit execution | Build and reference parity, then measured Metal/Core ML comparisons on Apple hardware |
| 5 | [#11726](https://github.com/OpenAgentsInc/openagents/issues/11726) | In progress | Expose accelerator serving and publish the parity status | Explicit server device selection; direct/HTTP correctness; current benchmark and platform gaps visible here |

Apple CPU/Metal/Core ML measurements and mobile build, memory, startup, and
thermal qualification remain in `NEEDS_OWNER.md`. They remain unqualified until
measured. Native CPU/CUDA optimization and accelerator serving are engineering
work tracked above. Router quality evaluation and routing promotion follow the
separate router experiment plan; faster inference does not establish model quality.

## Scope and current evidence

ONNX describes a computation graph, weights, input and output tensors,
and required operator versions. ORT executes that graph. Native Psionic
execution implements the architecture using Psionic's own kernels.
Supporting ONNX files does not require building a general ONNX interpreter
inside Psionic: ORT supplies that capability initially.

The router research records 3.3 ms p50 and 7.9 ms p99 in standalone ORT on
an M5 Max CPU, over 32 mostly short requests. Those numbers exclude a
Psionic integration and do not measure native execution, 512-token
performance, peak memory, or package size. They establish a useful
baseline, not a performance promise.

The implementation belongs in the separate Rust workspace at
[`crates/psionic`](../../../crates/psionic/README.md). Keep application
integration in Rust. The optional ORT adapter may call the native library
through its C API using Rust bindings; production execution needs no
Python process. Converter and reference-generation scripts are tooling.

## Shared execution boundary

Keep the ORT adapter reusable: model loading and execution accept named,
typed tensors and return named, typed tensors. Put the router's text
preparation, fixed question schema, temperatures, and answer formatting in
a shared model adapter. Native execution implements the same tensor
contract. Do not duplicate tokenization or probability processing per
engine.

Expose an explicit engine choice, `ort` or `native`, independently of the
device or ORT execution provider. The router server and benchmark expose these choices. An unavailable explicit choice fails; it never silently
selects the other engine. Identify the actual engine, provider, device,
precision, and any CPU graph fallback in benchmark results. Native is an
execution engine here, not another ORT provider.

Pin the source model, tokenizer, configuration, label order, and
calibration files. Native conversion records the source ONNX hash,
converter revision, tensor mapping, and converted artifact hashes. Keep
model identity separate from engine identity. Preserve the router's uint8
embeddings and dequantize after gathering rows in both paths; comparing
different quantizations is a separate experiment.

## Milestones

| Issue | Deliverable | Completion evidence |
| --- | --- | --- |
| [#11700](https://github.com/OpenAgentsInc/openagents/issues/11700) | Shared model contract and reference fixtures | Pin the router artifacts and capture reference token IDs, raw logits, and processed answers. Cover short requests, multilingual text, code, paths, paired context, truncation, and the 512-token boundary. Both engines use the same preparation and answer code |
| [#11701](https://github.com/OpenAgentsInc/openagents/issues/11701) | Optional ORT CPU execution | A narrow Rust adapter loads the original ONNX and executes it in process. Pin compatible Rust bindings and ORT versions, provision the library explicitly, bound threads and concurrency, and report errors. Builds without the feature require no ORT library. Integrated outputs pass the reference fixtures |
| [#11702](https://github.com/OpenAgentsInc/openagents/issues/11702) | Native ModernBERT CPU execution | A reproducible converter writes named safetensors and architecture configuration. Implement the pruned encoder, quantized embedding gather, attention, pooling, and heads. Native outputs pass the same fixtures and the direct ORT comparison |
| [#11703](https://github.com/OpenAgentsInc/openagents/issues/11703) | Reproducible CPU benchmark | One Rust harness selects either engine and emits comparable machine-readable results under the protocol below. Run both on the same Apple Silicon and Linux x86_64 machines; record unavailable machines as remaining coverage. Retain both engines regardless of which wins |
| [#11704](https://github.com/OpenAgentsInc/openagents/issues/11704) | Shared serving and caller integration | Support in-process calls and the existing `/v1/systemone` server path through the same model adapter. Validate the model's fixed question schema before execution, warm the selected engine before readiness, and verify transport adds no answer differences. Routing promotion still requires the separate application evaluation |
| [#11705](https://github.com/OpenAgentsInc/openagents/issues/11705) | Other model families | Add a pinned embedding or reranking model, then a small vision or audio model with a concrete product use. ORT runs the supported graph first; add native support and paired fixtures per family. Report ORT-only coverage explicitly until a native counterpart exists |
| [#11706](https://github.com/OpenAgentsInc/openagents/issues/11706) | Accelerator and deployment comparison | Compare native CUDA with ORT CUDA, then native Metal with ORT Core ML on supported models. Record graph partitioning and transfers. Assess mobile packaging after desktop results, including builds with and without ORT and reduced-operator packages |

Milestones 1–4 do not depend on training our own router or proving that
the released router should route Coder traffic. Milestone 5's serving
work can start after the ORT contract is working; each engine must pass
correctness before it serves. Broader model work reuses the ORT adapter
without waiting for every native port.

### Native encoder work

The first native family is ModernBERT/mmBERT, not a generic ONNX importer.
Implement and verify:

- Bidirectional attention without a KV cache, with padding masks and the
  kept layers' actual local/global schedule.
- Symmetric local windows and per-layer rotary frequencies from the
  ONNX graph. The pinned router has a 64-position radius and uses the
  same frequencies (base 160,000) on local and global layers.
- Bias-free LayerNorm, including embedding and final normalization and
  the first layer's attention-normalization exception.
- GeGLU with exact erf GELU, first-token pooling, and four linear heads.
- The tokenizer's ranked BPE merges, byte fallback, Metaspace behavior,
  pair template, context preparation, and second-sequence-only truncation.
- CPU threading using the existing qwen35 GEMM where applicable.

The [router gap analysis](s1-llm-auto-router.md#the-gaps) carries the model
dimensions and additional implementation details. Verify token IDs against
the reference tokenizer on at least 1,000 multilingual and code strings.
Converting weights alone does not implement these operations.

## Correctness and benchmark protocol

Use the same artifact revision and tokenizer for each pair. Native
weights may change layout, but conversion must preserve their values.
First replay identical token tensors to isolate execution; then measure
the shared text-to-answer path. Measure HTTP separately from in-process
execution. Disable answer caches for the comparison.

Before timing, compare raw logits and calibrated distributions against
the pinned reference and directly between engines. The router's existing
acceptance target is identical categorical argmaxes and flag decisions,
and [centered-log distance](calibration.md) below 0.01 per answer. For
independent flags, compare each `[p(true), p(false)]` pair. Also report
maximum logit and probability differences; speed does not excuse a
correctness failure. Set task-appropriate criteria before benchmarking
other model families rather than applying router criteria to embeddings
or images.

| Dimension | Required measurements |
| --- | --- |
| Inputs | Fixed short, medium, and 512-token router sets, plus multilingual/code cases and the 32-row `route-v1` set. Report each bucket separately |
| CPU resources | Match 1-thread and 4-thread runs, batch size, concurrency, precision, and affinity where supported. Prevent nested thread pools from exceeding the budget |
| Cold start | Fresh-process load, graph optimization, first inference, and time to readiness; record warm artifact caches separately from cold caches |
| Warm execution | Engine-only and text-to-answer p50/p95/p99, requests/s, and tokens/s. Include batch 1 and a fixed concurrent workload. Time each engine separately under the same total thread budget |
| Sampling | Use repeated, interleaved A/B runs on an otherwise idle machine. Record warm-up, sample count, repetitions, and spread; collect enough samples to make tail percentiles meaningful |
| Memory and size | Peak process RSS, steady memory after warm-up, model bytes, and incremental packaged runtime size compared with a native-only build. Include native libraries and accelerator dependencies |
| Reproduction | Commit, artifact hashes, converter revision, Rust and ORT versions, build features, optimization settings, provider assignment, hardware, OS, thread settings, and exact command |

The router's performance target remains p99 below 20 ms at 512 tokens on
four CPU cores. The standalone short-request result does not satisfy it.
Publish misses and keep both engines selectable; there is no requirement
that native beat ORT or that ORT replace native. Keep a pinned conservative
reference configuration and report any optimized or reduced-precision
configuration separately.

For accelerator comparisons, match the physical device where possible.
Core ML may use CPU, GPU, or Neural Engine; record its compute-unit
configuration and observed placement where available. An ORT Neural Engine
run and a native Metal GPU run are different hardware configurations, not
an isolated comparison of execution engines.

Store replay fixtures and a concise report in version control, following
the existing Psionic fixture convention. Keep downloaded weights and raw
scratch output outside the repository, in the directory reported by
`openagents scratch`. Use owned test inputs; production prompt collection
and model training remain separate opt-in work.

## Packaging and broader support

Start with CPU support and measure the dependency cost. ORT execution
providers cover different graph subsets; selecting an accelerator does
not establish that the entire model ran on it. Treat unsupported
operators, versions, shapes, and data types as explicit compatibility
results. Check each model's license before adding it.

Evaluate the maintained community `ort` Rust bindings at implementation
time and pin a compatible native runtime. Keep dependency downloading in
an explicit setup/build step, with version and checksum verification;
inference loads the installed runtime and model offline. A deployment
must be able to include only the native path or include both.

After measuring full packages, consider ORT builds restricted to the
operators required by the supported model set. Record that coverage:
smaller packages can reject other models, and ORT's basic minimal build
requires converted ORT-format models instead of loading ONNX directly.
Do not call package size excessive or negligible before measuring it.

Supporting arbitrary ONNX graphs with Psionic's own kernels is a later,
separate importer project. The initial native work supports named model
families. ORT supplies broader graph execution while native coverage
grows; both remain available for regression comparisons.

Routing adoption follows the [router experiment plan](s1-llm-auto-router.md#experiment-plan):
evaluate representative tasks, cheap-answer precision, calibration, and
outcomes before shadowing and promotion. Passing execution parity does
not prove that the model makes suitable routing judgments.

## CPU results (2026-10-15, #11703)

The [retained study](../../../crates/psionic/crates/psionic-inference/fixtures/reports/cpu-2026-10-15/study.json)
contains 28,000 timed inferences on an i7-14700K, with warm caches and
affinity to four distinct P cores. Each configuration has two alternating
A/B runs of 1,000 samples. This is a shared machine; other work was not
stopped. Values below are medians of the two run medians; p99 columns show
the range across those runs.

| Case | ORT p50 | Native p50 | ORT p99 | Native p99 |
| --- | --- | --- | --- | --- |
| `route-v1`, 1 thread | 9.1 ms | 16.1 ms | 15.2 ms | 24.9–25.2 ms |
| `route-v1`, 4 threads | 4.0 ms | 8.6 ms | 5.8–6.2 ms | 11.8 ms |
| Text to answer, 4 threads | 4.0 ms | 8.6 ms | 6.0–6.1 ms | 11.9–14.8 ms |
| 512 tokens, 4 threads | 81.2 ms | 112.0 ms | 83.8–84.8 ms | 132.2–156.4 ms |

Both engines miss the 20 ms target at 512 tokens. Four concurrent
single-thread instances deliver about 347 requests/s with ORT and 204
with native. Every run passes the 100 reference decisions first.

Eight additional focused probes keep model instances loaded while taking
memory snapshots. At concurrency 1 with four threads on `route-v1`, ORT
uses 391 MB resident and peaks at 450 MB; native uses 357 MB resident and
peaks at 518 MB. The baseline runs' resident-memory field was taken after
teardown and is not serving-memory evidence; the study records the
correction and separate probe files.

Unstripped release executables are 30.7 MB for native-only and 31.8 MB
with both engines. The CPU ORT library adds 24.3 MB: a combined increment
of 25.4 MB, excluding model assets and platform libraries. Apple Silicon
CPU measurements remain in `NEEDS_OWNER.md`; accelerator measurements
follow in #11706.

The shared HTTP server (#11704) passes direct Rust/HTTP answer parity for
both engines. Four additional runs retain 4,000 real loopback requests:
ORT p50 is 4.3–4.5 ms and p99 is 6.7–7.0 ms; native p50 is 15.3–15.4 ms
and p99 is 19.2–19.3 ms. These HTTP runs use a development server at
optimization level 1, with four model threads and warm caches. They are
not the release CPU baseline, so their difference is not a measurement of
transport overhead. The raw records include the server checksum, build
profile, command, and engine identity.

## Other model results (2026-10-15, #11705)

`psionic-inference::models` now includes `PotionModel` for local snippet
and note search, and `MnistModel` for cropped handwritten-digit input.
Both share preprocessing and output handling across native and ORT.
The manifests pin `minishlab/potion-base-2M` and ONNX Model Zoo's
`mnist-12`, including source revisions, hashes, and license documents.
Weights stay outside Git. These are in-process APIs; the existing HTTP
embedding endpoint is unchanged.

The [retained study](../../../crates/psionic/crates/psionic-inference/fixtures/reports/small-models-2026-10-15/study.json)
contains 16 alternating release runs, each with 1,000 timed tensor calls,
on the same Linux CPU and affinity as the router. Four-thread p50 is
0.0077 ms for ORT versus 0.0038 ms for native embeddings, and 0.0156 ms
versus 0.1107 ms for the digit CNN. Single-thread embeddings are faster
with native at 0.0024 ms. These small calls exclude preprocessing and
network transport; the embedding corpus mixes batches and long inputs.
The result favors keeping both engines.

Each engine passes 100 independent reference cases per model. Native
embeddings match exactly; native MNIST's maximum logit error is 0.0000153
with identical argmax on all cases. The vision corpus includes one
publisher fixture, one blank image, and seeded noise. It establishes
execution parity, not recognition accuracy. Retrieval quality, product
image quality, audio models, and device qualification remain separate
work.

## Accelerator and package results (2026-10-15, #11706)

The [retained study](../../../crates/psionic/crates/psionic-inference/fixtures/reports/accelerators-2026-10-15/study.json)
contains 8,000 CUDA timings, 18,000 CPU/package timings, provider profiles,
five dependency inventories, and reduced-runtime conversion/build receipts.
Every timing run checks all 100 reference cases first. These router runs
cycle the full reference corpus, including long sequences; they use a
different input mix from the earlier `route-v1` table.

| Model and engine | CPU p50 | CUDA p50 | CUDA p99 across two runs |
| --- | --- | --- | --- |
| Router, ORT | 6.78 ms | 1.27 ms | 2.70 ms |
| Router, native | 12.36 ms | 7.21 ms | 138.4–141.0 ms |
| MNIST, ORT | 0.0159 ms | 0.0706 ms | 0.0919–0.0969 ms |
| MNIST, native | 0.1314 ms | 0.1802 ms | 0.3044–0.3228 ms |

The GPU is an RTX 4080 with driver 595.71.05; the host uses the same four
P-core affinity. Native CUDA and Metal send the router's dense GEMMs and
the digit model's three GEMMs through existing Psionic backends. They
keep attention, norms, activations, pooling, and preparation on CPU and
report mixed execution and logical transfers. ORT CUDA's router profile
also contains CPU shape/index partitions; the digit graph's compute
nodes all use CUDA. Host I/O copies can occur outside ORT's profiled graph
nodes. Native Potion currently supports CPU only. CUDA helps this router;
CPU wins for the tiny digit CNN on this host.

CUDA qualification exposed an intermittent NVIDIA compiler cleanup abort
with lazy module loading, including on supported cuDNN versions. The
[compatibility record](../../../crates/psionic/crates/psionic-inference/fixtures/reports/accelerators-2026-10-15/cuda-compatibility.json)
records the failure and excludes those exploratory timings. The adapter
requires eager module and data loading before process startup. With
cuDNN 9.20.0.48, TF32 disabled, and the default convolution algorithm,
20 fresh process checks and all eight final benchmark processes exit
successfully. The default convolution algorithm uses cuDNN's fallback
heuristic on CUDA; CPU graph partitions are reported separately. Eager
loading can increase startup cost and GPU memory. The runner pins and
checks cuDNN and the driver; this qualifies the recorded configuration,
not every CUDA dependency combination.

| Bundle inventory | Executable and runtime libraries | Model assets and notices |
| --- | --- | --- |
| Native CPU | 4.58 MB | 225.4 MB |
| Both engines, full CPU ORT | 29.11 MB | 416.4 MB |
| Both engines, reduced CPU ORT | 10.88 MB | 416.6 MB |
| Native CUDA | 564.87 MB | 225.4 MB |
| Both engines, CUDA ORT | 1,496.78 MB | 416.4 MB |

These are uncompressed benchmark executables built with release
optimization and debug information disabled, plus their shared-library
and model inventories. They are not mobile app sizes. Ordinary OS/C++
libraries and the observed NVIDIA driver/compiler libraries are listed
separately: about 250.5 MB of system prerequisites for the CUDA cases.
The CUDA inventory includes CUDA 13.2, cuBLAS, cuRAND, and the full pinned
cuDNN library set where required. Select model families per deployment;
an app does not need every model in this inventory.

Native deployment loaders verify the pinned converted weights without
requiring a second copy of source ONNX or source test archives. Dual
bundles keep both weight representations. The reduced ORT library itself
is 2.42 MB, but its Protobuf/Abseil dependencies also count in the 10.88 MB
bundle. Its exported CPU ORT graphs pass every reference case. It is
slower here: router p50 is 7.23 ms, embeddings 0.0120 ms, and MNIST
0.0262 ms. Keep full ORT for original ONNX and broader graph coverage;
the basic minimal build supports only the exported graph/operator set.

Core ML compute-unit selection and native Metal execution are implemented,
but no Apple device results are claimed. The native CPU library passes
an ARM64 macOS compile check; checking accelerator dependencies requires
Apple's SDK. Apple Silicon and mobile build/device qualification remain
in `NEEDS_OWNER.md`. Fixed x86 CPU ORT exports do not qualify mobile,
Core ML, CUDA, or arbitrary models. No routing policy or product model
promotion follows from these execution comparisons.

## Native CUDA parity results (2026-10-15, #11722)

The [new paired study](../../../crates/psionic/crates/psionic-inference/fixtures/reports/native-parity-cuda-2026-10-15/study.json)
replaces the router's mixed CUDA implementation with device-resident float32
execution. Embeddings, norms, RoPE, attention, GeGLU, residuals, and heads stay
on GPU. Inputs and final logits are the only host transfers. Long attention uses
batched GEMMs; short attention uses a fused kernel. A bounded 64-length CUDA
graph cache replays repeated shapes. The final layer computes only the output
row consumed by pooling. Model precision and pinned weights are unchanged.

| CUDA router workload | ORT p50 | Native p50 | ORT p99 | Native p99 |
| --- | --- | --- | --- | --- |
| Full 100-case reference mix | 1.27 ms | 0.50 ms | 2.70 ms | 1.96–1.97 ms |
| Exactly 512 tokens | 2.72 ms | 1.97 ms | 3.09–3.16 ms | 2.20–2.25 ms |

Each bucket has two alternating 1,000-sample runs per engine on the same RTX
4080 and four-core affinity. Every process first checks all 100 reference cases;
native maximum raw-logit error is 0.00000382 and all reference decisions match.
Native is faster at both median and p99 in this comparison. These warm CUDA
measurements do not satisfy or replace the absolute four-core CPU target.
The source hashes, binary hashes, dependency configuration, exact commands,
individual timings, and host RSS are retained. Peak VRAM remains unmeasured.

The earlier accelerator table describes the previous mixed implementation and
remains historical evidence. Current CPU, digit, Metal, and serving work follows
below; Apple and mobile qualification remain unmeasured.

## Native digit CUDA results (2026-10-15, #11724)

The [paired digit study](../../../crates/psionic/crates/psionic-inference/fixtures/reports/native-parity-digit-cuda-2026-10-15/study.json)
contains 4,000 timings across two alternating rounds. Native runs two fused
convolution/ReLU/floor-pooling kernels and the dense head in one CUDA graph.
Weights and activations persist on GPU; only 784 image values and ten logits
cross the host boundary. No intermediate tensor computation runs on CPU.

Native p50 is 0.055–0.056 ms versus ORT 0.072–0.075 ms. Native p99 is
0.066–0.078 ms versus ORT 0.111–0.115 ms. Every run passes all 100 reference
images with identical argmax; native maximum raw-logit error is 0.0000382.
Native host RSS is 1.51 GB versus ORT 4.12–4.13 GB with the model loaded.
These host figures include runtime libraries and are not VRAM measurements.
Compressed ORT profiles and their hashes are retained with the raw runs.
CPU remains the faster device for this tiny CNN.

## Native Metal and serving implementation (#11725, #11726)

Native Metal now implements the complete encoder and digit graph with
persistent shared buffers, float32 GPU kernels, and one command buffer per
call. Fast math is disabled. The encoder covers quantized embeddings, norms,
rotary positions, key masks, local/global attention, GeGLU, residuals, and
output heads. The digit path fuses convolution, ReLU, and floor pooling.
Non-Apple builds return an explicit unavailable-device error.

The model dispatch compiles on Linux against unavailable-device stubs. The
Metal backend Rust API also passes a separate type check against `metal` 0.33.0
without framework linking. These checks do not compile Metal shaders or run an
Apple GPU. Fixture replay, matrix edge sizes, in-place normalization, attention
masks/windows, and HTTP parity checks are ready for an Apple machine. Follow
`NEEDS_OWNER.md` before claiming Metal/Core ML correctness or performance.

The existing server accepts `--router-device cpu|cuda|metal|coreml` with the
appropriate engine and build feature. ORT accelerator CPU partitions require
`--router-allow-cpu-fallback`; native serving reports the actual selected
provider without CPU fallback. CUDA HTTP answers match direct inference.
Paired HTTP latency and the final CPU scheduling comparison are being retained
separately; the implementation does not promote the router into product routing.

## References

- [ONNX model concepts](https://onnx.ai/onnx/intro/concepts.html)
- [ORT C API and session resources](https://onnxruntime.ai/docs/get-started/with-c.html)
- [Rust `ort` bindings](https://github.com/pykeio/ort)
- [Execution providers and graph assignment](https://onnxruntime.ai/docs/execution-providers/)
- [ORT custom and minimal builds](https://onnxruntime.ai/docs/build/custom.html)
