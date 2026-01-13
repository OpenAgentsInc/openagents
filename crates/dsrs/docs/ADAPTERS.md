# Adapters

- **Status:** Needs audit
- **Last verified:** d44f9cd3f
- **Source of truth:** `crates/dsrs/src/adapter/`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

How intent becomes prompts.

## Overview

Adapters control **serialization**:

- How inputs are turned into prompts
- How outputs are parsed back into structure

This is where formats like JSON, XML, and custom schemas live. Adapters let you change *representation* without changing *meaning*.

- **Mental model:** "Same intelligence, different surface syntax."

---

## ChatAdapter

The primary adapter for converting DSPy signatures into LLM chat messages.

```rust
// File: crates/dsrs/src/adapter/chat.rs

use dsrs::adapter::ChatAdapter;
```

### Format Flow

```
Signature + Example
        │
        ▼
┌───────────────────┐
│   format()        │
│                   │
│ 1. System message │  ← Field descriptions + instructions
│ 2. Demo examples  │  ← Few-shot examples
│ 3. User message   │  ← Input field values
│                   │
└────────┬──────────┘
         │
         ▼
    Chat Messages
         │
         ▼
┌───────────────────┐
│      LLM          │
└────────┬──────────┘
         │
         ▼
    Raw Response
         │
         ▼
┌───────────────────┐
│ parse_response()  │
│                   │
│ Extract fields:   │
│ [[ ## field ## ]] │
│                   │
└────────┬──────────┘
         │
         ▼
    Prediction
```

### Key Methods

```rust
impl ChatAdapter {
    /// Format signature + inputs into chat messages.
    pub fn format(
        signature: &dyn MetaSignature,
        inputs: &Example,
    ) -> Vec<ChatMessage>;

    /// Parse LLM response into structured prediction.
    pub fn parse_response(
        signature: &dyn MetaSignature,
        response: &str,
    ) -> Prediction;

    /// Execute LLM call with optional tools.
    pub async fn call(
        signature: &dyn MetaSignature,
        inputs: Example,
        lm: Arc<LM>,
        tools: Option<Vec<Arc<dyn ToolDyn>>>,
    ) -> Result<Prediction>;

    /// Execute with streaming callbacks.
    pub async fn call_streaming(
        signature: &dyn MetaSignature,
        inputs: Example,
        lm: Arc<LM>,
        tools: Option<Vec<Arc<dyn ToolDyn>>>,
        callback: Box<dyn DspyCallback>,
    ) -> Result<Prediction>;
}
```

### System Message Format

The system message includes:

1. **Signature instruction** - The task description
2. **Input field descriptions** - What each input represents
3. **Output field descriptions** - What each output should contain
4. **Format instructions** - How to structure the response

```
You are a helpful assistant.

## Task
{signature.instruction}

## Input Fields
- {field_name}: {field_description}
...

## Output Fields
- {field_name}: {field_description}
...

## Response Format
Respond with each output field labeled as:
[[ ## field_name ## ]]
field_value
```

### Demo Example Format

Few-shot examples are formatted as:

```
## Example {n}

### Inputs
- {field}: {value}
...

### Outputs
[[ ## field ## ]]
{value}
...
```

### Output Parsing

The adapter extracts structured fields using markers:

```rust
// Response from LLM:
// [[ ## answer ## ]]
// The capital of France is Paris.
// [[ ## confidence ## ]]
// 0.95

let prediction = ChatAdapter::parse_response(&signature, &response);
// prediction.get("answer") => "The capital of France is Paris."
// prediction.get("confidence") => 0.95
```

### JSON Extraction

For fields expecting JSON, the adapter uses robust extraction:

```rust
// File: crates/dsrs/src/adapter/chat.rs

fn extract_json_from_text(text: &str, expected_type: &str) -> Value {
    match expected_type {
        "array" => {
            // Find [...] pattern
            // Handle nested brackets
            // Parse as JSON array
        }
        "object" => {
            // Find {...} pattern
            // Handle nested braces
            // Parse as JSON object
        }
        "number" => {
            // Extract numeric value
        }
        _ => {
            // Return raw string
        }
    }
}
```

