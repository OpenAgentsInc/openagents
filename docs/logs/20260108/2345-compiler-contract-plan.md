# Wave 3: Compiler Contract Implementation Plan

Transform dsrs modules into first-class OpenAgents citizens with standardized traces, compiled module IDs, cost accounting, and replayability.

## Overview

Wave 3 bridges the DSPy compiler layer (`crates/dsrs/`) with the OpenAgents execution layer (Pylon, Nexus, Runtime) through:
- **CompiledModuleManifest**: Versioned, hashable module artifacts
- **Compatibility checking**: Runtime validation of requirements
- **TraceContract**: Map dsrs DAG → OpenAgents trace spans
- **DspyCallback**: Event emission for HUD/monitoring
- **Pylon Sandbox Provider**: Separate provider for CPU-intensive work
- **Refine pattern**: Meta-operator for retries and fallback

## Dependencies (All Complete ✓)

| Dependency | Location | Status |
|------------|----------|--------|
| Protocol crate | `crates/protocol/` | ✓ Job schemas, hashing, verification |
| dsrs core | `crates/dsrs/` | ✓ Module, MetaSignature, Predictor traits |
| DAG tracing | `crates/dsrs/src/trace/` | ✓ Graph, Node, NodeType |
| Pylon LM Provider | `crates/dsrs/src/core/lm/pylon.rs` | ✓ NIP-90 inference |
| Optimizers | `crates/dsrs/src/optimizer/` | ✓ GEPA, MIPRo, CoPRo |

## Implementation Steps

### Phase 1: Core Manifest Types

**Create `crates/dsrs/src/manifest.rs`**

```rust
pub struct CompiledModuleManifest {
    pub signature_name: String,
    pub compiled_id: String,          // SHA-256 of optimized artifact
    pub optimizer: String,            // "MIPROv2" | "GEPA" | "COPRO"
    pub trainset_id: String,          // Hash of training data
    pub scorecard: Scorecard,
    pub compatibility: Compatibility,
    pub created_at: u64,
}

pub struct Scorecard {
    pub proxy_metrics: HashMap<String, f32>,
    pub truth_metrics: HashMap<String, f32>,
    pub rollouts: usize,
    pub median_score: f32,
    pub p_fail: f32,
}

pub struct Compatibility {
    pub required_tools: Vec<String>,
    pub required_lanes: Vec<String>,
    pub privacy_modes_allowed: Vec<PrivacyMode>,
    pub min_provider_reputation: Option<f32>,
}

pub enum PrivacyMode {
    PublicOk,
    NoPii,
    PrivateRepoRedacted,
    PrivateRepoAllowed,
}
```

Key features:
- Uses `protocol::canonical_hash` for `compiled_id` generation
- Serde serialization with JSON schema
- `Hashable` trait implementation

### Phase 2: Callback System

**Create `crates/dsrs/src/callbacks.rs`**

```rust
pub trait DspyCallback: Send + Sync {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example);
    fn on_module_end(&self, call_id: Uuid, outputs: Result<&Prediction, &Error>);
    fn on_lm_start(&self, call_id: Uuid, model: &str, messages: &[Message]);
    fn on_lm_end(&self, call_id: Uuid, response: Result<&Message, &Error>, usage: &LmUsage);
    fn on_optimizer_candidate(&self, candidate_id: String, metrics: HashMap<String, f32>);
    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>);
}

pub struct HudCallback { /* Streams to Runtime /hud */ }
pub struct NostrCallback { /* Emits Nostr events */ }
pub struct CompositeCallback { /* Chains multiple callbacks */ }
```

**Modify `crates/dsrs/src/core/settings.rs`**:
- Add `callback: Option<Arc<dyn DspyCallback>>` to Settings
- Add `set_callback()` and `get_callback()` functions

**Modify `crates/dsrs/src/predictors/predict.rs`**:
- Call `on_module_start` before execution
- Call `on_module_end` after execution
- Pass callback through execution chain

### Phase 3: Pylon Sandbox Provider

**Create `crates/dsrs/src/adapter/pylon_sandbox.rs`**

```rust
pub struct PylonSandboxProvider {
    pub relay_url: String,
    pub keypair: Keys,
    pub profile: SandboxProfile,
}

pub enum SandboxProfile {
    Small  { vcpus: 1, memory_mb: 1024 },
    Medium { vcpus: 2, memory_mb: 4096 },
    Large  { vcpus: 4, memory_mb: 8192 },
}

impl PylonSandboxProvider {
    pub async fn run(&self, request: SandboxRunRequest) -> Result<SandboxRunResponse>;
}
```

Integration:
- Uses `protocol::jobs::SandboxRunRequest/Response`
- Publishes NIP-90 kind:5050 jobs to Nexus
- Handles payment flow (kind:7000 invoice)
- Returns results with provenance

### Phase 4: DAG → Nostr Bridge

**Create `crates/dsrs/src/trace/nostr_bridge.rs`**

```rust
pub struct NostrBridge {
    pub relay_url: String,
    pub keypair: Keys,
}

impl NostrBridge {
    /// Convert receiptable nodes to Nostr events
    pub async fn publish_trace(&self, graph: &Graph, manifest: &CompiledModuleManifest) -> Result<Vec<EventId>>;

    /// Map NodeType to appropriate Nostr event kind
    fn node_to_event(&self, node: &Node) -> Option<Event>;
}
```

