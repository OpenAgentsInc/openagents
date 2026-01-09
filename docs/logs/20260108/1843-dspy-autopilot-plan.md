# DSPy Integration Wave 2: Autopilot + Roadmap

## Executive Summary

Transform autopilot's freeform prompts into typed DSPy Signatures with optimization infrastructure. Create a roadmap document for post-autopilot DSPy integration across the codebase.

**Philosophy** (from Omar Khattab & Kevin Madura):
- DSPy is declarative AI programming, not just prompt optimization
- Signatures decouple AI specification from ML techniques
- Optimizers (GEPA/MIPROv2) find "latent requirements" you didn't specify
- Field names act as mini-prompts - naming matters
- Enable model portability without rewriting prompts

---

## Part 1: Autopilot DSPy Integration

### 1.1 Planning Phase Signature

**File:** `crates/autopilot/src/dspy_planning.rs` (NEW)

```rust
#[Signature]
struct PlanningSignature {
    /// Software Planning: Analyze repository and issue to create implementation plan.

    #[input] repository_summary: String,    // Codebase overview
    #[input] issue_description: String,     // What needs to be done
    #[input] relevant_files: String,        // Files discovered by preflight

    #[output] analysis: String,             // Understanding of the problem
    #[output] files_to_modify: String,      // JSON array of file paths
    #[output] implementation_steps: String, // JSON array of steps
    #[output] test_strategy: String,        // How to verify the solution
    #[output] risk_factors: String,         // Potential issues
    #[output] estimated_complexity: String, // LOW/MEDIUM/HIGH
    #[output] confidence: f32,              // 0.0-1.0
}

#[Signature(cot)]
struct DeepPlanningSignature {
    /// Deep Planning with Chain-of-Thought for complex tasks.
    // Same fields but with reasoning step
}
```

**Pipeline struct:**
```rust
pub struct PlanningPipeline {
    lm: Option<Arc<LM>>,
    planner: Predict,
    deep_planner: Predict,  // For complex tasks
}

pub struct PlanningResult {
    pub analysis: String,
    pub files_to_modify: Vec<String>,
    pub implementation_steps: Vec<String>,
    pub test_strategy: String,
    pub risk_factors: Vec<String>,
    pub complexity: Complexity,
    pub confidence: f32,
}
```

### 1.2 Execution Phase Signature

**File:** `crates/autopilot/src/dspy_execution.rs` (NEW)

```rust
#[Signature]
struct ExecutionStrategySignature {
    /// Execution Strategy: Decide how to implement the plan step by step.

    #[input] plan_step: String,             // Current step from plan
    #[input] current_file_state: String,    // Current file contents (if editing)
    #[input] execution_history: String,     // Previous tool calls and results

    #[output] next_action: String,          // EDIT_FILE/RUN_COMMAND/READ_FILE/COMPLETE
    #[output] action_params: String,        // JSON params for the action
    #[output] reasoning: String,            // Why this action
    #[output] progress_estimate: f32,       // 0.0-1.0 step completion
}

#[Signature]
struct ToolSelectionSignature {
    /// Tool Selection: Choose the right tool for the current task.

    #[input] task_description: String,
    #[input] available_tools: String,       // JSON array of tool definitions
    #[input] recent_context: String,        // Last few tool results

    #[output] selected_tool: String,        // Tool name
    #[output] tool_params: String,          // JSON params
    #[output] expected_outcome: String,     // What we expect to happen
    #[output] fallback_tool: String,        // If primary fails
}
```

### 1.3 Review Phase Enhancement

**Modify:** `crates/autopilot/src/dspy_verify.rs`

Add new signature for execution review:

```rust
#[Signature(cot)]
struct ExecutionReviewSignature {
    /// Execution Review: Verify execution matched the plan and assess quality.

    #[input] original_plan: String,         // The plan we were following
    #[input] execution_trace: String,       // Tool calls and results
    #[input] files_changed: String,         // Git diff summary

    #[output] plan_adherence: String,       // FULL/PARTIAL/DEVIATED
    #[output] unexpected_changes: String,   // Things not in plan
    #[output] missing_steps: String,        // Plan steps not executed
    #[output] quality_assessment: String,   // Code quality notes
    #[output] verdict: String,              // APPROVE/REVISE/REJECT
    #[output] confidence: f32,
}
```

### 1.4 Optimization Infrastructure

**File:** `crates/autopilot/src/dspy_optimization.rs` (NEW)

