# claude-mcp

MCP server that exposes Claude Code as a tool for other AI agents. Provides Model Context Protocol tools for running Claude Code queries, managing sessions, and controlling execution.

## Overview

The `claude-mcp` crate is a **Model Context Protocol (MCP) server** that wraps the `claude-agent-sdk` and exposes Claude Code capabilities as tools. It enables:

- **Running Claude Code queries** - Execute prompts with full Claude Code capabilities
- **Session management** - Continue, resume, and control active sessions
- **Permission control** - Allow/deny/auto mode for tool execution
- **Model selection** - Choose between Sonnet, Opus, and Haiku
- **Budget management** - Set token budgets and turn limits
- **MCP server configuration** - Attach additional MCP servers to Claude queries

This server allows **agents to delegate tasks to Claude Code**, creating a multi-agent architecture where specialized agents can invoke Claude Code for complex coding tasks.

## Quick Start

### Running the Server

```bash
# Run with default settings
cargo run -p claude-mcp

# Requires ANTHROPIC_API_KEY
export ANTHROPIC_API_KEY=your_api_key_here
cargo run -p claude-mcp
```

The server communicates via **JSON-RPC 2.0 over stdio**, making it compatible with any MCP-aware client (like Claude Code, Codex, or custom agents).

### MCP Configuration

