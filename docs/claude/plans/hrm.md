# Plan: Integrate HRM Insights into MechaCoder Learning System

## Executive Summary

The Hierarchical Reasoning Model (HRM) paper provides a brain-inspired architecture that achieves remarkable reasoning performance with only 27M parameters and ~1000 training examples. This plan integrates HRM's key insights into MechaCoder's existing learning system to improve Terminal-Bench performance.

---

## Key HRM Insights to Integrate

### 1. Two-Level State Architecture (zH, zL)
**HRM Concept:** Two recurrent modules operating at different timescales:
- zH (high-level): Slow, abstract planning - updates every T steps
- zL (low-level): Fast, detailed computation - updates every step

**MechaCoder Application:**
- Orchestrator = zH (strategy, plan embedding)
- Subagents = zL (execution-level reasoning trace)
- Explicitly model `{y, zL, zH}` where y = current solution candidate

### 2. Hierarchical Convergence
**HRM Concept:** L-module converges to local equilibrium, then H-module updates and resets L. Prevents premature convergence.

**MechaCoder Application:**
- Structure Golden Loop as nested cycles
- Inner loop: subagent attempts (fast L cycles)
- Outer loop: orchestrator plan updates (slow H updates)
- Reset subagent context after each orchestrator decision

### 3. Adaptive Computation Time (ACT)
**HRM Concept:** Q-learning decides when to halt vs continue thinking. Dynamic "thinking time" per task.

**MechaCoder Application:**
- Add halting mechanism to training loop
- Easy tasks: fewer retry cycles
- Hard tasks: more recursive reasoning steps
- Learn halt/continue policy from task outcomes

### 4. Deep Supervision
**HRM Concept:** Learn at multiple points during forward pass, not just final output. Detach state between segments.

**MechaCoder Application:**
- Every Gym episode produces candidate solution
- Evaluate and learn from each attempt
- Detach environmental state between attempts
- Each iteration = new supervised training example

### 5. Representational Dimensionality Hierarchy
**HRM Concept:** High-level state has 3x higher dimensionality (PR=89.95) than low-level (PR=30.22).

**MechaCoder Application:**
- Orchestrator embeddings: richer, higher-dimensional
- Subagent contexts: narrower, execution-focused
- Skill library operates at high-level (compositional)
- Tool sequences operate at low-level (atomic)

---

## Implementation Tasks

### Phase 1: Two-Level State Architecture

#### Task 1.1: Define Hierarchical State Schema
**File:** `src/learning/state.ts`
```typescript
interface HierarchicalState {
  // High-level (slow, abstract)
  zH: {
    planEmbedding: number[];      // Strategy representation
    taskDecomposition: string[];   // Current plan steps
    completedSteps: number;
    confidenceScore: number;
  };

  // Low-level (fast, detailed)
  zL: {
    currentTrace: string;          // Execution trace
    toolCallHistory: ToolCall[];
    errorContext: string | null;
    iterationCount: number;
  };

  // Proposed solution
  y: {
    candidate: string;             // Current solution attempt
    validationStatus: "pending" | "passed" | "failed";
    feedbackHistory: string[];
  };
}
```

#### Task 1.2: Update TrainingLoop with State Management
**File:** `src/learning/loop.ts`
- Add `HierarchicalState` to `LoopState`
- Track state transitions during training
- Persist state for analysis and debugging

#### Task 1.3: Update Gym to Use Hierarchical State
**File:** `src/trainer/gym.ts`
- Inject zH (orchestrator context) into subagent prompts
- Capture zL after each tool call
- Track y (solution candidate) evolution

---

### Phase 2: Hierarchical Convergence

#### Task 2.1: Add Nested Cycle Structure
**File:** `src/learning/hierarchical-loop.ts`
```typescript
interface CycleConfig {
  T: number;                    // L-module steps per H-module update
  N: number;                    // Total H-module cycles
  convergenceThreshold: number; // When to consider L converged
}

async function hierarchicalLoop(task: TrainingTask, config: CycleConfig) {
  let zH = initHighLevelState(task);

  for (let n = 0; n < config.N; n++) {
    // Inner loop: L-module cycles
    let zL = initLowLevelState(zH);
    for (let t = 0; t < config.T; t++) {
      zL = await lowLevelStep(zL, zH, task);
      if (isConverged(zL, config.convergenceThreshold)) break;
    }

    // Outer loop: H-module update
    zH = updateHighLevelState(zH, zL);

    // Deep supervision checkpoint
    yield* evaluateAndLearn(zH, zL, task);
  }
}
```

