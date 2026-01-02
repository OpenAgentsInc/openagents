# ML Inference Visualization: Sci-Fi HUD for Candle

## Overview

This document explores visualization possibilities for ML inference using the Candle integration. The goal: **make the invisible visible** - transform the abstract mathematics of neural network inference into compelling, real-time sci-fi HUD displays.

## What We Can Visualize

Candle exposes rich internal data during inference. Here's what's accessible and visualizable:

### Tier 1: High-Value, High-Feasibility

| Data Source | Shape | Access Method | Visual Potential |
|-------------|-------|---------------|------------------|
| **Attention Weights** | `[heads, seq, seq]` | `q.matmul(&k.t())` post-softmax | Heatmaps, flow animations |
| **Token Probabilities** | `[vocab_size]` | Final logits → softmax | Bar charts, probability cascades |
| **KV Cache State** | `[layers, seq, hidden]` | `cache.current_seq_len()` | Memory gauges, ring buffers |
| **Generation Speed** | scalar | Timestamp delta | Speedometer, pulse rate |
| **Top-K Candidates** | `[k]` | Sorted logits | Ranked probability bars |

### Tier 2: Medium Complexity

| Data Source | Shape | Access Method | Visual Potential |
|-------------|-------|---------------|------------------|
| **Layer Activations** | `[seq, hidden]` | Post-block hidden states | Layer activity bars, heatmaps |
| **RoPE Embeddings** | `[seq, head_dim/2]` | Precomputed cos/sin | Frequency spirals, position maps |
| **Attention Entropy** | `[heads]` | `-sum(p * log(p))` | Focus indicators, uncertainty gauges |
| **MoE Expert Routing** | `[experts]` | Router softmax weights | Load balancing bars |

### Tier 3: Advanced (Post-MVP)

| Data Source | Shape | Access Method | Visual Potential |
|-------------|-------|---------------|------------------|
| **Gradient Flow** | varies | Backward pass hooks | Training visualizations |
| **Quantization Loss** | scalar | FP32 vs Q4 comparison | Precision degradation meter |
| **Cross-Attention** | `[seq, context]` | Encoder-decoder models | Source-target flow |

---

## Visualization Concepts

### 1. INFERENCE MONITOR (Primary Dashboard)

**Purpose**: Real-time overview of the generation process.

```
┌─────────────────────────────────────────────────────────────┐
│ ╔══════════════════════════════════════════════════════════╗│
│ ║  INFERENCE ACTIVE                          ◉ 47.3 tok/s ║│
│ ╚══════════════════════════════════════════════════════════╝│
│                                                             │
│  ┌─ TOKEN STREAM ─────────────────────────────────────────┐ │
│  │ The quick brown fox jumps over the lazy█               │ │
│  │ ▁▃▅▇█▇▅▃▁  ← generation pulse                         │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌─ NEXT TOKEN ───────────────────────────────────────────┐ │
│  │  dog    ████████████████████████  0.847                │ │
│  │  cat    ████████                  0.089                │ │
│  │  bird   █████                     0.034                │ │
│  │  wolf   ███                       0.018                │ │
│  │  fox    ██                        0.012                │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌─ CACHE ──┐  ┌─ MEMORY ──┐  ┌─ ENTROPY ──┐               │
│  │ ▓▓▓▓▓░░░ │  │ ████░░░░░ │  │  ▂▄▆▇▆▄▂  │               │
│  │ 128/256  │  │ 47MB/128  │  │   1.24    │               │
│  └──────────┘  └───────────┘  └───────────┘               │
└─────────────────────────────────────────────────────────────┘
```

**Components**:
- **Token Stream**: Scrolling text with cursor blink, generation pulse wave
- **Next Token Distribution**: Horizontal bars showing top-5 candidates
- **Cache Gauge**: KV cache fill level (rotating cache shows as ring)
- **Memory Gauge**: GPU memory usage
- **Entropy Meter**: Decision confidence (low = certain, high = uncertain)

