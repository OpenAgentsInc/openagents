# DSPy Integration Roadmap for OpenAgents

## Vision

DSPy as the foundation for all AI decision-making in OpenAgents. Declarative specifications that are model-agnostic and optimization-ready.

**Philosophy** (from Omar Khattab & Kevin Madura):
- DSPy is declarative AI programming, not just prompt optimization
- Signatures decouple AI specification from ML techniques
- Optimizers (GEPA/MIPROv2) find "latent requirements" you didn't specify
- Field names act as mini-prompts - naming matters
- Enable model portability without rewriting prompts

---

## Completed

### Wave 1: RLM Document Analysis
- [x] `crates/rlm/src/dspy_orchestrator.rs` - 4-phase document pipeline
- [x] `crates/rlm/src/signatures.rs` - SpanRef-based evidence tracking
- [x] `crates/rlm/src/dspy_bridge.rs` - LM configuration and cost tracking

### Wave 2: Autopilot Integration
- [x] `crates/autopilot/src/dspy_planning.rs` - PlanningSignature + DeepPlanningSignature
- [x] `crates/autopilot/src/dspy_execution.rs` - ExecutionStrategySignature + ToolSelectionSignature
- [x] `crates/autopilot/src/dspy_verify.rs` - Verification + ExecutionReviewSignature
- [x] `crates/autopilot/src/dspy_optimization.rs` - Metrics + training data infrastructure

---

## Wave 3: OANIX (Agent OS Runtime)

Transform OANIX's rule-based decision making into DSPy signatures.

### Signatures to Create

```rust
#[Signature]
struct SituationAssessmentSignature {
    /// Situation Assessment: Analyze current system state and determine priorities.

    #[input] system_state: String,        // Current process states, resources
    #[input] pending_events: String,      // Events in queue
    #[input] recent_history: String,      // Last N decisions and outcomes

    #[output] priority_action: String,    // What to do next
    #[output] urgency: String,            // IMMEDIATE, NORMAL, DEFERRED
    #[output] reasoning: String,          // Why this action
    #[output] confidence: f32,
}

#[Signature]
struct IssueSelectionSignature {
    /// Issue Selection: Choose the best issue to work on from available options.

    #[input] available_issues: String,    // JSON array of issues
    #[input] agent_capabilities: String,  // What this agent can do
    #[input] current_context: String,     // Repository state, recent work

    #[output] selected_issue: String,     // Issue ID or number
    #[output] rationale: String,          // Why this issue
    #[output] estimated_complexity: String,
    #[output] confidence: f32,
}

#[Signature]
struct WorkPrioritizationSignature {
    /// Work Prioritization: Order tasks by importance and dependencies.

    #[input] task_list: String,           // JSON array of tasks
    #[input] dependencies: String,        // Task dependency graph
    #[input] deadlines: String,           // Any time constraints

    #[output] ordered_tasks: String,      // JSON array, priority order
    #[output] blocking_tasks: String,     // Tasks blocking others
    #[output] parallel_groups: String,    // Tasks that can run together
}

#[Signature(cot)]
struct LifecycleDecisionSignature {
    /// Lifecycle Decision: Determine agent state transitions.
    /// Use chain-of-thought to reason about state changes.

    #[input] current_state: String,       // Agent's current lifecycle state
    #[input] recent_events: String,       // What just happened
    #[input] resource_status: String,     // Memory, CPU, network

    #[output] next_state: String,         // IDLE, WORKING, BLOCKED, TERMINATING
    #[output] transition_reason: String,  // Why transition
    #[output] cleanup_needed: String,     // Any cleanup before transition
}
```

### Files to Create
- `crates/oanix/src/dspy_situation.rs`
- `crates/oanix/src/dspy_lifecycle.rs`

---

## Wave 4: Agent Orchestrator

Convert the 7 specialized agent prompts into DSPy Signatures.

### Current Agents (in `agent-orchestrator`)
1. **Sisyphus** - Master orchestrator
2. **Oracle** - Information retrieval
3. **Architect** - System design
4. **Coder** - Implementation
5. **Reviewer** - Code review
6. **DevOps** - Deployment
7. **Documenter** - Documentation

### Signatures to Create

```rust
#[Signature]
struct DelegationSignature {
    /// Delegation: Sisyphus decides which sub-agent should handle a task.

    #[input] task_description: String,
    #[input] available_agents: String,    // JSON with agent capabilities
    #[input] current_workload: String,    // What each agent is doing

    #[output] assigned_agent: String,
    #[output] task_refinement: String,    // Refined instructions for agent
    #[output] expected_deliverables: String,
    #[output] fallback_agent: String,
}

#[Signature]
struct OracleQuerySignature {
    /// Oracle: Find and synthesize information from various sources.

    #[input] query: String,
    #[input] search_scope: String,        // Codebase, docs, web
    #[input] relevance_criteria: String,

    #[output] findings: String,           // JSON array of findings
    #[output] sources: String,            // Where info came from
    #[output] confidence: f32,
    #[output] gaps: String,               // What couldn't be found
}

#[Signature(cot)]
struct ArchitectureSignature {
    /// Architecture: Design system changes with careful reasoning.

    #[input] requirements: String,
    #[input] existing_architecture: String,
    #[input] constraints: String,

    #[output] proposed_changes: String,
    #[output] component_diagram: String,
    #[output] migration_path: String,
    #[output] risks: String,
}

// Similar signatures for Coder, Reviewer, DevOps, Documenter...
```

