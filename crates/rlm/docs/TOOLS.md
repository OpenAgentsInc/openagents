# RLM Environment Tools

The tools module exposes the RLM environment (repository traversal, file reading, search) to DSPy predictors. Each tool returns SpanRefs for provenance tracking, enabling precise citation of evidence sources.

## Overview

| Tool | Purpose | Returns |
|------|---------|---------|
| `GrepTool` | Pattern search across files | `Vec<GrepHit>` with SpanRefs |
| `ReadLinesTool` | Read specific line ranges | `ReadResult` with SpanRef |
| `ListFilesTool` | Directory traversal | `Vec<FileInfo>` with metadata |
| `SymbolsTool` | Extract code symbols | `Vec<SymbolInfo>` with SpanRefs |

## Common Types

### RlmTool Trait

All tools implement this trait for JSON-based execution:

```rust
#[async_trait]
pub trait RlmTool: Send + Sync {
    /// Tool name for identification
    fn name(&self) -> &str;

    /// Human-readable description for LLM understanding
    fn description(&self) -> &str;

    /// JSON schema for the tool's arguments
    fn args_schema(&self) -> Value;

    /// Execute the tool with JSON arguments
    async fn execute(&self, args: Value) -> ToolResult<Value>;
}
```

### ToolConfig

Configuration for tool execution limits:

```rust
pub struct ToolConfig {
    /// Maximum number of results to return (default: 100)
    pub max_results: usize,

    /// Maximum file size to process in bytes (default: 10MB)
    pub max_file_size: u64,

    /// Timeout for tool execution in ms (default: 30000)
    pub timeout_ms: u64,

    /// Git commit to pin results to (optional)
    pub commit: Option<String>,
}
```

### ToolError

Common error types:

```rust
pub enum ToolError {
    Io(std::io::Error),
    PathNotFound(String),
    InvalidPattern(String),
    ParseError(String),
    ExecutionError(String),
}
```

## GrepTool

Pattern search across files with regex support. Returns matches with SpanRefs for precise citation.

### Basic Usage

```rust
use rlm::{GrepTool, GrepHit};
use std::path::PathBuf;

let grep = GrepTool::new(PathBuf::from("."));

// Search for pattern in Rust files
let hits = grep.search("fn main", &["**/*.rs"], 20).await?;

for hit in &hits {
    println!("{}:{} - {}",
        hit.span.path,
        hit.span.start_line,
        hit.line.trim()
    );
    println!("  Preview:\n{}", hit.preview);
}
```

### GrepHit Structure

```rust
pub struct GrepHit {
    /// SpanRef pointing to the matching location
    pub span: SpanRef,

    /// The matching line content
    pub line: String,

    /// Preview context (lines before and after)
    pub preview: String,

    /// Match score (based on position, frequency)
    pub score: f32,
}
```

### Configuration

```rust
use rlm::ToolConfig;

let config = ToolConfig {
    max_results: 50,
    max_file_size: 5 * 1024 * 1024,  // 5MB
    commit: Some("abc123".to_string()),
    ..Default::default()
};

let grep = GrepTool::with_config(PathBuf::from("."), config);
```

### JSON Interface

```rust
let result = grep.execute(serde_json::json!({
    "pattern": "fn\\s+\\w+",       // Regex pattern
    "paths": ["**/*.rs", "**/*.py"], // Glob patterns
    "max_hits": 30                  // Limit results
})).await?;

// Result structure:
// {
//   "hits": [...],
//   "total": 25,
//   "truncated": false
// }
```

### Regex Examples

```rust
// Literal search
grep.search("TODO", &["**/*"], 100).await?;

// Function definitions
grep.search(r"fn\s+\w+", &["**/*.rs"], 50).await?;

// Error handling patterns
grep.search(r"\.unwrap\(\)|\.expect\(", &["**/*.rs"], 100).await?;

// Import statements
grep.search(r"^use\s+\w+", &["**/*.rs"], 50).await?;
```

## ReadLinesTool

Read specific line ranges from files with SpanRef for provenance.

### Basic Usage

```rust
use rlm::{ReadLinesTool, ReadResult};

let reader = ReadLinesTool::new(PathBuf::from("."));

// Read lines 10-50 from a file
let result = reader.read("src/lib.rs", 10, 50).await?;

println!("Content ({} lines):", result.span.line_count());
println!("{}", result.content);

if result.truncated {
    println!("(truncated, total {} lines)", result.total_lines);
}
```

### ReadResult Structure

