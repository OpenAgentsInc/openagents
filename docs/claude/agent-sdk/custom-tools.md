# Custom Tools for Claude Agent SDK

How to define and use custom tools with our Rust Claude Agent SDK.

## Overview

Custom tools extend Claude Code's capabilities. Each tool has:

1. **Name** - Unique identifier
2. **Description** - What it does (helps Claude decide when to use it)
3. **Input Schema** - JSON Schema defining parameters
4. **Handler** - Code that executes when called

## Tool Naming Convention

MCP tools follow this pattern:

```
mcp__{server_name}__{tool_name}
```

**Examples:**
- `mcp__my-tools__get_weather`
- `mcp__database__query`
- `mcp__utilities__calculate`

Built-in tools use simple names: `Read`, `Write`, `Bash`, `Glob`, `Grep`, `Edit`, `TodoWrite`, etc.

---

## Our Rust SDK

Our `claude_agent_sdk` crate communicates with Claude Code CLI via JSONL over stdio.

### QueryOptions

```rust
pub struct QueryOptions {
    /// MCP server configurations
    pub mcp_servers: HashMap<String, McpServerConfig>,
    /// Whitelist specific tools (None = allow all)
    pub allowed_tools: Option<Vec<String>>,
    /// Blacklist specific tools
    pub disallowed_tools: Option<Vec<String>>,
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// Model to use
    pub model: Option<String>,
    /// Max conversation turns
    pub max_turns: Option<u32>,
    /// Skip permission checks
    pub dangerously_skip_permissions: bool,
    // ...
}
```

### MCP Server Types

```rust
pub enum McpServerConfig {
    /// Local command (most common)
    Stdio {
        command: String,
        args: Option<Vec<String>>,
        env: Option<HashMap<String, String>>,
    },
    /// Server-Sent Events
    Sse {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    /// HTTP endpoint
    Http {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
}
```

### Defining Tools

Tools are defined with JSON Schema:

```rust
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub fn get_custom_tools() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "subtask_complete".to_string(),
            description: "Signal that the current subtask is complete".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Brief summary of what was done"
                    },
                    "files_modified": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of modified files"
                    }
                },
                "required": ["summary"]
            }),
        },
    ]
}
```

### Permission Handler

Control which tools can execute:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait PermissionHandler: Send + Sync {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        suggestions: Option<Vec<PermissionUpdate>>,
        blocked_path: Option<String>,
        decision_reason: Option<String>,
        tool_use_id: &str,
        agent_id: Option<String>,
    ) -> Result<PermissionResult>;
}

pub enum PermissionResult {
    Allow {
        updated_input: serde_json::Value,
        updated_permissions: Option<Vec<PermissionUpdate>>,
        tool_use_id: Option<String>,
    },
    Deny {
        message: String,
        interrupt: Option<bool>,  // Stop entire execution if true
        tool_use_id: Option<String>,
    },
}

// Convenience constructors
impl PermissionResult {
    pub fn allow(input: serde_json::Value) -> Self { ... }
    pub fn deny(message: &str) -> Self { ... }
    pub fn deny_and_interrupt(message: &str) -> Self { ... }
}
```

**Built-in handlers:**
- `AllowAllPermissions` - Allow everything
- `DenyAllPermissions` - Deny everything
- `RulesPermissionHandler` - Tool-specific allow/deny rules
- `CallbackPermissionHandler` - Custom async callback

---

## JSON Schema Patterns

### Simple Types

```json
{
  "type": "object",
  "properties": {
    "name": { "type": "string" },
    "count": { "type": "integer" },
    "enabled": { "type": "boolean" },
    "tags": {
      "type": "array",
      "items": { "type": "string" }
    }
  },
  "required": ["name"]
}
```

### With Constraints

```json
{
  "type": "object",
  "properties": {
    "age": {
      "type": "integer",
      "minimum": 0,
      "maximum": 150
    },
    "email": {
      "type": "string",
      "format": "email"
    },
    "status": {
      "type": "string",
      "enum": ["active", "inactive", "pending"]
    },
    "priority": {
      "type": "integer",
      "default": 5
    }
  }
}
```

### Nested Objects

```json
{
  "type": "object",
  "properties": {
    "user": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "email": { "type": "string" }
      },
      "required": ["name", "email"]
    },
    "filters": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "field": { "type": "string" },
          "operator": { "type": "string", "enum": ["eq", "ne", "gt", "lt"] },
          "value": {}
        },
        "required": ["field", "operator", "value"]
      }
    }
  }
}
```

---

## Best Practices

### 1. Write Clear Descriptions

The description helps Claude decide when to use the tool:

```rust
// BAD - vague
McpToolDefinition {
    name: "process".to_string(),
    description: "Process data".to_string(),
    ...
}

