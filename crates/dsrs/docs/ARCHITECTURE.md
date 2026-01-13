# dsrs Architecture

> **Status:** Accurate
> **Last verified:** d44f9cd3f
> **Source of truth:** `crates/dsrs/src/`
> **Doc owner:** dsrs
> **If this doc conflicts with code, code wins.**

## Terminology

Before diving into traits, understand the key terms used throughout dsrs. For the complete canonical vocabulary, see [GLOSSARY.md](../../../GLOSSARY.md).

| Term | Definition |
|------|------------|
| **Adapter** | Prompt formatting + output parsing (e.g., `ChatAdapter`). Does NOT validate or retry. |
| **Provider** | LM backend implementation (e.g., Codex, GPT-OSS, PylonLM) |
| **Lane** | Selection category for LM routing (local/cheap/premium) as used by `LaneMux` |
| **Dispatcher** | NIP-90 job submitter to the swarm (e.g., `SwarmDispatcher`) |
| **Module** | Callable unit that transforms Examples to Predictions |
| **Signature** | Input/output contract for a reasoning task |
| **Predictor** | Module that executes a signature |
| **Execution Runtime** | The layer that validates tool params, enforces retries, and runs tools. Distinct from adapters. |

## Core Traits

### MetaSignature

The foundation of declarative AI programming. Signatures define input/output contracts.

```rust
pub trait MetaSignature: Send + Sync {
    fn demos(&self) -> Vec<Example>;
    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()>;
    fn instruction(&self) -> String;
    fn input_fields(&self) -> Value;
    fn output_fields(&self) -> Value;
    fn update_instruction(&mut self, instruction: String) -> Result<()>;
    fn append(&mut self, name: &str, value: Value) -> Result<()>;
}
```

**Key concepts:**
- `instruction` - The system prompt / task description
- `input_fields` / `output_fields` - JSON schema of expected fields
- `demos` - Few-shot examples for in-context learning

### Module

Any callable unit that transforms Examples to Predictions.

```rust
#[async_trait]
pub trait Module: Send + Sync {
    async fn forward(&self, inputs: Example) -> Result<Prediction>;
}
```

### Predictor

Extends Module with configuration options.

```rust
pub trait Predictor: Module {
    async fn forward_with_config(
        &self,
        inputs: Example,
        lm: Arc<LM>,
    ) -> Result<Prediction>;
}
```

### Optimizable

Modules that can have their signatures optimized.

```rust
// File: crates/dsrs/src/core/module.rs

pub trait Optimizable {
    /// Get the module's signature for analysis.
    fn get_signature(&self) -> &dyn MetaSignature;

    /// Get nested optimizable parameters (for composite modules).
    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable>;

    /// Update the signature's instruction text.
    fn update_signature_instruction(&mut self, instruction: String) -> Result<()>;
}
```

### Adapter

Transforms signatures into LM-specific prompts.

```rust
// File: crates/dsrs/src/adapter/mod.rs

#[async_trait]
pub trait Adapter: Send + Sync + 'static {
    /// Format signature + inputs into Chat messages.
    fn format(&self, signature: &dyn MetaSignature, inputs: Example) -> Chat;

    /// Parse LLM response into structured fields.
    fn parse_response(
        &self,
        signature: &dyn MetaSignature,
        response: Message,
    ) -> HashMap<String, Value>;

    /// Execute LLM call with optional tools.
    async fn call(
        &self,
        lm: Arc<LM>,
        signature: &dyn MetaSignature,
        inputs: Example,
        tools: Vec<Arc<dyn ToolDyn>>,
    ) -> Result<Prediction>;

    /// Execute with streaming callback support.
    async fn call_streaming(
        &self,
        lm: Arc<LM>,
        signature: &dyn MetaSignature,
        inputs: Example,
        tools: Vec<Arc<dyn ToolDyn>>,
        callback: Option<&dyn DspyCallback>,
    ) -> Result<Prediction>;
}
```

## Data Types

### Example

Input data container with field access.

```rust
pub struct Example {
    pub data: HashMap<String, Value>,
    pub node_id: Option<usize>,  // For DAG tracing
}

// Macro for creation
let ex = example! {
    "question" => "What is 2+2?",
    "context" => "Math quiz"
};
```

### Prediction

Output data with LM usage tracking.

```rust
pub struct Prediction {
    pub data: HashMap<String, Value>,
    pub lm_usage: LmUsage,
    pub node_id: Option<usize>,
}

pub struct LmUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_msats: u64,  // For Pylon swarm
}
```

## Predictors

### Predict

Base predictor that wraps a signature.

```rust
let predictor = Predict::new(MySignature::default());
let result = predictor.forward(inputs).await?;
```

Features:
- Emits `on_module_start` / `on_module_end` callbacks
- Supports tool use via `add_tool()`
- Records to DAG trace when tracing enabled

### Refine

Meta-operator for retry/fallback patterns.

```rust
let refined = Refine::new(predictor)
    .with_max_retries(3)
    .with_threshold(0.8)
    .with_reward_fn(|_inputs, prediction| {
        // Score the prediction
        0.9
    });

let result = refined.forward(inputs).await?;
```

Features:
- Multiple rollouts with reward function
- Best-of-N selection
- Fallback LM support
- Failure attribution in traces

## Optimizers

### MIPROv2

Instruction optimization via LM-generated candidates.

```rust
let optimizer = MIPROv2::new()
    .with_num_candidates(10)
    .with_max_iterations(5);

optimizer.optimize(&mut module, trainset, evaluator).await?;
```

### GEPA

Genetic-evolutionary prompt optimization.

```rust
let optimizer = GEPA::new()
    .with_population_size(20)
    .with_generations(10);

optimizer.optimize(&mut module, trainset, evaluator).await?;
```

### Pareto

Multi-objective optimization with Pareto frontier.

```rust
let optimizer = ParetoOptimizer::new()
    .add_objective("accuracy", 1.0)
    .add_objective("cost", -0.5);  // Minimize
```

## DAG Tracing

Execution is recorded as a directed acyclic graph.

```rust
// Enable tracing
dsrs::trace::start_tracing();

// Execute module (nodes recorded automatically)
let result = predictor.forward(inputs).await?;

// Get graph
let graph = dsrs::trace::get_graph();
```

### Node Types

```rust
pub enum NodeType {
    Root,
    Predict { signature_name: String, signature: Arc<dyn MetaSignature> },
    Operator { name: String },
    Map { mapping: Vec<(String, (usize, String))> },
}
```

### Graph Structure

```rust
pub struct Graph {
    pub nodes: Vec<Node>,
}

pub struct Node {
    pub id: usize,
    pub node_type: NodeType,
    pub input: Option<Example>,
    pub output: Option<Prediction>,
    pub parents: Vec<usize>,
}
```

## Global Settings

```rust
// Configure LM
dsrs::configure(LM::new("codex-3-sonnet"));

// Configure with callback
dsrs::configure_with_callback(
    LM::new("codex-3-sonnet"),
    ChatAdapter,
    LoggingCallback::new(),
);

// Get current settings
let callback = dsrs::get_callback();
```

## Caching

Hybrid memory + disk caching via foyer.

```rust
let lm = LM::new("codex-3-sonnet")
    .with_cache(true);

// Cache is automatically used for repeated prompts
```

Cache location: `~/.cache/dsrs/`

## Error Handling

All operations return `anyhow::Result`:

```rust
let result = predictor.forward(inputs).await?;
```

Common errors:
- `SettingsNotConfigured` - Call `configure()` first
- `SignatureParseError` - Invalid signature format
- `LMError` - Provider-specific errors
