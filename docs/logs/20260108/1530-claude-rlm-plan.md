# RLM + Claude Integration Plan

## Goal

Integrate RLMs (Recursive Language Models) into the Claude execution flow with two modes:
1. **Claude calls RLM**: Claude invokes RLM tools for deep recursive analysis
2. **Claude IS the RLM**: Claude powers the RLM engine as the LlmClient backend

```
┌──────────────────────────────────────────────────────────────────┐
│                     TASK EXECUTION FLOW                           │
└──────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┴─────────────────────┐
        ▼                                           ▼
┌───────────────────┐                    ┌───────────────────┐
│  MODE A: Claude   │                    │  MODE B: Claude   │
│  CALLS RLM        │                    │  AS RLM BACKEND   │
│  (MCP tools)      │                    │  (LlmClient)      │
├───────────────────┤                    ├───────────────────┤
│  Claude decides   │                    │  RlmEngine uses   │
│  when to fan out  │                    │  Claude for       │
│  to RLM analysis  │                    │  recursive calls  │
└───────────────────┘                    └───────────────────┘
        │                                           │
        ▼                                           ▼
┌───────────────────┐                    ┌───────────────────┐
│  rlm_query tool   │                    │  ClaudeLlmClient  │
│  rlm_fanout tool  │                    │  impl LlmClient   │
└───────────────────┘                    └───────────────────┘
```

---

## Current State

### RLM Implementation (`crates/rlm/`)
- **`LlmClient` trait**: `async fn complete(&self, prompt, max_tokens) -> Result<LlmResponse>`
- **`RlmEngine<C, E>`**: Generic over LlmClient and ExecutionEnvironment
- **`with_sub_client()`**: Separate models for root and sub-queries
- **Orchestrated mode**: Engine-driven chunking for large documents
- **`llm_query()` sub-calls**: Recursive queries within execution

### FRLM Implementation (`crates/frlm/`)
- **`FrlmConductor`**: Distributed fanout across swarm
- **`SubQueryScheduler`**: Async fanout with budget tracking
- **`Venue` enum**: Local, Swarm, Datacenter execution venues

### Claude Agent SDK (`crates/claude-agent-sdk/`)
- **`McpServerConfig`**: Stdio/Sse/Http MCP server configs
- **`AgentDefinition`**: Custom subagents with prompt, tools, model
- **`HookEvent`**: PreToolUse, PostToolUse, etc.
- **`QueryOptions`**: Rich configuration including hooks and MCP servers

---

## Implementation Plan

### Step 1: Create ClaudeLlmClient (Mode B)

Implement `LlmClient` trait using Claude as the backend.

**File:** `crates/rlm/src/claude_client.rs` (NEW)

```rust
//! Claude LLM client for RLM.
//!
//! Allows RlmEngine to use Claude (Pro/Max) as its inference backend.

use async_trait::async_trait;
use claude_agent_sdk::{query, QueryOptions, SdkMessage, SdkResultMessage};
use futures::StreamExt;
use std::path::PathBuf;

use crate::client::{LlmClient, LlmResponse};
use crate::error::RlmError;

/// LLM client that uses Claude via claude-agent-sdk.
pub struct ClaudeLlmClient {
    workspace_root: PathBuf,
    model: Option<String>,
}

impl ClaudeLlmClient {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            model: None,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[async_trait]
impl LlmClient for ClaudeLlmClient {
    async fn complete(
        &self,
        prompt: &str,
        _max_tokens: Option<usize>,
    ) -> Result<LlmResponse, RlmError> {
        let mut options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .max_turns(1)  // Single turn for RLM queries
            .tools(claude_agent_sdk::ToolsConfig::none());  // No tools for raw completion

        if let Some(ref model) = self.model {
            options = options.model(model);
        }

        let mut stream = query(prompt, options).await
            .map_err(|e| RlmError::ClientError(format!("Claude query failed: {}", e)))?;

        let mut content = String::new();

        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(SdkMessage::Assistant(msg)) => {
                    if let Some(text) = &msg.message.content {
                        content.push_str(text);
                    }
                }
                Ok(SdkMessage::Result(SdkResultMessage::Success(s))) => {
                    content = s.result;
                    break;
                }
                Err(e) => {
                    return Err(RlmError::ClientError(format!("Stream error: {}", e)));
                }
                _ => {}
            }
        }

        Ok(LlmResponse::new(content))
    }
}
```

### Step 2: Create RLM MCP Tools (Mode A)

Expose RLM capabilities as MCP tools that Claude can invoke.

**File:** `crates/rlm/src/mcp_tools.rs` (NEW)

