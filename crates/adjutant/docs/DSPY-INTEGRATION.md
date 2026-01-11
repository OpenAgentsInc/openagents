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

**ComplexityTrainingExample:** (decision pipeline)
```rust
{
    task_description: String,
    file_count: u32,
    estimated_tokens: usize,
    keywords: Vec<String>,
    expected_complexity: String,
    confidence: f32,
}
```

**DelegationTrainingExample:** (decision pipeline)
```rust
{
    task_description: String,
    complexity: String,
    file_count: u32,
    estimated_tokens: usize,
    should_delegate: bool,
    delegation_target: String,
    confidence: f32,
}
```

**RlmTriggerTrainingExample:** (decision pipeline)
```rust
{
    task_description: String,
    complexity: String,
    estimated_tokens: usize,
    use_rlm: bool,
    confidence: f32,
}
```

#### TrainingCollector

Auto-saves successful executions and high-confidence decisions:

```rust
let mut collector = TrainingCollector::new(auto_save: true)?;

// Task execution examples (only recorded if successful)
collector.record_planning(example)?;
collector.record_execution(example)?;
collector.record_synthesis(example)?;

// Decision examples (only recorded if confidence > 0.7)
collector.record_complexity(example)?;
collector.record_delegation(example)?;
collector.record_rlm_trigger(example)?;
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

**Adjutant LM Caching:**

Adjutant caches the decision LM for efficiency. The LM is lazily initialized on first decision call and reused for all subsequent calls:

```rust
// Internal implementation - LM cached in Adjutant struct
async fn get_or_create_decision_lm(&mut self) -> Option<Arc<LM>> {
    if self.decision_lm.is_none() {
        self.decision_lm = get_planning_lm().await.ok();
    }
    self.decision_lm.clone()
}
```

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

## Self-Improvement System

The DSPy integration now includes a complete self-improvement loop that automatically optimizes decision pipelines based on task outcomes.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Self-Improvement Loop                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Task Execution (Autopilot Loop)                              │
│     └── Decisions recorded (complexity, delegation, RLM)         │
│                                                                  │
│  2. Session Completion                                           │
│     └── Outcome recorded (Success/Failed/MaxIterations)          │
│                                                                  │
│  3. Outcome Feedback                                             │
│     └── Decisions labeled as correct/incorrect                   │
│     └── LabeledExamples stored with ground truth                 │
│                                                                  │
│  4. Performance Tracking                                         │
│     └── Rolling accuracy updated (window size: 50)               │
│     └── Per-signature metrics tracked                            │
│                                                                  │
│  5. Auto-Optimization Check                                      │
│     ├── Enough new examples? (default: 20)                       │
│     ├── Accuracy dropped? (default: <70%)                        │
│     └── Time since last optimization? (default: >24h)            │
│                                                                  │
│  6. MIPROv2 Optimization (if triggered)                          │
│     └── Optimizes lowest-accuracy signature                      │
│     └── Records optimization run                                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### New Components

#### SessionStore (`sessions.rs`)

Tracks autopilot sessions with all decisions made:

```rust
pub struct AutopilotSession {
    pub id: SessionId,
    pub task_title: String,
    pub task_description: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub decisions: Vec<DecisionRecord>,
    pub outcome: Option<SessionOutcome>,
    pub iterations_used: usize,
    pub verification_history: Vec<VerificationRecord>,
}

pub struct DecisionRecord {
    pub decision_type: String,      // "complexity", "delegation", "rlm_trigger"
    pub input_summary: String,
    pub output: serde_json::Value,
    pub predicted_confidence: f32,
    pub was_correct: Option<bool>,  // Set after outcome
}
```

#### OutcomeFeedback (`outcome_feedback.rs`)

Links task outcomes to decision correctness:

```rust
impl OutcomeFeedback {
    /// Evaluate whether each decision was correct based on outcome
    pub fn process_session(&mut self, session: &AutopilotSession) -> FeedbackResult;
}
```

**Correctness Logic:**

| Decision | Correct When |
|----------|--------------|
| Complexity | Task succeeded within expected iterations for complexity level |
| Delegation | Task succeeded (failures when delegated are less "wrong") |
| RLM | Task succeeded; OR large context (>100k) and RLM was used |

#### PerformanceTracker (`performance.rs`)

Maintains rolling accuracy windows:

```rust
pub struct RollingAccuracy {
    pub window_size: usize,         // Default: 50
    pub recent_outcomes: VecDeque<bool>,
    pub accuracy: f32,              // 0.0-1.0
    pub total_decisions: usize,
    pub total_correct: usize,
}

pub struct PerformanceTracker {
    pub signature_accuracy: HashMap<String, RollingAccuracy>,
    pub history: Vec<AccuracySnapshot>,
    pub optimization_runs: Vec<OptimizationRun>,
}
```

#### AutoOptimizer (`auto_optimizer.rs`)

Coordinates automatic optimization:

```rust
pub struct AutoOptimizerConfig {
    pub enabled: bool,
    pub min_labeled_examples: usize,      // Default: 20
    pub accuracy_threshold: f32,          // Default: 0.70
    pub min_hours_between_optimizations: u64,  // Default: 24
    pub background_optimization: bool,
    pub num_candidates: usize,
    pub num_trials: usize,
}

