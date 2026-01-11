# DSPy + Agent SDK Integration Strategy

This document analyzes how to integrate DSPy (via dsrs) with AI coding agent SDKs (Codex Agent SDK, Codex Agent SDK, and any future ACP-compatible agents) for constrained, optimizable agent execution.

## Executive Summary

**Recommendation: DSPy-Controlled Flow with SDK Execution**

DSPy should control the *decision-making flow* (planning, routing, verification) while Agent SDKs handle *execution* when agent tools are needed. This creates a hybrid architecture where:

1. DSPy signatures make structured decisions about what to do
2. DSPy orchestrates the overall task flow
3. Agent SDKs (Codex, Codex, or ACP) execute tool-heavy subtasks via constrained queries
4. Output is constrained to DSPy signature schemas using structured outputs

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            DSPy ORCHESTRATOR                                 │
│                                                                              │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────────┐  │
│  │ ComplexityPipeline│ →  │ DelegationPipeline│ →  │VerificationPipeline │  │
│  └──────────────────┘    └──────────────────┘    └──────────────────────┘  │
│           │                       │                         │                │
│           ▼                       ▼                         ▼                │
│      Local LM               Route Decision            Validate Output        │
│     (fast/cheap)         ┌────┬────┬────┐            (JSON Schema)           │
│                          │    │    │    │                                    │
│                   local  │ codex  │ codex                                   │
│                   tools  │ agent   │ agent                                   │
│                     │    │    │    │    │                                    │
│                     ▼    │    ▼    │    ▼                                    │
│              ┌──────────┐│┌───────────────────────────────────────────────┐ │
│              │dsrs tools│││         UNIFIED ACP ADAPTER LAYER             │ │
│              │(grep/    │││                                               │ │
│              │ edit)    │││  ┌─────────────────┐  ┌─────────────────────┐ │ │
│              └──────────┘││  │ Codex Agent SDK│  │  Codex Agent SDK    │ │ │
│                          ││  │                 │  │                     │ │ │
│                          ││  │ QueryOptions {  │  │ ThreadOptions {     │ │ │
│                          ││  │  output_format, │  │  output_schema,     │ │ │
│                          ││  │  max_turns,     │  │  sandbox_mode,      │ │ │
│                          ││  │  permission_mode│  │  approval_policy    │ │ │
│                          ││  │ }               │  │ }                   │ │ │
│                          ││  └─────────────────┘  └─────────────────────┘ │ │
│                          ││                                               │ │
│                          ││  Returns: structured output matching DSPy     │ │
│                          ││           signature schema                    │ │
│                          │└───────────────────────────────────────────────┘ │
│                          │                  │                                │
│                          └──────────────────┼────────────────────────────────┘
│                                             ▼
│                               Validate against signature
│                               Feed back to orchestrator
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Supported Agent SDKs

| SDK | Provider | Structured Output | Sandbox Control | Hooks |
|-----|----------|-------------------|-----------------|-------|
| **Codex Agent SDK** | OpenAI | `output_format` (JSON Schema) | Permission modes | Yes |
| **Codex Agent SDK** | OpenAI | `output_schema` (JSON Schema) | `SandboxMode` enum | No |
| **ACP Adapter** | OpenAgents | Via underlying SDK | Via underlying SDK | Via client |

### Codex Agent SDK

```rust
use codex_agent_sdk::{query, QueryOptions, OutputFormat, PermissionMode};

let options = QueryOptions::new()
    .output_format(OutputFormat {
        format_type: "json_schema".to_string(),
        schema: my_schema,
    })
    .max_turns(5)
    .permission_mode(PermissionMode::AcceptEdits);

let stream = query(&prompt, options).await?;
```

### Codex Agent SDK

```rust
use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions, SandboxMode};

let mut thread = Codex::new().start_thread(
    ThreadOptions::new()
        .sandbox_mode(SandboxMode::WorkspaceWrite)
        .model("gpt-4o")
);

let turn = thread.run(&prompt, TurnOptions::new()
    .output_schema(my_schema)
).await?;
```

