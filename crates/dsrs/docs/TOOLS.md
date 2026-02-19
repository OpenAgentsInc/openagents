# Tools

- **Status:** Accurate
- **Last verified:** d44f9cd3f
- **Source of truth:** `crates/rlm/src/tools/`, `crates/adjutant/src/tools.rs`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

Touching the outside world.

## Overview

**Tools** are how DSPy steps outside pure text generation:

- Code execution
- Search
- Databases
- APIs
- Filesystems
- Anything non-LLM

DSPy treats tools as **first-class capabilities**, not hacks glued onto prompts. The model decides *when* and *how* to use tools—guided by signatures and metrics.

- **Mental model:** "LLMs reason; tools act."

---

## CODING_AGENT_LOOP Tool Requirements (Additions)

*Italicized items are CODING_AGENT_LOOP additions or wiring targets.*

- *ToolCallSignature output is the single source of truth for tool name + params (no duplicate tool selection).*
- *Runtime validates tool params against JSON schema before execution and records ToolParamsSchemaMetric.*
- *Tool results are bounded for display, but hashes are computed over full output for receipts.*
- *Tool events must carry step context (step_id) into REPLAY/RECEIPT emission for traceability.*
- *Privacy defaults are enforced before any swarm dispatch or external emission.*

---

## RLM Tools

Tools for Recursive Language Model operations with provenance tracking.

### RlmTool Trait

```rust
// File: crates/rlm/src/tools/mod.rs

#[async_trait]
pub trait RlmTool: Send + Sync {
    /// Tool name for identification.
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// JSON schema for tool arguments.
    fn args_schema(&self) -> Value;

    /// Execute the tool with given arguments.
    async fn execute(&self, args: Value) -> Result<ToolOutput>;
}
```

### ToolConfig

```rust
// File: crates/rlm/src/tools/mod.rs

pub struct ToolConfig {
    /// Maximum results to return.
    pub max_results: usize,

    /// Maximum file size to read (bytes).
    pub max_file_size: u64,

    /// Timeout in milliseconds.
    pub timeout_ms: u64,

    /// Git commit to pin results to (for reproducibility).
    pub git_commit: Option<String>,
}
```

---

### GrepTool

Pattern search across repository files with provenance tracking.

```rust
// File: crates/rlm/src/tools/grep.rs

use rlm::tools::GrepTool;

let tool = GrepTool::new(repo_path, config);

// Execute search
let args = json!({
    "pattern": "fn\\s+authenticate",
    "globs": ["*.rs"],
    "max_hits": 50
});

let output = tool.execute(args).await?;

// Returns Vec<GrepHit> with:
// - path: File path
// - line: Line number
// - content: Matching line content
// - span_ref: SpanRef for provenance tracking
// - score: Relevance score
```

**Input Schema:**
| Field | Type | Description |
|-------|------|-------------|
| `pattern` | string | Regex pattern to search |
| `globs` | string[] | File patterns to include |
| `max_hits` | number | Maximum results |

**Output:** `Vec<GrepHit>` with SpanRef for each match

**Features:**
- Regex-based searching
- File size limits (respects `max_file_size`)
- Score-based ranking
- Preview context extraction
- Git commit pinning for reproducibility

---

### ReadLinesTool

Precise line range reading from files.

```rust
// File: crates/rlm/src/tools/read_lines.rs

use rlm::tools::ReadLinesTool;

let tool = ReadLinesTool::new(repo_path, config);

let args = json!({
    "path": "src/auth.rs",
    "start_line": 50,
    "end_line": 100
});

let output = tool.execute(args).await?;

// Returns ReadResult with:
// - content: File content
// - span_ref: SpanRef with exact byte offsets
// - total_lines: Total lines in file
// - truncated: Whether output was truncated
```

**Input Schema:**
| Field | Type | Description |
|-------|------|-------------|
| `path` | string | File path (relative to repo) |
| `start_line` | number | Start line (1-indexed) |
| `end_line` | number | End line (1-indexed) |