### Files to Create
- `crates/agent-orchestrator/src/dspy_delegation.rs`
- `crates/agent-orchestrator/src/dspy_agents.rs`

---

## Wave 5: Tool Invocation

Universal tool selection and interpretation layer.

### Signatures

```rust
#[Signature]
struct ToolSelectionSignature {
    /// Tool Selection: Choose the right tool for any task.

    #[input] task_description: String,
    #[input] available_tools: String,     // JSON tool definitions
    #[input] context: String,             // Recent tool results

    #[output] selected_tool: String,
    #[output] tool_params: String,        // JSON params
    #[output] expected_outcome: String,
    #[output] fallback_tool: String,
}

#[Signature]
struct ToolResultInterpretationSignature {
    /// Result Interpretation: Understand what a tool result means.

    #[input] tool_name: String,
    #[input] tool_output: String,
    #[input] original_intent: String,

    #[output] success: String,            // YES, PARTIAL, NO
    #[output] extracted_info: String,     // Key information from output
    #[output] next_steps: String,         // What to do next
    #[output] error_analysis: String,     // If failed, why
}

#[Signature]
struct ToolChainPlanningSignature {
    /// Tool Chain: Plan multi-tool sequences for complex tasks.

    #[input] goal: String,
    #[input] available_tools: String,
    #[input] constraints: String,

    #[output] tool_sequence: String,      // JSON array of tool calls
    #[output] dependencies: String,       // Which calls depend on others
    #[output] parallelizable: String,     // Which can run in parallel
}
```

### Files to Create
- `crates/openagents-runtime/src/dspy_tools.rs`

---

## Wave 6: Optimization Infrastructure

Production-ready optimization pipeline.

### Components

1. **DSPy Hub for OpenAgents**
   - Pre-optimized modules stored in `~/.openagents/dspy/optimized/`
   - Version modules with session hash
   - Share optimized modules across machines

2. **Automated Training Data Collection**
   - Extract examples from successful autopilot sessions
   - Store in `~/.openagents/dspy/training/`
   - Format: JSONL with inputs and expected outputs

3. **CI/CD for Signature Optimization**
   - Nightly optimization runs
   - Track optimization metrics over time
   - Automated regression testing

4. **A/B Testing Framework**
   - Compare optimized vs base signatures
   - Track success rates by signature
   - Gradual rollout of optimized versions

### Files to Create
- `crates/autopilot/src/dspy_hub.rs`
- `crates/autopilot/src/dspy_training.rs`
- `scripts/optimize_signatures.rs`

---

## Model Mixing Strategy

Different signatures benefit from different models:

| Signature Type | Recommended Model | Reasoning |
|----------------|-------------------|-----------|
| Planning (Deep) | Claude Opus | Complex reasoning needed |
| Planning (Simple) | Claude Sonnet | Balance of speed/quality |
| Execution | Claude Sonnet | Balance of speed/quality |
| Review/Verify | Claude Haiku | Fast validation |
| OANIX Situation | Local (Ollama) | Privacy, always-on |
| Tool Selection | Any fast model | Simple classification |
| Oracle Query | Claude Sonnet | Good at synthesis |
| Architecture | Claude Opus | Needs deep reasoning |

---

## Optimization Strategy

1. **Collect Training Data**
   - From successful autopilot sessions
   - From manual corrections
   - From user feedback

2. **Start with MIPROv2**
   - Instruction optimization first
   - Low computational cost
   - Good baseline improvements

3. **Graduate to GEPA**
   - For complex signatures
   - When MIPROv2 plateaus
   - Higher computational cost

4. **Store Optimized Modules**
   - In `~/.openagents/dspy/optimized/`
   - Version with session hash
   - Include optimization metrics

5. **Version Optimized Modules**
   - Track which sessions used which version
   - Enable rollback if needed
   - Compare versions over time

---

## Success Metrics

For each signature, track:
- **Task Success Rate**: Did the signature lead to successful outcomes?
- **Confidence Calibration**: Does confidence match actual success?
- **Latency**: How long does inference take?
- **Token Usage**: How many tokens per call?
- **User Corrections**: How often do users override?

---

## Implementation Priority

| Wave | Priority | Estimated Effort |
|------|----------|------------------|
| 3: OANIX | High | Medium |
| 4: Agent Orchestrator | High | Large |
| 5: Tool Invocation | Medium | Medium |
| 6: Optimization | Medium | Large |

Start with OANIX (Wave 3) since it's self-contained and will provide learnings for the larger Agent Orchestrator integration.

---

## References

- [DSPy Documentation](https://dspy.ai)
- [Omar Khattab: State of DSPy](../docs/transcripts/dspy/state-of-dspy.md)
- [Kevin Madura: DSPy is All You Need](../docs/transcripts/dspy/dspy-is-all-you-need.md)
- [dspy-rs Crate](../../../DSRs/crates/dspy-rs)