#### Task 2.2: Implement Convergence Detection
**File:** `src/learning/convergence.ts`
- Track residual between consecutive zL states
- Detect when L-module reaches local equilibrium
- Trigger H-module update when L converges

#### Task 2.3: Add L-Reset on H-Update
**File:** `src/learning/reset.ts`
- After H updates, reset L-module to fresh context
- Preserve only task context and H-state
- Clear execution trace and error history

---

### Phase 3: Adaptive Computation Time (ACT)

#### Task 3.1: Add Q-Head for Halt/Continue Decision
**File:** `src/learning/act.ts`
```typescript
interface ACTConfig {
  Mmax: number;           // Maximum segments
  epsilon: number;        // Exploration rate
}

interface QValues {
  halt: number;           // Expected reward for halting
  continue: number;       // Expected reward for continuing
}

function computeQValues(zH: HighLevelState): QValues {
  // Use FM to predict Q-values from current state
}

function shouldHalt(qValues: QValues, segmentCount: number, config: ACTConfig): boolean {
  if (segmentCount >= config.Mmax) return true;
  return qValues.halt > qValues.continue;
}
```

#### Task 3.2: Update TrainerService with ACT
**File:** `src/trainer/service.ts`
- Add ACT configuration to TrainingConfig
- Track Q-values during episodes
- Update Q-head based on task outcomes

#### Task 3.3: Add Q-Learning Update for ACT
**File:** `src/learning/q-learning.ts`
- Implement Q-learning targets (Ghat_halt, Ghat_continue)
- Binary cross-entropy loss for Q-head
- Track halt decisions and outcomes

---

### Phase 4: Deep Supervision Integration

#### Task 4.1: Add Per-Segment Evaluation
**File:** `src/learning/deep-supervision.ts`
```typescript
interface SupervisionSegment {
  segmentIndex: number;
  zH: HighLevelState;
  zL: LowLevelState;
  y: SolutionCandidate;
  loss: number;
  outcome: "success" | "failure" | "partial";
}

async function deepSupervisionStep(
  segment: SupervisionSegment,
  groundTruth: TaskResult
): Effect.Effect<SupervisionResult, LearningError> {
  // 1. Compute loss for current segment
  const loss = computeLoss(segment.y, groundTruth);

  // 2. Update skills/memory based on outcome
  yield* updateFromSegment(segment);

  // 3. Detach state for next segment
  return { ...segment, loss, detached: true };
}
```

#### Task 4.2: Integrate with Archivist
**File:** `src/archivist/service.ts`
- Record each segment as trajectory action
- Link segments with parent trajectory
- Enable segment-level pattern extraction

#### Task 4.3: Add Segment-Level Skill Extraction
**File:** `src/skills/segment-extractor.ts`
- Extract skills from successful segment sequences
- Track which segments led to breakthroughs
- Promote high-value segments to skills

---

### Phase 5: Representational Hierarchy

#### Task 5.1: Add Dimensionality Metrics
**File:** `src/learning/dimensionality.ts`
```typescript
interface DimensionalityMetrics {
  zH_participationRatio: number;  // Should be ~3x zL
  zL_participationRatio: number;
  ratio: number;
}

function computeParticipationRatio(embeddings: number[][]): number {
  // PR = (sum(λi))² / sum(λi²) where λi are eigenvalues of covariance
}
```

#### Task 5.2: Ensure Orchestrator Has Richer Context
**File:** `src/learning/orchestrator.ts`
- Orchestrator system prompt includes more context
- Higher skill injection count (top-10 vs top-5)
- More memory retrieval (top-10 vs top-5)
- Richer planning prompt with strategic thinking

#### Task 5.3: Keep Subagent Context Focused
**File:** `src/trainer/gym.ts`
- Subagent prompts are narrow and execution-focused
- Fewer skills (top-3 most relevant)
- Minimal memory (only directly relevant)
- Clear action space

---

### Phase 6: Visualization and Analysis

#### Task 6.1: Add Intermediate Prediction Visualization
**File:** `src/dashboard/trajectory-view.ts`
- Visualize y (solution) evolution over segments
- Show zH updates as major steps
- Show zL iterations as sub-steps

#### Task 6.2: Add Convergence Monitoring
**File:** `src/dashboard/convergence.ts`
- Plot forward residuals (like HRM Figure 3)
- Show hierarchical convergence pattern
- Alert on premature convergence

#### Task 6.3: Add Dimensionality Tracking
**File:** `src/dashboard/dimensionality.ts`
- Track zH/zL participation ratio over training
- Verify hierarchy emerges through training
- Compare to HRM's 2.98 ratio target

---

## Task Dependency Order