Add to your MCP settings file (e.g., `~/.config/claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "claude": {
      "command": "cargo",
      "args": ["run", "-p", "claude-mcp"],
      "env": {
        "ANTHROPIC_API_KEY": "your_api_key_here"
      }
    }
  }
}
```

Now tools like `mcp__claude__query` are available in your Claude Code sessions.

## Available Tools

The server exposes **8 MCP tools**:

### Core Operations

#### `claude_query`

Run a Claude Code query with full tool access.

**Input:**
```json
{
  "prompt": "List all Rust files in this directory",
  "model": "sonnet",          // optional: sonnet|opus|haiku (default: sonnet)
  "max_turns": 30,            // optional: max conversation turns (default: 30)
  "budget_tokens": 100000,    // optional: token budget (default: 100k)
  "working_dir": "/path",     // optional: working directory (default: current)
  "permission_mode": "auto"   // optional: auto|allow|deny (default: auto)
}
```

**Output:**
```json
{
  "result": "Files found: src/main.rs, src/lib.rs, tests/integration.rs",
  "cost_usd": 0.0234,
  "num_turns": 3,
  "session_id": "session_abc123"
}
```

#### `claude_query_stream`

Same as `claude_query` but streams intermediate messages (assistant thoughts, tool calls).

**Input:** Same as `claude_query`

**Output (streamed):**
```json
[
  {"type": "assistant", "message": "Let me list the Rust files..."},
  {"type": "tool_use", "name": "Glob", "input": {"pattern": "**/*.rs"}},
  {"type": "tool_result", "content": "src/main.rs\nsrc/lib.rs\n..."},
  {"type": "result", "result": "Files found: ...", "cost_usd": 0.0234}
]
```

### Session Management

#### `claude_continue`

Continue a previous Claude Code session with a new prompt.

**Input:**
```json
{
  "session_id": "session_abc123",
  "prompt": "Now count how many functions are in each file"
}
```

**Output:**
```json
{
  "result": "Found 15 functions total: main.rs (5), lib.rs (8), integration.rs (2)",
  "cost_usd": 0.0156,
  "num_turns": 2
}
```

#### `claude_resume`

Resume a Claude Code session from a saved trajectory file.

**Input:**
```json
{
  "trajectory_path": "/path/to/trajectory.json",
  "prompt": "Continue from where we left off"  // optional
}
```

**Output:**
```json
{
  "result": "Resumed session and completed the task",
  "cost_usd": 0.0189,
  "num_turns": 4,
  "session_id": "session_xyz789"
}
```

### Control Methods

#### `claude_interrupt`

Gracefully stop an in-progress Claude Code query.

**Input:**
```json
{
  "session_id": "session_abc123"
}
```

**Output:**
```
Interrupted session session_abc123
```

#### `claude_abort`

Forcefully kill a Claude Code query (hard stop).

**Input:**
```json
{
  "session_id": "session_abc123"
}
```

**Output:**
```
Aborted session session_abc123
```

### Configuration

#### `claude_set_model`

Change the model mid-session.

**Input:**
```json
{
  "session_id": "session_abc123",
  "model": "opus"  // sonnet|opus|haiku
}
```

**Output:**
```
Model changed to opus for session session_abc123
```

#### `claude_set_permission_mode`

Change permission mode mid-session.

**Input:**
```json
{
  "session_id": "session_abc123",
  "mode": "allow"  // auto|allow|deny
}
```

**Output:**
```
Permission mode changed to allow for session session_abc123
```

## Architecture

### Tech Stack

- **JSON-RPC 2.0** - Protocol for request/response
- **MCP (Model Context Protocol)** - Tools/resources/prompts specification
- **stdio** - Transport layer (stdin/stdout)
- **claude-agent-sdk** - Rust SDK for Claude Code CLI
- **Tokio** - Async runtime
- **Serde** - JSON serialization/deserialization

### Server Structure

```
crates/claude-mcp/
├── src/
│   └── main.rs          # MCP server implementation
├── Cargo.toml
└── README.md
```

### Code Organization

**main.rs** will contain:

```rust
// JSON-RPC types
struct JsonRpcRequest { jsonrpc, id, method, params }
struct JsonRpcResponse { jsonrpc, id, result, error }
struct JsonRpcError { code, message }

// MCP types
struct Tool { name, description, input_schema }

// Server state
struct McpServer {
    sessions: Arc<Mutex<HashMap<String, QueryHandle>>>
}

impl McpServer {
    fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse

    // MCP protocol handlers
    fn handle_initialize(&self, params: &Value) -> Result<Value, String>
    fn handle_tools_list(&self) -> Result<Value, String>
    fn handle_tools_call(&self, params: &Value) -> Result<Value, String>

    // Tool implementations
    fn tool_claude_query(&self, args: &Value) -> Result<String, String>
    fn tool_claude_query_stream(&self, args: &Value) -> Result<String, String>
    fn tool_claude_continue(&self, args: &Value) -> Result<String, String>
    fn tool_claude_resume(&self, args: &Value) -> Result<String, String>
    fn tool_claude_interrupt(&self, args: &Value) -> Result<String, String>
    fn tool_claude_abort(&self, args: &Value) -> Result<String, String>
    fn tool_claude_set_model(&self, args: &Value) -> Result<String, String>
    fn tool_claude_set_permission_mode(&self, args: &Value) -> Result<String, String>
}

fn main() {
    // Create McpServer
    // Loop: read JSON-RPC from stdin, handle, write to stdout
}
```

### Protocol Flow

1. **Initialize**: Client sends `initialize` method
   ```json
   {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {...}}
   ```
   Server responds with capabilities:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 1,
     "result": {
       "protocolVersion": "2024-11-05",
       "capabilities": {"tools": {}},
       "serverInfo": {"name": "claude-mcp", "version": "0.1.0"}
     }
   }
   ```

2. **List Tools**: Client sends `tools/list`
   ```json
   {"jsonrpc": "2.0", "id": 2, "method": "tools/list"}
   ```
   Server responds with tool definitions (8 tools).

3. **Call Tool**: Client sends `tools/call`
   ```json
   {
     "jsonrpc": "2.0",
     "id": 3,
     "method": "tools/call",
     "params": {
       "name": "claude_query",
       "arguments": {
         "prompt": "What files are in this directory?",
         "model": "sonnet"
       }
     }
   }
   ```
   Server executes tool and responds:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 3,
     "result": {
       "content": [{
         "type": "text",
         "text": "{\"result\": \"...\", \"cost_usd\": 0.023, \"session_id\": \"...\"}"
       }]
     }
   }
   ```

## Use Cases

### Multi-Agent Architecture

The claude-mcp server enables **delegation patterns** where one agent can invoke Claude Code for specialized tasks:

```
Coordinator Agent (Codex/Claude)
  ├─→ claude_query("Analyze this codebase architecture")
  ├─→ claude_query("Write integration tests for auth module")
  └─→ claude_query("Refactor database queries for performance")
```

### Agent Swarms

Multiple agents can work on different parts of a project simultaneously:

```bash
# Agent 1: Frontend
claude_query(prompt="Implement user dashboard UI", working_dir="./frontend")

# Agent 2: Backend
claude_query(prompt="Add REST API endpoints", working_dir="./backend")

# Agent 3: Tests
claude_query(prompt="Write E2E tests", working_dir="./tests")
```

### Codex Integration