**WGPUI Implementation**:
- `SignalMeter` for probability bars
- `Frame::corners()` for sci-fi borders
- `DotsGrid` animated background
- Custom scrolling text component with `Scanlines` overlay

---

### 2. ATTENTION VISUALIZER

**Purpose**: See where the model "looks" when generating each token.

```
┌─────────────────────────────────────────────────────────────┐
│  ATTENTION PATTERNS                    Layer 12 / Head 3    │
│                                                             │
│     The  quick brown  fox  jumps over  the  lazy  dog      │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ The   ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│   │
│  │ quick ░░██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│   │
│  │ brown ░░██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│   │
│  │ fox   ░░████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│   │
│  │ jumps ░░░░██████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░│   │
│  │ over  ░░░░░░██████████████████░░░░░░░░░░░░░░░░░░░░░│   │
│  │ the   ██░░░░░░░░████████████████████░░░░░░░░░░░░░░░│   │
│  │ lazy  ░░░░░░░░░░░░████████████████████████░░░░░░░░░│   │
│  │ dog   ░░░░░░░░████░░░░████████████████████████████░│   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  ◀ ════════════════════●══════════════════ ▶  [HEAD] [LAYER]│
│    0                   3                   7                │
└─────────────────────────────────────────────────────────────┘
```

**Visual Elements**:
- **Heatmap Grid**: Attention weights as colored cells
- **Causal Mask**: Upper triangle dimmed (future tokens)
- **Token Labels**: Row/column headers
- **Interactive Controls**: Layer/head selection sliders
- **Animation**: Smooth transition when changing layer/head

**Data Flow**:
```rust
// Extract attention weights after softmax
let attn_weights = q.matmul(&k.t())?;  // [batch, heads, seq, seq]
let attn_probs = softmax(&attn_weights, D::Minus1)?;

// Extract for visualization (single head)
let head_attn = attn_probs.i((0, head_idx, .., ..))?;
let data: Vec<Vec<f32>> = head_attn.to_vec2()?;
```

**WGPUI Implementation**:
- Grid of `Quad` elements with color intensity based on attention weight
- Color gradient: dark blue (0.0) → cyan → white (1.0)
- `GridLinesBackground` for cell borders
- Slider component for layer/head selection

---

### 3. NETWORK PULSE (Layer Activity)

**Purpose**: Visualize data flowing through transformer layers in real-time.

```
┌─────────────────────────────────────────────────────────────┐
│  NETWORK ACTIVITY                                           │
│                                                             │
│  Layer  Attention    MLP        Output                      │
│  ─────  ─────────    ───        ──────                      │
│   00    ▓▓▓░░░░░░    ▓▓▓▓░░░░   ▓▓░░░░░░                   │
│   01    ▓▓▓▓▓░░░░    ▓▓▓▓▓▓░░   ▓▓▓▓░░░░                   │
│   02    ▓▓▓▓▓▓░░░    ▓▓▓▓▓▓▓░   ▓▓▓▓▓░░░    ← active       │
│   03    ░░░░░░░░░    ░░░░░░░░   ░░░░░░░░                   │
│   04    ░░░░░░░░░    ░░░░░░░░   ░░░░░░░░                   │
│   ...                                                       │
│   25    ░░░░░░░░░    ░░░░░░░░   ░░░░░░░░                   │
│                                                             │
│  ════════════════════════════════════════════════════════   │
│  Activation norm: 2.847    Residual: 0.234    Temp: 0.7    │
└─────────────────────────────────────────────────────────────┘
```

**Animation**: "Pulse" travels down the layer stack as each token is processed.

**Data Source**:
```rust
// After each transformer block
let hidden_norm = hidden_states.sqr()?.mean(D::Minus1)?.sqrt()?;
let activation_magnitude = hidden_norm.to_vec1::<f32>()?[0];
```

**WGPUI Implementation**:
- Horizontal `SignalMeter` bars per layer
- `SpringAnimation` for smooth magnitude changes
- Pulsing highlight effect using `Animation` with `EaseOutElastic`

---

### 4. KV CACHE INSPECTOR

