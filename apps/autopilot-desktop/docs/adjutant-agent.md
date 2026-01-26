# Adjutant Agent - DSPy-Native ACP Agent

## Overview

The **Adjutant Agent** is a DSPy-native agent designed for the Autopilot ACP (Agent Client Protocol) system. Unlike Codex and Gemini agents that wrap external binaries, Adjutant is built natively with first-class DSPy/dsrs support, enabling sophisticated planning, task decomposition, and execution through DSPy signatures.

## Architecture

### Core Components

```
crates/autopilot-desktop-backend/src/agent/
├── adjutant/
│   ├── mod.rs              # Main module
│   ├── agent.rs            # Agent implementation
│   ├── config.rs           # Plan mode configuration
│   ├── lm_client.rs        # Local AI Gateway client
│   └── planning.rs         # Planning pipeline
```

**Signature sources**: plan-mode signatures live in `crates/dsrs/src/signatures/plan_mode.rs`
and are exposed via `crates/dsrs/src/signature_registry.rs`. Autopilot Desktop
imports these instead of defining local duplicates.

### Integration with ACP

Adjutant follows the same ACP integration pattern as Codex and Gemini:

1. **Implements `Agent` trait**: Standard interface for session management, messaging
2. **Uses `UnifiedEvent` system**: Consistent event handling across all agents
3. **Supports ACP lifecycle**: Connect, disconnect, start_session, send_message
4. **Native implementation**: No external process spawning - pure Rust/dsrs

## DSPy Signatures

Plan-mode signatures live in `crates/dsrs/src/signatures/plan_mode.rs` and are exposed via
`crates/dsrs/src/signature_registry.rs`.

Wired into the current `PlanModePipeline`:
- `TopicDecompositionSignature`: turns a user request + file tree into 2-4 exploration topics.
- `ParallelExplorationSignature`: explores a topic with file context and returns findings.
- `PlanSynthesisSignature`: synthesizes exploration results into an implementation plan.
- `ComplexityClassificationSignature`: routes requests toward deep planning or standard synthesis.
- `DeepPlanningSignature`: optional chain-of-thought planning for complex tasks.
- `ResultValidationSignature`: optional quality/issue validation for the plan output.

Registered but not wired into the plan pipeline yet:
- `ToolSelectionSignature`

## Planning Pipeline

### Plan Mode Architecture

Adjutant operates primarily in "Plan Mode" - a mode where the entire workflow is composed of DSPy signatures:

```rust
pub struct PlanModePipeline {
    topic_decomposer: Predict,
    synthesis_predictor: Predict,
    complexity_classifier: Predict,
    deep_planner: Option<Predict>,
    validator: Predict,
    lm: Option<Arc<LM>>,
}

impl PlanModePipeline {
    pub async fn execute_plan_mode(&self, user_prompt: &str) -> Result<PlanResult> {
        // 1. File tree context
        // 2. Complexity classification
        // 3. Topic decomposition
        // 4. Parallel exploration
        // 5. Plan synthesis or deep planning
        // 6. Optional validation
    }
}
```

### Decision Routing

```rust
#[Signature]
struct ComplexityClassificationSignature {
    /// Complexity Classifier: Route tasks to appropriate execution paths
    /// based on scope, file count, and domain complexity.
    
    #[input]
    task_description: String,
    
    #[input] 
    file_count: String,
    
    #[input]
    domain_indicators: String,
    
    #[output]
    complexity: String,  // "Low", "Medium", "High", "VeryHigh"
    
    #[output]
    routing_decision: String,  // Which pipeline to use
    
    #[output]
    reasoning: String,
}
```

## Implementation Plan

### Phase 1: Core Infrastructure
- [x] Create `crates/autopilot-desktop-backend/src/agent/adjutant/` module structure
- [x] Implement basic Agent trait for Adjutant
- [x] Wire dsrs signatures into the planning pipeline
- [x] Create planning pipeline foundation

### Phase 2: DSPy Integration
- [x] Implement core planning signatures (in dsrs)
- [x] Build execution engine with signature orchestration
- [x] Add tool selection and routing logic
- [x] Implement result synthesis

### Phase 3: ACP Integration
- [x] Integrate with unified event system
- [x] Add session management
- [x] Implement streaming responses via UnifiedEvent
- [ ] Add capability advertisements

### Phase 4: Optimization & Learning
- [x] Implement plan mode optimization pipeline (MIPROv2/COPRO/GEPA)
- [ ] Add counterfactual recording vs other agents
- [ ] Build policy bundle system
- [ ] Add shadow mode deployment

## Key Advantages

1. **Native DSPy Integration**: No external process overhead, direct signature execution
2. **Sophisticated Planning**: Multi-level planning with complexity routing
3. **Self-Optimizing**: Built-in DSPy optimization for continuous improvement  
4. **Tool-Agnostic**: Signature-based tool selection and routing
5. **Unified Interface**: Same ACP interface as Codex/Gemini for seamless integration

## Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeConfig {
    pub max_topics: usize,
    pub max_tool_calls_per_agent: usize,
    pub enable_deep_planning: bool,
    pub deep_planning_threshold: f32,
    pub enable_validation: bool,
    pub optimization: PlanModeOptimizationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeOptimizationConfig {
    pub enabled: bool,
    pub record_training: bool,
    pub benchmark_only: bool,
    pub min_examples: usize,
    pub max_examples: usize,
    pub min_hours_between_runs: u64,
    pub max_signatures_per_run: usize,
    pub optimize_all_signatures: bool,
    pub optimizer: PlanModeOptimizerKind,
    pub num_candidates: usize,
    pub num_trials: usize,
    pub minibatch_size: usize,
    pub temperature: f32,
    pub background_optimization: bool,
    pub apply_optimized_instructions: bool,
    pub log_benchmarks: bool,
}
```

### Plan Mode Optimization

Plan mode now records training examples, benchmarks signature quality, and writes optimized
instructions to per-signature manifests. Optimizations run on a cadence with configurable
thresholds and can be backgrounded.
Signature panels read optimized instructions when `apply_optimized_instructions` is enabled.

Optimizers: `MIPROv2`, `COPRO`, `GEPA`.

Key files (local machine):
- `~/.openagents/autopilot-desktop/training/plan_mode.json`
- `~/.openagents/autopilot-desktop/optimization/plan_mode.jsonl`
- `~/.openagents/autopilot-desktop/optimization/plan_mode_state.json`
- `~/.openagents/autopilot-desktop/manifests/plan_mode/*.json`
- `~/.openagents/autopilot-desktop/manifests/plan_mode/*.latest.json`

## Integration with Frontend

Adjutant integrates seamlessly with the existing frontend through the same interfaces:

- **TypeScript**: `src/agent/adjutant.ts` following same pattern as `codex.ts` and `gemini.ts`
- **Event Handling**: Emits `UnifiedEvent` (chat stream) plus `UiEvent` for signature-driven UI
- **Session Management**: Standard ACP session lifecycle
- **UI Components**: Effuse canvas renders UITree updates via `ui-event`

## Future Enhancements

1. **Multi-Modal Planning**: Integration with FRLM signatures for file/image analysis
2. **Collaborative Planning**: Multi-agent coordination signatures
3. **Learning Pipeline**: Counterfactual labels + policy bundle promotion
4. **Custom Signatures**: User-defined planning signatures for domain-specific tasks
5. **Performance Analytics**: DSPy execution metrics and optimization insights

## Implementation Status

### ✅ **Completed Implementation**

The Adjutant Agent has been successfully implemented and integrated into the Autopilot ACP system:

#### **Core Infrastructure**
- ✅ Added `AgentId::Adjutant` to unified agent system
- ✅ Created complete agent module at `crates/autopilot-desktop-backend/src/agent/adjutant/`
- ✅ Integrated with existing Agent trait and manager
- ✅ Updated TypeScript types and agent registry

#### **DSPy Signatures Framework**
- ✅ `TopicDecompositionSignature` - Breaks prompts into 2-4 exploration topics
- ✅ `ParallelExplorationSignature` - Individual agent exploration (8 tool calls max)
- ✅ `PlanSynthesisSignature` - Combines exploration results into implementation plans
- ✅ `ComplexityClassificationSignature` - Routes tasks based on complexity analysis
- ✅ `DeepPlanningSignature` - Chain-of-thought reasoning for sophisticated tasks
- ✅ `ToolSelectionSignature` - Registered (not wired into plan-mode pipeline yet)
- ✅ `ResultValidationSignature` - Validates plan quality and completeness

#### **Plan Mode Pipeline**
- ✅ Full planmode workflow implementation
- ✅ Topic decomposition with JSON schema enforcement
- ✅ Parallel exploration with isolated agent contexts
- ✅ Plan synthesis and complexity routing
- ✅ Quality validation and confidence scoring
- ✅ Training data capture + benchmarked optimization loop

#### **Frontend Integration**
- ✅ Available in agent selector alongside Codex and Gemini
- ✅ Uses unified streaming interface with chunked responses
- ✅ Emits signature-driven UI updates (`ui-event`) for the Effuse canvas

### ✅ **DSPy Integration Live**

Plan mode now runs real dsrs predictors with the local AI Gateway:

```rust
let pipeline = PlanModePipeline::new(workspace_path, config)
    .with_auto_lm()
    .await;

let plan = pipeline.execute_plan_mode(prompt).await?;
```

---

*This document serves as the technical specification for Adjutant as a first-class DSPy agent within the Autopilot ACP ecosystem.*
