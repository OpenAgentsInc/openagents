# Runtime Integration

How the ml crate integrates with the native runtime via `compute::backends::InferenceBackend`.

## Overview

`MlProvider` implements the compute backend trait used by Pylon's provider runtime. It loads Candle models
and exposes completion + streaming APIs compatible with NIP-90.

```
┌─────────────────────────────────────────────────────────────┐
│                        Pylon                                │
│                                                             │
│  ┌─────────────┐    ┌──────────────┐    ┌───────────────┐  │
│  │   Agent     │───▶│ BackendRegistry │─▶│ InferenceBackend││
│  └─────────────┘    └──────────────┘    └───────────────┘  │
│                                                │            │
└────────────────────────────────────────────────┼────────────┘
                                                 │
                                    ┌────────────┼────────────┐
                                    │            ▼            │
                                    │      MlProvider         │
                                    │    (Candle backend)     │
                                    │                         │
                                    │        crates/ml        │
                                    └─────────────────────────┘
```

## Provider Registration

`PylonProvider::new` attempts to register the backend when `ml-native` is enabled:

```rust
#[cfg(feature = "ml-native")]
if let Ok(config) = ml::MlProviderConfig::from_env() {
    if let Ok(provider) = ml::MlProvider::new(config).await {
        registry.register_with_id("ml-candle", Arc::new(RwLock::new(provider)));
    }
}
```

## Model Configuration

Provide models via environment variables:

```bash
export ML_MODEL_PATH="/path/to/model.gguf"
export ML_MODEL_KIND="llama2c-quantized"
export ML_MODEL_ID="llama2c"
export ML_TOKENIZER_PATH="/path/to/tokenizer.json"
```

Or pass a JSON array:

```bash
export ML_MODELS_JSON='[{"id":"llama2c","kind":"llama2c-quantized","weights":["/path/to/model.gguf"],"tokenizer":"/path/to/tokenizer.json"}]'
```

## Streaming

`complete_stream` emits `StreamChunk` items as tokens are generated. The final chunk includes a finish reason.