```rust
use dspy_rs::{Example, Prediction, Evaluator};

// ============================================================================
// Planning Metrics
// ============================================================================

/// Metric for evaluating planning quality
pub fn planning_metric(example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // 1. Check if files_to_modify are valid paths (25%)
    let files = prediction.get("files_to_modify", None);
    if valid_json_array(&files) && paths_look_valid(&files) {
        score += 0.25;
    }

    // 2. Check if steps are actionable (25%)
    let steps = prediction.get("implementation_steps", None);
    if valid_json_array(&steps) && steps_are_actionable(&steps) {
        score += 0.25;
    }

    // 3. Check test strategy is concrete (25%)
    let tests = prediction.get("test_strategy", None);
    if test_strategy_is_concrete(&tests) {
        score += 0.25;
    }

    // 4. Confidence matches complexity (25%)
    let complexity = prediction.get("estimated_complexity", None);
    let confidence = prediction.get("confidence", None);
    if confidence_matches_complexity(&complexity, &confidence) {
        score += 0.25;
    }

    score
}

/// Metric for evaluating execution decisions
pub fn execution_metric(example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // Action is valid
    let action = prediction.get("next_action", None);
    if valid_action(&action) { score += 0.33; }

    // Params are well-formed JSON
    let params = prediction.get("action_params", None);
    if valid_json(&params) { score += 0.33; }

    // Reasoning explains the choice
    let reasoning = prediction.get("reasoning", None);
    if reasoning_is_substantive(&reasoning) { score += 0.34; }

    score
}

// ============================================================================
// Training Data Collection
// ============================================================================

/// Load training examples from completed autopilot sessions
pub fn load_training_examples_from_sessions() -> Vec<Example> {
    // Load from ~/.openagents/autopilot/sessions/*/checkpoint.json
    // Filter for sessions with status == Complete
    // Extract planning inputs and outputs
}

/// Example dataset for planning optimization
pub struct PlanningDataset {
    pub examples: Vec<PlanningExample>,
}

pub struct PlanningExample {
    // Inputs
    pub repository_summary: String,
    pub issue_description: String,
    pub relevant_files: String,
    // Expected outputs (from successful sessions)
    pub expected_analysis: String,
    pub expected_files: Vec<String>,
    pub expected_steps: Vec<String>,
    pub expected_test_strategy: String,
}

// ============================================================================
// Optimization Runner
// ============================================================================

pub async fn optimize_planning_signature(
    dataset: &PlanningDataset,
    optimizer_type: OptimizerType,
) -> Result<Predict> {
    let base_predictor = Predict::new(PlanningSignature::new());

    match optimizer_type {
        OptimizerType::MIPRO => {
            let optimizer = MIPROv2::new(planning_metric);
            optimizer.compile(base_predictor, &dataset.to_examples()).await
        }
        OptimizerType::COPRO => {
            let optimizer = COPRO::new(planning_metric);
            optimizer.compile(base_predictor, &dataset.to_examples()).await
        }
    }
}
```

### 1.5 Integration into StartupState

**Modify:** `crates/autopilot/src/startup.rs`

```rust
#[cfg(feature = "dspy")]
use crate::dspy_planning::{PlanningPipeline, PlanningInput, PlanningResult};

impl StartupState {
    /// Run planning phase with DSPy signatures (when feature enabled)
    #[cfg(feature = "dspy")]
    pub async fn run_dspy_planning(&mut self) -> Result<PlanningResult> {
        let pipeline = PlanningPipeline::new();

        let input = PlanningInput {
            repository_summary: self.collect_repo_summary(),
            issue_description: self.user_prompt.clone().unwrap_or_default(),
            relevant_files: self.preflight_files.join("\n"),
        };

        let result = pipeline.plan(&input).await?;

        // Store structured result for execution phase
        self.structured_plan = Some(result.clone());

        // Log the plan for UI
        self.add_line(LogLine::new(
            LogStatus::Success,
            format!("Plan: {} files, {} steps",
                result.files_to_modify.len(),
                result.implementation_steps.len()),
        ));

        Ok(result)
    }

    /// Fallback to existing Claude-based planning when DSPy not enabled
    #[cfg(not(feature = "dspy"))]
    pub async fn run_dspy_planning(&mut self) -> Result<PlanningResult> {
        // Use existing claude.rs planning
        self.run_claude_planning().await
    }
}
```

---

## Part 2: File Structure

```
crates/autopilot/src/
├── dspy_planning.rs       # Planning signatures + pipeline (NEW)
├── dspy_execution.rs      # Execution signatures + pipeline (NEW)
├── dspy_verify.rs         # Verification signatures (EXISTING - enhance)
├── dspy_optimization.rs   # Metrics, datasets, optimization runners (NEW)
└── lib.rs                 # Add module declarations with feature flag
```

---

## Part 3: Roadmap Document

**Create:** `docs/DSPY_ROADMAP.md`

This document will outline the full DSPy integration vision:

### Contents:

