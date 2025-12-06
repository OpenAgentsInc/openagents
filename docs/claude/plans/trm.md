# Plan: Integrate TRM Insights into MechaCoder Learning System

## Executive Summary

The Tiny Recursive Model (TRM) paper demonstrates that a 7M parameter model with 2 layers can outperform HRM (27M params) and most LLMs on reasoning tasks. TRM achieves 45% on ARC-AGI-1 (vs HRM's 40%), 87% on Sudoku-Extreme (vs HRM's 55%). The key insight: **simpler is better** - one tiny network recursively refining its answer beats complex hierarchical architectures.

---

## Key TRM Insights to Integrate

### 1. Simplified State: {x, y, z} Instead of {zH, zL}
**TRM Concept:** Reinterpret HRM's "hierarchical" states as simply:
- **x** = input question (task description)
- **y** = current proposed solution (what HRM called zH)
- **z** = latent reasoning trace (what HRM called zL)

**Why 2 features (y, z)?**
- If we don't track y separately, z must store both reasoning AND solution
- If we don't track z separately, we forget HOW we got to y (like losing CoT)
- Tested: single feature (71.9%), multi-scale (77.6%), y+z (87.4%) - two is optimal

**MechaCoder Application:**
- y = current code/diff/solution candidate
- z = reasoning context (tool history, error patterns, plan state)
- x = task description + codebase context

### 2. Single Network Instead of Two
**TRM Concept:** HRM uses separate fL (low-level) and fH (high-level) networks. TRM uses ONE network for both:
- `z = net(x, y, z)` - update reasoning (includes x)
- `y = net(y, z)` - update solution (no x)

The presence/absence of x in input determines the operation.

**MechaCoder Application:**
- Single FM prompt template that adapts based on phase
- "Reasoning phase" prompt includes full task context
- "Solution phase" prompt focuses on refining current candidate

### 3. Full Backprop Through Recursion (No Fixed-Point Assumption)
**TRM Concept:** HRM assumes fixed-point convergence to justify 1-step gradient approximation. TRM just backprops through all n+1 recursions. Result: 56.5% → 87.4% accuracy.

**MechaCoder Application:**
- Don't assume convergence - track full execution trace
- Each tool call contributes to the reasoning chain
- No shortcuts in state tracking

### 4. Deep Supervision Remains Critical
**TRM Concept:** Both HRM and TRM use deep supervision - multiple improvement steps with state carried across. This is the PRIMARY driver of performance (doubles accuracy per ARC Prize Foundation analysis).

**MechaCoder Application:**
- Every Gym episode = one supervision step
- Carry (y, z) state across episodes
- Learn from EVERY attempt, not just final outcome

### 5. Less is More: Fewer Layers = Better Generalization
**TRM Concept:** Reducing from 4 layers to 2 layers IMPROVED accuracy (79.5% → 87.4%). More parameters = more overfitting on small data.

**MechaCoder Application:**
- Don't over-engineer the state representation
- Simpler prompts may generalize better than complex ones
- Focus recursion depth over prompt complexity

### 6. Simplified ACT: Single Forward Pass
**TRM Concept:** HRM's ACT requires 2 forward passes (for Q-learning). TRM simplifies to:
- Single binary prediction: "Have we reached correct solution?"
- Remove the "continue" loss entirely
- Result: Same accuracy, half the compute

**MechaCoder Application:**
- Simple halt decision: did tests pass? have we made progress?
- No need for complex Q-value estimation
- Rule-based or confidence-based halting

### 7. EMA for Stability
**TRM Concept:** Exponential Moving Average of weights prevents overfitting and divergence on small data.

**MechaCoder Application:**
- Track moving averages of success rates per task type
- Smooth out noisy feedback from individual episodes
- Stabilize skill confidence scores

### 8. Optimal Recursion Depth
**TRM Concept:** T=3, n=6 (42 effective recursions) optimal for TRM. More recursions help hard problems but have diminishing returns.

**MechaCoder Application:**
- Configure max retry cycles per task difficulty
- Track which recursion depth solved which tasks
- Adaptive depth based on task type

---

## Implementation Tasks

### Phase 1: Simplified State Model

#### Task 1.1: Define TRM-Style State Schema
**File:** `src/learning/trm-state.ts`
```typescript
interface TRMState {
  // Input question (task context)
  x: {
    taskDescription: string;
    codebaseContext: string;
    relevantFiles: string[];
  };

  // Current proposed solution
  y: {
    candidate: string;           // Current code/diff
    confidence: number;          // How confident are we?
    validationResult: "pending" | "passed" | "failed";
  };

  // Latent reasoning trace
  z: {
    reasoningHistory: string[];  // Chain of reasoning steps
    toolCallTrace: ToolCall[];   // What we've tried
    errorPatterns: string[];     // Errors we've seen
    hypotheses: string[];        // Current theories about solution
  };
}
```

