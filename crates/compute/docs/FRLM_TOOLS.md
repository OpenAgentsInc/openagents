# FRLM Tools Integration

This document describes the FRLM (Federated Recursive Language Models) tool integration in the
compute crate, enabling Apple Foundation Models to make recursive LLM sub-calls via the FRLM
conductor.

## Overview

The FRLM tools integration provides a bridge between the Foundation Models bridge (external) and
the Rust FRLM conductor, enabling:

- **Recursive LLM sub-calls**: FM can spawn sub-queries that are executed via FRLM
- **Fragment-based processing**: Load and select document fragments for context
- **Parallel execution**: Execute multiple sub-queries concurrently
- **Verification**: Verify results using multiple tiers (none, redundancy, objective, validated)
- **Budget management**: Track and enforce token/cost budgets
- **Observability**: Full trace event support for debugging and replay

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│              Foundation Models Bridge (localhost:11435)         │
│  ┌───────────────┐    ┌────────────────┐    ┌───────────────┐  │
│  │ @Generable    │───>│ Tool Selection │───>│ HTTP Request  │  │
│  │ FrlmToolCall  │    │ (heuristic/FM) │    │ to Rust       │  │
│  └───────────────┘    └────────────────┘    └───────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼ POST /v1/tools/execute
┌─────────────────────────────────────────────────────────────────┐
│                      Rust Compute Backend                        │
│  ┌───────────────┐    ┌────────────────┐    ┌───────────────┐  │
│  │ AppleFmBackend│───>│ FrlmToolHandler│───>│ FRLM          │  │
│  │ (apple_fm.rs) │    │                │    │ Conductor     │  │
│  └───────────────┘    └────────────────┘    └───────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## The 7 FRLM Tools

### 1. `llm_query_recursive`

Make a recursive sub-LM call. This is the core tool for RLM-style execution.

**Parameters:**
- `prompt` (required): The prompt to send to the sub-LM
- `context` (optional): Additional context to include
- `budget` (optional): Maximum tokens for this sub-query
- `verification` (optional): Verification tier (`none`, `redundancy`, `objective`, `validated`)
- `model` (optional): Model preference

**Example:**
```json
{
  "tool": "llm_query_recursive",
  "arguments": {
    "prompt": "Summarize this section",
    "context": "Section content here...",
    "budget": 500,
    "verification": "none"
  }
}
```

### 2. `load_environment`

Load fragments into the execution context for processing.

**Parameters:**
- `fragments` (required): Array of `{id, content}` objects
- `context_vars` (optional): Key-value context variables

**Example:**
```json
{
  "tool": "load_environment",
  "arguments": {
    "fragments": [
      {"id": "doc-1", "content": "First document..."},
      {"id": "doc-2", "content": "Second document..."}
    ],
    "context_vars": {"task": "summarization"}
  }
}
```

### 3. `select_fragments`

Select relevant fragments from loaded environment based on criteria.

**Parameters:**
- `query` (required): Query to match fragments against
- `max_fragments` (optional): Maximum number of fragments to return
- `filter` (optional): Metadata filter (key-value pairs)

**Example:**
```json
{
  "tool": "select_fragments",
  "arguments": {
    "query": "financial data",
    "max_fragments": 5
  }
}
```

### 4. `execute_parallel`

Execute multiple sub-queries in parallel using the FRLM scheduler.

**Parameters:**
- `queries` (required): Array of `{id, prompt, fragment_id?}` objects
- `fanout` (optional): Maximum concurrent executions (default: 8)
- `timeout_ms` (optional): Timeout per query in milliseconds

**Example:**
```json
{
  "tool": "execute_parallel",
  "arguments": {
    "queries": [
      {"id": "q1", "prompt": "Summarize section 1", "fragment_id": "doc-1"},
      {"id": "q2", "prompt": "Summarize section 2", "fragment_id": "doc-2"}
    ],
    "fanout": 4,
    "timeout_ms": 30000
  }
}
```

### 5. `verify_results`

Verify sub-query results using specified verification tier.

**Parameters:**
- `results` (required): Array of `{query_id, content}` objects
- `tier` (required): Verification tier
  - `none`: Skip verification
  - `redundancy`: N-of-M consensus verification
  - `objective`: JSON schema validation
  - `validated`: Cryptographic attestation
- `schema` (optional): JSON schema for `objective` verification
- `n_of_m` (optional): `{n, m}` for `redundancy` verification

**Example:**
```json
{
  "tool": "verify_results",
  "arguments": {
    "results": [
      {"query_id": "q1", "content": "Result 1"},
      {"query_id": "q2", "content": "Result 2"}
    ],
    "tier": "redundancy",
    "n_of_m": {"n": 2, "m": 3}
  }
}
```

### 6. `check_budget`

Check remaining token budget or update budget allocation.

**Parameters:**
- `action` (required): Budget action
  - `check`: Get remaining budget
  - `reserve`: Allocate tokens
  - `release`: Return tokens
- `tokens` (optional): Number of tokens for reserve/release

**Example:**
```json
{
  "tool": "check_budget",
  "arguments": {
    "action": "check"
  }
}
```

### 7. `get_trace_events`

Get execution trace events for debugging and observability.

**Parameters:**
- `run_id` (optional): Filter by run ID
- `event_types` (optional): Filter by event types
- `limit` (optional): Maximum events to return

**Event types:**
- `query_started`, `query_completed`
- `fragment_loaded`
- `subquery_dispatched`, `subquery_completed`
- `verification_started`, `verification_completed`
- `budget_updated`
- `error`, `warning`, `info`, `debug`

**Example:**
```json
{
  "tool": "get_trace_events",
  "arguments": {
    "event_types": ["subquery_dispatched", "subquery_completed"],
    "limit": 50
  }
}
```

## API Endpoints

### Swift FM Bridge

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/tools` | GET | List all FRLM tools |
| `/v1/tools/select` | POST | Select tool using guided generation |
| `/v1/tools/execute` | POST | Execute a tool |

### Rust Backend

The `AppleFmBackend` exposes:

```rust
// Get tool definitions for registration
pub fn get_frlm_tools(&self) -> Vec<FmToolDefinition>

// Execute a tool
pub async fn execute_frlm_tool(
    &self,
    name: &str,
    arguments: Value,
) -> Result<Value, BackendError>
```

## Files

| File | Description |
|------|-------------|
| `crates/compute/src/frlm_tools.rs` | Tool definitions with JSON schemas |
| `crates/compute/src/frlm_tool_handler.rs` | Tool execution handler |
| `crates/compute/src/backends/apple_fm.rs` | FM backend with FRLM integration |

## Usage Example

```rust
use compute::{create_frlm_tools, FrlmToolHandler};
use compute::backends::AppleFmBackend;
use serde_json::json;

// Get tool definitions
let tools = create_frlm_tools();
println!("Available tools: {:?}", tools.iter().map(|t| &t.name).collect::<Vec<_>>());

// Create backend with FRLM support
let backend = AppleFmBackend::from_env()?;

// Execute a tool
let result = backend.execute_frlm_tool(
    "llm_query_recursive",
    json!({
        "prompt": "What is the capital of France?",
        "budget": 100
    })
).await?;

println!("Result: {}", result);
```

## Configuration

Environment variables:
- `FM_BRIDGE_URL`: Swift FM Bridge URL (default: `http://localhost:11435`)
- `RUST_BACKEND_URL`: Rust backend URL for tool execution (default: `http://localhost:3000`)
