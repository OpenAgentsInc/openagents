# GPT-OSS Browser Runtime: Codex Instructions

## End State (Non-Negotiable)

When you're done, I click **one button** on `/gptoss` and:

1. **GGUF streams in** - Range fetches from URL, chunks load progressively
2. **Visualization goes crazy** - Loading bars, telemetry events, tensor names scrolling, GPU buffer allocations, cache stats, everything updating live in the sci-fi HUD
3. **Tokens start generating** - Harmony-formatted prompt runs through the model, tokens appear one by one with probability bars, attention visualizations, the whole thing

**Do not stop until this works end-to-end.**

---

## What Already Exists (Don't Reinvent)

### Completed Gates
- **Gate A**: GGUF parser works - tensor table, offsets, types
- **Gate B**: Range fetch works - deterministic byte-range reads
- **Gate C**: GPU compute works - Q8_0 dequant + matmul on WebGPU
- **Gate D**: Correctness verified - CPU/GPU match within tolerance

### Scaffolding Started
- `crates/web/client/src/gguf_web.rs` - GGUF parser + range fetch module
- `crates/web/client/src/gptoss_runtime.rs` - Runtime scaffold
- `crates/web/client/src/gptoss_viz.rs` - Telemetry visualization (has types + rendering)
- `crates/web/client/src/state.rs` - GptOssState with start button fields added

### Existing UI
- `/gptoss` route renders `gptoss_viz.rs`
- Sci-fi HUD styling exists (see `gfn.rs` for reference)
- Frame corners, signal meters, dots grids, scanlines all available from wgpui

---

## Implementation Steps

### Step 1: Wire the Start Button

In `gptoss_viz.rs`:
1. Draw a "LOAD MODEL" button (styled like GFN CTA buttons)
2. Track hover state via mouse events
3. On click, trigger the loading sequence
4. Button should show loading state while active

### Step 2: GGUF URL Input

Either:
- Hardcode the GPT-OSS GGUF URL for now: `https://huggingface.co/openai/gpt-oss-20b/resolve/main/gpt-oss-20b-Q8_0.gguf`
- Or use query param `?gguf=<URL>`

### Step 3: Streaming Load with Telemetry

When button clicked:
1. Fetch GGUF header (first 16MB) to get tensor table
2. Emit `LoadStage { stage: "gguf_parse", status: Started }` telemetry
3. Parse tensor table, emit `LoadStage { ..., status: Completed }`
4. For each required tensor:
   - Emit `LoadStage { stage: "weights_fetch", tensor_name, status: Started }`
   - Range-fetch tensor bytes
   - Emit `LoadStage { ..., status: Completed, bytes, duration_ms }`
5. Upload to GPU buffers, emit telemetry for each
6. Build runtime (KV cache, etc.)

### Step 4: Live Visualization

The `GptOssState` should track:
- `load_stages: Vec<GptOssStage>` - All loading events
- `load_progress: f32` - Overall progress 0.0-1.0
- `resident_tensors: Vec<TensorInfo>` - What's on GPU
- `gpu_memory_used: usize` - Current GPU allocation
- `current_stage: String` - What's happening now

Render:
- Progress bar for overall load
- Scrolling list of tensor loads with timing
- GPU memory gauge
- Stage status indicators

### Step 5: Implement Transformer Forward Pass

Start simple, add complexity:

1. **Token embedding lookup**
2. **For each layer:**
   - RMSNorm
   - QKV projection (Q8_0 matmul)
   - RoPE
   - Attention (dense or sliding window depending on layer)
   - MoE router (for MoE layers)
   - Expert MLP (top-k experts)
   - Residual add
3. **Final RMSNorm + LM head**
4. **Sampling**

Use existing WGSL kernels from `crates/ml/candle-wgpu/shaders/` or write new ones.

### Step 6: MoE Expert Paging

GPT-OSS is MoE - most params are experts, only top-k active per token.

1. Parse expert weights (ggml type 39)
2. Implement router top-k selection
3. Cache hot experts on GPU (LRU eviction)
4. Emit telemetry for cache hits/misses

### Step 7: KV Cache

1. Allocate per-layer KV buffers (f16 if available)
2. Implement append for decode phase
3. Handle sliding window for local attention layers
4. Emit cache utilization telemetry

### Step 8: Tokenizer + Harmony

1. Load GPT-OSS tokenizer (BPE)
2. Implement Harmony prompt format wrapper
3. Encode input, run prefill, decode tokens one at a time

### Step 9: Token Generation Visualization

During generation:
1. Show token stream with cursor
2. Show top-5 probability bars for each token
3. Show attention patterns (optional but cool)
4. Show tokens/sec, entropy, cache usage
5. Pulse animation on each new token

---

## Key Files to Modify/Create

| File | Purpose |
|------|---------|
| `crates/web/client/src/gptoss_viz.rs` | Main visualization + button logic |
| `crates/web/client/src/gptoss_runtime.rs` | Model loading + inference runtime |
| `crates/web/client/src/gguf_web.rs` | GGUF parsing + range fetch |
| `crates/web/client/src/state.rs` | GptOssState fields |
| `crates/ml/src/kernels/` | WGSL compute shaders |

---

## Constraints

- **WASM target**: No `std::time::Instant`, use `web_time::Instant`
- **WebGPU limits**: Max 128MB per buffer, query adapter limits
- **No blocking**: All I/O must be async
- **Shared GPU context**: Reuse WGPUI device/queue, don't create a second device

---

## Commit Strategy

**Commit often.** After each working milestone:

1. Start button renders and responds to click
2. GGUF header fetches and parses
3. Tensor range fetches work with telemetry
4. First GPU buffer upload works
5. Single Q8_0 matmul runs in browser
6. Single transformer block runs
7. Full forward pass runs (no generation)
8. Token generation loop works
9. End-to-end: button → load → generate → tokens visible

Each commit should:
- Have a descriptive message
- Include `Co-Authored-By` lines
- Push to remote

---

## Definition of Done

**Not done until:**
- [ ] `/gptoss` has a visible "LOAD MODEL" button
- [ ] Clicking button starts streaming GGUF from URL
- [ ] Loading progress visible with live telemetry
- [ ] Tensors load onto GPU with visual feedback
- [ ] Model runs inference (forward pass works)
- [ ] Tokens generate one by one
- [ ] Token visualization shows probabilities
- [ ] Works in Chrome WebGPU
- [ ] No console errors, no crashes

---

## Do Not

- Do not ask for clarification - figure it out
- Do not stop at "it compiles" - it must run
- Do not create placeholder/mock implementations
- Do not skip the visualization - the whole point is seeing it work
- Do not batch commits - commit and push incrementally

---

## Reference Docs

Read these if stuck:
- `crates/ml/docs/plans/gptoss-browser-plan-tonight.md` - Gate details
- `crates/ml/docs/plans/gptoss-browser-plan.md` - Full architecture
- `crates/ml/docs/COMPUTE-KERNELS.md` - WGSL kernel specs
- `crates/ml/docs/ml-inference-visualization.md` - Visualization specs

---

## Start Now

Begin with Step 1. Make the button work. Push. Then Step 2. And so on.

Don't stop until I can click a button and watch GPT-OSS load and generate tokens in my browser.
