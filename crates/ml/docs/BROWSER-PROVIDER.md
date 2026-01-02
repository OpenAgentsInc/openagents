# Browser DVM Provider

Serve NIP-90 inference jobs from a browser tab using Candle (WASM CPU + WebGPU context).

## Overview

The `BrowserDvmService` listens for NIP-90 jobs, runs inference with a Candle model, and publishes results.

```
┌─────────────────────────────────────────────────────────────┐
│                      Browser Tab                            │
│                                                             │
│  ┌─────────────────┐     ┌──────────────────┐              │
│  │ BrowserDvmService│────▶│   LoadedModel    │              │
│  └─────────────────┘     └──────────────────┘              │
│          │                        │                         │
│          │                        ▼                         │
│          │                Candle (WASM CPU)                 │
│          │                                                  │
│          ▼                                                  │
│  ┌─────────────────┐                                       │
│  │  Nostr WebSocket│                                       │
│  └─────────────────┘                                       │
│          │                                                  │
└──────────┼──────────────────────────────────────────────────┘
           │
           ▼
    ┌─────────────┐
    │ Nostr Relays │
    └─────────────┘
```

## WASM Entry Point

```rust
#[wasm_bindgen]
pub struct BrowserDvm {
    service: BrowserDvmService,
}

#[wasm_bindgen]
impl BrowserDvm {
    #[wasm_bindgen(constructor)]
    pub async fn new(
        private_key: String,
        model_url: String,
        tokenizer_url: Option<String>,
    ) -> Result<BrowserDvm, JsValue>;

    pub fn start(&self);
    pub fn pubkey(&self) -> String;
}
```

## Example JS Usage

```html
<script type="module">
  import init, { BrowserDvm } from './pkg/ml.js';

  async function main() {
    await init();

    const dvm = await new BrowserDvm(
      'nsec1...or hex secret...',
      'https://huggingface.co/karpathy/tinyllamas/resolve/main/stories42M.gguf',
      'https://huggingface.co/karpathy/tinyllamas/resolve/main/tokenizer.json'
    );

    console.log('pubkey', dvm.pubkey());
    dvm.start();
  }

  main();
</script>
```

## Supported Kinds

- 5050: Text generation request
- 6050: Text generation result
- 7000: Job feedback