---

## PylonSandboxProvider

Adapter for CPU-intensive operations in sandboxed environments via NIP-90.

```rust
// File: crates/dsrs/src/adapter/pylon_sandbox.rs

use dsrs::adapter::PylonSandboxProvider;

let provider = PylonSandboxProvider::builder()
    .profile(SandboxProfile::Medium)
    .timeout_secs(120)
    .network_policy(NetworkPolicy::Isolated)
    .build();

// Run commands in sandbox
let response = provider.run_commands(&[
    "cargo build",
    "cargo test",
]).await?;
```

### SandboxProfile

```rust
pub enum SandboxProfile {
    Small,   // 1 vCPU, 1GB RAM, 5GB disk
    Medium,  // 2 vCPU, 4GB RAM, 8GB disk
    Large,   // 4 vCPU, 8GB RAM, 10GB disk
    Custom { vcpus: u8, ram_gb: u8, disk_gb: u8 },
}
```

### SandboxRunResponse

```rust
pub struct SandboxRunResponse {
    /// Exit codes for each command.
    pub exit_codes: Vec<i32>,

    /// SHA256 hash of stdout.
    pub stdout_hash: String,

    /// SHA256 hash of stderr.
    pub stderr_hash: String,

    /// Execution provenance.
    pub provenance: SandboxProvenance,

    /// Actual stdout content (if not too large).
    pub stdout: Option<String>,

    /// Actual stderr content (if not too large).
    pub stderr: Option<String>,
}
```

### NIP-90 Integration

```rust
// Submits NIP-90 sandbox execution jobs (kind depends on schema; see PROTOCOL_SURFACE)

pub struct Nip90SandboxJob {
    pub schema_id: String,   // "oa.sandbox_run.v1"
    pub commands: Vec<String>,
    pub profile: SandboxProfile,
    pub timeout_secs: u64,
    pub network_policy: NetworkPolicy,
    pub image_digest: Option<String>,
}
```

### Modes

```rust
impl PylonSandboxProvider {
    /// Online mode: Submit actual jobs to Pylon network.
    pub fn online(config: PylonConfig) -> Self;

    /// Offline mode: Return mock responses (for testing).
    pub fn offline() -> Self;
}
```

---

## SwarmDispatcher

Dispatches jobs to the OpenAgents swarm via Nostr NIP-90 protocol.

```rust
// File: crates/dsrs/src/adapter/swarm_dispatch.rs

use dsrs::adapter::SwarmDispatcher;

let dispatcher = SwarmDispatcher::builder()
    .job_type(SwarmJobType::ChunkAnalysis)
    .privacy_policy(PrivacyPolicy::TrustedProviders)
    .build();

let result = dispatcher.dispatch(job_input).await?;
```

### Supported Job Types

```rust
pub enum SwarmJobType {
    /// Code chunk analysis.
    ChunkAnalysis,

    /// Candidate ranking/reranking.
    Rerank,

    /// Sandboxed command execution.
    SandboxRun,

    /// Vector embedding generation.
    Embeddings,
}
```

### Privacy Policy

```rust
pub enum PrivacyPolicy {
    /// Any provider can process.
    Public,

    /// Only trusted providers.
    TrustedProviders,

    /// Content validation before dispatch.
    ValidatedContent,

    /// Custom validation function.
    Custom(Box<dyn Fn(&Value) -> bool>),
}
```

### Job Dispatch

```rust
impl SwarmDispatcher {
    /// Dispatch a single job.
    pub async fn dispatch(&self, input: Value) -> Result<SwarmJobResult>;

    /// Dispatch multiple jobs in batch.
    pub async fn dispatch_batch(
        &self,
        inputs: Vec<Value>,
    ) -> Result<Vec<SwarmJobResult>>;

    /// Set privacy policy.
    pub fn with_privacy_policy(self, policy: PrivacyPolicy) -> Self;
}
```

### DVM Client Integration

