# Custom Tools for Claude Agent SDK

This document explains how to define, register, and use custom tools with the Claude Agent SDK. It covers the official TypeScript/Python SDK patterns, our Rust SDK implementation, and best practices for tool design.

## Overview

Custom tools extend Claude Code's capabilities by allowing it to interact with external services, APIs, databases, or perform specialized operations. Tools are defined with:

1. **Name** - Unique identifier for the tool
2. **Description** - What the tool does (helps Claude decide when to use it)
3. **Input Schema** - JSON Schema defining the tool's parameters
4. **Handler** - Function that executes when the tool is called

## Tool Naming Convention

When tools are exposed via MCP servers, they follow this naming pattern:

```
mcp__{server_name}__{tool_name}
```

**Examples:**
- `mcp__my-custom-tools__get_weather`
- `mcp__database-tools__query_database`
- `mcp__utilities__calculate`

This namespacing prevents collisions between tools from different servers.

---

## Defining Tools (Official SDK)

### TypeScript

Use `tool()` helper with Zod schemas for type-safe tool definitions:

```typescript
import { tool, createSdkMcpServer } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

const myTools = createSdkMcpServer({
  name: "my-tools",
  version: "1.0.0",
  tools: [
    tool(
      "get_user",                           // Tool name
      "Fetch user details by ID",           // Description
      {                                     // Input schema (Zod)
        user_id: z.string().describe("The user's unique ID"),
        include_profile: z.boolean().optional().default(false)
      },
      async (args) => {                     // Handler
        const user = await db.getUser(args.user_id);
        return {
          content: [{
            type: "text",
            text: JSON.stringify(user, null, 2)
          }]
        };
      }
    )
  ]
});
```

### Python

Use the `@tool` decorator:

```python
from claude_agent_sdk import tool, create_sdk_mcp_server
from typing import Any

@tool(
    "get_user",
    "Fetch user details by ID",
    {"user_id": str, "include_profile": bool}  # Simple type hints
)
async def get_user(args: dict[str, Any]) -> dict[str, Any]:
    user = await db.get_user(args["user_id"])
    return {
        "content": [{
            "type": "text",
            "text": json.dumps(user, indent=2)
        }]
    }

my_tools = create_sdk_mcp_server(
    name="my-tools",
    version="1.0.0",
    tools=[get_user]
)
```

---

## Input Schema Patterns

### Simple Types

```typescript
// TypeScript with Zod
{
  name: z.string(),
  count: z.number(),
  enabled: z.boolean(),
  tags: z.array(z.string()),
  metadata: z.record(z.any())  // Record<string, any>
}

// Python with type hints
{"name": str, "count": int, "enabled": bool, "tags": list}
```

### With Constraints

```typescript
// TypeScript
{
  age: z.number().min(0).max(150),
  email: z.string().email(),
  status: z.enum(["active", "inactive", "pending"]),
  priority: z.number().default(5),
  description: z.string().optional()
}
```

### Complex Nested Objects

```typescript
// TypeScript
{
  user: z.object({
    name: z.string(),
    email: z.string().email(),
    preferences: z.object({
      theme: z.enum(["light", "dark"]),
      notifications: z.boolean()
    }).optional()
  }),
  filters: z.array(z.object({
    field: z.string(),
    operator: z.enum(["eq", "ne", "gt", "lt"]),
    value: z.any()
  }))
}
```

### JSON Schema (Advanced Python)

For complex validation beyond type hints:

```python
@tool(
    "advanced_query",
    "Execute advanced database query",
    {
        "type": "object",
        "properties": {
            "table": {"type": "string", "enum": ["users", "orders", "products"]},
            "limit": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 100},
            "filters": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "field": {"type": "string"},
                        "op": {"type": "string", "enum": ["=", "!=", ">", "<"]},
                        "value": {}
                    },
                    "required": ["field", "op", "value"]
                }
            }
        },
        "required": ["table"]
    }
)
async def advanced_query(args: dict[str, Any]) -> dict[str, Any]:
    # Implementation
    pass
```

---

## Registering Tools with Query

