# Tools System

## Overview

The OpenAI Codex CLI implements a comprehensive tools system that enables the AI assistant to interact with the local environment, execute commands, manipulate files, and integrate with external services. The system supports both OpenAI's native tools and the broader Model Context Protocol (MCP) ecosystem.

## Architecture

### Core Tool Definition Structure

Location: `codex-rs/core/src/openai_tools.rs`

The tools system is built around the `OpenAiTool` enum that defines four main tool types:

```rust
pub enum OpenAiTool {
    Function(ResponsesApiTool),  // Standard JSON schema tools
    LocalShell {},              // OpenAI's local shell tool
    WebSearch {},               // OpenAI's web search tool  
    Freeform(FreeformTool),     // Custom grammar-based tools
}
```

#### Tool Schema System

- **`JsonSchema`**: Subset of JSON Schema for parameter validation
- **`ResponsesApiTool`**: Contains name, description, parameters, and strict mode flag
- **`FreeformTool`**: Custom tools using Lark grammar definitions

#### Tool Configuration

```rust
struct ToolsConfig {
    pub shell_type: ConfigShellToolType,
    pub plan_tool: bool,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub web_search_request: bool,
    pub include_view_image_tool: bool,
    pub experimental_unified_exec_tool: bool,
}
```

## Built-in Tools

### Shell/Execution Tools

#### Basic Shell Tool
- **Purpose**: Execute shell commands with timeout and permission controls
- **Configuration**: Sandbox mode, approval policies, timeout limits
- **Security**: Integrated with sandbox system for safe command execution

#### Unified Exec Tool
- **Purpose**: Advanced execution with session management and PTY support
- **Features**: 
  - Interactive sessions with `exec_command` and `write_stdin`
  - Streamable output for real-time feedback
  - Session persistence across multiple commands

#### Example Configuration:
```toml
[tools]
shell_type = "unified_exec"  # or "basic"
experimental_unified_exec_tool = true
```

### File Management Tools

#### Apply Patch Tool
- **Purpose**: File editing using custom patch format
- **Variants**: 
  - Freeform (grammar-based)
  - JSON (structured)
- **Operations**:
  - Add File: Create new files
  - Delete File: Remove existing files
  - Update File: Modify files with optional renaming
  - Context-aware hunks with `@@` headers

#### Grammar Definition
Location: `codex-rs/core/src/tool_apply_patch.lark`

```lark
start: operation+

operation: add_file_op | delete_file_op | update_file_op

add_file_op: "Add File:" path newline file_content
delete_file_op: "Delete File:" path
update_file_op: "Update File:" path rename_clause? newline hunk+

hunk: "@@" hunk_header "@@" newline hunk_content
```

#### View Image Tool
- **Purpose**: Attach local images to conversation context
- **Supported Formats**: Common image formats for AI model consumption
- **Configuration**: 
  ```toml
  [tools]
  include_view_image_tool = true
  ```

### Planning Tools

#### Update Plan Tool
- **Purpose**: Structured task planning and progress tracking
- **Features**:
  - Hierarchical task management
  - Progress tracking
  - Status updates
- **Configuration**:
  ```toml
  [tools]
  plan_tool = true
  ```

### OpenAI Native Tools

#### Local Shell
- **Purpose**: OpenAI's native local shell integration
- **Implementation**: Direct integration with OpenAI's tool definitions

#### Web Search
- **Purpose**: OpenAI's web search capability
- **Configuration**:
  ```toml
  [tools]
  web_search_request = true
  ```

## OpenAI API Integration

### Dual API Support

The system supports both OpenAI API formats:

#### Responses API (Newer)
- Structured response format with enhanced tool calling
- Better tool result handling
- Improved streaming capabilities

#### Chat Completions API (Traditional)
- Backward compatibility with existing chat-based workflows
- Function calling via traditional chat interface

### Tool Format Conversion

```rust
// Convert tools for different API formats
pub fn create_tools_json_for_responses_api(tools: &[OpenAiTool]) -> Result<Vec<serde_json::Value>>
pub fn create_tools_json_for_chat_completions_api(tools: &[OpenAiTool]) -> Result<Vec<serde_json::Value>>
```

### Model-Specific Configuration

- Different models use different tool types
- Tool availability configured per model family
- Schema validation varies by model capabilities

## MCP (Model Context Protocol) Integration

Location: `codex-rs/core/src/mcp_connection_manager.rs`

### Connection Management

The `McpConnectionManager` handles multiple MCP server connections:

```rust
pub struct McpServerConfig {
    command: String,
    args: Vec<String>,
    env: Option<HashMap<String, String>>,
    startup_timeout_sec: Option<Duration>,
    tool_timeout_sec: Option<Duration>,
}
```

### Tool Qualification System

MCP tools use qualified naming to avoid conflicts:
```rust
const MCP_TOOL_NAME_DELIMITER: &str = "__";
// Creates names like "server__tool_name"
```