**Purpose**: Visualize the rotating KV cache as a ring buffer.

```
┌─────────────────────────────────────────────────────────────┐
│  KV CACHE                                     Layer 12      │
│                                                             │
│                    ╭──────────────╮                         │
│                 ╭──╯   ▓▓▓▓▓▓▓▓   ╰──╮                     │
│               ╭─╯  ▓▓▓▓        ▓▓▓▓  ╰─╮                   │
│              │ ▓▓▓▓              ▓▓▓▓  │                   │
│              │▓▓▓                  ▓▓▓▓│  ← write head      │
│              │▓▓                    ░░░│                   │
│              │▓▓                    ░░░│                   │
│              │ ▓▓                  ░░░ │                   │
│               ╰─╮  ▓▓▓▓        ░░░░  ╭─╯                   │
│                 ╰──╮   ▓▓▓▓░░░░   ╭──╯                     │
│                    ╰──────────────╯                         │
│                                                             │
│  Position: 847/1024    Window: 512    Offset: 335          │
│  Keys: 24.3 MB         Values: 24.3 MB                     │
└─────────────────────────────────────────────────────────────┘
```

**Visual Elements**:
- **Ring Buffer**: Circular visualization of cache slots
- **Write Head**: Animated indicator showing current write position
- **Fill Level**: Color intensity shows used vs free slots
- **Layer Selector**: Switch between layer caches

**Data Source**:
```rust
// From RotatingKvCache
let seq_len = kv_cache.current_seq_len();
let max_len = kv_cache.max_seq_len();
let offset = kv_cache.offset();
```

**WGPUI Implementation**:
- Arc segments using `Quad` elements positioned in circle
- `Animation` for write head rotation
- `Frame::circle()` style border

---

### 5. TOKEN PROBABILITY WATERFALL

**Purpose**: Historical view of token selection probabilities.

```
┌─────────────────────────────────────────────────────────────┐
│  PROBABILITY HISTORY                                        │
│                                                             │
│  ─────────────────────────────────────────────────────────  │
│  │ Position: 12    │ Position: 13    │ Position: 14    │   │
│  │ "quick"         │ "brown"         │ "fox"           │   │
│  │ ████████ 0.92   │ ████████ 0.78   │ ████████ 0.85   │   │
│  │ ███      0.04   │ ████     0.12   │ ███      0.08   │   │
│  │ ██       0.02   │ ██       0.05   │ ██       0.04   │   │
│  │ █        0.01   │ █        0.03   │ █        0.02   │   │
│  │ █        0.01   │ █        0.02   │ █        0.01   │   │
│  ─────────────────────────────────────────────────────────  │
│                                                             │
│  Entropy: ▁▂▃▄▃▂▁▂▃▄▅▆▅▄▃▂▁▂▃▄▃                            │
│  ◀════════════════════════════════════════════════════════▶ │
└─────────────────────────────────────────────────────────────┘
```

**Visual Elements**:
- **Scrollable History**: Column per generated token
- **Probability Bars**: Top-5 candidates at each position
- **Entropy Line**: Running entropy graph below
- **Selected Token**: Highlighted with glow effect

**Animation**: New columns slide in from right with `EaseOutCubic`.

---

### 6. ATTENTION FLOW (Advanced)

**Purpose**: Animated visualization of attention as information flow.

```
┌─────────────────────────────────────────────────────────────┐
│  ATTENTION FLOW                              "dog" → ???    │
│                                                             │
│     The ─────┐                                              │
│              │                                              │
│   quick ─────┼───┐                                          │
│              │   │                                          │
│   brown ─────┼───┼───┐                                      │
│              │   │   │                                      │
│     fox ═════╪═══╪═══╪═══════════════════╗                  │
│              │   │   │                   ║                  │
│   jumps ─────┼───┼───┼───────────┐       ║                  │
│              │   │   │           │       ║                  │
│    over ─────┼───┼───┼───────────┼───┐   ║                  │
│              │   │   │           │   │   ║                  │
│     the ─────┼───┼───┼───────────┼───┼───╫─┐                │
│              │   │   │           │   │   ║ │                │
│    lazy ─────┴───┴───┴───────────┴───┴───╫─┴─▶ [dog]        │
│                                          ║                  │
│     ══════════════════════════════════════╝ (strongest)     │
└─────────────────────────────────────────────────────────────┘
```

