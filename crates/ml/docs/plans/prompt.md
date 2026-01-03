# GPT-OSS Browser Runtime: Codex Instructions

## End State (Non-Negotiable)

When you're done, I click **one button** on `/gptoss` and:

1. **GGUF streams in** - Range fetches from URL, chunks load progressively
2. **Visualization goes crazy** - Loading bars, telemetry events, tensor names scrolling, GPU buffer allocations, cache stats, everything updating live in the sci-fi HUD
3. **Tokens start generating** - Harmony-formatted prompt runs through the model, tokens appear one by one with probability bars, attention visualizations, the whole thing

**Do not stop until this works end-to-end.**

---

## Allowed Fallbacks (Critical)

You have permission to ship intermediate correctness. These fallbacks keep you shipping instead of thrashing forever on one missing kernel:

### MoE Fallback (if ggml type 39 not supported yet)
- Run **dense-only path**: router selects expert 0 always.
- **Clearly label in HUD**: "MoE: fallback mode (expert 0 only)"
- This still generates tokens, just worse quality.

### Attention Fallback (if banded attention not implemented)
- Use **dense attention everywhere** (slower but correct).
- Land sliding window later as optimization.
- Label in HUD: "Attention: dense mode"

### Sampling Fallback (if GPU sampling is hard)
- Do sampling on **CPU from readback logits**.
- Copy logits to CPU, run top-k/top-p there, return token ID.
- This is the default path for Phase 2a anyway.

### Layer Fallback (for bring-up)
- Run only **first N layers** initially (N=1, 2, 4...).
- Show how many layers are active in HUD.
- Iterate until full 24 layers run.

**The goal is "button → tokens", not "perfect architecture."**

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

**Important:** HuggingFace URLs often have CORS issues. Prefer local hosting.

URL priority:
1. **Local `gguf_serve`** (default for dev): `http://localhost:8080/gpt-oss-20b-Q8_0.gguf`
   - Start with: `cargo run -p ml --bin gguf_serve -- crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf`
2. **Query param override**: `?gguf=<URL>` for user-provided URLs
3. HuggingFace URLs are "user-provided" only - don't default to them

On startup, validate that the URL supports:
- `Accept-Ranges: bytes`
- CORS headers for Range requests

If not, show error: "Host does not support Range/CORS. Start gguf_serve."

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

### Step 5: Produce Logits ASAP (dopamine first!)

**Goal:** See tokens early, even before real layers work.

Implement minimal path:
1. `token_embd` lookup
2. (Optional) RMSNorm
3. `lm_head` projection (Q8_0 matmul)
4. Copy logits to CPU
5. Show top-5 tokens in HUD

**Done when:**
- [ ] A hardcoded prompt encodes to token IDs
- [ ] Embedding lookup works
- [ ] lm_head produces logits
- [ ] Top-5 tokens display (even if nonsense - no layers yet!)

This proves: tokenizer + prompt formatting + display loop work.

### Step 6: Single Layer Partial

Add one transformer block:
1. RMSNorm before attention
2. QKV projections (Q8_0 matmul)
3. RoPE application
4. Residual plumbing
5. **Attention can be stubbed to identity for this step**

**Done when:**
- [ ] Layer 0 runs with stubbed attention
- [ ] Residual connections work
- [ ] Output shape is correct

### Step 7: Dense Attention + KV

Implement real attention for 1 layer, then all layers:
1. Scaled dot-product attention
2. GQA (group size 8)
3. Causal masking
4. KV cache append

**Done when:**
- [ ] A single prompt runs through ≥1 real attention layer
- [ ] KV cache grows by 1 token per decode step
- [ ] Top-5 tokens render and make some sense

### Step 8: MoE Bring-Up

1. Map ggml type 39 (MXFP4/block-fp4)
2. Router top-k on CPU first is fine
3. Single expert path (expert 0)
4. Emit telemetry for router decisions

**Done when:**
- [ ] Router weights load
- [ ] Top-k expert indices computed
- [ ] At least expert 0 MLP runs
- [ ] HUD shows "MoE: active" (even if fallback mode)

### Step 9: Caches + Performance

1. Expert LRU cache
2. Continuous batching (multiple sessions)
3. Speculative decode (optional but high-leverage)

**Done when:**
- [ ] Expert cache hit/miss telemetry visible
- [ ] Multiple decode steps run without OOM
- [ ] tok/s metric displayed

### Step 10: Tokenizer + Harmony

1. Load GPT-OSS tokenizer (BPE from GGUF metadata)
2. Implement Harmony prompt format wrapper
3. Encode input, run prefill, decode tokens one at a time

