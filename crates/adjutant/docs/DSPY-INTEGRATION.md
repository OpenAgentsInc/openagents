# DSPy Integration

Adjutant now integrates with [dsrs](../../dsrs/) (Rust DSPy implementation) for optimizable prompt engineering. This enables automatic prompt improvement via MIPROv2 optimization.

## Overview

The DSPy integration replaces hardcoded string prompts with typed, optimizable signatures. Instead of:

```rust
const PLANNER_SYSTEM_PROMPT: &str = "You are a task planner...";
```

We now use:

```rust
#[Signature]
struct SubtaskPlanningSignature {
    /// Task Planner: Break the given task into concrete, atomic subtasks.

    #[input]
    pub task_title: String,
    #[input]
    pub task_description: String,
    #[input]
    pub context: String,

    #[output]
    pub subtasks: String,
    #[output]
    pub reasoning: String,
    #[output]
    pub confidence: f32,
}
```

This enables:
- **Automatic prompt optimization** via MIPROv2
- **Training data collection** from successful executions
- **Type-safe inputs/outputs** with validation
- **Evaluation metrics** for quality assessment

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      TieredExecutor                          │
│                                                              │
│  ExecutionMode::Gateway ──────► Original gateway execution   │
│  ExecutionMode::Dsrs    ──────► DSPy-powered execution       │
└──────────────────────────────────┬───────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│                     AdjutantModule                           │
│  ┌─────────────────────────────────────────────────────┐    │
│  │             SubtaskPlanningSignature                 │    │
│  │  GLM 4.7 · Breaks tasks into atomic subtasks        │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │            SubtaskExecutionSignature                 │    │
│  │  Qwen-3-32B · Executes individual subtasks          │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │            ResultSynthesisSignature                  │    │
│  │  GLM 4.7 · Synthesizes results into final outcome   │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│                    TrainingCollector                         │
│  Records successful executions for MIPROv2 optimization      │
│  Storage: ~/.openagents/adjutant/training/dataset.json       │
└─────────────────────────────────────────────────────────────┘
```

## Components

### Signatures

Located in `src/dspy/module.rs`:

#### SubtaskPlanningSignature

Replaces `PLANNER_SYSTEM_PROMPT`. Breaks a task into atomic subtasks.

| Field | Type | Direction | Description |
|-------|------|-----------|-------------|
| `task_title` | String | input | Title of the task |
| `task_description` | String | input | Detailed description |
| `context` | String | input | Repository context (file contents) |
| `subtasks` | String | output | JSON array of subtasks |
| `reasoning` | String | output | Explanation of planning approach |
| `confidence` | f32 | output | Confidence score (0.0-1.0) |

#### SubtaskExecutionSignature

Replaces `EXECUTOR_SYSTEM_PROMPT`. Executes a single subtask.

| Field | Type | Direction | Description |
|-------|------|-----------|-------------|
| `action` | String | input | Action type: read, edit, bash |
| `target` | String | input | Target file path |
| `instruction` | String | input | What to do |
| `file_context` | String | input | Current file content |
| `result` | String | output | JSON result with action-specific fields |
| `reasoning` | String | output | Explanation of what was done |
| `success` | bool | output | Whether action completed |

#### ResultSynthesisSignature

Replaces `SYNTHESIZER_SYSTEM_PROMPT`. Synthesizes results into final outcome.

| Field | Type | Direction | Description |
|-------|------|-----------|-------------|
| `task_title` | String | input | Original task title |
| `subtask_results` | String | input | Formatted results from subtasks |
| `success` | bool | output | Overall success status |
| `summary` | String | output | What was accomplished/failed |
| `modified_files` | String | output | JSON array of modified files |
| `confidence` | f32 | output | Confidence in assessment (0.0-1.0) |

### AdjutantModule

The composite dsrs module implementing `Module`, `Evaluator`, and `Optimizable` traits:

```rust
#[derive(Builder)]
pub struct AdjutantModule {
    pub planner: Predict,      // SubtaskPlanningSignature
    pub executor: Predict,     // SubtaskExecutionSignature
    pub synthesizer: Predict,  // ResultSynthesisSignature
}
```

Key methods:
- `plan(title, description, context)` - Execute planning phase
- `execute_subtask(action, target, instruction, file_context)` - Execute one subtask
- `synthesize(title, subtask_results)` - Execute synthesis phase
- `forward(inputs)` - Full three-phase execution (for Module trait)

### Metrics

Located in `src/dspy/metrics.rs`. Used by MIPROv2 for optimization.

#### subtask_planning_metric

Evaluates planning quality (score 0.0-1.0):
- 25%: Valid JSON array with required fields
- 25%: Valid action types (read/edit/bash)
- 25%: Valid target paths
- 25%: Actionable instructions (start with verb)

#### subtask_execution_metric

Evaluates execution quality (score 0.0-1.0):
- 33%: Valid result JSON
- 33%: Well-formed edit strings or command
- 34%: Substantive reasoning

#### synthesis_metric

Evaluates synthesis quality (score 0.0-1.0):
- 25%: Valid success boolean
- 25%: Substantive summary (15+ chars)
- 25%: Valid modified_files JSON array
- 25%: Calibrated confidence (0.0-1.0)

#### combined_metric

Weighted combination for overall AdjutantModule evaluation:
- 60% planning score
- 40% synthesis score

### Training Data

Located in `src/dspy/training.rs`. Collects training examples for optimization.

#### Storage

Training data is stored at:
```
~/.openagents/adjutant/training/dataset.json
```

#### Example Types

**PlanningTrainingExample:**
```rust
{
    task_title: String,
    task_description: String,
    context: String,
    expected_subtasks: Vec<SubtaskData>,
    success: bool,
}
```

**ExecutionTrainingExample:**
```rust
{
    action: String,
    target: String,
    instruction: String,
    file_context: String,
    expected_result: Value,
    success: bool,
}
```

**SynthesisTrainingExample:**
```rust
{
    task_title: String,
    subtask_results: String,
    expected_success: bool,
    expected_summary: String,
    expected_modified_files: Vec<String>,
}
```

#### TrainingCollector

Auto-saves successful executions:

```rust
let mut collector = TrainingCollector::new(auto_save: true)?;
collector.record_planning(example)?;
collector.record_execution(example)?;
collector.record_synthesis(example)?;
```

### LM Configuration

Located in `src/dspy/lm_config.rs`. Multi-provider support with smart priority/fallback.

```rust
// Auto-detect best available provider
let planning_lm = get_planning_lm().await?;
let execution_lm = get_execution_lm().await?;