```rust
pub struct ReadResult {
    /// SpanRef for the read content
    pub span: SpanRef,

    /// The content of the specified lines
    pub content: String,

    /// Total lines in the file
    pub total_lines: u32,

    /// Whether the range was truncated
    pub truncated: bool,
}
```

### Additional Methods

```rust
// Read entire file
let result = reader.read_all("README.md").await?;

// Read with context around a specific line
let result = reader.read_with_context(
    "src/main.rs",
    42,    // Center line
    10     // Context lines before/after
).await?;

// Read from an existing SpanRef
let result = reader.read_span(&existing_span).await?;
```

### JSON Interface

```rust
let result = reader.execute(serde_json::json!({
    "path": "src/lib.rs",
    "start_line": 10,  // Default: 1
    "end_line": 50     // Default: 100
})).await?;
```

## ListFilesTool

Directory traversal with language detection and metadata.

### Basic Usage

```rust
use rlm::{ListFilesTool, FileInfo};

let lister = ListFilesTool::new(PathBuf::from("."));

// List all Rust files
let files = lister.list("**/*.rs").await?;

for file in &files {
    println!("{}: {} bytes, {} lines, lang={:?}",
        file.path,
        file.size,
        file.lines.unwrap_or(0),
        file.language
    );
}
```

### FileInfo Structure

```rust
pub struct FileInfo {
    /// File path relative to repository root
    pub path: String,

    /// File size in bytes
    pub size: u64,

    /// Detected language (based on extension)
    pub language: Option<String>,

    /// Number of lines (if computed)
    pub lines: Option<u32>,

    /// Whether the file is binary
    pub is_binary: bool,
}
```

### Language-Based Listing

```rust
// List by language
let rust_files = lister.list_by_language("rust").await?;
let python_files = lister.list_by_language("python").await?;
let ts_files = lister.list_by_language("typescript").await?;

// Supported languages:
// rust, python, javascript, typescript, go, java, c, cpp,
// ruby, php, swift, kotlin, scala, csharp, fsharp, haskell,
// ocaml, elixir, erlang, clojure, lua, r, julia, dart, zig,
// nim, vlang, crystal, shell, powershell, sql, html, css,
// json, yaml, toml, xml, markdown, text
```

### Repository Summary

```rust
let summary = lister.summary().await?;

// Returns JSON:
// {
//   "total_files": 150,
//   "total_size_bytes": 1234567,
//   "total_lines": 45000,
//   "binary_files": 5,
//   "by_language": [
//     {"language": "rust", "count": 50, "size_bytes": 500000},
//     {"language": "python", "count": 30, "size_bytes": 200000},
//     ...
//   ]
// }
```

### JSON Interface

```rust
let result = lister.execute(serde_json::json!({
    "glob": "src/**/*.rs",        // Glob pattern
    "language": "rust"            // Or filter by language
})).await?;
```

### Binary Detection

```rust
// FileInfo includes binary detection
for file in files {
    if file.is_binary {
        println!("{} is binary", file.path);
    }
}

// Manual check
let is_binary = FileInfo::is_binary_content(&file_bytes);
```

## SymbolsTool

Extract symbols (functions, classes, types) from source files using regex-based parsing.

### Basic Usage

```rust
use rlm::{SymbolsTool, SymbolInfo, SymbolKind};

let symbols_tool = SymbolsTool::new(PathBuf::from("."));

let symbols = symbols_tool.extract("src/lib.rs").await?;

for sym in &symbols {
    println!("{:?} {} at line {} - {}",
        sym.kind,
        sym.name,
        sym.span.start_line,
        sym.signature
    );
    if let Some(doc) = &sym.doc {
        println!("  Doc: {}", doc);
    }
}
```

### SymbolInfo Structure

```rust
pub struct SymbolInfo {
    /// Symbol name
    pub name: String,

    /// Kind of symbol
    pub kind: SymbolKind,

    /// SpanRef pointing to the symbol definition
    pub span: SpanRef,

    /// Parent symbol (for methods, nested items)
    pub parent: Option<String>,

    /// Signature/declaration line
    pub signature: String,

    /// Documentation comment if present
    pub doc: Option<String>,
}

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
    Class,
    Interface,
    Variable,
    Unknown,
}
```

### Supported Languages

The tool uses regex-based extraction optimized for:

**Rust:**
- Functions (`fn`, `pub fn`, `async fn`)
- Structs, Enums, Traits
- Impl blocks
- Constants, Statics, Type aliases
- Modules
- Doc comments (`///`)

**Python:**
- Functions (`def`)
- Classes
- Methods (indented `def`)
- Module-level variables
- Docstrings