**Output:** `ReadResult` with content and SpanRef

**Features:**
- 1-indexed line numbers (like editors)
- Byte offset tracking for SpanRef
- File size validation
- Automatic truncation for large ranges

---

### SymbolsTool

Extract language symbols (functions, types, constants) via regex-based parsing.

```rust
// File: crates/rlm/src/tools/symbols.rs

use rlm::tools::SymbolsTool;

let tool = SymbolsTool::new(repo_path, config);

let args = json!({
    "path": "src/lib.rs"
});

let output = tool.execute(args).await?;

// Returns Vec<SymbolInfo> with:
// - name: Symbol name
// - kind: SymbolKind enum
// - signature: Full signature text
// - documentation: Doc comments
// - span_ref: Location in file
```

**SymbolKind Enum:**
```rust
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    Static,
    Type,
    Module,
    Class,      // For non-Rust languages
    Interface,  // For non-Rust languages
    Variable,
    Unknown,
}
```

**Supported Languages:**
- Rust (primary)
- Python, JavaScript, TypeScript
- Go, Java, C, C++
- And more via regex patterns

---

### ListFilesTool

Directory traversal with language detection.

```rust
// File: crates/rlm/src/tools/list_files.rs

use rlm::tools::ListFilesTool;

let tool = ListFilesTool::new(repo_path, config);

let args = json!({
    "path": "src/",
    "recursive": true,
    "include_hidden": false
});

let output = tool.execute(args).await?;

// Returns Vec<FileInfo> with:
// - path: File path
// - size: Size in bytes
// - language: Detected language
// - line_count: Number of lines
// - is_binary: Whether file is binary
```

**Language Detection:**
Maps file extensions to language identifiers:

| Extension | Language |
|-----------|----------|
| `.rs` | rust |
| `.py` | python |
| `.js` | javascript |
| `.ts` | typescript |
| `.go` | go |
| `.java` | java |
| `.c`, `.h` | c |
| `.cpp`, `.hpp` | cpp |
| `.rb` | ruby |
| `.php` | php |
| `.swift` | swift |
| `.kt` | kotlin |
| `.scala` | scala |
| `.cs` | csharp |
| `.fs` | fsharp |
| `.hs` | haskell |
| `.ml` | ocaml |
| `.ex`, `.exs` | elixir |
| `.erl` | erlang |
| `.clj` | clojure |
| `.lua` | lua |
| `.r` | r |
| `.jl` | julia |
| `.dart` | dart |
| `.zig` | zig |
| `.nim` | nim |
| `.v` | v |
| `.cr` | crystal |
| `.sh`, `.bash` | shell |
| `.ps1` | powershell |
| `.sql` | sql |
| `.html` | html |
| `.css` | css |
| `.json` | json |
| `.yaml`, `.yml` | yaml |
| `.toml` | toml |
| `.xml` | xml |
| `.md` | markdown |

---

## Adjutant ToolRegistry

High-level tool management for the Adjutant agent.

```rust
// File: crates/adjutant/src/tools.rs

use adjutant::tools::{ToolRegistry, AvailableTools, ToolOutput};

let registry = ToolRegistry::new(workspace_path);

// Execute a tool
let output = registry.execute(
    AvailableTools::Read,
    json!({ "path": "src/main.rs" })
).await?;

match output {
    ToolOutput::Success(content) => println!("{}", content),
    ToolOutput::Failure(error) => eprintln!("Error: {}", error),
}
```

### Available Tools

```rust
pub enum AvailableTools {
    Read,   // Read file contents
    Edit,   // Replace old_string with new_string
    Write,  // Write new files
    Bash,   // Execute bash commands
    Glob,   // Find files by pattern
    Grep,   // Search file contents
}
```

### Read Tool

```rust
let output = registry.execute(
    AvailableTools::Read,
    json!({ "path": "src/main.rs" })
).await?;
```

