# FRLM Integration in FM Bridge

This document describes the FRLM (Federated Recursive Language Models) integration in the Swift Foundation Models Bridge, enabling FM to make recursive LLM sub-calls.

## Overview

The FM Bridge now supports FRLM tools, allowing Apple Foundation Models to:

- Select appropriate tools using guided generation
- Execute tools via the Rust FRLM backend
- Enable recursive LLM sub-calls (RLM pattern)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   FM Bridge (Swift)                          │
│                                                              │
│  ┌─────────────┐    ┌──────────────┐    ┌───────────────┐  │
│  │ GuidedTypes │    │ ToolHandler  │    │    Server     │  │
│  │ @Generable  │    │              │    │               │  │
│  │ FrlmToolCall│───>│selectFrlmTool│<───│/v1/tools/*    │  │
│  └─────────────┘    │executeFrlmTool│   └───────────────┘  │
│                     └──────┬───────┘                        │
└────────────────────────────┼────────────────────────────────┘
                             │ HTTP POST
                             ▼
┌─────────────────────────────────────────────────────────────┐
│              Rust FRLM Backend (localhost:3000)              │
│                                                              │
│  ┌─────────────┐    ┌──────────────┐    ┌───────────────┐  │
│  │AppleFmBackend│───>│FrlmToolHandler│───>│FrlmConductor │  │
│  └─────────────┘    └──────────────┘    └───────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## API Endpoints

### GET /v1/tools

List all available FRLM tools.

**Response:**
```json
{
  "tools": [
    {"name": "llm_query_recursive", "description": "Make a recursive sub-LM call"},
    {"name": "load_environment", "description": "Load fragments into execution context"},
    {"name": "select_fragments", "description": "Select relevant fragments from loaded environment"},
    {"name": "execute_parallel", "description": "Execute multiple sub-queries in parallel"},
    {"name": "verify_results", "description": "Verify sub-query results using specified verification tier"},
    {"name": "check_budget", "description": "Check remaining token budget or update allocation"},
    {"name": "get_trace_events", "description": "Get execution trace events for debugging"}
  ],
  "count": 7
}
```

### POST /v1/tools/select

Select an FRLM tool based on a prompt using guided generation or heuristics.

**Request:**
```json
{
  "prompt": "What is my remaining budget?",
  "available_tools": ["check_budget", "llm_query_recursive"]
}
```

**Response:**
```json
{
  "tool": "check_budget",
  "arguments": "{\"action\": \"check\"}",
  "reasoning": "Prompt mentions budget/cost/tokens, using check_budget tool"
}
```

### POST /v1/tools/execute

Execute an FRLM tool by forwarding to the Rust backend.

**Request:**
```json
{
  "tool": "llm_query_recursive",
  "arguments": "{\"prompt\": \"Summarize this text\", \"budget\": 500}"
}
```

**Response:**
```json
{
  "result": "{\"query_id\": \"sq-abc123\", \"status\": \"submitted\"}",
  "success": true,
  "error": null
}
```

## Guided Generation Types

### FrlmTool Enum

```swift
@Generable(description: "An FRLM tool for recursive LLM execution")
enum FrlmTool: String, Codable, CaseIterable {
    case llmQueryRecursive = "llm_query_recursive"
    case loadEnvironment = "load_environment"
    case selectFragments = "select_fragments"
    case executeParallel = "execute_parallel"
    case verifyResults = "verify_results"
    case checkBudget = "check_budget"
    case getTraceEvents = "get_trace_events"
}
```

### FrlmToolCall Struct

```swift
@Generable(description: "A tool call request from FM to FRLM")
struct FrlmToolCall: Codable {
    @Guide(description: "The FRLM tool to call", .anyOf([
        "llm_query_recursive",
        "load_environment",
        "select_fragments",
        "execute_parallel",
        "verify_results",
        "check_budget",
        "get_trace_events"
    ]))
    var tool: String

    @Guide(description: "Tool arguments as JSON string")
    var arguments: String
}
```

### Argument Types

```swift
@Generable(description: "Arguments for recursive LLM query")
struct LlmQueryArgs: Codable {
    var prompt: String
    var context: String?
    var budget: Int?
    var verification: String?  // "none", "redundancy", "objective", "validated"
}

@Generable(description: "Arguments for budget management")
struct CheckBudgetArgs: Codable {
    var action: String  // "check", "reserve", "release"
    var tokens: Int?
}
```

## Tool Selection

The `ToolHandler.selectToolHeuristic()` method provides heuristic-based tool selection:

| Pattern in Prompt | Selected Tool |
|-------------------|---------------|
| budget, cost, token | `check_budget` |
| load, fragment, context | `load_environment` |
| parallel, batch, multiple | `execute_parallel` |
| verify, check, validate | `verify_results` |
| trace, debug, log | `get_trace_events` |
| (default) | `llm_query_recursive` |

In production, this can be replaced with actual guided generation:

```swift
let session = LanguageModelSession()
let toolCall: FrlmToolCall = try await session.respond(
    to: prompt,
    generating: FrlmToolCall.self
)
```

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `RUST_BACKEND_URL` | `http://localhost:3000` | Rust FRLM backend URL |

## Files

| File | Description |
|------|-------------|
| `Sources/foundation-bridge/GuidedTypes.swift` | `@Generable` FRLM types |
| `Sources/foundation-bridge/Server.swift` | HTTP endpoints |
| `Sources/foundation-bridge/Handlers/ToolHandler.swift` | Tool handling logic |

## Usage Example

```swift
// 1. List available tools
let tools = try await URLSession.shared.data(
    from: URL(string: "http://localhost:11435/v1/tools")!
)

// 2. Select a tool
var selectRequest = URLRequest(url: URL(string: "http://localhost:11435/v1/tools/select")!)
selectRequest.httpMethod = "POST"
selectRequest.httpBody = try JSONEncoder().encode([
    "prompt": "I need to check how many tokens I have left"
])
let selection = try await URLSession.shared.data(for: selectRequest)

// 3. Execute the selected tool
var executeRequest = URLRequest(url: URL(string: "http://localhost:11435/v1/tools/execute")!)
executeRequest.httpMethod = "POST"
executeRequest.httpBody = try JSONEncoder().encode([
    "tool": "check_budget",
    "arguments": "{\"action\": \"check\"}"
])
let result = try await URLSession.shared.data(for: executeRequest)
```

## Error Handling

All endpoints return errors in a consistent format:

```json
{
  "error": "Error message here"
}
```

HTTP status codes:
- `200`: Success
- `400`: Invalid request (missing body, invalid format)
- `500`: Server error (backend unavailable, execution failed)
