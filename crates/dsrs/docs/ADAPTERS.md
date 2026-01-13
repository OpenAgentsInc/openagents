# Adapters

How intent becomes prompts.

## Overview

Adapters control **serialization**:

- How inputs are turned into prompts
- How outputs are parsed back into structure

This is where formats like JSON, XML, and custom schemas live. Adapters let you change *representation* without changing *meaning*.

> **Mental model:** "Same intelligence, different surface syntax."

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
// Creates NIP-90 kind:5102 jobs for sandbox execution

pub struct Nip90SandboxJob {
    pub kind: u16,           // 5102
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

### DspyCallback Trait

```rust
// File: crates/dsrs/src/callbacks.rs

#[async_trait]
pub trait DspyCallback: Send + Sync {
    /// Called when module execution starts.
    async fn on_module_start(&self, module_name: &str);

    /// Called when module execution ends.
    async fn on_module_end(&self, module_name: &str, prediction: &Prediction);

    /// Called for each generated token (streaming).
    async fn on_token(&self, token: &str);

    /// Called when a tool is executed.
    async fn on_tool_execution(&self, tool_name: &str, args: &Value, result: &Value);
}
```

### Usage with ChatAdapter

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

ChatAdapter validates tool parameters before execution and retries on failure.

### Validation Flow

```
ToolCallSignature
         │
         ▼
┌─────────────────────┐
│ ChatAdapter.call()  │
│                     │
│  1. Parse params    │
│  2. Validate schema │
│  3. Execute or retry│
└─────────┬───────────┘
          │
          ├── Schema valid → Execute tool
          │
          └── Schema invalid → Retry with error context
                               (up to 3 attempts)
```

### Validation Implementation

```rust
impl ChatAdapter {
    pub async fn call_with_tool_validation(
        signature: &dyn MetaSignature,
        inputs: Example,
        lm: Arc<LM>,
        tools: Vec<Arc<dyn ToolDyn>>,
        max_retries: usize,
    ) -> Result<Prediction> {
        let mut attempt = 0;
        let mut last_error = None;

        while attempt < max_retries {
            let prediction = Self::call(signature, inputs.clone(), lm.clone(), Some(tools.clone())).await?;

            // Extract tool call from prediction
            let tool_name = prediction.get("tool", None).as_str().unwrap_or("");
            let params = prediction.get("params", None);

            // Validate against tool schema
            if let Some(tool) = tools.iter().find(|t| t.name() == tool_name) {
                match validate_tool_params(tool.as_ref(), &params) {
                    Ok(_) => return Ok(prediction),
                    Err(e) => {
                        last_error = Some(e.clone());
                        // Append error to inputs for retry
                        inputs = inputs.with_context(format!(
                            "Previous attempt failed: {}. Please fix the parameters.",
                            e
                        ));
                    }
                }
            }

            attempt += 1;
        }

        Err(anyhow!("Tool validation failed after {} attempts: {:?}", max_retries, last_error))
    }
}
```

### Error Context Format

```
Previous tool call failed validation:
- Field 'pattern': required field missing
- Field 'max_hits': value 5000 exceeds maximum 1000

Please fix the parameters and try again.
```

---

## Replay Serialization Format

Canonical JSONL event format for REPLAY.jsonl files.

### Event Types

```rust
// File: crates/dsrs/src/adapter/replay.rs

#[derive(Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum ReplayEvent {
    /// Session started
    SessionStart {
        t: DateTime<Utc>,
        session_id: String,
        issue_number: Option<i64>,
        policy_version: String,
    },

    /// Plan generated
    PlanStart {
        t: DateTime<Utc>,
        plan_hash: String,
        step_count: usize,
    },

    /// Tool call initiated
    ToolCall {
        t: DateTime<Utc>,
        id: String,
        tool: String,
        params: Value,
        params_hash: String,
        step_id: String,
    },

    /// Tool result received
    ToolResult {
        t: DateTime<Utc>,
        id: String,
        output_hash: String,
        exit_code: Option<i32>,
        step_utility: f32,
        latency_ms: u64,
    },

    /// Step completed
    StepComplete {
        t: DateTime<Utc>,
        step_id: String,
        status: StepStatus,
        iterations: u8,
    },

    /// Verification run
    Verification {
        t: DateTime<Utc>,
        commands: Vec<String>,
        exit_codes: Vec<i32>,
        verification_delta: i32,
    },

    /// Session ended
    SessionEnd {
        t: DateTime<Utc>,
        status: SessionStatus,
        confidence: f32,
        total_tool_calls: usize,
        total_latency_ms: u64,
    },
}

#[derive(Serialize, Deserialize)]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
    MaxIterationsReached,
}

#[derive(Serialize, Deserialize)]
pub enum SessionStatus {
    Success,
    Failed,
    Cancelled,
    Timeout,
}
```

### Canonical Serialization

```rust
impl ReplayEvent {
    /// Serialize to canonical JSONL line.
    /// - Keys sorted alphabetically
    /// - No extra whitespace
    /// - Deterministic output
    pub fn to_canonical_json(&self) -> String {
        // Use serde_json with sorted keys
        let value = serde_json::to_value(self).unwrap();
        canonical_serialize(&value)
    }
}

fn canonical_serialize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut sorted: Vec<_> = map.iter().collect();
            sorted.sort_by_key(|(k, _)| *k);
            let pairs: Vec<String> = sorted
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k, canonical_serialize(v)))
                .collect();
            format!("{{{}}}", pairs.join(","))
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_serialize).collect();
            format!("[{}]", items.join(","))
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}
```

### Example REPLAY.jsonl

```jsonl
{"event":"SessionStart","issue_number":42,"policy_version":"v1.2.3","session_id":"sess_abc123","t":"2025-01-13T10:00:00Z"}
{"event":"PlanStart","plan_hash":"sha256:abc123...","step_count":3,"t":"2025-01-13T10:00:01Z"}
{"event":"ToolCall","id":"tc_001","params":{"path":"src/auth.rs"},"params_hash":"sha256:def456...","step_id":"step-1","t":"2025-01-13T10:00:02Z","tool":"file_read"}
{"event":"ToolResult","exit_code":null,"id":"tc_001","latency_ms":45,"output_hash":"sha256:ghi789...","step_utility":0.8,"t":"2025-01-13T10:00:02Z"}
{"event":"StepComplete","iterations":1,"status":"Success","step_id":"step-1","t":"2025-01-13T10:00:03Z"}
{"event":"Verification","commands":["cargo check","cargo test"],"exit_codes":[0,0],"t":"2025-01-13T10:05:30Z","verification_delta":3}
{"event":"SessionEnd","confidence":0.92,"status":"Success","t":"2025-01-13T10:05:32Z","total_latency_ms":5320,"total_tool_calls":5}
```

### Replay Reader

```rust
// File: crates/dsrs/src/adapter/replay.rs

pub struct ReplayReader {
    events: Vec<ReplayEvent>,
}

impl ReplayReader {
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let events: Vec<ReplayEvent> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();
        Ok(Self { events })
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ReplayEvent> {
        self.events.iter().filter(|e| matches!(e, ReplayEvent::ToolCall { .. }))
    }

    pub fn total_latency_ms(&self) -> u64 {
        self.events.iter()
            .filter_map(|e| match e {
                ReplayEvent::ToolResult { latency_ms, .. } => Some(latency_ms),
                _ => None,
            })
            .sum()
    }

    pub fn average_step_utility(&self) -> f32 {
        let utilities: Vec<f32> = self.events.iter()
            .filter_map(|e| match e {
                ReplayEvent::ToolResult { step_utility, .. } => Some(*step_utility),
                _ => None,
            })
            .collect();
        if utilities.is_empty() { 0.0 } else {
            utilities.iter().sum::<f32>() / utilities.len() as f32
        }
    }
}
```

---

## Verification Mode

PylonSandboxProvider and SwarmDispatcher support verification mode for deterministic replay.

### PylonSandboxProvider Verification

```rust
impl PylonSandboxProvider {
    /// Run with verification mode enabled.
    /// Returns job hash for reproducibility verification.
    pub async fn run_verified(
        &self,
        commands: &[&str],
    ) -> Result<(SandboxRunResponse, JobVerification)> {
        // Compute deterministic job hash
        let job_hash = self.compute_job_hash(commands);

        // Execute
        let response = self.run_commands(commands).await?;

        // Build verification record
        let verification = JobVerification {
            job_hash,
            input_hash: canonical_hash(&json!({ "commands": commands })),
            output_hash: response.stdout_hash.clone(),
            stderr_hash: response.stderr_hash.clone(),
            exit_codes: response.exit_codes.clone(),
            provenance: response.provenance.clone(),
        };

        Ok((response, verification))
    }

    fn compute_job_hash(&self, commands: &[&str]) -> String {
        let job_spec = json!({
            "commands": commands,
            "profile": self.profile,
            "timeout_secs": self.timeout_secs,
            "network_policy": self.network_policy,
            "image_digest": self.image_digest,
        });
        canonical_hash(&job_spec)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobVerification {
    pub job_hash: String,
    pub input_hash: String,
    pub output_hash: String,
    pub stderr_hash: String,
    pub exit_codes: Vec<i32>,
    pub provenance: SandboxProvenance,
}
```

### SwarmDispatcher Job Hashing

```rust
impl SwarmDispatcher {
    /// Dispatch with job hash for verification.
    pub async fn dispatch_verified(
        &self,
        input: Value,
    ) -> Result<(SwarmJobResult, JobHash)> {
        let job_hash = self.compute_job_hash(&input);

        let result = self.dispatch(input.clone()).await?;

        Ok((result, JobHash {
            input_hash: canonical_hash(&input),
            job_type: self.job_type.clone(),
            provider_pubkey: result.provider_pubkey.clone(),
            result_hash: canonical_hash(&result.output),
        }))
    }

    fn compute_job_hash(&self, input: &Value) -> String {
        let job_spec = json!({
            "job_type": self.job_type,
            "input": input,
            "privacy_policy": self.privacy_policy,
        });
        canonical_hash(&job_spec)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobHash {
    pub input_hash: String,
    pub job_type: SwarmJobType,
    pub provider_pubkey: Option<String>,
    pub result_hash: String,
}
```

### Verification in RECEIPT.json

```json
{
  "session_id": "sess_abc123",
  "sandbox_jobs": [
    {
      "job_hash": "sha256:abc123...",
      "input_hash": "sha256:def456...",
      "output_hash": "sha256:ghi789...",
      "exit_codes": [0, 0],
      "provider": "pylon-node-1"
    }
  ],
  "swarm_jobs": [
    {
      "job_hash": "sha256:jkl012...",
      "job_type": "ChunkAnalysis",
      "provider_pubkey": "npub1...",
      "result_hash": "sha256:mno345..."
    }
  ]
}
```

---

## Adapter Index

| Adapter | Location | Purpose |
|---------|----------|---------|
| ChatAdapter | `dsrs/src/adapter/chat.rs` | Prompt formatting & output parsing |
| PylonSandboxProvider | `dsrs/src/adapter/pylon_sandbox.rs` | Sandboxed execution (NIP-90) |
| SwarmDispatcher | `dsrs/src/adapter/swarm_dispatch.rs` | Swarm job dispatch (NIP-90) |
| ReplayWriter | `dsrs/src/adapter/replay.rs` | REPLAY.jsonl emission |
| ReplayReader | `dsrs/src/adapter/replay.rs` | REPLAY.jsonl parsing |

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **Signatures** | ChatAdapter formats signatures into prompts |
| **Modules** | Predict uses ChatAdapter for LLM calls |
| **Tools** | ChatAdapter handles tool call formatting |
| **Optimizers** | Adapters serialize optimized prompts |
| **Metrics** | Parsed outputs feed into metric evaluation |