#### Task 1.2: Migrate from HierarchicalState to TRMState
**File:** `src/learning/state.ts`
- Refactor HierarchicalState (zH, zL) to TRMState (x, y, z)
- Update all consumers to use new naming
- Simpler, more intuitive state management

#### Task 1.3: Update TrainingLoop to Use TRM State
**File:** `src/learning/loop.ts`
- Replace HierarchicalState with TRMState
- Track (y, z) across supervision steps
- Detach state between episodes

---

### Phase 2: Single Network Pattern

#### Task 2.1: Create Unified Reasoning Function
**File:** `src/learning/trm-reasoning.ts`
```typescript
interface RecursionConfig {
  n: number;  // Latent reasoning steps per cycle
  T: number;  // Total cycles
}

// Single function handles both z-update and y-update
async function trmRecursion(
  x: TaskContext,
  y: SolutionCandidate,
  z: ReasoningTrace,
  config: RecursionConfig
): Effect.Effect<{ y: SolutionCandidate; z: ReasoningTrace }, LearningError> {
  // T-1 cycles without "gradients" (just execution)
  for (let t = 0; t < config.T - 1; t++) {
    // n steps of latent reasoning (z updates)
    for (let i = 0; i < config.n; i++) {
      z = yield* updateReasoning(x, y, z);  // z = net(x, y, z)
    }
    // 1 step of solution refinement (y update)
    y = yield* refineSolution(y, z);  // y = net(y, z)
  }

  // Final cycle (would have gradients in neural version)
  for (let i = 0; i < config.n; i++) {
    z = yield* updateReasoning(x, y, z);
  }
  y = yield* refineSolution(y, z);

  return { y, z };
}
```

#### Task 2.2: Implement Update Functions
**File:** `src/learning/trm-updates.ts`
- `updateReasoning(x, y, z)` - FM call with full context
- `refineSolution(y, z)` - FM call focused on solution improvement
- Single underlying FM, different prompts

#### Task 2.3: Create TRM Prompt Templates
**File:** `src/learning/trm-prompts.ts`
```typescript
// Reasoning prompt (includes x)
const reasoningPrompt = `
Task: ${x.taskDescription}
Current solution: ${y.candidate}
Reasoning so far: ${z.reasoningHistory}
Errors seen: ${z.errorPatterns}

Update your reasoning about how to solve this task.
`;

// Solution prompt (no x, focuses on refinement)
const solutionPrompt = `
Current solution: ${y.candidate}
Latest reasoning: ${z.reasoningHistory.slice(-3)}

Refine the solution based on your reasoning.
`;
```

---

### Phase 3: Deep Supervision Integration

#### Task 3.1: Implement TRM Training Loop
**File:** `src/learning/trm-loop.ts`
```typescript
async function trmTrainingLoop(
  task: TrainingTask,
  config: TRMConfig
): Effect.Effect<TrainingResult, LearningError> {
  let y = initSolution(task);
  let z = initReasoning(task);

  for (let step = 0; step < config.Nsup; step++) {
    const x = embedTask(task);

    // Run TRM recursion
    const result = yield* trmRecursion(x, y, z, config);
    y = result.y;
    z = result.z;

    // Evaluate current solution
    const evaluation = yield* evaluate(y, task.groundTruth);

    // Learn from this step (deep supervision)
    yield* learnFromStep({ x, y, z, evaluation, step });

    // Detach state for next step
    y = detach(y);
    z = detach(z);

    // Simple halt check
    if (evaluation.passed) break;
    if (yield* shouldHalt(y, step)) break;
  }

  return { finalY: y, finalZ: z, steps: step };
}
```

#### Task 3.2: Implement Per-Step Learning
**File:** `src/learning/trm-supervision.ts`
- Extract skills from successful steps
- Record failure patterns from unsuccessful steps
- Update memory with step-level insights

#### Task 3.3: State Detachment Between Steps
**File:** `src/learning/trm-detach.ts`
- Clear execution-specific context
- Preserve reasoning insights
- Reset for fresh attempt with accumulated knowledge

---

### Phase 4: Simplified ACT

