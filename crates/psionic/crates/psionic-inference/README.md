# Shared model inference

`psionic-inference` owns the named tensor contract and the router's shared
tokenizer and calibrated answer processing. Engine implementations use the
same inputs and return raw head logits. It belongs to the separate
Psionic Cargo workspace.

The pinned model manifest records the MIT-licensed router's source
revision and file checksums. Weights stay outside Git. Reference fixtures
contain 100 independent ORT output cases and 1,000 Python-tokenizer cases
over synthetic inputs, generated with ORT 1.28.0 and tokenizers 0.23.2.

From the repository root:

```sh
python3 scripts/dev/psionic-router.py download /path/to/models/s1-router
cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-inference
PSIONIC_ROUTER_MODEL_DIR=/path/to/models/s1-router \
  cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-inference \
  --test router -- --ignored
```

The ignored artifact test requires the downloaded model and checks all
1,000 tokenizer cases. Ordinary tests require no downloads or model
runtime. To regenerate reference fixtures, install the Python dependencies
pinned in `scripts/dev/psionic-router.py` into a scratch virtual
environment, then run its `fixtures` action with the same model directory.

The [roadmap](../../../../docs/research/decisionmodels/psionic-onnx-native-roadmap.md)
tracks execution engines, comparisons, and serving work.

## ONNX Runtime

Enable the `ort` Cargo feature for the reusable `OrtEngine`. Rust bindings
are pinned to `ort` 2.0.0-rc.13 with API 28; the tested native runtime is
ORT 1.28.0. Provision its library explicitly from a verified release or
your own pinned build. Supply the absolute library path, its expected
SHA-256, and CPU thread count through `OrtOptions`. Loading or executing a
model never downloads a runtime. A different library cannot replace the
one already loaded in a process.

```sh
PSIONIC_ROUTER_MODEL_DIR=/path/to/models/s1-router \
PSIONIC_ORT_LIBRARY=/path/to/libonnxruntime.so.1.28.0 \
  cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-inference \
  --features ort --test ort -- --ignored
```

The optional adapter accepts named float32, int64, and uint8 tensors. It
checks input names and dimensions, limits CPU threads, and reports the
actual runtime build and library checksum. Native-only builds do not
depend on ORT. The model's operators determine precision; the router uses
uint8 embeddings and float32 encoder operations.

## Native ModernBERT

Run the tooling's `convert` action with the pinned model directory to
write `native.safetensors` and `native.json`. Conversion preserves ONNX's
matrix layout and embedding quantization, records each source tensor,
and derives attention masks and rotary frequencies from the graph. The
checked-in conversion receipt pins the converted weight checksum and
configuration. No weights are committed.

`ModernBertEngine` implements the encoder in Rust with a bounded Rayon
pool, packed AVX2/FMA projections on x86, and portable `gemm` fallbacks.
Weights retain float32 precision. Cached rotary tables, vector norms and
activations, bounded scratch buffers, and first-token final-layer pruning
reduce repeated work. Workers preserve the caller's allowed CPU mask so the
OS can schedule around other work.

```sh
python3 scripts/dev/psionic-router.py convert /path/to/models/s1-router
PSIONIC_ROUTER_MODEL_DIR=/path/to/models/s1-router \
  cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-inference \
  --test native -- --ignored --nocapture
```

The converter additionally needs `onnx==1.20.1`, `numpy==2.5.4`, and
`safetensors==0.7.0`. The explicit native test compares all 100 reference
cases, including long sequences, without loading ONNX Runtime.

## CPU benchmark

`psionic-model-bench` runs one engine per process, checks all reference
cases before timing, and emits one JSON record. Choose `--engine native`
or `--engine ort`, `--mode engine` or `--mode text`, and a `--bucket` of
`short`, `medium`, `long`, or `route-v1`. The long bucket is checked to be
512 tokens. `--threads` is the total CPU budget; `--concurrency` divides
it across independent engine instances. A native-only build refuses an
explicit ORT choice.

Build release binaries with and without `--features ort`, keeping a copy
of the native-only binary. Run `scripts/dev/psionic-model-bench.py --help`
for the repeated A/B matrix. It records raw runs, package sizes, binary
checksums, and remaining platform coverage. Each run uses at least 1,000
samples by default; the driver alternates engine order and never runs the
two engines at the same time. Filesystem caches are warm. Pin CPU affinity
with `--cpus` on Linux; report other system load separately.

## Shared service and HTTP