```rust
//! RLM MCP tools for Claude integration.
//!
//! Exposes RLM capabilities as MCP tools:
//! - rlm_query: Run recursive analysis on a query
//! - rlm_fanout: Distribute query across swarm (if available)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Tool: rlm_query
/// Deep recursive analysis using prompt-execute-loop.
#[derive(Debug, Serialize, Deserialize)]
pub struct RlmQueryInput {
    /// The query to analyze
    pub query: String,
    /// Optional context (file content, data, etc.)
    pub context: Option<String>,
    /// Max iterations (default: 10)
    pub max_iterations: Option<u32>,
    /// Use orchestrated mode for large context (default: auto)
    pub orchestrated: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RlmQueryOutput {
    pub answer: String,
    pub iterations: u32,
    pub execution_log: Vec<String>,
}

/// Tool: rlm_fanout
/// Distribute query across multiple providers (swarm or datacenter).
#[derive(Debug, Serialize, Deserialize)]
pub struct RlmFanoutInput {
    /// The query to distribute
    pub query: String,
    /// Context to analyze
    pub context: String,
    /// Number of parallel workers (default: 3)
    pub workers: Option<u32>,
    /// Venue: "local", "swarm", "datacenter"
    pub venue: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RlmFanoutOutput {
    pub answer: String,
    pub worker_results: Vec<WorkerResult>,
    pub total_cost_sats: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerResult {
    pub worker_id: String,
    pub answer: String,
    pub confidence: f32,
}

/// MCP tool definitions for RLM.
pub fn rlm_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "rlm_query",
            "description": "Run deep recursive analysis using RLM (Recursive Language Model). \
                           Use when you need to analyze complex problems iteratively, \
                           execute code to verify hypotheses, or process large documents.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The analysis query"
                    },
                    "context": {
                        "type": "string",
                        "description": "Optional context (file contents, data)"
                    },
                    "max_iterations": {
                        "type": "integer",
                        "description": "Max analysis iterations (default: 10)"
                    }
                },
                "required": ["query"]
            }
        }),
        serde_json::json!({
            "name": "rlm_fanout",
            "description": "Distribute analysis across multiple workers. \
                           Use for large documents that benefit from parallel analysis, \
                           or when you want consensus from multiple providers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The analysis query"
                    },
                    "context": {
                        "type": "string",
                        "description": "Context to analyze"
                    },
                    "workers": {
                        "type": "integer",
                        "description": "Number of parallel workers (default: 3)"
                    },
                    "venue": {
                        "type": "string",
                        "enum": ["local", "swarm", "datacenter"],
                        "description": "Where to run workers"
                    }
                },
                "required": ["query", "context"]
            }
        })
    ]
}
```

### Step 3: Create RLM MCP Server

Standalone MCP server binary that exposes RLM tools.

**File:** `crates/rlm/src/bin/rlm-mcp-server.rs` (NEW)

```rust
//! RLM MCP Server
//!
//! Exposes RLM tools via stdio MCP protocol.
//! Usage: claude --mcp-server "rlm:stdio:rlm-mcp-server"

use rlm::mcp_tools::{rlm_tool_definitions, RlmQueryInput, RlmFanoutInput};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

#[tokio::main]
async fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.expect("Failed to read line");
        let request: Value = serde_json::from_str(&line).expect("Invalid JSON");

        let response = match request["method"].as_str() {
            Some("initialize") => json!({
                "protocolVersion": "0.1.0",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "rlm-mcp-server", "version": "0.1.0" }
            }),
            Some("tools/list") => json!({
                "tools": rlm_tool_definitions()
            }),
            Some("tools/call") => {
                let tool_name = request["params"]["name"].as_str().unwrap_or("");
                let args = &request["params"]["arguments"];
                handle_tool_call(tool_name, args).await
            }
            _ => json!({ "error": "Unknown method" })
        };

        let output = json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": response
        });

        writeln!(stdout, "{}", output).expect("Failed to write");
        stdout.flush().expect("Failed to flush");
    }
}

async fn handle_tool_call(tool_name: &str, args: &Value) -> Value {
    match tool_name {
        "rlm_query" => {
            let input: RlmQueryInput = serde_json::from_value(args.clone())
                .unwrap_or_else(|_| RlmQueryInput {
                    query: args["query"].as_str().unwrap_or("").to_string(),
                    context: args["context"].as_str().map(String::from),
                    max_iterations: None,
                    orchestrated: None,
                });
            // Execute RLM query
            execute_rlm_query(input).await
        }
        "rlm_fanout" => {
            let input: RlmFanoutInput = serde_json::from_value(args.clone())
                .unwrap_or_else(|_| RlmFanoutInput {
                    query: args["query"].as_str().unwrap_or("").to_string(),
                    context: args["context"].as_str().unwrap_or("").to_string(),
                    workers: None,
                    venue: None,
                });
            // Execute RLM fanout
            execute_rlm_fanout(input).await
        }
        _ => json!({ "error": format!("Unknown tool: {}", tool_name) })
    }
}

async fn execute_rlm_query(input: RlmQueryInput) -> Value {
    // Implementation connects to RlmEngine
    // For now, return placeholder
    json!({
        "content": [{
            "type": "text",
            "text": format!("RLM analysis for: {}", input.query)
        }]
    })
}

async fn execute_rlm_fanout(input: RlmFanoutInput) -> Value {
    // Implementation connects to FrlmConductor
    json!({
        "content": [{
            "type": "text",
            "text": format!("RLM fanout for: {} ({} workers)",
                input.query,
                input.workers.unwrap_or(3))
        }]
    })
}
```

