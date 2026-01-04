# FRLM SubQuery Tools Support

This document describes the tools support added to the `SubQuery` type, enabling tool-augmented recursive LLM execution.

## Overview

The `SubQuery` struct now includes a `tools` field that specifies which FRLM tools are available for that particular sub-query. This enables the RLM pattern where sub-queries can themselves spawn additional tool calls.

## SubQuery Type

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQuery {
    /// Unique identifier for this sub-query.
    pub id: String,
    /// The prompt to send to the LLM.
    pub prompt: String,
    /// Optional fragment this query operates on.
    pub fragment_id: Option<String>,
    /// Model preference (if any).
    pub model: Option<String>,
    /// Maximum tokens for response.
    pub max_tokens: Option<u32>,
    /// Available tools for this sub-query (FRLM tool names).
    #[serde(default)]
    pub tools: Vec<String>,
}
```

## Usage

### Creating a Sub-Query with Tools

```rust
use frlm::SubQuery;

// Create a sub-query with specific tools enabled
let query = SubQuery::new("sq-1", "Analyze this document")
    .with_fragment("doc-1")
    .with_max_tokens(500)
    .with_tools(vec![
        "llm_query_recursive".to_string(),
        "verify_results".to_string(),
    ]);
```

### Available Tools

The following FRLM tools can be specified:

| Tool Name | Description |
|-----------|-------------|
| `llm_query_recursive` | Make recursive sub-LM calls |
| `load_environment` | Load fragments into context |
| `select_fragments` | Select relevant fragments |
| `execute_parallel` | Execute sub-queries in parallel |
| `verify_results` | Verify sub-query results |
| `check_budget` | Check/update token budget |
| `get_trace_events` | Get execution trace events |

### Serialization

The `tools` field defaults to an empty vector when deserializing JSON that doesn't include it:

```json
{
  "id": "sq-1",
  "prompt": "Analyze this",
  "tools": ["llm_query_recursive", "check_budget"]
}
```

Empty tools array (default):
```json
{
  "id": "sq-2",
  "prompt": "Simple query"
}
// tools defaults to []
```

## Integration with FRLM Conductor

When the FRLM conductor executes a sub-query, it can check the `tools` field to determine which capabilities the sub-LM has access to:

```rust
impl FrlmConductor {
    async fn execute_subquery(&self, query: &SubQuery) -> Result<SubQueryResult> {
        // Check if recursive queries are allowed
        if query.tools.contains(&"llm_query_recursive".to_string()) {
            // Enable recursive sub-query capability
        }

        // Check if verification is enabled
        if query.tools.contains(&"verify_results".to_string()) {
            // Enable result verification
        }

        // ... execute query
    }
}
```

## Backward Compatibility

The `#[serde(default)]` attribute ensures backward compatibility:
- Existing JSON without the `tools` field will deserialize with an empty `Vec`
- Existing code that doesn't use tools continues to work unchanged
- The `with_tools()` builder method is additive

## Related Files

| File | Description |
|------|-------------|
| `crates/frlm/src/types.rs` | SubQuery type definition |
| `crates/compute/src/frlm_tools.rs` | Tool definitions |
| `crates/compute/src/frlm_tool_handler.rs` | Tool execution |