```rust
// For actual job submission
let dispatcher = SwarmDispatcher::builder()
    .dvm_client(dvm_client)
    .online()
    .build();

// For testing
let dispatcher = SwarmDispatcher::offline();
```

---

## Output Parsing Utilities

### extract_json_from_text

Robust JSON extraction from potentially noisy LLM output.

```rust
// File: crates/dsrs/src/adapter/chat.rs

// Extract array from text
let text = "Here are the results: [1, 2, 3] and more text";
let json = extract_json_from_text(text, "array");
// => [1, 2, 3]

// Extract object from text
let text = "The config is: {\"key\": \"value\"} as shown above";
let json = extract_json_from_text(text, "object");
// => {"key": "value"}

// Handle nested structures
let text = "Result: [[1, 2], [3, 4]]";
let json = extract_json_from_text(text, "array");
// => [[1, 2], [3, 4]]
```

### Field Marker Parsing

```rust
// Parse field markers from LLM response
let response = r#"
[[ ## summary ## ]]
This is the summary text.

[[ ## confidence ## ]]
0.95

[[ ## items ## ]]
["item1", "item2", "item3"]
"#;

let prediction = ChatAdapter::parse_response(&signature, response);
// prediction.get("summary") => "This is the summary text."
// prediction.get("confidence") => 0.95
// prediction.get("items") => ["item1", "item2", "item3"]
```

---

## Serialization Utilities

### JSONL Loading

```rust
// File: crates/dsrs/src/data/serialize.rs

use dsrs::data::serialize::load_jsonl;

// Load examples from JSONL file (parallel parsing)
let examples = load_jsonl("training_data.jsonl").await?;
```

### JSONL Saving

```rust
use dsrs::data::serialize::save_examples_as_jsonl;

// Save examples to JSONL file
save_examples_as_jsonl(&examples, "output.jsonl").await?;
```

### Value Iteration

```rust
// File: crates/dsrs/src/utils/serde_utils.rs

use dsrs::utils::serde_utils::get_iter_from_value;

// Iterate over JSON object fields
let value = json!({"a": 1, "b": 2});
for (key, val) in get_iter_from_value(&value) {
    println!("{}: {}", key, val);
}
```

---

## Callback Infrastructure

> See [CALLBACKS.md](CALLBACKS.md) for the canonical callback API.

ChatAdapter supports streaming callbacks for real-time observability:

```rust
let callback = Box::new(MyCallback::new());

let prediction = ChatAdapter::call_streaming(
    &signature,
    inputs,
    lm,
    tools,
    callback,
).await?;
```

---

## Tool Params Validation

> See [TOOLS.md](TOOLS.md#tool-schema-validation) for the canonical validation API.

Tool parameters are validated against tool schemas before tool execution. Adapters only serialize/parse; validation is performed by the execution runtime. If validation fails, the runtime retries with error context (up to 3 attempts).

---

## Replay & Artifacts

> See [REPLAY.md](REPLAY.md) for the canonical REPLAY.jsonl format.
> See [ARTIFACTS.md](ARTIFACTS.md) for the full MVP artifact specification.

The session executor (not adapters) emits replay events during execution for audit and reproducibility. Adapters only handle prompt formatting and response parsing.

---

## Adapter Index

| Adapter | Location | Purpose |
|---------|----------|---------|
| ChatAdapter | `dsrs/src/adapter/chat.rs` | Prompt formatting & output parsing |
| PylonSandboxProvider | `dsrs/src/adapter/pylon_sandbox.rs` | Sandboxed execution (NIP-90) |
| SwarmDispatcher | `dsrs/src/adapter/swarm_dispatch.rs` | Swarm job dispatch (NIP-90) |

- **Note:** Replay emission is handled by the session executor, not adapters. See [REPLAY.md](REPLAY.md) and [ARTIFACTS.md](ARTIFACTS.md).

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **Signatures** | ChatAdapter formats signatures into prompts |
| **Modules** | Predict uses ChatAdapter for LLM calls |
| **Tools** | ChatAdapter handles tool call formatting |
| **Optimizers** | Adapters serialize optimized prompts |
| **Metrics** | Parsed outputs feed into metric evaluation |