`service::RouterService` exposes the same model in process. Load the
selected engine, call `warm_up()` before latency-sensitive work, and pass
a System One request to `decide()`. `service::questions()` returns the
seven supported question definitions; callers can request a subset.
Changing a question's instructions, type, or options returns
`not_admitted` before inference. This fixed model cannot answer arbitrary
decision questions.

The state is either request text or an object with `request` and optional
`context`. The response includes normal Choice, Score, and Noul answers,
input usage, and the actual execution engine and artifact identities.

The existing server has a router-only mode:

```sh
cargo build --manifest-path crates/psionic/Cargo.toml -p psionic-serve \
  --release --bin psionic-openai-server --features router-ort,router-cuda
psionic-openai-server --router-model-dir /path/to/models/s1-router \
  --router-engine native --router-device cuda --router-threads 4 \
  --host 127.0.0.1 --port 8080
```

For ORT, select `--router-engine ort` and provide `--ort-library FILE`
and `--ort-sha256 HASH`. Select `--router-device cpu|cuda|coreml` for ORT
or `cpu|cuda|metal` for native; CPU is the default. Enable `router-metal`
on a Mac. ORT accelerator CPU partitions require the explicit
`--router-allow-cpu-fallback` flag. A build without `router-ort` refuses ORT.
Run GGUF models in a separate server. Exact browser origins can use the
existing `--allow-origin` option.

`GET /v1/models` includes the model's question definitions and execution
identity. `POST /v1/systemone` uses the in-process service. The model warms
up before the listener binds and `/health` becomes available. Requests
are limited to 128 KiB and `--router-max-queue` bounds active and waiting
work; a full queue returns `busy` with `Retry-After: 1`.

The explicit `http` integration test checks real loopback HTTP against
direct Rust calls for both engines. `scripts/dev/psionic-http-bench.py`
benchmarks the existing server on ephemeral ports and stops every server
it starts. Record the server build profile; a development build is not
the release CPU baseline.


## Embeddings and handwritten digits

The same engine interface supports two additional pinned models:

- `models::PotionModel` uses MIT-licensed `minishlab/potion-base-2M` for
  64-dimensional text embeddings and cosine ranking of local snippets or
  notes. `encode()` and `rank()` share one tokenizer across engines. It is
  a static WordPiece embedding model; evaluate retrieval quality on your
  corpus before using it to select information for an answer.
- `models::MnistModel` classifies cropped 28 × 28 grayscale handwritten
  digits. `classify()` accepts 784 bytes, scales them to [0, 1], and
  returns the digit, raw logits, and probabilities. Use white foreground
  on a black background. This supports a constrained handwritten-number
  input; it does not detect text, crop images, or provide general OCR.

Both APIs load with `EngineKind::Native` or `EngineKind::Ort`. Native
Potion implements mean embedding bags and L2 normalization; native MNIST
implements two padded convolution/ReLU/pooling stages and a linear head.
The manifests pin original sources, license documents, tokenizer, and
native weights. MNIST's model card declares MIT; the source repository
also supplies an Apache-2.0 license. Preserve both source documents when
redistributing it. No model weights are committed.

```sh
python3 scripts/dev/psionic-small-models.py download potion /path/to/models/potion
python3 scripts/dev/psionic-small-models.py download mnist /path/to/models/mnist
python3 scripts/dev/psionic-small-models.py prepare mnist /path/to/models/mnist
PSIONIC_SMALL_MODELS_DIR=/path/to/models \
PSIONIC_ORT_LIBRARY=/path/to/libonnxruntime.so.1.28.0 \
  cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-inference \
  --features ort --test small_models -- --ignored --nocapture
```

Use the same pinned Python dependencies as the router converter. The
`fixtures` action regenerates 100 independent ORT cases per family.
Embedding parity requires maximum element error below 1e-5, cosine above
0.99999 for nonempty vectors, and exact zero vectors for empty bags.
MNIST requires identical argmax and maximum logit error below 1e-3.
The vision fixtures include the publisher's input/output pair, a blank
image, and 98 seeded noise inputs; they check execution, not accuracy.

Build `psionic-small-model-bench` with `--release --features ort` and run
`scripts/dev/psionic-small-model-bench.py --help` for the alternating
CPU comparison. It checks every reference case before timing and keeps
the model loaded during memory measurement. The
[retained comparison](fixtures/reports/small-models-2026-10-15/study.json)
contains 16,000 timed tensor calls. The new APIs run in process; the
existing server's embedding endpoint still uses its existing model.

