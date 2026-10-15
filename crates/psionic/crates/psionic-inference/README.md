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
pool and portable SIMD GEMMs from `matrixmultiply`. The existing qwen35
GEMM specializes in quantized AVX-512 projections; this encoder uses
float32 matrices and also runs on AVX2 and ARM CPUs.

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
  --bin psionic-openai-server --features router-ort
psionic-openai-server --router-model-dir /path/to/models/s1-router \
  --router-engine native --router-threads 4 --host 127.0.0.1 --port 8080
```

For ORT, select `--router-engine ort` and provide `--ort-library FILE`
and `--ort-sha256 HASH`. A build without `router-ort` refuses that engine.
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