#### Task 4.1: Implement Simple Halt Decision
**File:** `src/learning/trm-halt.ts`
```typescript
interface HaltDecision {
  shouldHalt: boolean;
  confidence: number;
  reason: string;
}

async function shouldHalt(
  y: SolutionCandidate,
  step: number,
  config: ACTConfig
): Effect.Effect<boolean, LearningError> {
  // Max steps reached
  if (step >= config.Nsup) return true;

  // Solution passed validation
  if (y.validationResult === "passed") return true;

  // High confidence in current solution
  if (y.confidence > config.haltThreshold) return true;

  // No progress in last N steps (stuck detection)
  if (isStuck(y, config.stuckThreshold)) return true;

  return false;
}
```

#### Task 4.2: Remove Complex Q-Learning ACT
**File:** Various
- Remove Q-head, Q-values, continue loss from HRM tasks
- Simplify to binary halt decision
- Track halt decisions for analysis

#### Task 4.3: Add Progress Detection
**File:** `src/learning/trm-progress.ts`
- Detect if solution is improving between steps
- Detect if we're stuck (same errors repeating)
- Use progress to inform halt decision

---

### Phase 5: EMA and Stability

#### Task 5.1: Add EMA for Success Rates
**File:** `src/learning/trm-ema.ts`
```typescript
interface EMAConfig {
  decay: number;  // 0.999 typical
}

interface TaskTypeStats {
  taskType: string;
  emaSuccessRate: number;
  emaOptimalDepth: number;
  sampleCount: number;
}

function updateEMA(
  stats: TaskTypeStats,
  newSuccess: boolean,
  depth: number,
  config: EMAConfig
): TaskTypeStats {
  const decay = config.decay;
  return {
    ...stats,
    emaSuccessRate: decay * stats.emaSuccessRate + (1 - decay) * (newSuccess ? 1 : 0),
    emaOptimalDepth: decay * stats.emaOptimalDepth + (1 - decay) * depth,
    sampleCount: stats.sampleCount + 1,
  };
}
```

#### Task 5.2: Stabilize Skill Confidence
**File:** `src/skills/confidence.ts`
- Apply EMA to skill success rates
- Prevent wild swings from single failures
- Smooth out noisy feedback

#### Task 5.3: Add Divergence Detection
**File:** `src/learning/trm-stability.ts`
- Detect if training is diverging (success rate dropping)
- Alert on instability
- Auto-adjust parameters if needed

---

### Phase 6: Recursion Depth Optimization

#### Task 6.1: Track Optimal Depth Per Task Type
**File:** `src/learning/trm-depth.ts`
```typescript
interface DepthStats {
  taskType: string;
  depthHistogram: Map<number, { successes: number; attempts: number }>;
  optimalDepth: number;
}

function recordDepthOutcome(
  stats: DepthStats,
  depth: number,
  succeeded: boolean
): DepthStats {
  const current = stats.depthHistogram.get(depth) ?? { successes: 0, attempts: 0 };
  stats.depthHistogram.set(depth, {
    successes: current.successes + (succeeded ? 1 : 0),
    attempts: current.attempts + 1,
  });

  // Recompute optimal depth
  stats.optimalDepth = computeOptimalDepth(stats.depthHistogram);
  return stats;
}
```

#### Task 6.2: Adaptive Depth Selection
**File:** `src/learning/trm-adaptive.ts`
- Start with default depth
- Adjust based on task type statistics
- More depth for hard tasks, less for easy

#### Task 6.3: Depth vs Success Analysis
**File:** `src/dashboard/depth-analysis.ts`
- Visualize depth vs success rate curves
- Identify optimal depth per task category
- Track how optimal depth changes over training

---

### Phase 7: Comparison with HRM Integration

#### Task 7.1: Create Unified State Interface
**File:** `src/learning/state-unified.ts`
- Abstract over TRM (x,y,z) and HRM (zH,zL) representations
- Allow switching between approaches
- Compare performance

#### Task 7.2: A/B Testing Framework
**File:** `src/learning/ab-test.ts`
- Run same tasks with TRM vs HRM approaches
- Track which performs better on which task types
- Auto-select best approach per task

#### Task 7.3: Document Findings
**File:** `docs/research/trm-vs-hrm-findings.md`
- Record empirical results
- Note which approach works better where
- Recommendations for future work

---

## Key Differences: TRM vs HRM