### ACP Adapter (Unified Interface)

The ACP adapter provides a unified interface for both SDKs via the Agent Client Protocol:

```rust
use acp_adapter::{AcpAgentConnection, AgentCommand};

// Works with any ACP-compatible agent (Codex, Codex, custom)
let connection = AcpAgentConnection::stdio(
    "codex",  // or "codex"
    AgentCommand::new("codex").args(["--output-format", "stream-json"]),
    &cwd,
).await?;

let session = connection.new_session(cwd.clone()).await?;
connection.prompt(&session.session_id, &prompt).await?;
```

## Architecture Options Analyzed

### Option 1: DSPy Controls Flow, SDK Executes (Recommended)

DSPy drives the decision-making pipeline. When a decision routes to an agent, we use the appropriate SDK with:

- **Structured outputs** matching the DSPy signature's output schema
- **Turn/budget limits** for predictable execution
- **Tool/sandbox constraints** to limit agent capabilities
- **Hooks** (Codex) or policies (Codex) for validation

```rust
// DSPy orchestrator decides to delegate
let delegation = delegation_pipeline.decide(&input).await?;

match delegation.delegation_target.as_str() {
    "codex_agent" => {
        let schema = build_schema_from_signature::<ExecutionOutputSignature>();

        let options = QueryOptions::new()
            .output_format(OutputFormat {
                format_type: "json_schema".to_string(),
                schema,
            })
            .max_turns(5)
            .permission_mode(PermissionMode::AcceptEdits);

        let result = codex_executor.execute(&prompt, options).await?;
    }
    "codex_agent" => {
        let schema = build_schema_from_signature::<ExecutionOutputSignature>();

        let options = TurnOptions::new().output_schema(schema);
        let result = codex_executor.execute(&prompt, options).await?;
    }
    "local_tools" => {
        let result = local_executor.execute(&prompt).await?;
    }
}
```

**Pros:**
- DSPy maintains control over the overall flow
- Outputs are constrained to signature schemas
- Training data is collected at DSPy layer
- Self-improvement loop works naturally
- Can choose best agent per task

**Cons:**
- Adds latency for subprocess spawning
- Multiple LLM "brains" (DSPy router + agent executor)

### Option 2: ACP Unified Interface

Use ACP adapter as the single abstraction layer, hiding SDK differences:

```rust
pub struct UnifiedAgentExecutor {
    connections: HashMap<String, AcpAgentConnection>,
}

impl UnifiedAgentExecutor {
    pub async fn execute(
        &self,
        agent: &str,  // "codex" or "codex"
        prompt: &str,
    ) -> Result<ExecutionResult> {
        let connection = self.connections.get(agent)
            .ok_or_else(|| anyhow::anyhow!("Unknown agent: {}", agent))?;

        let session = connection.new_session(self.cwd.clone()).await?;
        connection.prompt(&session.session_id, prompt).await?;

        // Collect notifications and convert to unified result
        // ACP normalizes events from both Codex and Codex
        self.collect_result(&session).await
    }
}
```

**Pros:**
- Single interface for all agents
- Event normalization built-in
- Session recording/replay for free
- Future agents "just work"

**Cons:**
- ACP doesn't yet support structured output constraints
- Less fine-grained control than direct SDK

### Option 3: Bidirectional Delegation

Codex and Codex can delegate to each other based on task requirements:

```rust
// DSPy decides Codex should handle this, but Codex can delegate to Codex
let decision = delegation_pipeline.decide(&input).await?;

if decision.delegation_target == "codex_agent" {
    // Codex gets the task
    let codex_result = codex_executor.execute(&prompt, options).await?;

    // If Codex decides Codex is better for a subtask, it invokes Codex
    // via the /codex skill or MCP tool
}
```

This enables:
- Codex delegating compute-heavy refactoring to Codex
- Codex delegating analysis/review to Codex
- Multi-agent workflows leveraging different strengths

## Recommended Architecture: Hybrid DSPy-Controlled Execution

### Core Principle