**Done when:**
- [ ] Real prompts encode correctly
- [ ] Harmony format applied
- [ ] Generated text is coherent

### Step 11: Token Generation Visualization

During generation:
1. Show token stream with cursor
2. Show top-5 probability bars for each token
3. Show attention patterns (optional but cool)
4. Show tokens/sec, entropy, cache usage
5. Pulse animation on each new token

**Done when:**
- [ ] Tokens stream to screen one by one
- [ ] Probability bars animate
- [ ] Stats update in real-time

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

### Limits Probe Requirements (mandatory at startup)

On startup, log AND render in HUD:
- `maxBufferSize`
- `maxStorageBufferBindingSize`
- `maxBindGroups`
- `maxBindingsPerBindGroup`
- `shader-f16` support (true/false)

All kernels must:
- Select tile sizes from these limits (no hardcoded constants)
- Fail loudly if a buffer exceeds `maxStorageBufferBindingSize`
- Auto-retry with smaller tiles if initial config fails

Example HUD display:
```
GPU Limits:
  maxBuffer: 256MB
  maxBinding: 128MB
  bindGroups: 4
  f16: ✓
```

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

## CLI Testing (Don't Wait for the GUI)

**Critical:** Test as much as possible via CLI before testing in browser. The browser adds complexity (WASM, CORS, async) - verify core logic works natively first.

### Gate Verification (run these first)

```bash
# Gate A: GGUF parsing
cargo run -p ml --no-default-features --features native --bin gguf_dump -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --limit 20

# Gate B: Range reads
cargo run -p ml --no-default-features --features native --bin gguf_range -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --tensor output.weight --len 1048576 --repeat 2

# Gate C: GPU compute
cargo run -p ml --no-default-features --features native,wgpu --bin gguf_gate_c -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --tensor output.weight --k 128 --n 64

# Gate D: Correctness test
cargo test -p ml --no-default-features --features native,wgpu gguf_gate_d
```

### Runtime Component Tests

Create CLI binaries that test each subsystem in isolation:

```bash
# Test tokenizer (encode/decode roundtrip)
cargo run -p ml --bin test_tokenizer -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --text "Hello, world!"

# Test embedding lookup
cargo run -p ml --bin test_embed -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --tokens 1234,5678

# Test lm_head logits (embeddings → logits, no layers)
cargo run -p ml --bin test_lm_head -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --show-top 5

# Test single layer forward
cargo run -p ml --bin test_layer -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --layer 0

# Test attention (with real KV)
cargo run -p ml --bin test_attention -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --seq-len 4

# Test MoE router
cargo run -p ml --bin test_moe_router -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --layer 0 --show-experts

# Full native inference (end-to-end without browser)
cargo run -p ml --bin gptoss_cli -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --prompt "Once upon a time" \
  --max-tokens 20
```

### What Each CLI Test Validates

| Test | Validates | Pass Criteria |
|------|-----------|---------------|
| `test_tokenizer` | BPE encode/decode | Roundtrip matches |
| `test_embed` | Embedding lookup | Shape [seq, hidden] correct |
| `test_lm_head` | Q8_0 matmul, logits | Top-5 tokens print |
| `test_layer` | RMSNorm, QKV, residual | Output shape correct |
| `test_attention` | SDPA, KV cache | No NaN, cache grows |
| `test_moe_router` | Router top-k | Expert IDs in range |
| `gptoss_cli` | Full pipeline | Coherent text output |

### Test Before Browser

**Before testing in browser, ensure:**
1. All gate tests pass (`cargo test -p ml`)
2. `gptoss_cli` generates tokens natively
3. No panics, no NaN, no OOM

**Only then** test in browser. Browser-specific issues should be limited to:
- CORS/Range fetch
- WASM async patterns
- WebGPU device differences

### Automated Test Suite

```bash
# Run all ML tests
cargo test -p ml --no-default-features --features native,wgpu

# Run with verbose output
cargo test -p ml --no-default-features --features native,wgpu -- --nocapture

# Run specific gate
cargo test -p ml gate_d --no-default-features --features native,wgpu
```

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
- [ ] **All CLI tests pass before browser testing** (new requirement)

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
- `/Users/christopherdavid/code/harmony/` - The harmony repo
- `/Users/christopherdavid/code/gpt-oss/` - The gpt-oss repo
- `/Users/christopherdavid/code/candle/` - Candle Rust ML library - you can use or adapt any code from here

---

## Start Now

Begin with Step 1. Make the button work. Push. Then Step 2. And so on.

Don't stop until I can click a button and watch GPT-OSS load and generate tokens in my browser.
