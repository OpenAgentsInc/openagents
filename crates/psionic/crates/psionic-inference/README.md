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

The [roadmap](../../../../../docs/research/decisionmodels/psionic-onnx-native-roadmap.md)
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