### Step 4: Add RLM Custom Agent to Claude

Define an RLM subagent that Claude can delegate to.

**File:** `crates/adjutant/src/rlm_agent.rs` (NEW)

```rust
//! RLM Custom Agent for Claude.
//!
//! Defines a Claude subagent that uses RLM patterns for deep analysis.

use claude_agent_sdk::{AgentDefinition, AgentModel};

/// Create the RLM agent definition.
pub fn rlm_agent_definition() -> AgentDefinition {
    AgentDefinition {
        description: "Deep recursive analysis agent. Use for complex problems \
                     requiring iterative code execution, large document analysis, \
                     or multi-step reasoning with verification.".to_string(),
        prompt: RLM_AGENT_PROMPT.to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Bash".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ]),
        disallowed_tools: Some(vec![
            "Edit".to_string(),
            "Write".to_string(),
        ]),
        model: Some(AgentModel::Sonnet),  // Use Sonnet for cost efficiency
        critical_system_reminder_experimental: None,
    }
}

const RLM_AGENT_PROMPT: &str = r#"
You are an RLM (Recursive Language Model) analysis agent. Your approach:

1. DECOMPOSE: Break the problem into verifiable sub-questions
2. EXECUTE: Write Python code to gather evidence
3. VERIFY: Check your hypotheses against execution results
4. ITERATE: Refine your analysis based on findings
5. SYNTHESIZE: Combine findings into a final answer

## Execution Pattern

For each analysis step:
1. State your hypothesis
2. Write Python code to test it
3. Execute and observe results
4. Update your understanding

## Code Execution

Use ```python blocks for analysis code. Available functions:
- search_context(pattern): Find text in loaded context
- llm_query(prompt, text): Sub-query for complex reasoning

## Output Format

When done, output your final answer as:
FINAL: [your synthesized answer]

Do not output FINAL until you have verified your answer through code execution.
"#;
```

### Step 5: Update ClaudeExecutor to Support RLM

Modify ClaudeExecutor to include RLM tools and agent.

**File:** `crates/adjutant/src/claude_executor.rs` (UPDATE)

```rust
// Add to existing ClaudeExecutor

use crate::rlm_agent::rlm_agent_definition;

impl ClaudeExecutor {
    /// Execute a task with RLM support.
    pub async fn execute_with_rlm(
        &self,
        task: &Task,
        context: &str,
        enable_rlm_tools: bool,
    ) -> Result<TaskResult, AdjutantError> {
        let mut options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .max_turns(30)  // More turns for RLM-style iteration
            .include_partial_messages(true);

        // Add RLM custom agent
        options = options.agent("rlm-analyzer", rlm_agent_definition());

        // Add RLM MCP server if tools enabled
        if enable_rlm_tools {
            options = options.mcp_server(
                "rlm",
                McpServerConfig::Stdio {
                    command: "rlm-mcp-server".to_string(),
                    args: None,
                    env: None,
                }
            );
        }

        // Build prompt with context
        let prompt = if context.is_empty() {
            task.to_prompt()
        } else {
            format!(
                "{}\n\n## Context\n\n{}",
                task.to_prompt(),
                context
            )
        };

        // Execute query
        let mut stream = query(&prompt, options).await.map_err(|e| {
            AdjutantError::ExecutionFailed(format!("Claude query failed: {}", e))
        })?;

        // ... rest of execution handling
    }
}
```

### Step 6: Add RLM to Adjutant Task Execution

Update main execution path to use RLM for complex tasks.

**File:** `crates/adjutant/src/lib.rs` (UPDATE)

```rust
// Add to Adjutant impl

