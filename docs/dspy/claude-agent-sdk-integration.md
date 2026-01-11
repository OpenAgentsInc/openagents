# DSPy + Claude Agent SDK Integration Strategy

This document analyzes how to best integrate DSPy (via dsrs) with the Claude Agent SDK for constrained, optimizable agent execution.

## Executive Summary

**Recommendation: DSPy-Controlled Flow with SDK Execution**

DSPy should control the *decision-making flow* (planning, routing, verification) while Claude Agent SDK handles *execution* when Claude's tools are needed. This creates a hybrid architecture where:

1. DSPy signatures make structured decisions about what to do
2. DSPy orchestrates the overall task flow
3. Claude Agent SDK executes tool-heavy subtasks via constrained queries
4. Output is constrained to DSPy signature schemas using structured outputs

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          DSPy ORCHESTRATOR                               │
│                                                                          │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐  │
│  │ ComplexityPipeline│ →  │ DelegationPipeline│ →  │VerificationPipeline│
│  └──────────────────┘    └──────────────────┘    └──────────────────┘  │
│           │                       │                       │              │
│           ▼                       ▼                       ▼              │
│      Local LM               Route Decision          Validate Output      │
│     (fast/cheap)              ┌───┴───┐              (JSON Schema)       │
│                               │       │                                  │
│                      local_tools    claude_agent                          │
│                          │              │                                │
│                          ▼              ▼                                │
│                   ┌───────────┐   ┌──────────────────────────────────┐  │
│                   │dsrs tools │   │    CLAUDE AGENT SDK              │  │
│                   │(grep/edit)│   │                                  │  │
│                   └───────────┘   │  query("subtask", QueryOptions { │  │
│                                   │    output_format: JsonSchema,    │  │
│                                   │    max_turns: 3,                 │  │
│                                   │    permission_mode: AcceptEdits, │  │
│                                   │  })                              │  │
│                                   │                                  │  │
│                                   │  Returns: structured output      │  │
│                                   │  matching DSPy signature schema  │  │
│                                   └──────────────────────────────────┘  │
│                                              │                           │
│                                              ▼                           │
│                                   Validate against signature            │
│                                   Feed back to orchestrator             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Architecture Options Analyzed

### Option 1: DSPy Controls Flow, SDK Executes (Recommended)

DSPy drives the decision-making pipeline. When a decision routes to `claude_agent`, we use Claude Agent SDK with:

- **Structured outputs** matching the DSPy signature's output schema
- **Max turns** limit for predictable execution
- **Tool constraints** to limit what Claude can do
- **Hooks** to intercept and validate intermediate steps

```rust
// DSPy orchestrator decides to delegate to Claude
let delegation = delegation_pipeline.decide(&input).await?;

if delegation.delegation_target == "claude_agent" {
    // Build JSON Schema from DSPy signature output fields
    let schema = build_schema_from_signature::<ExecutionOutputSignature>();

    let options = QueryOptions::new()
        .output_format(OutputFormat {
            format_type: "json_schema".to_string(),
            schema,
        })
        .max_turns(5)
        .permission_mode(PermissionMode::AcceptEdits)
        .disallowed_tools(vec!["WebFetch".to_string()]); // Constrain tools

    // SDK query returns structured output matching our signature
    let mut stream = query(&subtask_prompt, options).await?;

    // Collect structured result
    let result = collect_structured_output(&mut stream).await?;

    // Validate and convert to DSPy Example for training
    let validated = ExecutionOutputSignature::from_json(&result)?;
}
```

**Pros:**
- DSPy maintains control over the overall flow
- Outputs are constrained to signature schemas
- Training data is collected at DSPy layer
- Self-improvement loop works naturally
- Claude handles complex multi-tool tasks

**Cons:**
- Adds latency for subprocess spawning
- Two LLM "brains" (DSPy router + Claude executor)

### Option 2: Claude SDK with DSPy Hooks

Use Claude Agent SDK as the primary executor, with hooks that invoke DSPy signatures for decisions.

```rust
let options = QueryOptions::new()
    .hook(HookEvent::PreToolUse, HookCallbackMatcher::new()
        .hook(Arc::new(DspyToolValidationHook::new())))
    .hook(HookEvent::SessionStart, HookCallbackMatcher::new()
        .hook(Arc::new(DspyPlanningHook::new())));

// Claude drives, DSPy validates/guides at hook points
let stream = query(&task, options).await?;
```

**Pros:**
- Single LLM controls execution
- Hooks provide interception points
- Lower latency for simple tasks

**Cons:**
- DSPy signatures become reactive, not proactive
- Harder to optimize the full pipeline
- Training data collection more complex
- Less structured flow

### Option 3: DSPy Signatures as Claude Custom Agents

Define Claude sub-agents that map 1:1 to DSPy signatures.

