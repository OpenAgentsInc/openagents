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