impl Adjutant {
    /// Execute a task - decides whether to use RLM based on complexity.
    pub async fn execute(&mut self, task: &Task) -> Result<TaskResult, AdjutantError> {
        let plan = self.plan_task(task).await?;

        // Use RLM for high complexity or large context
        let use_rlm = plan.complexity >= Complexity::High
            || plan.estimated_tokens > 50_000
            || task.description.contains("analyze")
            || task.description.contains("recursive");

        if has_claude_cli() {
            let executor = ClaudeExecutor::new(&self.workspace_root);
            if use_rlm {
                tracing::info!("Using Claude with RLM support");
                return executor.execute_with_rlm(task, &context, true).await;
            }
            return executor.execute(task, &context, &mut self.tools).await;
        }

        // Fallback to TieredExecutor or analysis-only
        // ...existing fallback code...
    }
}
```

---

## Files to Create/Modify

| File | Change |
|------|--------|
| `crates/rlm/src/claude_client.rs` | NEW - ClaudeLlmClient implementing LlmClient |
| `crates/rlm/src/mcp_tools.rs` | NEW - RLM MCP tool definitions |
| `crates/rlm/src/bin/rlm-mcp-server.rs` | NEW - Standalone MCP server |
| `crates/rlm/src/lib.rs` | Export new modules |
| `crates/rlm/Cargo.toml` | Add claude-agent-sdk dependency |
| `crates/adjutant/src/rlm_agent.rs` | NEW - RLM custom agent definition |
| `crates/adjutant/src/claude_executor.rs` | Add execute_with_rlm method |
| `crates/adjutant/src/lib.rs` | Add RLM routing logic |

---

## Execution Modes

### Mode A: Claude Calls RLM Tools

```
User Query: "Analyze this 500KB codebase for security issues"
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│  ClaudeExecutor with RLM MCP Server                     │
│  Claude sees: rlm_query, rlm_fanout tools               │
└─────────────────────────────────────────────────────────┘
    │
    │  Claude decides: "This needs deep analysis"
    ▼
┌─────────────────────────────────────────────────────────┐
│  Claude calls: rlm_fanout({                             │
│    query: "Find security vulnerabilities",              │
│    context: <codebase>,                                 │
│    workers: 5,                                          │
│    venue: "local"                                       │
│  })                                                     │
└─────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│  RLM MCP Server                                         │
│  - Chunks codebase                                      │
│  - Runs 5 parallel RlmEngine instances                  │
│  - Synthesizes findings                                 │
│  - Returns to Claude                                    │
└─────────────────────────────────────────────────────────┘
    │
    ▼
Claude synthesizes final security report
```

### Mode B: Claude AS the RLM Backend

```
pylon rlm "Explain quantum entanglement" --backend claude
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│  RlmEngine<ClaudeLlmClient, PythonExecutor>             │
│  - Claude for llm_query() calls                         │
│  - Python for code execution                            │
└─────────────────────────────────────────────────────────┘
    │
    │  Iteration 1: Claude generates hypothesis code
    │  Iteration 2: Execute, observe results
    │  Iteration 3: Claude refines understanding
    │  Iteration N: FINAL answer
    ▼
RlmResult { output: "...", iterations: N, ... }
```

---

## Configuration

```bash
# Enable RLM tools in Claude sessions
export ADJUTANT_ENABLE_RLM=1

# Use Claude as RLM backend (instead of local models)
export RLM_BACKEND=claude

# Swarm fanout venue
export FRLM_DEFAULT_VENUE=swarm  # or "local", "datacenter"
```

---

## A/B Testing Different Approaches

The implementation supports comparing different execution strategies:

| Strategy | When to Use | Config |
|----------|-------------|--------|
| Claude Direct | Simple tasks, good quality | Default |
| Claude + RLM Tools | Complex analysis, large docs | `ADJUTANT_ENABLE_RLM=1` |
| Claude as RLM Backend | Recursive reasoning tasks | `RLM_BACKEND=claude` |
| Swarm Fanout | Consensus, distributed analysis | `FRLM_DEFAULT_VENUE=swarm` |

Metrics to track:
- Task completion rate
- Token usage / cost
- Time to completion
- Answer quality (manual review)

---

## Benefits

| Approach | Pros | Cons |
|----------|------|------|
| Claude + RLM Tools | Claude decides when to use RLM | Extra tool overhead |
| Claude as Backend | Best quality for recursive tasks | Higher cost (Claude per iteration) |
| Swarm Fanout | Distributed, cost-effective | Requires network, coordination |
| Mixed Mode | Flexibility | Complexity in routing |