### Edit Tool

```rust
let output = registry.execute(
    AvailableTools::Edit,
    json!({
        "path": "src/main.rs",
        "old_string": "fn old_name(",
        "new_string": "fn new_name("
    })
).await?;
```

### Write Tool

```rust
let output = registry.execute(
    AvailableTools::Write,
    json!({
        "path": "src/new_file.rs",
        "content": "// New file content"
    })
).await?;
```

### Bash Tool

```rust
let output = registry.execute(
    AvailableTools::Bash,
    json!({ "command": "cargo test" })
).await?;
```

### Glob Tool

```rust
let output = registry.execute(
    AvailableTools::Glob,
    json!({ "pattern": "**/*.rs" })
).await?;

// Returns list of matching file paths
// Ignores: .git, target, node_modules
```

### Grep Tool

```rust
let output = registry.execute(
    AvailableTools::Grep,
    json!({
        "pattern": "TODO",
        "path": "src/"
    })
).await?;

// Uses ripgrep if available, falls back to grep
```

---

## Tool Integration with Predict

Tools can be attached to `Predict` modules for LLM-driven tool use.

```rust
// File: crates/dsrs/src/predictors/predict.rs

use dsrs::Predict;
use rig::tool::ToolDyn;

// Create tools
let tools: Vec<Arc<dyn ToolDyn>> = vec![
    Arc::new(MyTool::new()),
    Arc::new(AnotherTool::new()),
];

// Attach to predictor
let predictor = Predict::new(signature)
    .with_tools(tools);

// During execution, the LLM can call tools
let prediction = predictor.forward(example).await?;
```

---

## External Tool Protocols

### Codex-MCP Protocol

Model Context Protocol for tool communication.

```rust
// File: crates/codex-mcp/src/protocol.rs

/// Tool definition
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,  // JSON Schema
}

/// Tool list response
pub struct ToolsListResult {
    pub tools: Vec<Tool>,
}

/// Tool call parameters
pub struct ToolCallParams {
    pub name: String,
    pub arguments: Value,  // JSON
}

/// Tool call result
pub struct ToolCallResult {
    pub content: String,   // Text output
    pub is_error: bool,
}
```

### GPT-OSS Tool Integration

OpenAI-compatible tool format.

```rust
// File: crates/gpt-oss/src/types.rs

/// Standard OpenAI tool definition
pub struct GptOssToolDefinition {
    pub r#type: String,  // "function"
    pub function: GptOssToolFunction,
}

/// Function specification
pub struct GptOssToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,  // JSON Schema
}

/// Tool call from model
pub struct GptOssToolCall {
    pub id: String,
    pub r#type: String,
    pub function: GptOssToolCallFunction,
}

pub struct GptOssToolCallFunction {
    pub name: String,
    pub arguments: String,  // JSON string
}
```

### Harmony Tool Spec

```rust
// File: crates/gpt-oss/src/harmony.rs

pub struct HarmonyToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,  // JSON Schema
}
```

---

## Tool Execution Flow

```
User Request
    │
    ▼
┌─────────────────┐
│ Predict Module  │
│ (with tools)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ChatAdapter    │──────┐
│ (formats tools) │      │
└────────┬────────┘      │
         │               │
         ▼               │ Tool calls
┌─────────────────┐      │
│      LLM        │◄─────┘
│  (decides use)  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Tool Execution  │
│ (GrepTool, etc) │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ ChatAdapter     │
│ (parses result) │
└────────┬────────┘
         │
         ▼
    Prediction
```

---

**CODING_AGENT_LOOP note:** Insert runtime validation + receipt emission between ToolCallSignature output and tool execution, and emit ToolCall/ToolResult replay events with hashes and step_id.

## Tool Schema Validation

Every tool call must validate parameters against a JSON schema before execution.

### Validation Flow