**Visual Elements**:
- **Source Tokens**: Left column
- **Flow Lines**: Animated lines with thickness = attention weight
- **Target Token**: Right side, receiving attention
- **Dominant Path**: Highlighted strongest attention source

**Animation**: Lines "flow" with moving dash pattern, intensity pulses.

---

## Implementation Architecture

### Data Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│                    CANDLE INFERENCE                         │
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │   Forward   │───▶│  Telemetry  │───▶│   Channel   │     │
│  │    Pass     │    │   Hooks     │    │   (mpsc)    │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
│         │                                     │             │
└─────────┼─────────────────────────────────────┼─────────────┘
          │                                     │
          ▼                                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    VISUALIZATION LAYER                      │
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │ InferenceViz│◀───│  VizState   │◀───│  Receiver   │     │
│  │  Component  │    │   Buffer    │    │  (channel)  │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
│         │                                                   │
│         ▼                                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    WGPUI Scene                       │   │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐        │   │
│  │  │ Quads  │ │  Text  │ │ Frames │ │Animate │        │   │
│  │  └────────┘ └────────┘ └────────┘ └────────┘        │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Telemetry Hook API

```rust
/// Telemetry data emitted during inference
pub enum InferenceTelemetry {
    /// Token generated with probabilities
    TokenGenerated {
        token_id: u32,
        token_text: String,
        top_k: Vec<(u32, f32)>,  // (token_id, probability)
        entropy: f32,
        tokens_per_sec: f32,
    },

    /// Attention weights for a layer/head
    AttentionWeights {
        layer: usize,
        head: usize,
        weights: Vec<Vec<f32>>,  // [query_pos, key_pos]
    },

    /// Layer activation magnitude
    LayerActivation {
        layer: usize,
        attention_norm: f32,
        mlp_norm: f32,
        output_norm: f32,
    },

    /// KV cache status
    CacheStatus {
        layer: usize,
        seq_len: usize,
        max_len: usize,
        offset: usize,  // for rotating cache
        memory_bytes: usize,
    },

    /// Memory snapshot
    MemoryUsage {
        gpu_allocated: usize,
        cache_total: usize,
        activations: usize,
    },
}

/// Hook trait for instrumented inference
pub trait InferenceHook: Send + Sync {
    fn on_telemetry(&self, telemetry: InferenceTelemetry);
}
```

### Visualization State

```rust
/// State buffer for visualization components
pub struct VizState {
    // Token generation
    pub generated_tokens: VecDeque<TokenInfo>,
    pub current_probs: Vec<(String, f32)>,
    pub tokens_per_sec: f32,

    // Attention (cached for selected layer/head)
    pub attention_weights: Option<Vec<Vec<f32>>>,
    pub selected_layer: usize,
    pub selected_head: usize,

    // Layer activity
    pub layer_activations: Vec<LayerActivity>,

    // Cache
    pub cache_status: Vec<CacheInfo>,

    // Memory
    pub memory_history: VecDeque<MemorySnapshot>,

    // Animation state
    pub frame_animator: FrameAnimator,
    pub pulse_position: f32,
}
```

---

## Feasibility Assessment

### Phase 1: Core Dashboard (Recommended Start)

| Component | Complexity | WGPUI Support | Data Access |
|-----------|------------|---------------|-------------|
| Token Stream | Low | Text + Animation | Easy |
| Probability Bars | Low | SignalMeter | Easy |
| Cache Gauge | Low | SignalMeter | Easy |
| Speed Metric | Low | Text | Easy |
| Frame/HUD Chrome | Low | Frame component | N/A |

**Effort**: 3-5 days
**Value**: High - immediately useful, visually impressive

### Phase 2: Attention Visualization

