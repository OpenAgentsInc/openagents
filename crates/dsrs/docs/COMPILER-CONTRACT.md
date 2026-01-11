# Compiler Contract (Wave 3)

The Compiler Contract bridges DSPy's compiler layer with OpenAgents execution, providing:
- Versioned, hashable module artifacts
- OTel-compatible trace spans
- Nostr event publishing
- Sandbox execution
- Retry/fallback patterns

## CompiledModuleManifest

Every compiled module emits a manifest for versioning and compatibility checking.

```rust
pub struct CompiledModuleManifest {
    /// Name of the signature
    pub signature_name: String,

    /// SHA-256 hash of the optimized artifact
    pub compiled_id: Option<String>,

    /// Optimizer used (MIPROv2, GEPA, COPRO)
    pub optimizer: String,

    /// Hash of training set used
    pub trainset_id: Option<String>,

    /// Metrics from optimization
    pub scorecard: Scorecard,

    /// Runtime requirements
    pub compatibility: Compatibility,

    /// Unix timestamp of creation
    pub created_at: u64,

    /// Optimized instruction text
    pub instruction: Option<String>,

    /// Number of demos in compiled module
    pub demo_count: usize,
}
```

### Scorecard

Metrics from optimization runs.

```rust
pub struct Scorecard {
    /// Proxy metric (cheap, frequent evaluation)
    pub proxy_score: f32,

    /// Truth metric (expensive, definitive evaluation)
    pub truth_score: Option<f32>,

    /// Total optimization cost in millisatoshis
    pub optimization_cost_msats: u64,

    /// Number of optimization iterations
    pub iterations: usize,

    /// Custom metrics
    pub custom_metrics: HashMap<String, f32>,
}
```

### Compatibility

Machine-checkable runtime requirements.

```rust
pub struct Compatibility {
    /// Required tools (e.g., ["ripgrep", "pytest"])
    pub required_tools: Vec<String>,

    /// Required retrieval lanes (e.g., ["lsp", "semantic"])
    pub required_lanes: Vec<String>,

    /// Allowed privacy modes
    pub privacy_modes_allowed: Vec<PrivacyMode>,

    /// Minimum provider reputation (0.0-1.0)
    pub min_provider_reputation: Option<f32>,
}

pub enum PrivacyMode {
    PublicOk,
    NoPii,
    PrivateRepoRedacted,
    PrivateRepoAllowed,
}
```

### Usage

```rust
use dsrs::manifest::*;

// Create manifest after optimization
let manifest = CompiledModuleManifest::new("PlanningSignature")
    .with_optimizer("MIPROv2")
    .with_scorecard(Scorecard {
        proxy_score: 0.85,
        truth_score: Some(0.92),
        optimization_cost_msats: 5000,
        iterations: 10,
        custom_metrics: HashMap::new(),
    })
    .with_compatibility(Compatibility {
        required_tools: vec!["ripgrep".to_string()],
        required_lanes: vec!["lsp".to_string()],
        privacy_modes_allowed: vec![PrivacyMode::PublicOk],
        min_provider_reputation: Some(0.8),
    });

// Get deterministic compiled_id
let compiled_id = manifest.compiled_id();
// "sha256:abc123..."
```

## TraceContract

Converts dsrs execution DAG to OpenTelemetry-compatible spans.

```rust
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: SpanKind,
    pub start_time_unix_nano: u64,
    pub end_time_unix_nano: u64,
    pub attributes: HashMap<String, Value>,
    pub status: SpanStatus,
}

pub enum SpanKind {
    Client,    // LM calls
    Server,    // Sandbox execution
    Internal,  // Root/routing
}

pub enum SpanStatus {
    Unset,
    Ok,
    Error { message: String },
}
```

### Converting DAG to Spans

```rust
use dsrs::trace::{Graph, TraceContract};

let graph = dsrs::trace::get_graph();
let manifest = Some(&compiled_manifest);

let spans = TraceContract::graph_to_spans(
    &graph,
    manifest,
    "trace-id-123",
);

// Each span includes:
// - dsrs.signature_name
// - dsrs.compiled_id (if manifest provided)
// - lm.model
// - lm.total_tokens
// - lm.cost_msats
```

### Span Attributes

| Attribute | Description |
|-----------|-------------|
| `dsrs.module` | Module type (Predict, Refine, etc.) |
| `dsrs.signature_name` | Signature name |
| `dsrs.compiled_id` | Hash of compiled module |
| `lm.model` | LM model ID |
| `lm.prompt_tokens` | Input tokens |
| `lm.completion_tokens` | Output tokens |
| `lm.total_tokens` | Total tokens |
| `lm.cost_msats` | Cost in millisatoshis |

### Trace Summary

```rust
let summary = TraceContract::summarize(&spans);
// TraceSummary {
//     span_count: 5,
//     predict_count: 3,
//     total_tokens: 1500,
//     total_cost_msats: 750,
// }
```

## NostrBridge

Publishes execution traces to Nostr relays for distributed observability.