```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";

// Create async generator for streaming input (required for MCP)
async function* messages() {
  yield {
    type: "user" as const,
    message: { role: "user" as const, content: "Get user 123" }
  };
}

for await (const message of query({
  prompt: messages(),
  options: {
    mcpServers: {
      "my-tools": myTools  // Register as object, not array
    },
    allowedTools: [
      "mcp__my-tools__get_user",  // Whitelist specific tools
      // Omit to allow all tools
    ],
    maxTurns: 5
  }
})) {
  // Process messages
}
```

---

## Tool Response Format

Tools must return a response with `content` array:

```typescript
// Text response
return {
  content: [{
    type: "text",
    text: "Operation completed successfully"
  }]
};

// Image response
return {
  content: [{
    type: "image",
    data: base64EncodedImage,
    mimeType: "image/png"
  }]
};

// Multiple content blocks
return {
  content: [
    { type: "text", text: "Found 3 results:" },
    { type: "text", text: JSON.stringify(results, null, 2) }
  ]
};

// Error response
return {
  content: [{
    type: "text",
    text: `Error: ${error.message}`
  }],
  isError: true  // Signals error to Claude
};
```

---

## Our Rust SDK Approach

Our `claude_agent_sdk` crate communicates with the Claude Code CLI via JSONL over stdio. Tools are registered differently:

### MCP Server Configuration

```rust
// In QueryOptions
pub struct QueryOptions {
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub allowed_tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    // ...
}

pub enum McpServerConfig {
    Stdio {
        command: String,
        args: Option<Vec<String>>,
        env: Option<HashMap<String, String>>,
    },
    Sse {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    Http {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
}
```

### Defining MCP Tools in Rust

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

Control which tools can be executed:

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

// Built-in implementations:
// - AllowAllPermissions
// - DenyAllPermissions
// - RulesPermissionHandler (tool-specific rules)
// - CallbackPermissionHandler (custom logic)
```

---

## Best Practices

### 1. Write Clear Descriptions

The description helps Claude decide when to use the tool:

```typescript
// BAD - vague
tool("process", "Process data", ...)

// GOOD - specific
tool(
  "calculate_compound_interest",
  "Calculate compound interest for an investment. Returns final amount, interest earned, and ROI percentage.",
  ...
)
```

### 2. Use Descriptive Parameter Names

```typescript
// BAD
{ n: z.number(), s: z.string() }

// GOOD
{
  num_items: z.number().describe("Number of items to retrieve"),
  search_query: z.string().describe("Search term to filter results")
}
```

### 3. Validate Inputs

```typescript
{
  email: z.string().email(),
  age: z.number().min(0).max(150),
  url: z.string().url(),
  date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  status: z.enum(["active", "pending", "completed"])
}
```

### 4. Handle Errors Gracefully

```typescript
tool("fetch_data", "...", schema, async (args) => {
  try {
    const data = await fetchData(args.url);
    return { content: [{ type: "text", text: JSON.stringify(data) }] };
  } catch (error) {
    // Return meaningful error, don't throw
    return {
      content: [{
        type: "text",
        text: `Failed to fetch data: ${error.message}\n\nSuggestions:\n- Check if the URL is accessible\n- Verify network connectivity`
      }],
      isError: true
    };
  }
});
```

### 5. Return Structured Data

```typescript
// BAD - unstructured text
return { content: [{ type: "text", text: "User John, age 30, email john@example.com" }] };

// GOOD - structured JSON
return {
  content: [{
    type: "text",
    text: JSON.stringify({
      name: "John",
      age: 30,
      email: "john@example.com"
    }, null, 2)
  }]
};
```

### 6. Avoid Side Effects Without Confirmation

For destructive operations, consider requiring explicit confirmation:

```typescript
tool(
  "delete_records",
  "Delete records matching filter. Use with caution - this is irreversible.",
  {
    filter: z.object({ /* ... */ }),
    confirm: z.literal(true).describe("Must be true to confirm deletion")
  },
  async (args) => {
    if (!args.confirm) {
      return { content: [{ type: "text", text: "Deletion not confirmed. Set confirm: true to proceed." }] };
    }
    // Proceed with deletion
  }
)
```

### 7. Limit Tool Scope

Create focused tools rather than general-purpose ones:

```typescript
// BAD - too general
tool("database", "Do database stuff", { operation: z.string(), ... })