```
ToolCallSignature Output
         │
         ▼
┌─────────────────────┐
│ Schema Validation   │ ← Validates params against tool's args_schema()
└─────────┬───────────┘
          │
          ├── PASS: Continue to execution
          │
          └── FAIL: Return ToolParamsSchemaMetric = 0.0
                    │
                    ▼
              Retry with feedback
```

### Tool Schema Example

```rust
impl RlmTool for GrepTool {
    fn args_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "globs": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File patterns to include"
                },
                "max_hits": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 50
                }
            }
        })
    }
}
```

### Validation Implementation

```rust
// File: crates/dsrs/src/tools/validation.rs

use jsonschema::JSONSchema;

pub fn validate_tool_params(
    tool: &dyn RlmTool,
    params: &Value,
) -> Result<(), ValidationError> {
    let schema = tool.args_schema();
    let compiled = JSONSchema::compile(&schema)?;

    if let Err(errors) = compiled.validate(params) {
        let error_msgs: Vec<String> = errors
            .map(|e| format!("{}: {}", e.instance_path, e))
            .collect();
        return Err(ValidationError::SchemaViolation(error_msgs));
    }

    Ok(())
}
```

### Retry Behavior

When validation fails, the execution runtime re-prompts with error context:

```rust
// Execution runtime retry logic (not adapter responsibility)
if let Err(validation_error) = validate_tool_params(&tool, &params) {
    // Append error to execution_history
    let retry_context = format!(
        "Previous tool call failed validation: {}\nPlease fix the parameters.",
        validation_error
    );

    // Re-invoke ToolCallSignature with updated context
    let retried = tool_call_predictor.forward(example_with_retry_context).await?;
}
```

### Schema Validation Metrics

```rust
/// Proxy metric: did tool params pass schema validation?
pub struct ToolParamsSchemaMetric;

impl Metric for ToolParamsSchemaMetric {
    fn tier(&self) -> MetricTier { MetricTier::Proxy }

    fn score(&self, _example: &Example, prediction: &Prediction) -> f32 {
        let params = prediction.get("params", None);
        let tool_name = prediction.get("tool", None).as_str().unwrap_or("");

        if let Some(tool) = get_tool_by_name(tool_name) {
            match validate_tool_params(tool, &params) {
                Ok(_) => 1.0,
                Err(_) => 0.0,
            }
        } else {
            0.0 // Unknown tool
        }
    }
}
```

---

## Receipt Hooks

Every tool execution emits a receipt for audit and reproducibility.

### Receipt Structure

```rust
// File: crates/dsrs/src/tools/receipt.rs

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolReceipt {
    /// Unique receipt ID
    pub id: String,

    /// Tool that was executed
    pub tool: String,

    /// Deterministic hash of input parameters
    pub params_hash: String,

    /// Deterministic hash of output
    pub output_hash: String,

    /// Wall-clock latency in milliseconds
    pub latency_ms: u64,

    /// Side effects produced (files written, commands run)
    pub side_effects: Vec<SideEffect>,

    /// Timestamp of execution
    pub timestamp: DateTime<Utc>,

    /// Git commit at execution time (if available)
    pub git_commit: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SideEffect {
    FileWritten { path: String, hash: String },
    FileDeleted { path: String },
    CommandExecuted { command: String, exit_code: i32 },
}
```

### Deterministic Hashing

```rust
// Canonical JSON serialization for deterministic hashing
pub fn canonical_hash(value: &Value) -> String {
    // Sort object keys, normalize whitespace
    let canonical = serde_json::to_string(&canonicalize(value)).unwrap();
    format!("sha256:{}", sha256::digest(canonical.as_bytes()))
}

// Usage
let params_hash = canonical_hash(&params);
let output_hash = canonical_hash(&output);
```

### Receipt Emission