### Configuration Example

```toml
[mcp_servers.docs]
command = "docs-server"
args = ["--port", "4000"]
startup_timeout_sec = 10
tool_timeout_sec = 60

[mcp_servers.filesystem]
command = "filesystem-server"
args = ["/path/to/workspace"]
```

### Tool Discovery and Execution

1. **Discovery**: Tools discovered via MCP `tools/list` requests
2. **Schema Sanitization**: Ensures compatibility with OpenAI requirements
3. **Routing**: Tool calls routed to appropriate MCP servers
4. **Result Handling**: Standardized response format across all tools

## Tool Execution Flow

### Execution Pipeline

1. **Tool Discovery**: 
   - Aggregate tools from built-ins and MCP servers
   - Apply model-specific filtering

2. **Schema Validation**:
   - JSON schemas sanitized and validated
   - Grammar-based tools parsed and validated

3. **Model Configuration**:
   - Tools filtered based on model capabilities
   - Per-model tool configurations applied

4. **Execution Routing**:
   - Built-in tools: Direct execution
   - MCP tools: Route to appropriate server

5. **Security Check**:
   - Approval workflow evaluation
   - Sandbox policy enforcement

6. **Execution**:
   - Command/tool execution with monitoring
   - Event generation for user feedback

7. **Result Processing**:
   - Standardized response format
   - Error handling and reporting

### Session Management

Location: `codex-rs/core/src/exec_command/session_manager.rs`

- **Persistent Sessions**: Commands can maintain state across calls
- **PTY Support**: Interactive tools with terminal emulation
- **Resource Management**: Automatic cleanup of long-running processes

## Security and Sandboxing

### Approval Workflows

Tools integrate with the approval system:
- **`untrusted`**: Always ask for approval
- **`on-failure`**: Escalate on sandbox failures
- **`on-request`**: Let model decide when to ask
- **`never`**: Never prompt, return failures to model

### Sandbox Integration

Tools respect sandbox policies:
- **`read-only`**: File operations restricted to read access
- **`workspace-write`**: Write access limited to workspace
- **`danger-full-access`**: Unrestricted access (discouraged)

### Permission Escalation

```rust
pub struct ExecParams {
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
    // ... other fields
}
```

## Tool Customization

### MCP Tool Customization

The `CodexToolCallParam` allows extensive customization:

```rust
pub struct CodexToolCallParam {
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub base_instructions: Option<String>,
    pub config_overrides: Option<Vec<ConfigOverride>>,
}
```

### Configuration Hierarchy

1. **Model Family Defaults**: Base tool configuration per model
2. **User Configuration**: Override via config files
3. **Runtime Parameters**: Per-session customization

Example:
```toml
[tools]
shell_type = "unified_exec"
plan_tool = true
include_view_image_tool = true

[model_families.gpt-3]
shell_type = "basic"  # Override for older models
```

## Event System

### Tool Events

Tools generate events for monitoring and user interaction:

```rust
pub enum Event {
    ToolCallBegin { tool_name: String, call_id: String },
    ToolCallEnd { call_id: String, result: ToolResult },
    ExecCommandOutputDelta { stream: ExecOutputStream, delta: String },
    // ... other events
}
```

### User Interaction

- **Real-time Feedback**: Streaming output for long-running operations
- **Approval Prompts**: Interactive approval for dangerous operations
- **Progress Tracking**: Task progress and status updates

## Extension Points

### Adding New Built-in Tools

1. **Define Tool Enum Variant**:
   ```rust
   pub enum OpenAiTool {
       // ... existing variants
       MyCustomTool { config: MyToolConfig },
   }
   ```

2. **Implement Tool Logic**:
   ```rust
   async fn handle_my_custom_tool(params: MyToolParams) -> ToolResult {
       // Implementation
   }
   ```

3. **Add to Tool Resolution**:
   ```rust
   match tool {
       OpenAiTool::MyCustomTool { .. } => handle_my_custom_tool(params).await,
       // ... other cases
   }
   ```

### MCP Server Integration

1. **Create MCP Server**: Implement MCP protocol
2. **Configure in Codex**:
   ```toml
   [mcp_servers.my_server]
   command = "my-mcp-server"
   args = ["--config", "config.json"]
   ```

3. **Tool Auto-Discovery**: Tools automatically available after server startup

## Key Design Principles

1. **Extensibility**: Easy addition of new tool types through enum variants
2. **Schema Flexibility**: Support for both structured JSON and grammar-based tools
3. **Multi-Protocol Support**: Native OpenAI tools + MCP ecosystem integration
4. **Security First**: Comprehensive approval and sandboxing systems
5. **Event-Driven**: Rich event system for monitoring and user interaction
6. **Model Agnostic**: Tool availability adapts to model capabilities

The tools system provides a robust foundation for extending Codex capabilities while maintaining security and ease of use. The dual support for OpenAI native tools and the MCP ecosystem ensures broad compatibility and extensibility.