| Aspect | HRM | TRM | MechaCoder Impact |
|--------|-----|-----|-------------------|
| State | zH + zL (hierarchical) | y + z (simple) | Simpler state schema |
| Networks | 2 (fH, fL) | 1 | Single FM prompt pattern |
| Gradient | 1-step approximation | Full backprop | Track full execution |
| ACT | Q-learning (2 passes) | Binary halt (1 pass) | Simple halt rules |
| Layers | 4 | 2 | Simpler prompts |
| Params | 27M | 7M | Less complexity |
| Accuracy | 40% ARC-AGI | 45% ARC-AGI | TRM approach preferred |

---

## Task IDs for .openagents/tasks.jsonl

| ID | Title | Priority | Labels |
|----|-------|----------|--------|
| oa-trm-01 | Define TRM-style state schema (x, y, z) | P1 | trm, learning, schema |
| oa-trm-02 | Migrate from HierarchicalState to TRMState | P1 | trm, learning, refactor |
| oa-trm-03 | Update TrainingLoop to use TRM state | P1 | trm, learning |
| oa-trm-04 | Create unified reasoning function | P1 | trm, learning |
| oa-trm-05 | Implement z-update and y-update functions | P1 | trm, learning |
| oa-trm-06 | Create TRM prompt templates | P1 | trm, prompts |
| oa-trm-07 | Implement TRM training loop with deep supervision | P1 | trm, deep-supervision |
| oa-trm-08 | Implement per-step learning | P1 | trm, learning |
| oa-trm-09 | State detachment between steps | P2 | trm, learning |
| oa-trm-10 | Implement simple halt decision | P1 | trm, act |
| oa-trm-11 | Remove complex Q-learning ACT | P2 | trm, act, cleanup |
| oa-trm-12 | Add progress/stuck detection | P2 | trm, monitoring |
| oa-trm-13 | Add EMA for success rates | P2 | trm, stability |
| oa-trm-14 | Stabilize skill confidence with EMA | P2 | trm, skills |
| oa-trm-15 | Add divergence detection | P3 | trm, monitoring |
| oa-trm-16 | Track optimal depth per task type | P2 | trm, metrics |
| oa-trm-17 | Adaptive depth selection | P2 | trm, adaptive |
| oa-trm-18 | Depth vs success analysis dashboard | P3 | trm, dashboard |
| oa-trm-19 | Create unified state interface (TRM/HRM) | P2 | trm, hrm, interface |
| oa-trm-20 | A/B testing framework for TRM vs HRM | P3 | trm, hrm, testing |
| oa-trm-21 | Document TRM vs HRM findings | P3 | trm, hrm, docs |

---

## Success Criteria

1. **TRM State Working**: Training loop uses {x, y, z} state representation
2. **Single Network Pattern**: One FM prompt template adapts for reasoning vs solution
3. **Deep Supervision**: Every step produces learning signal
4. **Simple Halt**: Binary halt decision without Q-learning complexity
5. **EMA Stability**: Success rates smoothed, no wild swings
6. **Depth Tracking**: Know optimal recursion depth per task type
7. **TB Improvement**: Terminal-Bench success rate improves

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/learning/trm-state.ts` | TRM state schema (x, y, z) |
| `src/learning/trm-reasoning.ts` | Unified recursion function |
| `src/learning/trm-updates.ts` | z-update and y-update implementations |
| `src/learning/trm-prompts.ts` | TRM prompt templates |
| `src/learning/trm-loop.ts` | TRM training loop |
| `src/learning/trm-supervision.ts` | Per-step learning |
| `src/learning/trm-detach.ts` | State detachment |
| `src/learning/trm-halt.ts` | Simple halt decision |
| `src/learning/trm-progress.ts` | Progress/stuck detection |
| `src/learning/trm-ema.ts` | EMA for stability |
| `src/learning/trm-depth.ts` | Depth tracking and optimization |
| `src/learning/trm-adaptive.ts` | Adaptive depth selection |

---

## Key Insights from TRM Paper

1. **Less is More**: 7M params with 2 layers beats 27M params with 4 layers
2. **Simplicity Wins**: No fixed-point theorem, no hierarchy, no biological arguments
3. **y + z is Optimal**: Tested 1, 2, and n+1 features - 2 is best
4. **Single Network**: One network adapts based on input presence
5. **Deep Supervision is Key**: Primary driver of performance (doubles accuracy)
6. **Full Recursion**: Backprop through all steps beats 1-step approximation
7. **Simple ACT**: Binary halt beats Q-learning ACT
8. **EMA Stabilizes**: Prevents overfitting on small data

This integration will simplify MechaCoder's learning architecture while potentially improving performance by:
- Reducing complexity (simpler state, single network pattern)
- Maintaining deep supervision benefits
- Simplifying halt decisions
- Adding stability through EMA
- Tracking optimal recursion depth