## Accelerators and deployment packages

`OrtEngine::load_with_execution()` selects CPU, CUDA, or Core ML through
`OrtExecution`. Provider registration errors stop loading. Accelerator
CPU partitions require `allow_cpu_fallback: true`; the default refuses
CPU fallback. CUDA uses device 0 by default, disables TF32, and uses the default
convolution algorithm (the fallback heuristic in cuDNN 9). Before starting
the process, set `CUDA_MODULE_LOADING=EAGER` and
`CUDA_MODULE_DATA_LOADING=EAGER`. The adapter refuses lazy loading because
it triggered an intermittent NVIDIA compiler cleanup abort on this host;
eager loading passed repeated process-lifecycle checks. Eager loading can
increase startup time and GPU memory. Core ML
uses ML Program format and explicitly selects its compute units; the
default is CPU and GPU. Core ML's internal device placement is unknown,
even when ORT reports a Core ML graph partition.

For native execution, enable `native-cuda` or `native-metal` and call
`router::load_native_on()` or `models::load_native_on()` with a
`DeviceChoice`. ModernBERT on CUDA now keeps the full encoder and heads on
the device, with persistent weights and activations. It transfers token IDs and
masks in and 22 logits out. A bounded CUDA graph cache removes repeated launch
setup; the first call at each sequence length captures a graph. The last layer
computes only the output row used by first-token pooling.

MNIST on CUDA uses three captured float32 kernels: convolution/ReLU/pooling
for each stage, then the dense head. Metal implementations keep the complete
encoder and digit graph on GPU with persistent buffers and one command buffer
per call. Metal uses float32 tiled matrices and disables fast math. Apple
shader compilation, fixture parity, and Metal/Core ML performance remain
unqualified until checked on an Apple device. Potion's native path supports
CPU only; unsupported explicit choices return an error.

Device contexts and buffers stay on one dedicated worker thread, which
shuts down when its model drops. The Rayon setting bounds model CPU work;
the GPU control worker is one additional thread. `execution_report()`
records placement, logical buffer transfers, and execution counters.
On unified-memory Metal, those bytes do not imply physical bus copies.
The CUDA comparison driver sets `NVIDIA_TF32_OVERRIDE=0`.

`psionic-small-model-bench` now also accepts `--family router`,
`--device cuda|metal|coreml`, `--allow-cpu-fallback`, and
`--coreml-units all|cpu-and-gpu|cpu-and-neural-engine|cpu-only`. ORT
accelerator runs require a scratch `--profile PREFIX`. The harness checks
all 100 references, ends profiling before timing, and records operator
counts by provider and graph copy events. Host I/O bytes are reported
separately because ORT can copy them outside profiled graph nodes.

Build the binary with `--release --features ort,native-cuda` on Linux,
or `--release --features ort,native-metal` on a Mac. Provision the matching
ORT library and provider dependencies before running. Use
`scripts/dev/psionic-accelerator-bench.py --help` for two alternating
1,000-sample rounds on the same device. The CUDA driver pins cuDNN 9.20.0:
supply `--cudnn-library FILE --cudnn-sha256 HASH`. It verifies the library
and requires NVIDIA driver 580.65.06 or newer before starting any runs.
Other CUDA/cuDNN configurations need their own compatibility check and
can use the binary directly. Apple comparisons use Core ML's
CPU-and-GPU setting; an ANE run is a separate hardware configuration.
Router HTTP serving selects the same device through `--router-device`;
CUDA loopback tests compare exact HTTP and in-process answers and the
published execution identity.

The optional ORT feature still loads a separately provisioned library;
it never downloads one during inference. Native-only builds exclude it.
To prepare a reduced CPU package, use the pinned Python environment:

```sh
python3 scripts/dev/psionic-ort-reduced.py prepare \
  --models /path/to/models --output /path/to/package
python3 scripts/dev/psionic-ort-reduced.py build \
  --source /path/to/onnxruntime --build /path/to/build \
  --output /path/to/package --jobs 8
```

The source checkout must be ORT v1.28.0 at the exact commit pinned by the
tool, with its submodules. Provision CMake and Ninja explicitly. Build
setup can fetch dependencies. Preparation verifies the three source
models, exports fixed CPU ORT-format graphs, and records checksums and
the union of their required operators and types. The build uses those
operators in a basic minimal CPU library. It cannot load original ONNX,
CUDA or Core ML providers, or arbitrary other models. Keep full ORT for
broader graph support. In the benchmark, select an exported graph with
`--ort-model FILE --ort-model-sha256 HASH` and supply the reduced runtime
library and checksum. Every reference case must still pass.


