# Tools

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

> **Mental model:** "LLMs reason; tools act."

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
