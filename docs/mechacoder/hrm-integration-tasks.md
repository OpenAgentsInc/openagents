# HRM Integration Tasks

This document explains each task created to integrate insights from the Hierarchical Reasoning Model (HRM) paper into MechaCoder's learning system.

## Background

The HRM paper demonstrates that a 27M parameter model can outperform much larger LLMs on reasoning tasks (ARC-AGI, Sudoku, Maze) by using:
- Deep recursive computation instead of model scale
- Two-level state hierarchy (high-level planning + low-level execution)
- Adaptive computation time (think longer on hard problems)
- Learning from every iteration, not just final outcomes

These insights align perfectly with MechaCoder's architecture and Apple FM on-device inference strategy.

---

## Phase 1: Two-Level State Architecture

### oa-ee0011: Define HierarchicalState schema

**Description:** Create `src/learning/state.ts` with HierarchicalState interface containing zH (high-level: planEmbedding, taskDecomposition, completedSteps, confidenceScore), zL (low-level: currentTrace, toolCallHistory, errorContext, iterationCount), and y (candidate, validationStatus, feedbackHistory).

**Why:** HRM's core insight is that reasoning benefits from two explicit state representations operating at different timescales:
- **zH (high-level):** Abstract planning state that updates slowly. In MechaCoder, this maps to the orchestrator's strategic understanding.
- **zL (low-level):** Detailed execution state that updates rapidly. In MechaCoder, this maps to subagent tool traces.
- **y (solution):** The current proposed answer being refined.

Without explicit state tracking, MechaCoder loses the ability to reason about *where* it is in the problem-solving process. This schema makes the implicit explicit.

---

### oa-98ebd8: Update TrainingLoop with state management

**Description:** Modify `src/learning/loop.ts` to add HierarchicalState to LoopState. Track state transitions during training and persist state for analysis and debugging.

**Why:** The training loop is where MechaCoder learns. By integrating HierarchicalState:
1. Each training iteration captures the full reasoning context
2. State transitions become observable and debuggable
3. We can analyze which state patterns lead to success/failure
4. The Archivist can record richer trajectory data

This transforms the loop from "run agent, check result" to "track reasoning evolution, learn from process."

---

### oa-3960ce: Update Gym to use hierarchical state

**Description:** Modify `src/trainer/gym.ts` to inject zH (orchestrator context) into subagent prompts, capture zL after each tool call, and track y (solution candidate) evolution.

**Why:** The Gym is where subagents execute. Currently, subagents operate somewhat independently. With hierarchical state:
1. Subagents receive zH context (they know the high-level plan)
2. Every tool call updates zL (we capture execution details)
3. Solution evolution (y) is tracked across attempts

This creates the two-speed system HRM uses: orchestrator thinks slowly (zH), subagents act quickly (zL).

---

## Phase 2: Hierarchical Convergence

### oa-0becba: Add nested cycle structure (hierarchical loop)

**Description:** Create `src/learning/hierarchical-loop.ts` with CycleConfig (T: L-module steps per H-update, N: total H cycles, convergenceThreshold) and hierarchicalLoop function implementing nested cycles.

**Why:** HRM shows that naive iteration causes premature convergence—the model settles on a wrong answer too quickly. The solution is nested cycles:
- **Inner loop (T steps):** L-module (subagent) iterates until it converges locally
- **Outer loop (N cycles):** H-module (orchestrator) updates the plan and resets L

This prevents getting stuck. When subagents exhaust their local approach, the orchestrator can redirect with fresh context.

---

### oa-42d7d2: Implement convergence detection

**Description:** Create `src/learning/convergence.ts` to track residual between consecutive zL states, detect when L-module reaches local equilibrium, and trigger H-module update when L converges.

**Why:** We need to know *when* the L-module has converged so we can trigger an H-update. HRM tracks the "forward residual"—how much the state changes between iterations. When changes become small, the module has converged.

For MechaCoder, this means detecting when a subagent is spinning its wheels (repeating similar tool calls, getting same errors). That's the signal to escalate to the orchestrator.

---

### oa-3880bd: Add L-reset on H-update

**Description:** Create `src/learning/reset.ts` to reset L-module to fresh context after H-module updates. Preserve only task context and H-state, clear execution trace and error history.