**DSPy controls policy; Agent SDKs execute actions.**

DSPy signatures should drive:
- Task complexity classification
- Agent selection (Codex vs Codex vs local)
- Execution routing decisions
- Verification and validation
- Training data collection

Agent SDKs should execute:
- Multi-file code changes
- Complex tool sequences
- Tasks requiring LLM reasoning

### Implementation Pattern

#### 1. Unified Execution Signature

Define a signature that works with any agent:

```rust
#[Signature]
struct AgentExecutionSignature {
    /// Execute a coding task using an AI agent's tools.
    /// Return the execution result in structured format.

    /// The specific task to execute
    #[input]
    pub task: String,

    /// Files that may need to be modified
    #[input]
    pub relevant_files: String,

    /// Constraints on execution (JSON)
    #[input]
    pub constraints: String,

    /// Summary of changes made
    #[output]
    pub changes_summary: String,

    /// Files that were modified (JSON array)
    #[output]
    pub modified_files: String,

    /// Whether the task succeeded
    #[output]
    pub success: bool,

    /// Error message if failed
    #[output]
    pub error: String,
}
```

#### 2. Multi-Agent Executor

Support both Codex and Codex:

```rust
pub struct MultiAgentExecutor {
    codex: Option<CodexSdkExecutor>,
    codex: Option<CodexSdkExecutor>,
}

impl MultiAgentExecutor {
    pub async fn execute(
        &self,
        agent: AgentType,
        task: &str,
        constraints: &ExecutionConstraints,
    ) -> Result<AgentExecutionResult> {
        // Build JSON schema from signature output fields
        let output_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "changes_summary": { "type": "string" },
                "modified_files": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "success": { "type": "boolean" },
                "error": { "type": "string" }
            },
            "required": ["changes_summary", "modified_files", "success"]
        });

        match agent {
            AgentType::Codex => {
                let executor = self.codex.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Codex not configured"))?;

                let options = QueryOptions::new()
                    .model(&constraints.model)
                    .max_turns(constraints.max_turns)
                    .permission_mode(PermissionMode::AcceptEdits)
                    .output_format(OutputFormat {
                        format_type: "json_schema".to_string(),
                        schema: output_schema,
                    });

                executor.execute(task, options).await
            }
            AgentType::Codex => {
                let executor = self.codex.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Codex not configured"))?;

                let thread_options = ThreadOptions::new()
                    .model(&constraints.model)
                    .sandbox_mode(SandboxMode::WorkspaceWrite);

                let turn_options = TurnOptions::new()
                    .output_schema(output_schema);

                executor.execute(task, thread_options, turn_options).await
            }
        }
    }
}
```

#### 3. Agent Selection Pipeline

DSPy signature for choosing the best agent:

```rust
#[Signature]
struct AgentSelectionSignature {
    /// Agent Selector: Choose the best AI agent for this task.
    /// Consider: task type, complexity, required capabilities.
    /// - codex: Better for analysis, architecture, multi-step reasoning
    /// - codex: Better for large refactoring, file operations, sandbox control
    /// - local: Simple edits, fast operations, no LLM needed

    /// Description of the task
    #[input]
    pub task_description: String,

    /// Complexity level (Low/Medium/High/VeryHigh)
    #[input]
    pub complexity: String,

    /// Number of files likely affected
    #[input]
    pub file_count: String,

    /// Best agent for this task: codex, codex, or local
    #[output]
    pub agent: String,

    /// Explanation of the selection
    #[output]
    pub reasoning: String,

    /// Confidence in selection (0.0 to 1.0)
    #[output]
    pub confidence: f32,
}
```

#### 4. Complete Orchestrator