// GOOD - specific
McpToolDefinition {
    name: "calculate_compound_interest".to_string(),
    description: "Calculate compound interest for an investment. \
                  Returns final amount, interest earned, and ROI percentage.".to_string(),
    ...
}
```

### 2. Use Descriptive Parameter Names

```json
// BAD
{ "n": { "type": "integer" }, "s": { "type": "string" } }

// GOOD
{
  "num_items": {
    "type": "integer",
    "description": "Number of items to retrieve"
  },
  "search_query": {
    "type": "string",
    "description": "Search term to filter results"
  }
}
```

### 3. Use Enums for Fixed Options

```json
{
  "method": {
    "type": "string",
    "enum": ["GET", "POST", "PUT", "DELETE"],
    "description": "HTTP method to use"
  },
  "format": {
    "type": "string",
    "enum": ["json", "csv", "xml"],
    "default": "json"
  }
}
```

### 4. Mark Required Fields

```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string" },
    "limit": { "type": "integer", "default": 100 }
  },
  "required": ["query"]  // limit is optional with default
}
```

### 5. Add Defaults for Optional Fields

```json
{
  "properties": {
    "timeout_ms": { "type": "integer", "default": 5000 },
    "retry_count": { "type": "integer", "default": 3 },
    "verbose": { "type": "boolean", "default": false }
  }
}
```

### 6. Constrain Dangerous Inputs

```json
{
  "limit": {
    "type": "integer",
    "minimum": 1,
    "maximum": 1000,
    "description": "Max rows (capped at 1000)"
  },
  "path": {
    "type": "string",
    "pattern": "^[a-zA-Z0-9_/.-]+$",
    "description": "File path (alphanumeric, no special chars)"
  }
}
```

### 7. Create Focused Tools

```rust
// BAD - too general
McpToolDefinition {
    name: "database".to_string(),
    description: "Do database stuff".to_string(),
    input_schema: json!({
        "properties": {
            "operation": { "type": "string" },
            "data": {}
        }
    }),
}

// GOOD - focused tools
let tools = vec![
    McpToolDefinition {
        name: "query_users".to_string(),
        description: "Search for users by criteria".to_string(),
        ...
    },
    McpToolDefinition {
        name: "create_user".to_string(),
        description: "Create a new user account".to_string(),
        ...
    },
    McpToolDefinition {
        name: "delete_user".to_string(),
        description: "Delete a user account (requires confirmation)".to_string(),
        ...
    },
];
```

### 8. Require Confirmation for Destructive Operations

```json
{
  "type": "object",
  "properties": {
    "record_ids": {
      "type": "array",
      "items": { "type": "string" }
    },
    "confirm_delete": {
      "type": "boolean",
      "description": "Must be true to confirm deletion"
    }
  },
  "required": ["record_ids", "confirm_delete"]
}
```

---

## Tool Response Format

Tools return content blocks:

```rust
// Text response
serde_json::json!({
    "content": [{
        "type": "text",
        "text": "Operation completed successfully"
    }]
})

// Error response
serde_json::json!({
    "content": [{
        "type": "text",
        "text": "Error: Connection failed"
    }],
    "isError": true
})

// Structured data (as JSON string in text block)
serde_json::json!({
    "content": [{
        "type": "text",
        "text": serde_json::to_string_pretty(&result)?
    }]
})
```

---

## Example: Complete Tool Definition

```rust
use serde_json::json;

pub fn database_query_tool() -> McpToolDefinition {
    McpToolDefinition {
        name: "query_database".to_string(),
        description: "Execute a read-only SQL query against the database. \
                      Returns up to 1000 rows as JSON. Use for SELECT queries only.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "SQL SELECT query to execute"
                },
                "params": {
                    "type": "array",
                    "items": {},
                    "description": "Query parameters for prepared statement",
                    "default": []
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 100,
                    "description": "Maximum rows to return"
                },
                "format": {
                    "type": "string",
                    "enum": ["json", "csv", "table"],
                    "default": "json",
                    "description": "Output format"
                }
            },
            "required": ["query"]
        }),
    }
}
```

---

## References

- [MCP Protocol](https://modelcontextprotocol.io)
- [JSON Schema](https://json-schema.org/understanding-json-schema/)
- Our Rust SDK: `crates/claude_agent_sdk/`