**Why:** When the orchestrator updates its plan (zH changes), the subagent should start fresh. Carrying over stale execution traces and error context from the previous approach can bias the new attempt.

HRM explicitly detaches/resets the L-state after each H-update. This gives the subagent a clean slate to try the new plan without being anchored to past failures.

---

## Phase 3: Adaptive Computation Time (ACT)

### oa-8090ef: Add Q-head for ACT halt/continue

**Description:** Create `src/learning/act.ts` with ACTConfig (Mmax, epsilon), QValues interface, computeQValues function to predict Q-values from zH state, and shouldHalt function.

**Why:** Not all tasks need the same amount of thinking. HRM uses Q-learning to decide: "Should I stop and output an answer, or keep thinking?"

For MechaCoder:
- **Easy tasks:** Halt after 1-2 attempts (save compute)
- **Hard tasks:** Keep trying for 10+ cycles
- **Stuck tasks:** Eventually halt and report failure

This adaptive depth is crucial for efficiency. Without it, we either waste compute on easy tasks or give up too early on hard ones.

---

### oa-1e3eec: Update TrainerService with ACT

**Description:** Modify `src/trainer/service.ts` to add ACT configuration to TrainingConfig, track Q-values during episodes, and update Q-head based on task outcomes.

**Why:** The TrainerService orchestrates training. It needs to:
1. Configure ACT parameters (max segments, exploration rate)
2. Query the Q-head at each cycle ("halt or continue?")
3. Record halt decisions and their outcomes
4. Update Q-values based on whether halting was correct

This integrates ACT into the existing training infrastructure.

---

### oa-ea7259: Add Q-learning update for ACT

**Description:** Create `src/learning/q-learning.ts` with Q-learning targets (Ghat_halt, Ghat_continue), binary cross-entropy loss for Q-head, and halt decision/outcome tracking.

**Why:** The Q-head needs to learn when to halt. This requires:
1. **Targets:** Compute expected reward for halting vs continuing
2. **Loss:** Binary cross-entropy to train the decision boundary
3. **Updates:** Adjust Q-values based on actual outcomes

HRM uses this to achieve 2-3x compute savings on easy tasks while still solving hard tasks that need many iterations.

---

## Phase 4: Deep Supervision

### oa-4249d4: Add per-segment evaluation (deep supervision)

**Description:** Create `src/learning/deep-supervision.ts` with SupervisionSegment interface and deepSupervisionStep function. Compute loss for each segment, update skills/memory, and detach state for next segment.