```rust
pub struct AutopilotOrchestrator {
    complexity_pipeline: ComplexityPipeline,
    agent_selection_pipeline: AgentSelectionPipeline,
    verification_pipeline: VerificationPipeline,
    multi_agent_executor: MultiAgentExecutor,
    local_executor: LocalToolExecutor,
    training_collector: TrainingCollector,
}

impl AutopilotOrchestrator {
    pub async fn execute_task(&self, task: &str) -> Result<TaskResult> {
        // 1. Classify complexity (DSPy signature on local/fast LM)
        let complexity = self.complexity_pipeline.classify(&ComplexityInput {
            task_description: task.to_string(),
            ..Default::default()
        }).await?;

        // 2. Select agent (DSPy signature)
        let selection = self.agent_selection_pipeline.select(&AgentSelectionInput {
            task_description: task.to_string(),
            complexity: complexity.complexity.clone(),
            file_count: complexity.estimated_file_count.to_string(),
        }).await?;

        // 3. Execute based on selection
        let execution_result = match selection.agent.as_str() {
            "codex" => {
                let constraints = ExecutionConstraints {
                    model: "codex-sonnet-4-5".to_string(),
                    max_turns: self.max_turns_for_complexity(&complexity),
                };
                self.multi_agent_executor.execute(
                    AgentType::Codex, task, &constraints
                ).await?
            }
            "codex" => {
                let constraints = ExecutionConstraints {
                    model: "gpt-4o".to_string(),
                    max_turns: self.max_turns_for_complexity(&complexity),
                };
                self.multi_agent_executor.execute(
                    AgentType::Codex, task, &constraints
                ).await?
            }
            "local" => {
                self.local_executor.execute(task).await?
            }
            _ => anyhow::bail!("Unknown agent: {}", selection.agent),
        };

        // 4. Verify result (DSPy signature)
        let verification = self.verification_pipeline.verify(&VerificationInput {
            task: task.to_string(),
            result: execution_result.changes_summary.clone(),
        }).await?;

        // 5. Collect training data if high confidence
        if verification.confidence > 0.8 && execution_result.success {
            self.training_collector.record(TrainingExample {
                input: task.to_string(),
                complexity: complexity.clone(),
                agent_selection: selection.clone(),
                result: execution_result.clone(),
                verification: verification.clone(),
            });
        }

        Ok(TaskResult {
            success: verification.is_valid,
            agent_used: selection.agent,
            execution: execution_result,
            verification,
        })
    }
}
```

## Structured Output Mapping

Both SDKs support JSON Schema for structured output. Map DSPy signature fields:

```rust
/// Generate JSON Schema from a DSPy signature's output fields.
pub fn signature_to_json_schema<S: MetaSignature>() -> serde_json::Value {
    let sig = S::new();
    let mut properties = serde_json::Map::new();
    let mut required = vec![];

    for field in sig.output_fields() {
        let field_schema = match field.field_type.as_str() {
            "String" => serde_json::json!({"type": "string"}),
            "bool" => serde_json::json!({"type": "boolean"}),
            "f32" | "f64" => serde_json::json!({"type": "number"}),
            "i32" | "i64" | "u32" | "u64" => serde_json::json!({"type": "integer"}),
            _ => serde_json::json!({"type": "string"}),
        };

        properties.insert(field.name.clone(), field_schema);
        required.push(serde_json::Value::String(field.name.clone()));
    }

    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
```

## Model Mixing Strategy

Different parts of the pipeline benefit from different models:

| Pipeline Stage | Codex Model | Codex Model | Reason |
|----------------|--------------|-------------|--------|
| ComplexityPipeline | Haiku | gpt-4o-mini | Fast classification, low cost |
| AgentSelectionPipeline | Haiku | gpt-4o-mini | Simple routing decision |
| Execution (High complexity) | Opus | gpt-4o-with-reasoning | Complex multi-tool reasoning |
| Execution (Medium) | Sonnet | gpt-4o | Balanced quality/cost |
| VerificationPipeline | Haiku | gpt-4o-mini | Fast validation |