```rust
impl ToolRegistry {
    pub async fn execute_with_receipt(
        &self,
        tool: AvailableTools,
        params: Value,
    ) -> Result<(ToolOutput, ToolReceipt)> {
        let start = Instant::now();
        let params_hash = canonical_hash(&params);

        // Execute tool
        let output = self.execute(tool, params.clone()).await?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let output_hash = canonical_hash(&serde_json::to_value(&output)?);

        // Collect side effects
        let side_effects = self.collect_side_effects(&tool, &params);

        let receipt = ToolReceipt {
            id: uuid::Uuid::new_v4().to_string(),
            tool: tool.to_string(),
            params_hash,
            output_hash,
            latency_ms,
            side_effects,
            timestamp: Utc::now(),
            git_commit: get_current_git_commit(),
        };

        // Emit to REPLAY.jsonl
        self.emit_to_replay(&receipt).await?;

        Ok((output, receipt))
    }
}
```

### Linking Receipts to RECEIPT.json

```rust
// Final session RECEIPT.json includes all tool receipts
pub struct SessionReceipt {
    pub session_id: String,
    pub tool_calls: Vec<ToolReceipt>,  // All receipts from this session
    pub verification: VerificationResult,
    pub final_confidence: f32,
}

// Tool receipts are linked by ID
impl SessionReceipt {
    pub fn get_tool_receipt(&self, receipt_id: &str) -> Option<&ToolReceipt> {
        self.tool_calls.iter().find(|r| r.id == receipt_id)
    }

    pub fn total_latency_ms(&self) -> u64 {
        self.tool_calls.iter().map(|r| r.latency_ms).sum()
    }

    pub fn side_effects_summary(&self) -> Vec<SideEffect> {
        self.tool_calls.iter()
            .flat_map(|r| r.side_effects.clone())
            .collect()
    }
}
```

### Replay Verification

```rust
// Verify a replay matches the original receipt
pub fn verify_replay(
    replay_events: &[ReplayEvent],
    original_receipt: &SessionReceipt,
) -> VerificationResult {
    let mut mismatches = Vec::new();

    for (event, receipt) in replay_events.iter().zip(&original_receipt.tool_calls) {
        if event.output_hash != receipt.output_hash {
            mismatches.push(format!(
                "Tool {} output mismatch: expected {}, got {}",
                receipt.tool, receipt.output_hash, event.output_hash
            ));
        }
    }

    VerificationResult {
        success: mismatches.is_empty(),
        mismatches,
    }
}
```

---

## Tool Index

| Tool | Location | Type | Purpose |
|------|----------|------|---------|
| **RLM Tools** |
| GrepTool | `rlm/src/tools/grep.rs` | Search | Pattern search with SpanRef |
| ReadLinesTool | `rlm/src/tools/read_lines.rs` | Read | Line range reading |
| SymbolsTool | `rlm/src/tools/symbols.rs` | Analysis | Symbol extraction |
| ListFilesTool | `rlm/src/tools/list_files.rs` | Discovery | Directory traversal |
| **Adjutant Tools** |
| Read | `adjutant/src/tools.rs` | Read | File contents |
| Edit | `adjutant/src/tools.rs` | Write | String replacement |
| Write | `adjutant/src/tools.rs` | Write | New file creation |
| Bash | `adjutant/src/tools.rs` | Execute | Shell commands |
| Glob | `adjutant/src/tools.rs` | Search | File pattern matching |
| Grep | `adjutant/src/tools.rs` | Search | Content search |
| **Protocols** |
| Codex-MCP | `codex-mcp/src/protocol.rs` | Protocol | MCP tool interface |
| GPT-OSS | `gpt-oss/src/types.rs` | Protocol | OpenAI tool format |
| Harmony | `gpt-oss/src/harmony.rs` | Protocol | Harmony tool spec |

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **Signatures** | Tools can be called during signature execution |
| **Modules** | Predict module supports `with_tools()` |
| **Adapters** | ChatAdapter formats tool calls for LLM |
| **Optimizers** | Tool usage patterns can be optimized |
| **Metrics** | Tool execution results feed into metrics |