```
1.1 → 1.2 → 1.3 (State Architecture)
       ↓
2.1 → 2.2 → 2.3 (Hierarchical Convergence)
       ↓
3.1 → 3.2 → 3.3 (Adaptive Computation Time)
       ↓
4.1 → 4.2 → 4.3 (Deep Supervision)
       ↓
5.1 → 5.2 → 5.3 (Representational Hierarchy)
       ↓
6.1 → 6.2 → 6.3 (Visualization)
```

---

## Task IDs for .openagents/tasks.jsonl

| ID | Title | Priority | Labels |
|----|-------|----------|--------|
| oa-hrm-01 | Define HierarchicalState schema | P1 | hrm, learning, schema |
| oa-hrm-02 | Update TrainingLoop with state management | P1 | hrm, learning |
| oa-hrm-03 | Update Gym to use hierarchical state | P1 | hrm, trainer |
| oa-hrm-04 | Add nested cycle structure (hierarchical loop) | P1 | hrm, learning |
| oa-hrm-05 | Implement convergence detection | P1 | hrm, learning |
| oa-hrm-06 | Add L-reset on H-update | P1 | hrm, learning |
| oa-hrm-07 | Add Q-head for ACT halt/continue | P1 | hrm, act |
| oa-hrm-08 | Update TrainerService with ACT | P1 | hrm, trainer |
| oa-hrm-09 | Add Q-learning update for ACT | P2 | hrm, act, q-learning |
| oa-hrm-10 | Add per-segment evaluation (deep supervision) | P1 | hrm, deep-supervision |
| oa-hrm-11 | Integrate segments with Archivist | P2 | hrm, archivist |
| oa-hrm-12 | Add segment-level skill extraction | P2 | hrm, skills |
| oa-hrm-13 | Add dimensionality metrics | P2 | hrm, metrics |
| oa-hrm-14 | Ensure orchestrator has richer context | P1 | hrm, orchestrator |
| oa-hrm-15 | Keep subagent context focused | P2 | hrm, subagent |
| oa-hrm-16 | Add intermediate prediction visualization | P3 | hrm, dashboard |
| oa-hrm-17 | Add convergence monitoring | P3 | hrm, dashboard |
| oa-hrm-18 | Add dimensionality tracking | P3 | hrm, dashboard |

---

## Success Criteria

1. **Hierarchical State**: Training loop maintains explicit {y, zL, zH} state
2. **Nested Cycles**: Inner L-cycles and outer H-cycles visible in logs
3. **ACT Working**: Tasks use variable computation depth based on difficulty
4. **Deep Supervision**: Each segment produces learning signal
5. **Dimensionality Ratio**: zH/zL PR approaches HRM's ~3x ratio
6. **TB Improvement**: Terminal-Bench success rate improves

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/learning/loop.ts` | Add hierarchical state, nested cycles |
| `src/learning/orchestrator.ts` | Add HierarchicalState, richer context |
| `src/trainer/gym.ts` | Use hierarchical state, focused subagent context |
| `src/trainer/service.ts` | Add ACT configuration |
| `src/archivist/service.ts` | Add segment-level trajectory recording |
| `src/skills/service.ts` | Add segment-level skill extraction |
| `src/dashboard/reporter.ts` | Add convergence and dimensionality views |

## New Files

| File | Purpose |
|------|---------|
| `src/learning/state.ts` | HierarchicalState schema |
| `src/learning/hierarchical-loop.ts` | Nested cycle implementation |
| `src/learning/convergence.ts` | Convergence detection |
| `src/learning/act.ts` | Adaptive Computation Time |
| `src/learning/q-learning.ts` | Q-learning for ACT |
| `src/learning/deep-supervision.ts` | Per-segment evaluation |
| `src/learning/dimensionality.ts` | PR metrics |

---

## Key Insights from HRM Paper

1. **Recursion > Model Size**: HRM's 27M params beat much larger LLMs by using deep recursive computation
2. **Hierarchical Convergence**: Prevents premature RNN convergence through L-reset after H-update
3. **1-Step Gradient**: O(1) memory, no BPTT - aligns with MechaCoder's skill replay approach
4. **ACT Saves Compute**: Easy tasks halt early, hard tasks think longer
5. **Dimensionality Hierarchy Emerges**: Training produces zH >> zL dimensionality naturally
6. **Turing-Complete in Practice**: With sufficient depth, HRM can solve polynomial-time problems

This integration will enhance MechaCoder's ability to solve complex Terminal-Bench tasks by:
- Providing structured reasoning with explicit state management
- Enabling variable thinking depth per task
- Learning from every attempt, not just final outcomes
- Maintaining appropriate representational capacity at each level