```rust
let options = QueryOptions::new()
    .agent("task_planner", AgentDefinition {
        description: "Plans implementation steps for coding tasks",
        prompt: PLANNING_SIGNATURE_PROMPT.to_string(), // DSPy signature instruction
        tools: Some(vec!["Read".to_string(), "Glob".to_string()]),
        model: Some(AgentModel::Haiku), // Fast for classification
        ..Default::default()
    })
    .agent("code_executor", AgentDefinition {
        description: "Executes code changes based on plan",
        prompt: EXECUTION_SIGNATURE_PROMPT.to_string(),
        tools: Some(vec!["Edit".to_string(), "Write".to_string(), "Bash".to_string()]),
        model: Some(AgentModel::Sonnet),
        ..Default::default()
    });
```

**Pros:**
- Leverages Claude's native agent system
- Model mixing per agent
- Clean separation of concerns

**Cons:**
- Loses DSPy optimization capabilities
- No self-improvement loop
- Signatures become static prompts

## Recommended Architecture: Hybrid DSPy-Controlled Execution

### Core Principle

**DSPy controls policy; Claude SDK executes actions.**

DSPy signatures should drive:
- Task complexity classification
- Execution routing decisions
- Verification and validation
- Training data collection

Claude Agent SDK should execute:
- Multi-file code changes
- Complex tool sequences
- Tasks requiring Claude's reasoning

### Implementation Pattern

#### 1. DSPy Signature for Claude Execution

Define a signature that wraps Claude SDK execution:

```rust
#[Signature]
struct ClaudeExecutionSignature {
    /// Execute a coding task using Claude's tools.
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

#### 2. Claude SDK Executor

Implement a custom executor that uses the SDK:

```rust
pub struct ClaudeSdkExecutor {
    permission_handler: Arc<dyn PermissionHandler>,
}

impl ClaudeSdkExecutor {
    pub async fn execute(
        &self,
        task: &str,
        constraints: &ExecutionConstraints,
    ) -> Result<ClaudeExecutionResult> {
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

        let options = QueryOptions::new()
            .model(&constraints.model)
            .max_turns(constraints.max_turns)
            .max_budget_usd(constraints.budget_usd)
            .permission_mode(PermissionMode::AcceptEdits)
            .output_format(OutputFormat {
                format_type: "json_schema".to_string(),
                schema: output_schema,
            })
            .disallowed_tools(constraints.disallowed_tools.clone());

        let mut stream = query_with_permissions(
            task,
            options,
            self.permission_handler.clone(),
        ).await?;

        // Collect result
        let mut result = None;
        while let Some(msg) = stream.next().await {
            if let SdkMessage::Result(SdkResultMessage::Success(success)) = msg? {
                if let Some(output) = success.structured_output {
                    result = Some(serde_json::from_value(output)?);
                }
            }
        }

        result.ok_or_else(|| anyhow::anyhow!("No structured output received"))
    }
}
```

#### 3. Integration in DSPy Pipeline

Wire the executor into the DSPy pipeline:

```rust
pub struct AutopilotOrchestrator {
    complexity_pipeline: ComplexityPipeline,
    delegation_pipeline: DelegationPipeline,
    verification_pipeline: VerificationPipeline,
    claude_executor: ClaudeSdkExecutor,
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

        // 2. Decide delegation (DSPy signature)
        let delegation = self.delegation_pipeline.decide(&DelegationInput {
            task_description: task.to_string(),
            complexity: complexity.complexity.clone(),
            ..Default::default()
        }).await?;

        // 3. Execute based on routing
        let execution_result = match delegation.delegation_target.as_str() {
            "claude_agent" => {
                // Use Claude Agent SDK with constrained output
                let constraints = ExecutionConstraints {
                    model: self.select_model(&complexity),
                    max_turns: self.max_turns_for_complexity(&complexity),
                    budget_usd: 1.0,
                    disallowed_tools: vec![],
                };
                self.claude_executor.execute(task, &constraints).await?
            }
            "local_tools" => {
                // Use local dsrs tools
                self.local_executor.execute(task).await?
            }
            _ => {
                anyhow::bail!("Unknown delegation target: {}", delegation.delegation_target);
            }
        };

        // 4. Verify result (DSPy signature)
        let verification = self.verification_pipeline.verify(&VerificationInput {
            task: task.to_string(),
            result: execution_result.changes_summary.clone(),
            ..Default::default()
        }).await?;

        // 5. Collect training data if high confidence
        if verification.confidence > 0.8 && execution_result.success {
            self.training_collector.record(TrainingExample {
                input: task.to_string(),
                complexity: complexity.clone(),
                delegation: delegation.clone(),
                result: execution_result.clone(),
                verification: verification.clone(),
            });
        }

        Ok(TaskResult {
            success: verification.is_valid,
            execution: execution_result,
            verification,
        })
    }
}
```

### Hook-Based Validation

Use SDK hooks to enforce DSPy constraints during execution:

```rust
pub struct DspyValidationHook {
    allowed_tools: HashSet<String>,
    file_patterns: Vec<Regex>,
}

