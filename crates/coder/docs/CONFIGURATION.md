# Coder Configuration Guide

This document provides a comprehensive reference for all configuration options in Coder.

## Table of Contents

1. [Environment Variables](#environment-variables)
2. [Service Configuration](#service-configuration)
3. [Feature Flags](#feature-flags)
4. [CLAUDE.md Files](#claudemd-files)
5. [Agent Configuration](#agent-configuration)
6. [Provider Configuration](#provider-configuration)
7. [Tool Configuration](#tool-configuration)
8. [Storage Configuration](#storage-configuration)

---

## Environment Variables

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | Anthropic API key for Claude models (or use provider-specific keys below) | `sk-ant-api03-...` |

### Optional Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CODER_WORKING_DIR` | Default working directory for sessions | Current directory |
| `CODER_DATABASE` | Path to SQLite database file | `coder.db` |
| `CODER_DEFAULT_AGENT` | Default agent for new sessions | `build` |
| `CODER_DEFAULT_MODEL` | Default model ID | `claude-sonnet-4-5-20250929` |
| `CODER_DEFAULT_PROVIDER` | Default provider ID | `anthropic` |
| `ANTHROPIC_BASE_URL` | Custom Anthropic API endpoint | `https://api.anthropic.com` |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `OPENAI_BASE_URL` | Custom OpenAI endpoint (proxy) | `https://api.openai.com/v1` |
| `OPENROUTER_API_KEY` | OpenRouter API key | - |
| `OPENROUTER_BASE_URL` | Custom OpenRouter endpoint | `https://openrouter.ai/api/v1` |
| `OLLAMA_BASE_URL` / `OLLAMA_HOST` | Ollama OpenAI-compatible endpoint | `http://localhost:11434/v1` |
| `FM_BRIDGE_URL` | Apple FM bridge endpoint | `http://localhost:3030` |
| `RUST_LOG` | Logging level for Rust crates | `info` |

### Example .env File

```bash
# Required
ANTHROPIC_API_KEY=sk-ant-api03-your-key-here

# Optional overrides
CODER_WORKING_DIR=/home/user/projects/myproject
CODER_DATABASE=/home/user/.coder/coder.db
CODER_DEFAULT_MODEL=claude-sonnet-4-5-20250929
CODER_DEFAULT_AGENT=build
OPENAI_API_KEY=sk-openai-...
OPENROUTER_API_KEY=sk-or-...
# OLLAMA_BASE_URL=http://localhost:11434/v1
# FM_BRIDGE_URL=http://localhost:3030

# Logging
RUST_LOG=coder_app=debug,coder_service=debug,llm=trace
```

---

## Service Configuration

### ServiceConfig

The main configuration struct for ChatService:

```rust
pub struct ServiceConfig {
    /// Default working directory for new sessions.
    /// Environment: CODER_WORKING_DIR
    pub working_directory: PathBuf,

    /// Database path for SQLite storage.
    /// Environment: CODER_DATABASE
    pub database_path: PathBuf,

    /// Default agent ID.
    /// Environment: CODER_DEFAULT_AGENT
    pub default_agent: String,

    /// Default model ID.
    /// Environment: CODER_DEFAULT_MODEL
    pub default_model: String,

    /// Default provider ID.
    /// Environment: CODER_DEFAULT_PROVIDER
    pub default_provider: String,

    /// Maximum turns in a conversation loop.
    /// Prevents infinite tool-use loops.
    pub max_turns: usize,

    /// Processor configuration.
    pub processor_config: ProcessorConfig,
}
```

### Default Values

```rust
ServiceConfig {
    working_directory: std::env::current_dir().unwrap_or(PathBuf::from(".")),
    database_path: PathBuf::from("coder.db"),
    default_agent: "build".to_string(),
    default_model: "claude-sonnet-4-5-20250929".to_string(),
    default_provider: "anthropic".to_string(),
    max_turns: 50,
    processor_config: ProcessorConfig::default(),
}
```

### Programmatic Configuration

```rust
use coder_service::{ChatService, ServiceConfig};
use std::path::PathBuf;

let config = ServiceConfig {
    working_directory: PathBuf::from("/my/project"),
    database_path: PathBuf::from("/data/coder.db"),
    default_agent: "build".to_string(),
    default_model: "claude-sonnet-4-5-20250929".to_string(),
    default_provider: "anthropic".to_string(),
    max_turns: 100,
    ..Default::default()
};

let service = ChatService::new(config).await?;
```

---

## Feature Flags

### Cargo Features

| Feature | Description | Default |
|---------|-------------|---------|
| `coder-service` | Use ChatService with built-in LLM providers | Yes |
| `legacy` | Use mechacoder backend (Claude Code CLI) | No |
| `web` | Enable WASM/WebGPU support | No |

### Usage Examples

```bash
# Default (ChatService)
cargo run -p coder_app

# Legacy backend
cargo run -p coder_app --no-default-features --features legacy

# Web build
cargo build -p coder_app --target wasm32-unknown-unknown --features web
```

### Feature Detection in Code

```rust
#[cfg(feature = "coder-service")]
fn use_chat_service() {
    // ChatService code path
}

#[cfg(not(feature = "coder-service"))]
fn use_legacy() {
    // Legacy mechacoder code path
}
```

---

## CLAUDE.md Files

Coder automatically loads instructions from `CLAUDE.md` files in the directory hierarchy.

### Search Order

Instructions are loaded from most general to most specific:

```
/home/user/CLAUDE.md                    # User-level
/home/user/projects/CLAUDE.md           # Workspace-level
/home/user/projects/myproject/CLAUDE.md # Project-level
```

### File Format

```markdown
# Project Instructions

This is a Rust project using async/await patterns.

## Tech Stack
- Rust 2024 edition
- Tokio for async runtime
- SQLite for storage

## Conventions
- Use `thiserror` for error types
- Prefer `Arc<T>` over `Rc<T>` for shared state
- Tests go in `src/tests/` directories
```

### Loading Instructions Programmatically

```rust
use coder_session::prompt::load_instructions;
use std::path::Path;

let instructions = load_instructions(Path::new("/path/to/project"));
// Returns Vec<String> with all CLAUDE.md contents
```

---

## Agent Configuration

### AgentConfig

Per-session agent settings:

```rust
pub struct AgentConfig {
    /// Agent ID (e.g., "build", "plan", "explore")
    pub agent_id: String,

    /// Model ID (e.g., "claude-sonnet-4-20250514")
    pub model_id: String,

    /// Provider ID (e.g., "anthropic")
    pub provider_id: String,

    /// Maximum tokens for completion
    pub max_tokens: Option<u32>,

    /// Temperature for sampling (0.0 - 1.0)
    pub temperature: Option<f32>,
}
```

### Built-in Agents

| Agent | Mode | Description | Permissions |
|-------|------|-------------|-------------|
| `build` | Primary | Full-capability implementation agent | Permissive |
| `plan` | Primary | Planning mode with restricted permissions | Read-only + safe commands |
| `general` | Subagent | General-purpose for complex tasks | Permissive |
| `explore` | Subagent | Read-only file search specialist | Read-only |

### Agent Permissions

```rust
pub struct AgentPermission {
    /// File edit permission
    pub edit: Permission,

    /// Bash command permissions (pattern -> permission)
    pub bash: IndexMap<String, Permission>,

    /// Web fetch permission
    pub webfetch: Permission,

    /// Doom loop prevention
    pub doom_loop: Permission,

    /// External directory access
    pub external_directory: Permission,
}

pub enum Permission {
    Allow,  // Auto-allow without prompting
    Ask,    // Ask user for permission
    Deny,   // Auto-deny
}
```

### Permission Presets

```rust
// Permissive - allows everything
AgentPermission::permissive()

// Read-only - no writes, limited bash
AgentPermission::read_only()

// Plan mode - read + safe commands
AgentPermission::plan_mode()
```

---

## Provider Configuration

### Anthropic Provider

```rust
// Auto-configured from environment
let provider = AnthropicProvider::new()?;

// Custom configuration
let provider = AnthropicProvider::with_config(AnthropicConfig {
    api_key: "sk-ant-...".to_string(),
    base_url: Some("https://custom.endpoint.com".to_string()),
    default_model: "claude-sonnet-4-20250514".to_string(),
})?;
```

### Available Models

| Model ID | Description | Max Tokens |
|----------|-------------|------------|
| `claude-sonnet-4-20250514` | Claude Sonnet 4 (recommended) | 8192 |
| `claude-opus-4-5-20251101` | Claude Opus 4.5 (most capable) | 8192 |

### Provider Capabilities

```rust
pub struct ProviderCapabilities {
    /// Supports streaming responses
    pub streaming: bool,

    /// Supports tool use
    pub tools: bool,

    /// Supports vision (image input)
    pub vision: bool,

    /// Supports extended thinking
    pub extended_thinking: bool,

    /// Supports prompt caching
    pub prompt_caching: bool,
}
```

---

## Tool Configuration

### Standard Tools

The tool registry includes these built-in tools:

| Tool | Description | Permission Type |
|------|-------------|-----------------|
| `bash` | Execute shell commands | `bash` |
| `read` | Read file contents | None (always allowed) |
| `write` | Create/overwrite files | `file_write` |
| `edit` | Edit files with string replacement | `file_write` |
| `grep` | Search file contents | None (always allowed) |
| `find` | Find files by glob pattern | None (always allowed) |

### Tool Schemas

```rust
// Bash tool input
pub struct BashInput {
    pub command: String,
    pub timeout_ms: Option<u64>,
    pub working_dir: Option<PathBuf>,
}

// Read tool input
pub struct ReadInput {
    pub path: PathBuf,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

// Write tool input
pub struct WriteInput {
    pub path: PathBuf,
    pub content: String,
}

// Edit tool input
pub struct EditInput {
    pub path: PathBuf,
    pub old_string: String,
    pub new_string: String,
}

// Grep tool input
pub struct GrepInput {
    pub pattern: String,
    pub path: Option<PathBuf>,
    pub ignore_case: Option<bool>,
    pub max_results: Option<usize>,
}

// Find tool input
pub struct FindInput {
    pub pattern: String,
    pub path: Option<PathBuf>,
    pub max_results: Option<usize>,
}
```

### Custom Tool Registration

```rust
use tool_registry::{Tool, ToolRegistry, ToolInfo, ToolContext, ToolResult, ToolOutput};

#[derive(Debug)]
struct MyCustomTool;

#[async_trait]
impl Tool for MyCustomTool {
    type Input = MyInput;

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "my_tool".to_string(),
            description: "Does something custom".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "param": { "type": "string" }
                },
                "required": ["param"]
            }),
        }
    }

    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput> {
        // Implementation
        Ok(ToolOutput::text("Result"))
    }
}

let mut registry = ToolRegistry::with_standard_tools();
registry.register(MyCustomTool);
```

---

## Storage Configuration

### SQLite Storage

```rust
use coder_storage::Storage;

// File-based storage
let storage = Storage::open("coder.db")?;

// In-memory storage (for testing)
let storage = Storage::open(":memory:")?;
```

### Database Schema

```sql
-- Threads table
CREATE TABLE IF NOT EXISTS threads (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    working_directory TEXT NOT NULL,
    title TEXT,
    status TEXT NOT NULL,
    agent_config TEXT NOT NULL,
    total_cost REAL NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### Database Location

Default: `coder.db` in the current directory

Recommended locations:
- **Development**: `./coder.db`
- **Production**: `~/.coder/coder.db`
- **Testing**: `:memory:`

---

## Logging Configuration

### Log Levels

| Level | Description |
|-------|-------------|
| `error` | Error conditions |
| `warn` | Warning conditions |
| `info` | Informational messages (default) |
| `debug` | Debug information |
| `trace` | Very verbose tracing |

### Per-Crate Logging

```bash
# Set different levels per crate
RUST_LOG=coder_app=debug,coder_service=debug,llm=trace,tool_registry=warn

# Quiet mode
RUST_LOG=error

# Verbose mode
RUST_LOG=debug
```

### Structured Logging

The codebase uses `tracing` for structured logging:

```rust
use tracing::{info, debug, warn, error};

info!(session_id = %session.id, "Session created");
debug!(tool_name = %name, "Executing tool");
warn!(error = %e, "Recoverable error occurred");
error!(fatal = true, "Unrecoverable error");
```

---

## Configuration Precedence

Configuration values are resolved in this order (later overrides earlier):

1. **Default values** (compiled into binary)
2. **CLAUDE.md files** (project-specific instructions)
3. **Environment variables** (runtime configuration)
4. **Programmatic configuration** (code overrides)

---

## Related Documentation

- [SERVICE_LAYER.md](./SERVICE_LAYER.md) - ChatService API reference
- [AI_INFRASTRUCTURE.md](./AI_INFRASTRUCTURE.md) - AI subsystem details
- [GETTING_STARTED.md](./GETTING_STARTED.md) - Setup guide