// GOOD - focused tools
tool("query_users", "Search for users by criteria", { ... })
tool("create_user", "Create a new user account", { ... })
tool("update_user", "Update user profile fields", { ... })
tool("delete_user", "Delete a user account", { ... })
```

---

## Forcing Specific Data Shapes

### Structured Output

Use `outputFormat` option to force Claude's final response to match a schema:

```typescript
for await (const message of query({
  prompt: messages(),
  options: {
    outputFormat: {
      type: "json_schema",
      schema: {
        type: "object",
        properties: {
          analysis: { type: "string" },
          confidence: { type: "number", minimum: 0, maximum: 1 },
          recommendations: {
            type: "array",
            items: { type: "string" }
          }
        },
        required: ["analysis", "confidence", "recommendations"]
      }
    }
  }
})) {
  if (message.type === "result" && message.subtype === "success") {
    const structured = message.structured_output;
    // structured matches the schema
  }
}
```

### Tool Input Validation

Zod schemas enforce input shape at runtime:

```typescript
tool(
  "process_order",
  "Process a customer order",
  {
    order: z.object({
      customer_id: z.string().uuid(),
      items: z.array(z.object({
        product_id: z.string(),
        quantity: z.number().int().positive(),
        price: z.number().positive()
      })).min(1),
      shipping_address: z.object({
        street: z.string(),
        city: z.string(),
        zip: z.string().regex(/^\d{5}(-\d{4})?$/),
        country: z.string().length(2)  // ISO country code
      }),
      payment_method: z.enum(["credit_card", "paypal", "bank_transfer"])
    })
  },
  async (args) => {
    // args.order is guaranteed to match the schema
    // TypeScript knows the exact shape
  }
)
```

---

## Common Tool Patterns

### Database Query Tool

```typescript
tool(
  "query_database",
  "Execute a read-only SQL query against the database",
  {
    query: z.string().describe("SQL SELECT query to execute"),
    params: z.array(z.any()).optional().describe("Query parameters for prepared statement"),
    limit: z.number().max(1000).default(100).describe("Maximum rows to return")
  },
  async (args) => {
    const results = await db.query(args.query, args.params, args.limit);
    return {
      content: [{
        type: "text",
        text: `Found ${results.length} rows:\n${JSON.stringify(results, null, 2)}`
      }]
    };
  }
)
```

### HTTP API Tool

```typescript
tool(
  "api_request",
  "Make authenticated HTTP request to external API",
  {
    service: z.enum(["github", "slack", "stripe"]),
    endpoint: z.string().describe("API endpoint path"),
    method: z.enum(["GET", "POST", "PUT", "DELETE"]).default("GET"),
    body: z.record(z.any()).optional()
  },
  async (args) => {
    const config = getServiceConfig(args.service);
    const response = await fetch(`${config.baseUrl}${args.endpoint}`, {
      method: args.method,
      headers: { Authorization: `Bearer ${config.apiKey}` },
      body: args.body ? JSON.stringify(args.body) : undefined
    });
    return {
      content: [{ type: "text", text: await response.text() }]
    };
  }
)
```

### File Operation Tool

```typescript
tool(
  "write_json_file",
  "Write structured data to a JSON file",
  {
    path: z.string().describe("File path relative to workspace"),
    data: z.any().describe("Data to serialize as JSON"),
    pretty: z.boolean().default(true).describe("Format with indentation")
  },
  async (args) => {
    const content = args.pretty
      ? JSON.stringify(args.data, null, 2)
      : JSON.stringify(args.data);
    await fs.writeFile(args.path, content);
    return {
      content: [{
        type: "text",
        text: `Wrote ${content.length} bytes to ${args.path}`
      }]
    };
  }
)
```

---

## References

- [Official Custom Tools Docs](https://platform.claude.com/docs/en/agent-sdk/custom-tools)
- [TypeScript SDK Reference](https://platform.claude.com/docs/en/agent-sdk/typescript)
- [Python SDK Reference](https://platform.claude.com/docs/en/agent-sdk/python)
- [MCP Protocol](https://modelcontextprotocol.io)
- Our Rust SDK: `crates/claude_agent_sdk/`