```markdown
# DSPy Integration Roadmap for OpenAgents

## Vision
DSPy as the foundation for all AI decision-making in OpenAgents.
Declarative specifications that are model-agnostic and optimization-ready.

## Completed
- [x] RLM DspyOrchestrator (document analysis with provenance)
- [x] RLM signatures.rs (SpanRef-based evidence tracking)
- [x] Autopilot dspy_verify.rs (verification pipeline)

## Wave 2: Autopilot (Current)
- [ ] PlanningSignature + DeepPlanningSignature
- [ ] ExecutionStrategySignature + ToolSelectionSignature
- [ ] ExecutionReviewSignature
- [ ] Optimization infrastructure (metrics + training data)

## Wave 3: OANIX
- [ ] SituationAssessmentSignature (replace rule-based if-else)
- [ ] IssueSelectionSignature (smart prioritization)
- [ ] WorkPrioritizationSignature (what to work on next)
- [ ] LifecycleDecisionSignature (agent state transitions)

## Wave 4: Agent Orchestrator
- [ ] Convert 7 agent prompts (sisyphus, oracle, etc.) to Signatures
- [ ] DelegationSignature for Sisyphus orchestration
- [ ] SpecializationSignature for each sub-agent
- [ ] Model mixing strategy (different LMs for different agents)

## Wave 5: Tool Invocation
- [ ] Universal ToolSelectionSignature
- [ ] ToolResultInterpretationSignature
- [ ] ToolChainPlanningSignature (multi-tool sequences)

## Wave 6: Optimization Infrastructure
- [ ] DSPy Hub for OpenAgents (pre-optimized modules)
- [ ] Automated training data collection from sessions
- [ ] CI/CD pipeline for signature optimization
- [ ] A/B testing framework for optimized vs base signatures

## Model Mixing Strategy
| Signature Type | Recommended Model | Reasoning |
|----------------|-------------------|-----------|
| Planning | Claude Opus | Complex reasoning needed |
| Execution | Claude Sonnet | Balance of speed/quality |
| Review/Verify | Claude Haiku | Fast validation |
| OANIX Situation | Local (Ollama) | Privacy, always-on |
| Tool Selection | Any fast model | Simple classification |

## Optimization Strategy
1. Collect training data from successful sessions
2. Start with MIPROv2 for instruction optimization
3. Graduate to GEPA for complex signatures
4. Store optimized modules in ~/.openagents/dspy/optimized/
5. Version optimized modules with session hash
```

---

## Implementation Order

| Step | Task | Files |
|------|------|-------|
| 1 | Create dspy_planning.rs | `crates/autopilot/src/dspy_planning.rs` |
| 2 | Create dspy_execution.rs | `crates/autopilot/src/dspy_execution.rs` |
| 3 | Add ExecutionReviewSignature to dspy_verify.rs | `crates/autopilot/src/dspy_verify.rs` |
| 4 | Create dspy_optimization.rs | `crates/autopilot/src/dspy_optimization.rs` |
| 5 | Wire signatures into startup.rs | `crates/autopilot/src/startup.rs` |
| 6 | Update lib.rs exports | `crates/autopilot/src/lib.rs` |
| 7 | Create DSPY_ROADMAP.md | `docs/DSPY_ROADMAP.md` |
| 8 | Add example training data | `crates/autopilot/examples/dspy_examples.json` |

---

## Key Design Decisions

1. **Feature-gated**: All DSPy code behind `#[cfg(feature = "dspy")]`
2. **Composable**: Each signature is independent, can be used standalone
3. **Optimization-ready**: Metrics defined upfront for each signature
4. **Model-agnostic**: No hardcoded model names in signatures
5. **Training data from sessions**: Collect examples from successful autopilot runs
6. **Graceful fallback**: When DSPy feature disabled, use existing claude.rs

---

## Success Criteria

1. `cargo build -p autopilot --features dspy` compiles
2. Planning/Execution/Review signatures produce structured outputs
3. Metrics are measurable and return scores 0.0-1.0
4. DSPY_ROADMAP.md provides clear multi-wave plan
5. At least 5 example training datapoints documented
6. Signatures can be optimized with MIPROv2

---

## Critical Files

### New Files
- `crates/autopilot/src/dspy_planning.rs`
- `crates/autopilot/src/dspy_execution.rs`
- `crates/autopilot/src/dspy_optimization.rs`
- `docs/DSPY_ROADMAP.md`
- `crates/autopilot/examples/dspy_examples.json`

### Modified Files
- `crates/autopilot/src/dspy_verify.rs` - Add ExecutionReviewSignature
- `crates/autopilot/src/startup.rs` - Wire in DSPy pipelines
- `crates/autopilot/src/lib.rs` - Export new modules
- `crates/autopilot/Cargo.toml` - Already has dspy feature (no changes needed)