```rust
use dsrs::trace::{NostrBridge, NostrBridgeConfig};
use nostr::Keypair;

// Create bridge with keys
let bridge = NostrBridge::generate();  // Random keys
// Or with existing keypair
let bridge = NostrBridge::new(keypair);

// Configure
let config = NostrBridgeConfig {
    relay_urls: vec!["wss://nexus.openagents.com".to_string()],
    wait_for_ok: true,
    default_tags: vec![("app".to_string(), "autopilot".to_string())],
};
let bridge = bridge.with_config(config);

// Convert graph to events
let events = bridge.graph_to_events(&graph, Some(&manifest))?;

// Get event IDs
let ids = NostrBridge::get_event_ids(&events);
```

### Event Structure

Events are kind:1 (text note) with structured content and tags:

```json
{
  "kind": 1,
  "content": "{\"node_id\":1,\"signature_name\":\"Planning\",\"has_output\":true,...}",
  "tags": [
    ["dsrs", "predict"],
    ["signature", "PlanningSignature"],
    ["compiled_id", "sha256:abc123..."],
    ["app", "autopilot"]
  ]
}
```

### Event Types

| Node Type | Tags | Content |
|-----------|------|---------|
| Predict | `["dsrs", "predict"]` | PredictTraceData |
| Operator | `["dsrs", "operator"]` | OperatorTraceData |
| Summary | `["dsrs", "trace_summary"]` | NostrTraceSummary |

## PylonSandboxProvider

Executes commands in sandboxed environments via Pylon.

```rust
use dsrs::adapter::{PylonSandboxProvider, SandboxProfile};

// Create provider
let provider = PylonSandboxProvider::generate()
    .with_profile(SandboxProfile::Medium)
    .with_image("sha256:abc123...")
    .with_network_policy(NetworkPolicy::None);

// Run commands
let response = provider.run_commands(vec![
    "cargo build",
    "cargo test",
]).await?;

// Check results
if PylonSandboxProvider::all_succeeded(&response) {
    println!("All commands passed!");
}
```

### Resource Profiles

| Profile | vCPUs | RAM | Disk | Timeout |
|---------|-------|-----|------|---------|
| Small | 1 | 1GB | 5GB | 60s |
| Medium | 2 | 4GB | 8GB | 120s |
| Large | 4 | 8GB | 10GB | 300s |

### Network Policies

| Policy | Description |
|--------|-------------|
| `None` | No network access |
| `Localhost` | Only localhost connections |
| `Allowlist` | Specific domains only |
| `Full` | Unrestricted (use with caution) |

### Response Structure

```rust
pub struct SandboxRunResponse {
    pub env_info: EnvInfo,
    pub runs: Vec<CommandResult>,
    pub artifacts: Vec<Artifact>,
    pub status: SandboxStatus,
    pub error: Option<String>,
    pub provenance: Provenance,
}

pub struct CommandResult {
    pub cmd: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub stdout_preview: Option<String>,
    pub stderr_preview: Option<String>,
}
```

## Refine Meta-Operator

Wraps modules with retry/fallback logic for robust execution.

```rust
use dsrs::predictors::Refine;

let refined = Refine::new(my_module)
    .with_max_retries(3)
    .with_threshold(0.8)
    .with_best_of_n(true)
    .with_reward_fn(|_inputs, prediction| {
        // Custom scoring logic
        let answer = prediction.get("answer", None);
        if answer.contains("correct") { 1.0 } else { 0.5 }
    });

let result = refined.forward(inputs).await?;
```

### Configuration

| Option | Description | Default |
|--------|-------------|---------|
| `max_retries` | Maximum attempts | 3 |
| `threshold` | Minimum score to accept | 0.5 |
| `best_of_n` | Return best across all attempts | false |
| `fallback_lm` | Backup LM if primary fails | None |

### Behavior

1. Execute module with reward function
2. If score >= threshold, return immediately
3. If score < threshold, retry up to max_retries
4. In best_of_n mode, track best prediction across all attempts
5. Return best prediction if any attempt reached 50% of threshold
6. Otherwise return error

## Integration Example

Full example combining all Wave 3 features:

```rust
use dsrs::prelude::*;

// Configure with callback
configure_with_callback(
    LM::new("codex-3-sonnet"),
    ChatAdapter,
    LoggingCallback::new(),
);

// Create module with Refine wrapper
let module = Refine::new(Predict::new(MySignature::default()))
    .with_max_retries(3)
    .with_threshold(0.8);

// Execute with tracing
dsrs::trace::start_tracing();
let result = module.forward(inputs).await?;
let graph = dsrs::trace::get_graph();
dsrs::trace::stop_tracing();

// Create manifest
let manifest = CompiledModuleManifest::new("MySignature")
    .with_optimizer("runtime")
    .compute_compiled_id();

// Convert to OTel spans
let spans = TraceContract::graph_to_spans(&graph, Some(&manifest), "trace-123");

// Publish to Nostr
let bridge = NostrBridge::generate();
let events = bridge.graph_to_events(&graph, Some(&manifest))?;
```