**Why:** Traditional training only learns from final outcomes. HRM learns from *every* intermediate step:
1. Each segment produces a candidate solution
2. Evaluate that candidate (even if we'll continue)
3. Update skills/memory based on partial success
4. Detach state before next segment

For MechaCoder, this means every Gym attempt contributes to learning—not just the final success/failure. A subagent that makes partial progress teaches us something valuable.

---

### oa-168328: Integrate segments with Archivist

**Description:** Modify `src/archivist/service.ts` to record each segment as trajectory action, link segments with parent trajectory, and enable segment-level pattern extraction.

**Why:** The Archivist stores trajectory data for pattern mining. Currently it records episode-level trajectories. With segments:
1. Each segment becomes a first-class trajectory action
2. Segments link to their parent trajectory (maintains hierarchy)
3. Pattern extraction can find segment-level patterns

This enables questions like "What segment patterns predict breakthrough on stuck problems?"

---

### oa-a85fe8: Add segment-level skill extraction

**Description:** Create `src/skills/segment-extractor.ts` to extract skills from successful segment sequences, track which segments led to breakthroughs, and promote high-value segments to skills.

**Why:** Current skill extraction works at episode level. But sometimes a single segment contains the "aha moment" that unlocks a solution. Segment-level extraction:
1. Identifies high-value segments (ones that led to progress)
2. Extracts skills from segment sequences (not just full episodes)
3. Promotes breakthrough segments to the skill library

This creates finer-grained, more reusable skills.

---

## Phase 5: Representational Hierarchy

### oa-b4324f: Add dimensionality metrics

**Description:** Create `src/learning/dimensionality.ts` with DimensionalityMetrics interface and computeParticipationRatio function (PR = (sum(λi))² / sum(λi²)).

**Why:** HRM discovered that high-level states naturally develop ~3x higher dimensionality than low-level states (PR=89.95 vs PR=30.22). This ratio is a sign of healthy hierarchy.

For MechaCoder, tracking this metric:
1. Validates that the hierarchy is forming correctly
2. Alerts if zH becomes too narrow (under-planning)
3. Alerts if zL becomes too broad (over-thinking execution)

---

### oa-b40a96: Ensure orchestrator has richer context

**Description:** Modify `src/learning/orchestrator.ts` to give orchestrator richer embeddings: more context in system prompt, higher skill injection count (top-10 vs top-5), more memory retrieval (top-10 vs top-5).

**Why:** The orchestrator (zH) should have high dimensionality—rich, abstract understanding. This means:
1. More comprehensive system prompts
2. More skills available for composition
3. More memory for strategic pattern matching
4. Richer planning prompts that encourage strategic thinking

This ensures zH has the representational capacity for high-level reasoning.

---

### oa-e46b17: Keep subagent context focused

**Description:** Modify `src/trainer/gym.ts` to keep subagent prompts narrow and execution-focused: fewer skills (top-3 most relevant), minimal memory (only directly relevant), clear action space.

**Why:** Subagents (zL) should have low dimensionality—tight, efficient execution. This means:
1. Focused prompts about the immediate task
2. Only the most relevant skills (top-3)
3. Minimal memory (avoid distraction)
4. Clear, bounded action space

This ensures zL stays efficient and doesn't get lost in irrelevant context. The orchestrator handles breadth; subagents handle depth.

---

## Phase 6: Visualization and Analysis

### oa-e1141a: Add intermediate prediction visualization

**Description:** Create `src/dashboard/trajectory-view.ts` to visualize y (solution) evolution over segments, show zH updates as major steps, and show zL iterations as sub-steps.

**Why:** Understanding hierarchical reasoning requires visualization:
1. See how the solution (y) evolves across segments
2. Distinguish H-updates (major plan changes) from L-iterations (refinements)
3. Observe the rhythm of convergence and reset

This helps debug problems like "orchestrator never updates plan" or "subagent oscillates without converging."

---

### oa-18efc5: Add convergence monitoring

**Description:** Create `src/dashboard/convergence.ts` to plot forward residuals (like HRM Figure 3), show hierarchical convergence pattern, and alert on premature convergence.

**Why:** HRM's Figure 3 shows beautiful convergence patterns—residuals drop in steps as L converges, then spike when H resets. Monitoring this:
1. Confirms the hierarchical loop is working
2. Detects premature convergence (L never triggers H-update)
3. Detects thrashing (H updates too frequently)
4. Helps tune convergence thresholds

---

### oa-c2b296: Add dimensionality tracking

**Description:** Create `src/dashboard/dimensionality.ts` to track zH/zL participation ratio over training, verify hierarchy emerges through training, and compare to HRM's 2.98 ratio target.

**Why:** The ~3x dimensionality ratio should emerge through training—it's not hard-coded. Tracking this:
1. Confirms hierarchy is developing naturally
2. Compares to HRM's empirical ratio (2.98)
3. Detects if hierarchy collapses (ratio → 1)
4. Validates the representational architecture

---

## Implementation Order

Tasks should be implemented in phase order:

```
Phase 1 (State) → Phase 2 (Convergence) → Phase 3 (ACT) → Phase 4 (Deep Supervision) → Phase 5 (Hierarchy) → Phase 6 (Visualization)
```

Within each phase, tasks are ordered by dependency:
- Task X.1 creates the schema/interface
- Task X.2 integrates with existing systems
- Task X.3 adds advanced features

---

## Success Criteria

1. **Hierarchical State:** Training loop maintains explicit {y, zL, zH} state
2. **Nested Cycles:** Inner L-cycles and outer H-cycles visible in logs
3. **ACT Working:** Tasks use variable computation depth based on difficulty
4. **Deep Supervision:** Each segment produces learning signal
5. **Dimensionality Ratio:** zH/zL PR approaches HRM's ~3x ratio
6. **TB Improvement:** Terminal-Bench success rate improves

---

## References

- HRM Paper: "Hierarchical Reasoning Model" - 27M params, 40.3% ARC-AGI-1
- Plan file: `/Users/christopherdavid/.claude/plans/vivid-noodling-forest.md`
- Analysis: `docs/research/paper-summaries/hrm.md`