impl AutoOptimizer {
    /// Check if optimization should be triggered
    pub fn should_optimize(&self, store: &LabeledExamplesStore, perf: &PerformanceTracker)
        -> Option<OptimizationTrigger>;

    /// Get signature that most needs optimization
    pub fn signature_to_optimize(&self, store: &LabeledExamplesStore, perf: &PerformanceTracker)
        -> Option<String>;
}
```

#### SelfImprover (`auto_optimizer.rs`)

High-level coordinator:

```rust
impl SelfImprover {
    /// Process session completion for self-improvement
    pub fn process_session_completion(&mut self, session: &AutopilotSession)
        -> Result<SelfImprovementResult>;
}

pub struct SelfImprovementResult {
    pub decisions_labeled: usize,
    pub correct_count: usize,
    pub incorrect_count: usize,
    pub optimization_needed: Option<String>,
    pub optimization_trigger: Option<OptimizationTrigger>,
}
```

### CLI Commands

```bash
# View recent sessions
autopilot dspy sessions
autopilot dspy sessions --failed
autopilot dspy sessions --limit 20

# View performance metrics
autopilot dspy performance

# Configure auto-optimization
autopilot dspy auto-optimize --enable
autopilot dspy auto-optimize --disable
autopilot dspy auto-optimize --min-examples 30
autopilot dspy auto-optimize --accuracy-threshold 0.75
autopilot dspy auto-optimize --min-hours 12
```

### Storage

```
~/.openagents/adjutant/
├── training/
│   ├── dataset.json           # Existing training examples
│   └── labeled/               # NEW: Labeled examples with ground truth
│       ├── complexity.json
│       ├── delegation.json
│       └── rlm_trigger.json
├── sessions/                  # NEW: Session tracking
│   ├── index.json
│   └── <year>/<month>/<session-id>.json
├── metrics/                   # NEW: Performance metrics
│   └── performance.json
└── config/                    # NEW: Configuration
    └── auto_optimizer.json