Native deployment verification requires converted weights, model metadata,
licenses, and tokenizers, without requiring the original ONNX weights or
source test archives. The original hashes stay pinned in the conversion
receipt. The explicit `native_packages` test loads all three models from
such bundles. Reference-generation tooling still verifies the full source
download. The high-level ORT loaders use original ONNX files and a full
runtime; a minimal ORT-format package uses `OrtEngine` with the exported
graph and the same shared preparation and output adapters.


The `ort_lifecycle` example runs 1,000 MNIST calls, optionally ends a
profiling pass after 100 calls, and drops the session before exit. Pass
an ORT library and MNIST ONNX path as positional arguments; set
`PSIONIC_PROBE_PROFILE` to a scratch prefix to include profiling. Run it
in fresh processes when qualifying a driver/runtime stack. The benchmark
driver treats a nonzero child exit as a failure even if it printed a
result before shutdown.


The [accelerator and package report](fixtures/reports/accelerators-2026-10-15/study.json)
contains the matched CUDA/CPU measurements, the failed lazy-loading
configuration, and all package hashes. `scripts/dev/psionic-package-bench.py`
compares native CPU, full ORT, and reduced ORT on identical fixtures.
`scripts/dev/psionic-package-size.py` inventories binaries, model assets,
explicit lazy-loaded libraries, and transitive Linux shared dependencies.
It lists ordinary system libraries and NVIDIA driver/compiler libraries
separately. Use the recorded commands to reproduce each inventory.

The [native CUDA parity report](fixtures/reports/native-parity-cuda-2026-10-15/study.json)
records the device-resident router: native p50 is 0.50 ms versus ORT 1.27 ms
on the full reference mix, with p99 1.96–1.97 ms versus 2.70 ms. At exactly
512 tokens, native p50 is 1.97 ms versus 2.72 ms and p99 2.20–2.25 ms versus
3.09–3.16 ms. These are CUDA results, not the four-core CPU target. Use
`--families router` and `--min-tokens 512 --max-tokens 512` on the paired
accelerator runner to reproduce the separate long-input bucket.


The [native digit CUDA study](fixtures/reports/native-parity-digit-cuda-2026-10-15/study.json)
retains 4,000 paired timings and compressed ORT profiles. Native median is
0.055–0.056 ms versus ORT 0.072–0.075 ms, with lower p99 in both rounds.
CPU remains faster for this tiny model.


The [CUDA HTTP comparison](fixtures/reports/native-parity-http-cuda-2026-10-15/study.json)
retains two alternating 1,000-request rounds per engine over the full corpus.
Native median is 0.605–0.611 ms versus ORT 1.462–1.475 ms, with lower p99
and 1,472–1,483 serial requests/s versus 646–650. All 4,400 timed and preflight
answers match direct inference exactly. This uses the production Axum routes
inside the integration-test host, including client JSON encoding and decoding.
To reproduce, set `PSIONIC_ROUTER_MODEL_DIR`, `PSIONIC_HTTP_CUDA_ORT_LIBRARY`,
and a new scratch `PSIONIC_HTTP_BENCH_OUTPUT`, provision the pinned CUDA stack,
then run the ignored `paired_cuda_http` test with features
`ort,http,native-cuda,native-metal` in release mode under an explicit CPU mask.


The [final native CPU study](fixtures/reports/native-parity-cpu-2026-10-15/final-study.json)
retains the completed relative comparison and intermediate misses. At exactly
512 tokens and four threads, native median is 66.1 ms versus ORT 81.1 ms;
p99 is 73.8–78.6 ms versus 96.2–101.7 ms. Both still miss the separate 20 ms
CPU p99 target. Four concurrent one-thread router instances reach 346–367
requests/s versus ORT 315–341 on inputs of at most 34 tokens. Native CPU
digit median is about 15 µs, faster than ORT at both thread budgets.

Use `--chunks 10` with `--samples 1000` on the CPU comparison driver to
alternate engines in smaller process batches within each round. Aggregate
all 1,000 timings per engine/round before calculating percentiles; a
100-sample child is not the full round. The default remains one process
per engine/round. Keep model/thread budgets and CPU affinity identical.