**TypeScript/JavaScript:**
- Functions (`function`, `async function`)
- Classes, Interfaces
- Type aliases
- Constants (`const`), Variables (`let`, `var`)
- Methods
- JSDoc comments

**Go:**
- Functions (`func`)
- Structs, Interfaces
- Constants, Variables
- Doc comments

### Filtering by Kind

```rust
// Get only functions
let functions: Vec<_> = symbols.iter()
    .filter(|s| s.kind == SymbolKind::Function)
    .collect();

// Via JSON interface
let result = symbols_tool.execute(serde_json::json!({
    "path": "src/lib.rs",
    "kind": "function"  // Filter by kind
})).await?;
```

### JSON Interface

```rust
let result = symbols_tool.execute(serde_json::json!({
    "path": "src/main.rs",
    "kind": "struct"  // Optional: filter by kind
})).await?;

// Result structure:
// {
//   "symbols": [...],
//   "count": 15
// }
```

## Using Tools with DSPy

### Direct Tool Calls

```rust
use rlm::{GrepTool, RlmTool};

let grep = GrepTool::new(repo_path);

// Use the trait interface
println!("Tool: {}", grep.name());
println!("Description: {}", grep.description());
println!("Schema: {}", grep.args_schema());

let result = grep.execute(serde_json::json!({
    "pattern": "fn main",
    "paths": ["**/*.rs"]
})).await?;
```

### In DSPy Signatures

Tools can be exposed to DSPy predictors for agent-like behavior:

```rust
// Define a signature that uses tools
#[Signature]
struct ResearchSignature {
    #[input] pub query: String,
    #[input] pub available_tools: String,  // JSON tool descriptions

    #[output] pub tool_calls: String,      // JSON array of tool calls
    #[output] pub findings: String,
}

// Build tool descriptions
let tools = vec![
    grep.description(),
    reader.description(),
    lister.description(),
];
let tool_json = serde_json::to_string(&tools)?;

// Execute and parse tool calls
let result = predictor.forward(example! {
    "query": "input" => "Find all TODO comments",
    "available_tools": "input" => tool_json
}).await?;

// Execute requested tools
let calls: Vec<ToolCall> = serde_json::from_str(&result.tool_calls)?;
for call in calls {
    match call.tool.as_str() {
        "grep" => {
            let result = grep.execute(call.args).await?;
            // Process results...
        }
        _ => {}
    }
}
```

## Performance Considerations

### File Size Limits

All tools respect `max_file_size` to avoid processing huge files:

```rust
let config = ToolConfig {
    max_file_size: 1024 * 1024,  // 1MB limit
    ..Default::default()
};
```

### Result Limits

Use `max_results` to control output size:

```rust
let config = ToolConfig {
    max_results: 50,  // Limit grep hits, file listings, etc.
    ..Default::default()
};
```

### Binary File Handling

ListFilesTool and GrepTool skip binary files automatically:

```rust
// Binary files are detected but not processed for content
for file in lister.list("**/*").await? {
    if file.is_binary {
        // file.lines will be None
        // GrepTool will skip this file
    }
}
```

## Error Handling

```rust
use rlm::{ToolError, ToolResult};

match grep.search("pattern", &["**/*.rs"], 10).await {
    Ok(hits) => {
        // Process hits
    }
    Err(ToolError::PathNotFound(path)) => {
        eprintln!("Path not found: {}", path);
    }
    Err(ToolError::InvalidPattern(msg)) => {
        eprintln!("Invalid regex: {}", msg);
    }
    Err(ToolError::Io(e)) => {
        eprintln!("IO error: {}", e);
    }
    Err(e) => {
        eprintln!("Tool error: {}", e);
    }
}
```

## Git Integration

Tools can pin results to specific Git commits:

```rust
use rlm::tools::get_current_commit;

// Get current HEAD commit
let commit = get_current_commit(&repo_path);

// Configure tools with commit
let config = ToolConfig {
    commit: commit,
    ..Default::default()
};

let grep = GrepTool::with_config(repo_path, config);

// All SpanRefs will include the commit SHA
let hits = grep.search("fn main", &["**/*.rs"], 10).await?;
for hit in hits {
    println!("{}@{}", hit.span.path, hit.span.commit.as_deref().unwrap_or("HEAD"));
}
```

## Related Documentation

- [Provenance Tracking](./PROVENANCE.md) - SpanRef details
- [DSPy Integration](./DSPY.md) - Using tools with DSPy
- [RLM Architecture](./ARCHITECTURE.md) - System overview