```

## Counterfactual Recording (Proposed)

To enable comparison between DSPy decisions and legacy rules, and to support proper shadow mode, we need to record what the alternative path would have done.

### DecisionRecord Extensions

The existing `DecisionRecord` struct is extended with counterfactual fields:

```rust
// crates/adjutant/src/dspy/sessions.rs

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DecisionRecord {
    // Existing fields
    pub signature: String,
    pub inputs_hash: String,
    pub dspy_output: serde_json::Value,
    pub dspy_confidence: f32,
    pub dspy_latency_ms: u64,

    // NEW: Counterfactual tracking
    /// What the legacy rule-based system would have output
    pub legacy_output: Option<serde_json::Value>,

    /// Whether we fell back to legacy (confidence < threshold, parse error, etc.)
    pub fallback_used: bool,

    /// Why fallback was used (if applicable)
    pub fallback_reason: Option<FallbackReason>,

    /// Which provider lane was used ("claude" | "swarm" | "local")
    pub provider_lane: String,

    /// What lane legacy would have used (for lane routing analysis)
    pub provider_lane_without_dspy: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FallbackReason {
    /// DSPy confidence below threshold (includes the confidence value)
    LowConfidence(f32),

    /// Failed to parse DSPy output
    ParseError(String),

    /// DSPy call timed out
    Timeout,

    /// Required tool not available
    ToolMissing(String),

    /// No LM provider available
    ProviderUnavailable,
}
```

### Recording Counterfactuals

```rust
// When making a decision, record both paths:
let dspy_result = complexity_pipeline.classify(&task).await;
let legacy_result = legacy_complexity_rules(&task);  // Run legacy too

let record = DecisionRecord {
    signature: "ComplexityClassifier".into(),
    inputs_hash: hash_inputs(&task),
    dspy_output: serde_json::to_value(&dspy_result)?,
    dspy_confidence: dspy_result.confidence,
    dspy_latency_ms: elapsed.as_millis() as u64,

    // Counterfactual
    legacy_output: Some(serde_json::to_value(&legacy_result)?),
    fallback_used: dspy_result.confidence < 0.7,
    fallback_reason: if dspy_result.confidence < 0.7 {
        Some(FallbackReason::LowConfidence(dspy_result.confidence))
    } else {
        None
    },
    provider_lane: "claude".into(),
    provider_lane_without_dspy: Some("local".into()),
};

session.decisions.push(record);
```

### Shadow Mode Analysis

With counterfactuals recorded, we can analyze:

```rust
// After session completes with outcome
for decision in session.decisions.iter() {
    if let Some(legacy) = &decision.legacy_output {
        let dspy_correct = decision.was_correct.unwrap_or(false);
        let legacy_would_match = would_legacy_succeed(&legacy, &session.outcome);

        if dspy_correct && !legacy_would_match {
            // DSPy won - record as training signal
            metrics.dspy_wins += 1;
        } else if !dspy_correct && legacy_would_match {
            // Legacy would have won - DSPy needs improvement
            metrics.legacy_wins += 1;
        }
    }
}
```

## Outcome-Coupled Metrics (Proposed)

Current training only captures format correctness (valid JSON, proper field names). This is necessary but not sufficient for good agent performance.

### The Problem

Existing metrics reward:
- Valid JSON output ✓
- Correct field types ✓
- Reasonable confidence values ✓

Missing signals:
- Did this tool call actually help?
- Did verification improve after this step?
- Was this a repeated/wasted action?
- How much did this cost?

### LabeledToolCall Structure

```rust
// crates/adjutant/src/dspy/training.rs

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LabeledToolCall {
    /// Which signature was used
    pub signature: String,

    /// Signature inputs (hashed for dedup)
    pub inputs: serde_json::Value,

    /// Signature outputs
    pub outputs: serde_json::Value,

    // Outcome-coupled labels (computed post-hoc)

    /// Step utility from ToolResultSignature (-1.0 to +1.0)
    pub step_utility: f32,

    /// Change in failing tests after this step (negative = improvement)
    pub verification_delta: i32,

    /// Tokens used for this call
    pub cost_tokens: u32,

    /// Tool calls made (usually 1)
    pub cost_tool_calls: u32,

    /// Did we make this exact call before in this session?
    pub was_repeated: bool,
}
```

### Step Utility Metric

Computed by `ToolResultSignature` after each tool execution:

| Score | Meaning | Example |
|-------|---------|---------|
| +1.0 | Directly advances goal | Found the bug, tests now pass |
| +0.5 | Partial progress | Narrowed down to 3 files |
| 0.0 | No-op | Search returned nothing, no harm |
| -0.5 | Wasted effort | Repeated same search, opened same file again |
| -1.0 | Made things worse | Broke the build, added more test failures |

### Scoring Function for MIPRO

The combined score for training optimization:

```rust
fn compute_step_score(call: &LabeledToolCall) -> f32 {
    let mut score = 0.0;

    // Step utility (40% weight) - the core learning signal
    score += call.step_utility * 0.4;

    // Verification improvement (30% weight)
    if call.verification_delta < 0 {
        // Fewer failures = good
        score += 0.3;
    }

    // Repetition penalty (20% weight)
    if call.was_repeated {
        score -= 0.2;
    }

    // Cost penalty (10% weight, capped)
    let cost_penalty = (call.cost_tokens as f32 / 10000.0).min(0.1);
    score -= cost_penalty;

    score
}
```

### Recording During Execution

```rust
// After each tool execution in the autopilot loop:
let tool_result = tool_result_predictor.forward(/* ... */).await?;

let labeled = LabeledToolCall {
    signature: "ToolCallSignature".into(),
    inputs: tool_call_inputs.clone(),
    outputs: tool_call_outputs.clone(),
    step_utility: tool_result.get("step_utility").as_f32().unwrap_or(0.0),
    verification_delta: prev_failing - curr_failing,
    cost_tokens: tokens_used,
    cost_tool_calls: 1,
    was_repeated: execution_history.contains(&call_hash),
};

training_collector.record_tool_call(labeled)?;
```

### Storage

```
~/.openagents/adjutant/training/
├── dataset.json              # Existing format-correct examples
├── labeled/
│   ├── complexity.json       # Labeled decision examples
│   ├── delegation.json
│   ├── rlm_trigger.json
│   └── tool_calls.json       # NEW: Outcome-coupled tool call examples
└── metrics/
    └── step_utility_histogram.json  # NEW: Distribution of step utilities
```

### Benefits

1. **Better MIPRO optimization** - Rewards outcomes, not just format
2. **Cost awareness** - Penalizes expensive wasteful calls
3. **Repetition detection** - Agents learn to avoid loops
4. **Verification coupling** - Training signal tied to actual success

## Future Improvements

1. ~~**Parallel Optimization** - Optimize all three signatures simultaneously~~ (Now prioritizes lowest-accuracy)
2. **Few-Shot Learning** - Include demos in signature context
3. ~~**A/B Testing** - Compare gateway vs DSPy performance~~ (Shadow mode in DspyHub)
4. **Custom Metrics** - Task-specific evaluation functions
5. ~~**Model Selection** - Dynamic model routing based on complexity~~ (LaneMux)
6. **Pattern Learning** - Extract failure patterns to warn about risky tasks
7. **Counterfactual Analysis** - Compare DSPy vs legacy decisions offline
8. **Step Utility Optimization** - Train on outcome-coupled metrics

## See Also

- [README.md](./README.md) - Adjutant overview
- [TIERED-EXECUTOR.md](./TIERED-EXECUTOR.md) - Tiered execution details
- [../../dsrs/README.md](../../dsrs/README.md) - dsrs (Rust DSPy) documentation