| Component | Complexity | WGPUI Support | Data Access |
|-----------|------------|---------------|-------------|
| Heatmap Grid | Medium | Custom quads | Medium |
| Layer/Head Selector | Low | Slider | N/A |
| Token Labels | Low | Text | Easy |
| Animation | Medium | Spring/Keyframe | N/A |

**Effort**: 5-7 days
**Value**: High - reveals model behavior, very sci-fi

### Phase 3: Advanced Visualizations

| Component | Complexity | WGPUI Support | Data Access |
|-----------|------------|---------------|-------------|
| Ring Buffer Cache | High | Custom geometry | Medium |
| Layer Pulse | Medium | Animation system | Medium |
| Attention Flow | High | Custom lines | Medium |
| Probability Waterfall | Medium | Virtual scroll | Easy |

**Effort**: 7-10 days each
**Value**: Medium-High - impressive but diminishing returns

---

## Browser Constraints

For WebGPU (browser) visualizations:

| Constraint | Impact | Mitigation |
|------------|--------|------------|
| 128MB GPU buffer limit | Limits cache viz detail | Subsample, show summary |
| No shared memory | Can't share tensors | Copy to viz buffer |
| 60fps target | Limits update rate | Throttle telemetry to 30Hz |
| WASM overhead | Serialization cost | Use typed arrays, avoid copying |

**Recommendation**: Throttle telemetry updates to 30Hz, batch attention weight extraction, use ring buffers with fixed capacity.

---

## Visual Design Principles

Following the GFN visualization aesthetic:

1. **Dense Information**: Multiple data panels, no wasted space
2. **High Contrast**: Bright elements on dark backgrounds
3. **Animated Frames**: Corner bracket animations on focus
4. **Glow Effects**: Layered borders for "active" elements
5. **Color Coding**:
   - Cyan (#7FD3E5): Primary data
   - Orange (#FF9900): Warnings/attention
   - Green (#00FF88): Positive/complete
   - Red (#FF4444): Errors/high values
6. **Vera Mono**: Consistent monospace typography
7. **Grid Backgrounds**: Subtle dot/line grids for depth

---

## Recommended Implementation Order

1. **Inference Monitor Dashboard** - Core value, proves the concept
2. **Attention Heatmap** - Highest visual impact, reveals model internals
3. **Layer Activity Pulse** - Relatively simple, very sci-fi
4. **KV Cache Ring** - Useful for debugging, cool visual
5. **Probability Waterfall** - Nice-to-have, historical analysis
6. **Attention Flow Animation** - Complex, save for polish phase

---

## Files to Create/Modify

### New Files
```
crates/ml/src/
├── telemetry.rs          # InferenceTelemetry enum, InferenceHook trait
├── viz/
│   ├── mod.rs
│   ├── state.rs          # VizState buffer
│   ├── inference_monitor.rs
│   ├── attention_heatmap.rs
│   ├── layer_activity.rs
│   ├── cache_inspector.rs
│   └── probability_history.rs
```

### Modified Files
```
crates/ml/src/
├── provider.rs           # Add telemetry hook to inference loop
├── candle-wgpu/src/
│   └── device.rs         # Memory tracking for viz

crates/wgpui/src/components/hud/
├── mod.rs                # Export new viz components
├── heatmap.rs            # New: generic heatmap component
├── ring_gauge.rs         # New: circular gauge component
```

---

## Summary

The Candle integration provides rich internal data for visualization:
- **Attention patterns** reveal how the model processes context
- **Token probabilities** show decision-making in real-time
- **KV cache** demonstrates memory management
- **Layer activations** visualize data flow through the network

WGPUI's GPU-accelerated rendering and existing HUD components (frames, signal meters, grids, animations) provide the foundation for sci-fi visualizations. The GFN visualization demonstrates the target aesthetic: dense, high-contrast, animated.

**Start with the Inference Monitor Dashboard** - it delivers immediate value with relatively low complexity, and proves the telemetry pipeline works. From there, attention heatmaps provide the highest visual impact for understanding model behavior.