#[async_trait]
impl HookCallback for DspyValidationHook {
    async fn call(&self, input: HookInput, _tool_use_id: Option<String>) -> Result<HookOutput> {
        if let HookInput::PreToolUse(pre) = input {
            // Validate tool is allowed by DSPy policy
            if !self.allowed_tools.contains(&pre.tool_name) {
                return Ok(SyncHookOutput::block(format!(
                    "Tool {} not allowed by DSPy policy",
                    pre.tool_name
                )).into());
            }

            // Validate file paths match expected patterns
            if pre.tool_name == "Edit" || pre.tool_name == "Write" {
                if let Some(path) = pre.tool_input.get("file_path").and_then(|v| v.as_str()) {
                    let allowed = self.file_patterns.iter().any(|p| p.is_match(path));
                    if !allowed {
                        return Ok(SyncHookOutput::block(format!(
                            "File {} not in allowed paths",
                            path
                        )).into());
                    }
                }
            }
        }

        Ok(SyncHookOutput::continue_execution().into())
    }
}
```

### Structured Output Mapping

Map DSPy signature fields to JSON Schema for Claude's structured output:

```rust
/// Generate JSON Schema from a DSPy signature's output fields.
pub fn signature_to_json_schema<S: MetaSignature>() -> serde_json::Value {
    let sig = S::new();
    let mut properties = serde_json::Map::new();
    let mut required = vec![];

    for field in sig.output_fields() {
        let (json_type, field_schema) = match field.field_type.as_str() {
            "String" => ("string", serde_json::json!({"type": "string"})),
            "bool" => ("boolean", serde_json::json!({"type": "boolean"})),
            "f32" | "f64" => ("number", serde_json::json!({"type": "number"})),
            "i32" | "i64" | "u32" | "u64" => ("integer", serde_json::json!({"type": "integer"})),
            _ => ("string", serde_json::json!({"type": "string"})), // Default to string
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

| Pipeline Stage | Model | Reason |
|----------------|-------|--------|
| ComplexityPipeline | Haiku / Local | Fast classification, low cost |
| DelegationPipeline | Haiku / Local | Simple routing decision |
| Claude Execution | Sonnet / Opus | Complex multi-tool reasoning |
| VerificationPipeline | Haiku | Fast validation |
| Training Optimization | Swarm / Cheap | High volume, cost-sensitive |

```rust
fn select_model_for_stage(stage: PipelineStage, complexity: &str) -> String {
    match (stage, complexity) {
        (PipelineStage::Classification, _) => "claude-haiku".to_string(),
        (PipelineStage::Execution, "VeryHigh") => "claude-opus-4-5".to_string(),
        (PipelineStage::Execution, "High") => "claude-sonnet-4-5".to_string(),
        (PipelineStage::Execution, _) => "claude-sonnet-4-5".to_string(),
        (PipelineStage::Verification, _) => "claude-haiku".to_string(),
    }
}
```

## Self-Improvement Loop

The DSPy layer maintains the self-improvement loop:

```
Session → TrainingCollector → LabeledExamples → MIPROv2 → Optimized Signatures
    ↑                                                              │
    └──────────────────────────────────────────────────────────────┘
```

1. **Session Recording**: Every execution records decisions and outcomes
2. **Training Extraction**: High-confidence, successful sessions become training examples
3. **Optimization**: MIPROv2 optimizes signature instructions and demos
4. **Deployment**: Optimized signatures improve future decisions

```rust
impl TrainingCollector {
    pub fn record(&self, example: TrainingExample) {
        // Only record high-quality examples
        if example.verification.confidence < 0.7 {
            return;
        }

        // Convert to DSPy Example format
        let dspy_example = example! {
            "task_description": "input" => example.input,
            "complexity": "output" => example.complexity.complexity,
            "delegation_target": "output" => example.delegation.delegation_target,
        };

        // Store for optimization
        self.store.append(example.signature_name(), dspy_example);
    }
}
```

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/adjutant/src/dspy/sdk_executor.rs` | Claude SDK wrapper with structured output |
| `crates/adjutant/src/dspy/orchestrator.rs` | Main pipeline orchestrator |
| `crates/adjutant/src/dspy/hooks.rs` | DSPy validation hooks for SDK |
| `crates/adjutant/src/dspy/schema_gen.rs` | Signature → JSON Schema conversion |
| `crates/dsrs/src/core/lm/claude_sdk.rs` | Existing Claude SDK LM provider |

## Summary

**Best Practice: DSPy drives decisions, Claude Agent SDK executes.**

1. Use DSPy signatures for all **decisions** (complexity, routing, verification)
2. Use Claude Agent SDK for **execution** of complex, multi-tool tasks
3. Constrain Claude with **structured outputs** matching signature schemas
4. Use **hooks** to validate tool usage during execution
5. Collect **training data** at the DSPy layer for self-improvement
6. **Mix models** appropriately for cost/quality tradeoffs

This architecture gives you:
- Predictable, constrained output from Claude execution
- Full DSPy optimization and self-improvement capabilities
- Clear separation between policy (DSPy) and execution (Claude SDK)
- Model mixing for cost optimization
- Training data collection for continuous improvement