Node type mapping:
| NodeType | Nostr Kind | Description |
|----------|------------|-------------|
| Predict (LM) | 5050 | Job request |
| Predict (result) | 6050 | Job result |
| Operator (sandbox) | 5050 | Sandbox job |
| Root/Map | None | Internal only |

### Phase 5: TraceContract (OTel Compatibility)

**Create `crates/dsrs/src/trace/contract.rs`**

```rust
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: SpanKind,
    pub start_time: u64,
    pub end_time: u64,
    pub attributes: HashMap<String, Value>,
    pub status: SpanStatus,
}

pub enum SpanKind {
    Internal,
    Client,    // LM calls
    Producer,  // Job submissions
}

pub struct TraceContract;

impl TraceContract {
    /// Convert dsrs Graph to OTel-compatible spans
    pub fn graph_to_spans(graph: &Graph, manifest: &CompiledModuleManifest) -> Vec<TraceSpan>;
}
```

Attributes include:
- `dsrs.signature_name`
- `dsrs.compiled_id`
- `dsrs.node_type`
- `lm.model`, `lm.tokens`, `lm.cost_msats`

### Phase 6: Refine Meta-Operator

**Create `crates/dsrs/src/predictors/refine.rs`**

```rust
pub struct Refine<M: Module> {
    module: M,
    max_retries: usize,
    reward_fn: Box<dyn Fn(&Example, &Prediction) -> f32 + Send + Sync>,
    threshold: f32,
    fallback_lane: Option<Arc<LM>>,
}

impl<M: Module> Refine<M> {
    pub fn new(module: M) -> Self;
    pub fn with_retries(self, n: usize) -> Self;
    pub fn with_reward(self, f: impl Fn(&Example, &Prediction) -> f32 + Send + Sync + 'static) -> Self;
    pub fn with_threshold(self, t: f32) -> Self;
    pub fn with_fallback(self, lm: Arc<LM>) -> Self;
}

impl<M: Module> Module for Refine<M> {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        for attempt in 0..self.max_retries {
            let pred = self.module.forward(inputs.clone()).await?;
            let score = (self.reward_fn)(&inputs, &pred);
            if score >= self.threshold {
                return Ok(pred);
            }
            // Try fallback lane if available
        }
        Err(anyhow!("Refine failed after {} attempts", self.max_retries))
    }
}
```

### Phase 7: Integration & Exports

**Modify `crates/dsrs/src/lib.rs`**:
```rust
pub mod manifest;
pub mod callbacks;

// Re-exports
pub use manifest::{CompiledModuleManifest, Compatibility, Scorecard, PrivacyMode};
pub use callbacks::{DspyCallback, HudCallback, NostrCallback};
```

**Modify `crates/dsrs/src/trace/mod.rs`**:
```rust
pub mod nostr_bridge;
pub mod contract;

pub use nostr_bridge::NostrBridge;
pub use contract::{TraceContract, TraceSpan, SpanKind};
```

**Modify `crates/dsrs/src/predictors/mod.rs`**:
```rust
pub mod refine;
pub use refine::Refine;
```

**Modify `crates/dsrs/src/adapter/mod.rs`**:
```rust
pub mod pylon_sandbox;
pub use pylon_sandbox::PylonSandboxProvider;
```

## Files to Create

| File | Purpose |
|------|---------|
| `crates/dsrs/src/manifest.rs` | CompiledModuleManifest, Scorecard, Compatibility |
| `crates/dsrs/src/callbacks.rs` | DspyCallback trait + implementations |
| `crates/dsrs/src/adapter/pylon_sandbox.rs` | Sandbox execution via Pylon |
| `crates/dsrs/src/trace/nostr_bridge.rs` | DAG → Nostr event publishing |
| `crates/dsrs/src/trace/contract.rs` | TraceContract, OTel-compatible spans |
| `crates/dsrs/src/predictors/refine.rs` | Refine meta-operator |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/dsrs/src/lib.rs` | Export new modules |
| `crates/dsrs/src/core/settings.rs` | Add callback to global settings |
| `crates/dsrs/src/predictors/predict.rs` | Emit callback events |
| `crates/dsrs/src/predictors/mod.rs` | Export Refine |
| `crates/dsrs/src/trace/mod.rs` | Export nostr_bridge, contract |
| `crates/dsrs/src/adapter/mod.rs` | Export pylon_sandbox |
| `crates/dsrs/Cargo.toml` | Add protocol dependency |

## Testing Strategy

1. **Unit tests** for each new module
2. **Integration test**: Compile a module → verify manifest hash is deterministic
3. **Callback test**: Verify events are emitted during execution
4. **Sandbox test**: Submit job to local Pylon, verify response
5. **Nostr bridge test**: Publish trace, verify events appear on relay

## Success Criteria

1. `cargo build -p dsrs` compiles with new modules
2. `cargo test -p dsrs` passes all tests
3. CompiledModuleManifest produces deterministic `compiled_id`
4. Callbacks emit events during module execution
5. PylonSandboxProvider can execute sandbox jobs
6. TraceContract produces valid OTel-compatible spans
7. Refine operator retries and falls back correctly

## Estimated Scope

~1,500-2,000 lines of new code across 6 new files and 6 modified files.