// Check what provider is being used
if let Some(provider) = get_active_provider() {
    println!("Using: {}", provider);  // e.g., "Claude SDK (headless)"
}

// Force specific provider
let lm = create_lm(&LmProvider::Cerebras).await?;
```

**Provider Priority:**
1. **Claude SDK** - Uses Claude Code headless mode (requires `claude` CLI)
2. **Pylon Swarm** - Distributed inference via NIP-90 (requires `PYLON_MNEMONIC`)
3. **Cerebras** - Fast, cheap execution (requires `CEREBRAS_API_KEY`)
4. **Pylon Local** - Ollama fallback (requires Ollama running on :11434)

## Usage

### Execution Modes

The TieredExecutor supports two execution modes:

```rust
// Default: Original gateway-based execution
let executor = TieredExecutor::new()?;

// DSPy-powered execution with training collection
let executor = TieredExecutor::with_mode(ExecutionMode::Dsrs)?;
```

### DSPy Execution

```rust
let mut executor = TieredExecutor::with_mode(ExecutionMode::Dsrs)?;

let task = Task::new("#123", "Add error handling", "Add Result types to auth");
let context = "--- src/auth.rs ---\n...";
let mut tools = ToolRegistry::new(&workspace_root);

// Uses DSPy signatures with training collection
let result = executor.execute_dsrs(&task, &context, &mut tools).await?;
```

### Programmatic Module Usage

```rust
use adjutant::dspy::AdjutantModule;

let module = AdjutantModule::new();

// Plan a task
let plan = module.plan("Add feature", "Add new API endpoint", "// context...").await?;
let subtasks = plan.get("subtasks", None);

// Execute a subtask
let result = module.execute_subtask("edit", "src/api.rs", "Add endpoint", "// file...").await?;

// Synthesize results
let final_result = module.synthesize("Add feature", "- [OK] 1: Added endpoint").await?;
```

### Optimization with MIPROv2

After collecting training data:

```rust
use adjutant::dspy::{AdjutantModule, AdjutantTrainingDataset};
use dsrs::{MIPROv2, Optimizer};

// Load training data
let dataset = AdjutantTrainingDataset::load()?;
let examples = dataset.planning_as_examples();

// Create and optimize module
let mut module = AdjutantModule::new();
let optimizer = MIPROv2::builder()
    .num_candidates(10)
    .num_trials(20)
    .build();

optimizer.compile(&mut module, examples).await?;