The **Codex agent** (OpenAI's coding agent) can use Claude Code via this MCP server:

```json
// Codex calls claude_query tool
{
  "prompt": "Fix the authentication bug in login.rs",
  "model": "sonnet",
  "permission_mode": "auto"
}
// Claude Code handles the fix, Codex reviews the result
```

This enables **best-of-both-worlds**: Codex's planning with Claude's tool execution.

### Autopilot Integration

The autopilot system can delegate complex issues to fresh Claude Code sessions:

```bash
# Autopilot gets blocked on a complex refactor
issue_get(number=42)
→ "Refactor entire auth system to use JWT"

# Delegate to Claude Code
claude_query(
  prompt="See issue #42 in autopilot.db and implement the refactor",
  max_turns=50,
  budget_tokens=200000
)
→ Claude Code implements, tests, commits
```

## Session Management

The server maintains a **session registry** to track active Claude Code processes:

```rust
struct Session {
    id: String,
    handle: QueryHandle,
    created_at: DateTime<Utc>,
    working_dir: PathBuf,
}
```

Sessions are indexed by `session_id` (UUID). Methods like `claude_continue` and `claude_interrupt` operate on sessions via their ID.

**Session lifecycle:**

1. `claude_query` → Creates new session, returns `session_id`
2. `claude_continue` → Extends session with new prompt
3. `claude_set_model` → Modifies session configuration
4. `claude_interrupt` → Gracefully stops session
5. Session cleanup on query completion

## Permission Modes

Claude Code queries can run in three permission modes:

| Mode | Behavior |
|------|----------|
| `auto` | Auto-approve safe tools (Read, Glob, Grep), prompt for dangerous (Write, Bash, Edit) |
| `allow` | Auto-approve all tools (fully autonomous) |
| `deny` | Deny all tools requiring permission (read-only mode) |

**Recommendation for MCP usage:**
- Use `allow` for trusted, sandboxed tasks (tests, analysis)
- Use `auto` for general tasks (balances safety and autonomy)
- Use `deny` for untrusted prompts or security-sensitive contexts

## Budget Management

Set resource limits per query:

```json
{
  "prompt": "Analyze this codebase",
  "max_turns": 30,        // Limit conversation turns
  "budget_tokens": 100000 // Limit API token usage
}
```

**Defaults:**
- `max_turns`: 30
- `budget_tokens`: 100,000 (~$0.30 for Sonnet)

Budget exceeded → Query stops gracefully, returns partial results.

## MCP Server Configuration

Attach additional MCP servers to Claude Code queries:

```json
{
  "prompt": "Create issues for all TODOs in the codebase",
  "mcp_servers": [
    {
      "name": "issues",
      "command": "cargo",
      "args": ["run", "-p", "issues-mcp"],
      "env": {"ISSUES_DB": "/path/to/autopilot.db"}
    }
  ]
}
```

This makes the `issue_create` tool available to Claude Code during the query.

**Common MCP servers to attach:**
- `issues-mcp` - Issue tracking
- `filesystem` - Advanced file operations
- `postgres` - Database access
- `github` - GitHub API

## Error Handling

The server uses **nested error handling**:

1. **JSON-RPC level**: Malformed requests return `JsonRpcError`
   ```json
   {
     "jsonrpc": "2.0",
     "id": null,
     "error": {
       "code": -32603,
       "message": "Unknown method: foo"
     }
   }
   ```

2. **Tool level**: Tool errors return success response with `isError: true`
   ```json
   {
     "jsonrpc": "2.0",
     "id": 5,
     "result": {
       "content": [{"type": "text", "text": "Error: ANTHROPIC_API_KEY not set"}],
       "isError": true
     }
   }
   ```

3. **Claude Code level**: Errors from Claude Code (budget exceeded, tool failures) are captured in the result:
   ```json
   {
     "result": null,
     "error": "Budget exceeded: used 101,234 tokens",
     "cost_usd": 0.304,
     "num_turns": 28
   }
   ```

## Development

### Running Tests

```bash
# Test claude-agent-sdk (underlying library)
cargo test -p claude-agent-sdk

# Manual MCP server testing
cargo run -p claude-mcp
```

### Testing the MCP Server

Manual testing via stdin/stdout:

```bash
# Start server
cargo run -p claude-mcp

# Send initialize (paste JSON, hit Enter)
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}

# Send tools/list
{"jsonrpc":"2.0","id":2,"method":"tools/list"}

# Run query
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"claude_query","arguments":{"prompt":"List Rust files","model":"haiku"}}}
```

### Debugging

Enable debug output (stderr):

```bash
RUST_LOG=debug cargo run -p claude-mcp 2>debug.log
```

MCP communication happens on stdout/stdin, so logs go to stderr.

## Environment Variables

- **`ANTHROPIC_API_KEY`** - API key for Claude Code CLI (required)
- **`CLAUDE_BIN`** - Custom path to Claude Code binary (default: searches PATH)
- **`RUST_LOG`** - Logging level (use `debug` for verbose output)

## MCP Protocol Version

This server implements **MCP protocol version 2024-11-05**.

## Tool Input Schemas

All tools use **JSON Schema** for input validation.

Example schema (claude_query):

```json
{
  "type": "object",
  "properties": {
    "prompt": {
      "type": "string",
      "description": "The prompt to send to Claude Code"
    },
    "model": {
      "type": "string",
      "enum": ["sonnet", "opus", "haiku"],
      "description": "Claude model to use",
      "default": "sonnet"
    },
    "max_turns": {
      "type": "integer",
      "description": "Maximum conversation turns",
      "default": 30
    },
    "budget_tokens": {
      "type": "integer",
      "description": "Token budget limit",
      "default": 100000
    },
    "working_dir": {
      "type": "string",
      "description": "Working directory for the query"
    },
    "permission_mode": {
      "type": "string",
      "enum": ["auto", "allow", "deny"],
      "default": "auto"
    }
  },
  "required": ["prompt"]
}
```

## Security Considerations

- **API key exposure** - Requires ANTHROPIC_API_KEY in environment (keep secure)
- **Arbitrary code execution** - Claude Code can run bash commands (use permission_mode carefully)
- **File system access** - Claude Code has read/write access to working_dir
- **Cost control** - Set budget_tokens to prevent excessive API usage
- **Session isolation** - Sessions are isolated by session_id but share server process

**Recommendation**: Only run this server locally for trusted agents. Do NOT expose over network.

## Performance

Server performance characteristics:

- **Startup time**: <100ms (no heavy initialization)
- **Query latency**: ~2-10s (depends on Claude Code execution)
- **Streaming latency**: ~200ms (first message from Claude Code)
- **Concurrency**: Multi-session (each session is independent)
- **Memory usage**: ~50MB base + ~20MB per active session

The stdio protocol adds minimal overhead (~1ms per request/response).

## Comparison to Direct SDK Usage

**Why use MCP instead of calling claude-agent-sdk directly?**

1. **Language agnostic** - Any MCP client (TypeScript, Python, Rust) can call Claude Code
2. **Process isolation** - Claude Code runs in separate process, no shared state
3. **Standardized interface** - MCP protocol is cross-compatible
4. **Agent composition** - Agents can delegate to Claude Code without SDK dependency
5. **Session management** - Server handles session lifecycle automatically

**When to use direct SDK:**
- Building Rust applications with embedded Claude Code
- Need full control over streaming/permissions
- Performance-critical applications (avoid JSON-RPC overhead)

## Example: Codex Using Claude Code

Here's how the **Codex agent** (crates/codex-agent-sdk) uses claude-mcp:

```rust
// Codex receives task: "Fix all failing tests"

// 1. Codex analyzes the problem
let analysis = codex.analyze("What tests are failing?").await?;

// 2. Codex delegates fix to Claude Code via MCP
let claude_result = mcp_call("claude_query", json!({
    "prompt": "Fix the 3 failing tests in tests/integration.rs",
    "model": "sonnet",
    "permission_mode": "allow",
    "max_turns": 20
})).await?;

// 3. Codex reviews Claude's changes
let review = codex.review_changes(claude_result.diff).await?;

// 4. Codex commits if approved
if review.approved {
    codex.commit("Fix failing integration tests").await?;
}
```

This creates a **human-like workflow**: Codex plans, Claude Code executes, Codex reviews.

## Future Work

- [ ] Add resources (Claude Code templates, common prompts)
- [ ] Add prompts (query optimization guidance, model selection advice)
- [ ] WebSocket transport (for remote agents)
- [ ] Authentication/authorization
- [ ] Rate limiting per client
- [ ] Metrics/telemetry (query counts, costs, latencies)
- [ ] Query queuing (FIFO execution when server is busy)
- [ ] Artifact extraction (images, diffs, logs from query results)
- [ ] Trajectory export (save session to .json/.rlog for replay)
- [ ] Multi-model queries (try Haiku first, fall back to Sonnet if stuck)

## Related Documentation

- **Claude Agent SDK**: `crates/claude-agent-sdk/README.md`
- **Issues MCP**: `crates/issues-mcp/README.md`
- **Codex Agent SDK**: `crates/codex-agent-sdk/README.md`
- **MCP specification**: https://spec.modelcontextprotocol.io/
- **Claude Code docs**: https://docs.anthropic.com/claude-code

## License

Same as the OpenAgents workspace (MIT).
