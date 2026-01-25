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

The concrete plan-mode signatures are defined in `crates/dsrs/src/signatures/plan_mode.rs`.
The examples below illustrate the shape and intent of the signature contracts.

### Core Planning Signatures

```rust
use dsrs_macros::Signature;

#[Signature]
struct TaskPlanningSignature {
    /// Task Planning: Analyze user request and create detailed execution plan
    /// with subtasks, dependencies, and resource requirements.
    
    #[input]
    task_description: String,
    
    #[input] 
    workspace_context: String,
    
    #[input]
    available_tools: String,
    
    #[output]
    plan_analysis: String,
    
    #[output]
    subtasks: String,  // JSON array of subtasks
    
    #[output]
    execution_order: String,  // Dependency-ordered task sequence
    
    #[output]
    confidence: f32,
}

#[Signature(cot)]
struct DeepPlanningSignature {
    /// Deep Planning: Complex multi-step reasoning for sophisticated tasks
    /// requiring analysis across multiple files, systems, or domains.
    
    #[input]
    complex_request: String,
    
    #[input]
    codebase_analysis: String,
    
    #[input]
    constraints: String,
    
    #[output]
    reasoning: String,  // Chain-of-thought process
    
    #[output]
    strategy: String,
    
    #[output]
    implementation_plan: String,
    
    #[output]
    risk_assessment: String,
}
```

### Task Execution Signatures

```rust
#[Signature]
struct SubtaskExecutionSignature {
    /// Subtask Execution: Convert planned subtask into concrete actions
    /// with tool calls, file operations, and validation steps.
    
    #[input]
    subtask_description: String,
    
    #[input]
    current_context: String,
    
    #[input]
    available_actions: String,
    
    #[output]
    action_sequence: String,  // JSON array of actions
    
    #[output]
    validation_criteria: String,
    
    #[output]
    expected_outcome: String,
}

#[Signature]
struct ToolSelectionSignature {
    /// Tool Selection: Choose appropriate tools for current task context
    /// based on requirements, file types, and desired outcomes.
    
    #[input]
    task_context: String,
    
    #[input]
    available_tools: String,
    
    #[input]
    workspace_state: String,
    
    #[output]
    selected_tools: String,  // JSON array of tool names
    
    #[output]
    tool_reasoning: String,
    
    #[output]
    execution_strategy: String,
}
```

### Result Synthesis Signatures

```rust
#[Signature]
struct ResultSynthesisSignature {
    /// Result Synthesis: Combine outputs from multiple subtasks into
    /// coherent response with validation and quality assessment.
    
    #[input]
    completed_subtasks: String,  // JSON of subtask results
    
    #[input]
    original_request: String,
    
    #[input]
    validation_results: String,
    
    #[output]
    synthesized_result: String,
    
    #[output]
    quality_assessment: String,
    
    #[output]
    remaining_actions: String,
    
    #[output]
    success_confidence: f32,
}
```

## Planning Pipeline

### Plan Mode Architecture

Adjutant operates primarily in "Plan Mode" - a mode where the entire workflow is composed of DSPy signatures:

```rust
pub struct AdjutantPlanningPipeline {
    task_planner: Predict<TaskPlanningSignature>,
    deep_planner: Predict<DeepPlanningSignature>,
    subtask_executor: Predict<SubtaskExecutionSignature>,
    tool_selector: Predict<ToolSelectionSignature>,
    result_synthesizer: Predict<ResultSynthesisSignature>,
    lm: Arc<LM>,
}

impl AdjutantPlanningPipeline {
    pub async fn execute_task(&self, request: &str, context: &WorkspaceContext) -> Result<TaskResult> {
        // 1. Initial Planning
        let plan = self.create_initial_plan(request, context).await?;
        
        // 2. Complexity Assessment & Deep Planning (if needed)
        let refined_plan = if plan.complexity > 0.7 {
            self.deep_plan(request, context, &plan).await?
        } else {
            plan
        };
        
        // 3. Subtask Execution
        let mut results = Vec::new();
        for subtask in refined_plan.subtasks {
            let result = self.execute_subtask(&subtask, context).await?;
            results.push(result);
        }
        
        // 4. Result Synthesis
        let final_result = self.synthesize_results(&results, request).await?;
        
        Ok(final_result)
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
- [ ] Implement MIPROv2 optimization pipeline
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
#[derive(Debug, Clone)]
pub struct AdjutantConfig {
    pub model_config: ModelConfig,
    pub planning_mode: PlanningMode,
    pub optimization_enabled: bool,
    pub shadow_mode: bool,
    pub max_subtasks: usize,
    pub complexity_threshold: f32,
}

#[derive(Debug, Clone)]
pub enum PlanningMode {
    Simple,      // TaskPlanningSignature only
    Deep,        // Includes DeepPlanningSignature for complex tasks
    Adaptive,    // Dynamic routing based on complexity
}
```

## Integration with Frontend

Adjutant integrates seamlessly with the existing frontend through the same interfaces:

- **TypeScript**: `src/agent/adjutant.ts` following same pattern as `codex.ts` and `gemini.ts`
- **Event Handling**: Emits `UnifiedEvent` (chat stream) plus `UiEvent` for signature-driven UI
- **Session Management**: Standard ACP session lifecycle
- **UI Components**: Effuse canvas renders UITree updates via `ui-event`

## Future Enhancements

1. **Multi-Modal Planning**: Integration with FRLM signatures for file/image analysis
2. **Collaborative Planning**: Multi-agent coordination signatures
3. **Learning Pipeline**: Automated signature optimization from user feedback
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
- ✅ `ToolSelectionSignature` - Selects optimal tools for exploration
- ✅ `ResultValidationSignature` - Validates plan quality and completeness

#### **Plan Mode Pipeline**
- ✅ Full planmode workflow implementation
- ✅ Topic decomposition with JSON schema enforcement
- ✅ Parallel exploration with isolated agent contexts
- ✅ Plan synthesis and complexity routing
- ✅ Quality validation and confidence scoring

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