// Module now has optimized instructions
println!("Optimized: {}", module.planner.get_signature().instruction());
```

## Decision Pipelines

Located in `src/dspy/decision_pipelines.rs`. These pipelines enable intelligent routing decisions with DSPy-first logic and legacy fallback.

### ComplexityPipeline

Classifies task complexity to inform routing decisions.

| Field | Type | Direction | Description |
|-------|------|-----------|-------------|
| `task_description` | String | input | Task to classify |
| `file_count` | String | input | Number of files involved |
| `estimated_tokens` | String | input | Estimated context tokens |
| `keywords` | String | input | Task keywords (refactor, migrate, etc.) |
| `complexity` | String | output | Low, Medium, High, or VeryHigh |
| `reasoning` | String | output | Explanation of classification |
| `confidence` | f32 | output | Confidence score (0.0-1.0) |

### DelegationPipeline

Decides whether to delegate task execution and to which target.

| Field | Type | Direction | Description |
|-------|------|-----------|-------------|
| `task_description` | String | input | Task to evaluate |
| `complexity` | String | input | Classified complexity level |
| `file_count` | String | input | Number of files |
| `estimated_tokens` | String | input | Estimated context tokens |
| `should_delegate` | bool | output | Whether to delegate |
| `delegation_target` | String | output | claude_code, rlm, or local_tools |
| `reasoning` | String | output | Explanation of decision |
| `confidence` | f32 | output | Confidence score (0.0-1.0) |

### RlmTriggerPipeline

Decides whether to use RLM (Recursive Language Model) for deep analysis.

| Field | Type | Direction | Description |
|-------|------|-----------|-------------|
| `task_description` | String | input | Task to evaluate |
| `complexity` | String | input | Classified complexity level |
| `estimated_tokens` | String | input | Estimated context tokens |
| `use_rlm` | bool | output | Whether to use RLM |
| `reasoning` | String | output | Explanation of decision |
| `confidence` | f32 | output | Confidence score (0.0-1.0) |

### Usage in Adjutant.execute()

All three decision pipelines are wired into the main execution flow:

```rust
pub async fn execute(&mut self, task: &Task) -> Result<TaskResult, AdjutantError> {
    // 1. Plan the task (rule-based file discovery)
    let mut plan = self.plan_task(task).await?;

    // 1b. DSPy-first complexity classification with legacy fallback
    plan.complexity = self.determine_complexity_dspy(task, &plan).await;

    // 2. DSPy-first RLM decision with legacy fallback
    let use_rlm = self.determine_use_rlm(task, &plan).await;

    // 3. LM provider selection ...

    // 4. DSPy-first delegation decision with legacy fallback
    let delegation = self.determine_delegation(task, &plan).await;

    if delegation.should_delegate && delegation.confidence > 0.7 {
        match delegation.delegation_target.as_str() {
            "claude_code" => return self.delegate_to_claude_code(task).await,
            "rlm" => return self.execute_with_rlm_delegate(task, &plan).await,
            _ => {} // local_tools - fall through
        }
    }

    // Legacy fallback rules still apply if DSPy confidence is low
    // ...
}
```

**Fallback Strategy:** Each DSPy pipeline requires >0.7 confidence to override legacy rules. If confidence is low or the pipeline errors, the original rule-based logic is used.

## File Structure

```
crates/adjutant/src/dspy/
├── mod.rs               # Module exports
├── decision_pipelines.rs # Decision signatures + pipelines (complexity, delegation, RLM)
├── lm_config.rs         # Multi-provider LM configuration
├── module.rs            # AdjutantModule + task execution signatures
├── metrics.rs           # Evaluation metrics
└── training.rs          # Training data collection
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| (none) | Claude SDK | Uses `claude` CLI authentication |
| `PYLON_MNEMONIC` | Pylon Swarm | BIP-39 mnemonic for NIP-90 signing |
| `CEREBRAS_API_KEY` | Cerebras | Cerebras API key |
| (none) | Pylon Local | Auto-detects Ollama on :11434 |

At least one provider must be available for DSPy mode to work.

## Comparison: Gateway vs DSPy Mode

| Aspect | Gateway Mode | DSPy Mode |
|--------|--------------|-----------|
| Prompts | Hardcoded strings | Typed signatures |
| Optimization | Manual tuning | Automatic via MIPROv2 |
| Training | None | Automatic collection |
| Metrics | None | Built-in evaluation |
| Flexibility | Fixed | Optimizable |

## Future Improvements

1. **Parallel Optimization** - Optimize all three signatures simultaneously
2. **Few-Shot Learning** - Include demos in signature context
3. **A/B Testing** - Compare gateway vs DSPy performance
4. **Custom Metrics** - Task-specific evaluation functions
5. **Model Selection** - Dynamic model routing based on complexity

## See Also

- [README.md](./README.md) - Adjutant overview
- [TIERED-EXECUTOR.md](./TIERED-EXECUTOR.md) - Tiered execution details
- [../../dsrs/README.md](../../dsrs/README.md) - dsrs (Rust DSPy) documentation