```rust
fn select_model(agent: AgentType, stage: PipelineStage, complexity: &str) -> String {
    match (agent, stage, complexity) {
        (AgentType::Codex, PipelineStage::Classification, _) => "codex-haiku".to_string(),
        (AgentType::Codex, PipelineStage::Execution, "VeryHigh") => "codex-opus-4-5".to_string(),
        (AgentType::Codex, PipelineStage::Execution, _) => "codex-sonnet-4-5".to_string(),
        (AgentType::Codex, PipelineStage::Classification, _) => "gpt-4o-mini".to_string(),
        (AgentType::Codex, PipelineStage::Execution, "VeryHigh") => "gpt-4o-with-reasoning".to_string(),
        (AgentType::Codex, PipelineStage::Execution, _) => "gpt-4o".to_string(),
        _ => "codex-haiku".to_string(),
    }
}
```

## Self-Improvement Loop

The DSPy layer maintains the self-improvement loop regardless of which agent executes:

```
Session → TrainingCollector → LabeledExamples → MIPROv2 → Optimized Signatures
    ↑                                                              │
    └──────────────────────────────────────────────────────────────┘
```

1. **Session Recording**: Every execution records decisions and outcomes
2. **Training Extraction**: High-confidence, successful sessions become training examples
3. **Optimization**: MIPROv2 optimizes signature instructions and demos
4. **Deployment**: Optimized signatures improve future decisions (including agent selection)

```rust
impl TrainingCollector {
    pub fn record(&self, example: TrainingExample) {
        // Only record high-quality examples
        if example.verification.confidence < 0.7 {
            return;
        }

        // Record agent selection training data
        let selection_example = example! {
            "task_description": "input" => example.input,
            "complexity": "input" => example.complexity.complexity,
            "agent": "output" => example.agent_selection.agent,
        };
        self.store.append("agent_selection", selection_example);

        // Record execution training data
        let execution_example = example! {
            "task": "input" => example.input,
            "changes_summary": "output" => example.result.changes_summary,
            "success": "output" => example.result.success.to_string(),
        };
        self.store.append("execution", execution_example);
    }
}
```

## ACP Integration for Future Agents

The ACP (Agent Client Protocol) provides a standardized way to integrate new agents:

```rust
// Any ACP-compatible agent can be used with the same interface
let custom_agent = AcpAgentConnection::stdio(
    "my-custom-agent",
    AgentCommand::new("/path/to/my-agent")
        .arg("--acp-mode")
        .env("API_KEY", key),
    &cwd,
).await?;

// Works the same as Codex or Codex
let session = custom_agent.new_session(cwd.clone()).await?;
custom_agent.prompt(&session.session_id, &prompt).await?;
```

To add a new ACP-compatible agent:

1. Implement ACP protocol in your agent
2. Add converter in `acp_adapter::converters` for event mapping
3. Register in `AgentType` enum
4. DSPy agent selection will learn when to use it

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/adjutant/src/dspy/multi_agent_executor.rs` | Unified executor for Codex/Codex |
| `crates/adjutant/src/dspy/agent_selection.rs` | DSPy pipeline for agent selection |
| `crates/adjutant/src/dspy/orchestrator.rs` | Main pipeline orchestrator |
| `crates/adjutant/src/dspy/schema_gen.rs` | Signature → JSON Schema conversion |
| `crates/codex-agent-sdk/src/query.rs` | Codex SDK query interface |
| `crates/codex-agent-sdk/src/thread.rs` | Codex SDK thread interface |
| `crates/acp-adapter/src/agents/` | ACP wrappers for Codex and Codex |

## Summary

**Best Practice: DSPy drives decisions, Agent SDKs execute.**

1. Use DSPy signatures for all **decisions** (complexity, agent selection, routing, verification)
2. Use Codex/Codex SDKs for **execution** of complex, multi-tool tasks
3. Constrain agents with **structured outputs** matching signature schemas
4. Use **ACP** for a unified interface and future extensibility
5. Collect **training data** at the DSPy layer for self-improvement
6. **Mix models and agents** appropriately for cost/quality tradeoffs

This architecture gives you:
- Predictable, constrained output from any agent
- Full DSPy optimization and self-improvement capabilities
- Clear separation between policy (DSPy) and execution (Agent SDK)
- Agent selection optimization over time
- Easy addition of future ACP-compatible agents
