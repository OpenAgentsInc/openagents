# SOAR: Self-Improving Language Models for Program Synthesis

This document analyzes the SOAR paper ("Self-Improving Language Models for Evolutionary Program Synthesis") and its implications for MechaCoder's learning system.

## Executive Summary

SOAR achieves **52% on ARC-AGI** (state-of-the-art for open-source models) by:
1. Integrating LLMs into a self-improving evolutionary loop
2. Using hindsight learning to learn from ALL attempts (not just successes)
3. Joint training of sampling and refinement capabilities
4. Iterated self-improvement that breaks scaling plateaus

**Key Result**: A 7B model improved from 14.25% → 36.25% (2.5x) through self-improvement alone.

---

## Part 1: Core SOAR Concepts

### 1.1 The Self-Improving Loop

SOAR alternates between two phases:

```
┌─────────────────────────────────────────────────────────────┐
│                    SOAR Self-Improving Loop                 │
│                                                             │
│  ┌──────────────────┐         ┌──────────────────┐         │
│  │  Sample&Refine   │ ──────► │     Learning     │         │
│  │     (Search)     │         │   (Fine-tune)    │         │
│  └──────────────────┘         └──────────────────┘         │
│           ▲                           │                     │
│           │                           │                     │
│           └───────────────────────────┘                     │
│                   iterate N times                           │
└─────────────────────────────────────────────────────────────┘
```

**Sample&Refine Phase:**
- Sample 3k candidate programs from LLM
- Refine 3k times using execution feedback
- Use REX (Thompson sampling + exploration bonus) for refinement selection
- Weighted majority voting for final answer

**Learning Phase:**
- Collect search traces (successes AND failures)
- Apply hindsight relabeling to create synthetic training data
- Fine-tune LLM for both sampling and refinement
- Use greedy-diverse data selection

This creates a virtuous cycle:
- Better models → more effective search
- More effective search → richer training data
- Richer training data → better models

---

### 1.2 Hindsight Relabeling (Key Innovation)

**The Problem**: Most tasks fail—limiting training data to successes severely constrains learning.

**The Insight**: Any program f₀, while possibly incorrect for its intended task, is by definition correct for the task of mapping inputs to the outputs it produces.

```
Original attempt:
  Task: "Transform grid A → B"
  Generated program: f₀
  Result: f₀(A) = C ≠ B  (FAILED)

Hindsight relabeling:
  Synthetic task: "Transform grid A → C"
  Program: f₀
  Result: f₀(A) = C = C  (CORRECT by construction)
```

**Impact**:
- 400 tasks × 6k programs = 2.4M potential training examples
- Even "failed" attempts contribute to learning
- Dramatically expands training data

**For MechaCoder**: Every Gym attempt, even failures, can generate synthetic problem-solution pairs for the skill library.

---

### 1.3 Joint Sampling + Refinement Learning

SOAR discovers that training a single model for BOTH sampling and refinement outperforms specialized models.

| Configuration | Sample Accuracy | Search Accuracy |
|--------------|-----------------|-----------------|
| base + base | 29.67% | 34.83% |
| fine-samp + fine-ref | 36.46% | 43.88% |
| fine-both + fine-both | **39.79%** | **44.42%** |

**Key Findings**:
1. **Negative transfer**: Sampling-only fine-tuning hurts refinement
2. **Positive transfer**: Refinement-only fine-tuning improves sampling
3. **Synergy**: Joint training beats both specialized approaches

**Why It Works**: Both tasks share underlying knowledge about program structure and transformation patterns. Refinement teaches what makes programs better; sampling benefits from this knowledge.

**For MechaCoder**: Don't separate "initial attempt skills" from "refinement skills"—use a unified skill library.

---

### 1.4 Greedy-Diverse Data Selection

SOAR's data selection strategy outperforms pure greedy or uniform sampling:

| Strategy | Accuracy |
|----------|----------|
| correct-only (no hindsight) | 34.67% |
| uniform | 32.38% |
| greedy | 34.30% |
| **greedy-diverse** | **36.46%** |

**Greedy-Diverse Method**:
- 25 solutions: Highest accuracy (greedy)
- 25 solutions: Lowest accuracy (diversity)
- Apply hindsight relabeling to all

**Why It Works**: Pure greedy leads to mode collapse. Diversity maintains exploration capacity on unsolved problems.

**For Refinement Data**:
- Balance across accuracy bins: 0%, 1-34%, 34-98%, 100%
- Learn from easy, medium, and hard refinements

---

### 1.5 Iterated Self-Improvement

SOAR runs multiple iterations, each building on the previous:

```
Iteration 0: Base model search
Iteration 1: Fine-tuned on Iter 0 data → search
Iteration 2: Fine-tuned on Iter 1 data → search
Iteration 3: Fine-tuned on Iter 2 data → search
...
```

**Results on ARC-train**:

| Model Size | Iter 0 | Iter 3 | Improvement |
|------------|--------|--------|-------------|
| 7B | ~30% | ~57% | +27% |
| 14B | ~35% | ~59% | +24% |
| 32B | ~40% | ~60% | +20% |
| 72B | ~42% | ~61% | +19% |

**Key Observations**:
1. Smaller models show steeper improvements
2. All sizes continue benefiting from iterations
3. Gains slow but don't stop

---

### 1.6 Breaking Scaling Plateaus

**Model-Size Plateau**: Simply scaling model size yields diminishing returns with fixed search.

**Search-Budget Plateau**: Simply sampling more with a fixed model plateaus around 5.2k attempts.

**SOAR's Solution**: Improve the model itself, not just scale resources.

```
                        ▲ ARC-test %
                        │
                   50% ─┼────────────────────── SOAR (iter 3)
                        │           ┌──────────
                   40% ─┼───────────┘          SOAR (iter 2)
                        │       ┌──────────────
                   30% ─┼───────┘              SOAR (iter 1)
                        │   ┌──────────────────
                   20% ─┼───┘                  SOAR (no learning)
                        │   ┌──────────────────── plateau!
                   10% ─┼───┘
                        └─────────────────────────────────► Model Size
                           7B    14B    32B    72B
```

Each SOAR iteration lifts the entire scaling curve, enabling smaller models to match or exceed larger ones without learning.

---

### 1.7 Test-Time Training (No Ground Truth Needed)

SOAR can continue improving on target tasks WITHOUT access to ground truth:

1. Run search on test tasks
2. Select data by **training accuracy** (not test accuracy)
3. Use majority voting to estimate "correct" solutions
4. Apply hindsight relabeling
5. Fine-tune and repeat

**Results**: Additional 3-5% improvement across model sizes through test-time training alone.

**For MechaCoder**: Can improve on Terminal-Bench tasks using only test pass/fail as a proxy signal.

---

### 1.8 Cross-Model Diversity

SOAR discovers that different model sizes solve different problems:

- Smaller models learn faster, sometimes solve tasks larger ones miss
- Pooling solutions across model sizes significantly improves performance
- 52% accuracy from pooling 5 model sizes vs ~45% for best single model

**Oracle Gap**: 9.5% gap between majority voting (52%) and oracle (57.25%) suggests room for better ensembling.

---

## Part 2: SOAR vs TRM vs HRM

### 2.1 What Each Paper Contributes

| Aspect | HRM | TRM | SOAR |
|--------|-----|-----|------|
| Focus | Single attempt structure | Simplified architecture | Learning across attempts |
| State | zH + zL (hierarchy) | x, y, z (simple) | Search traces |
| Learning | Deep supervision | Deep supervision | Hindsight relabeling |
| Iteration | Nested cycles | Recursion depth | Self-improvement loop |
| Key Insight | Two timescales | Less is more | Learn from failures |

### 2.2 Synthesis for MechaCoder

These papers are **complementary**, not competing:

```
┌─────────────────────────────────────────────────────────────┐
│                    MechaCoder Architecture                   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              SOAR: Outer Self-Improvement           │   │
│  │                                                     │   │
│  │   ┌─────────────────────────────────────────────┐  │   │
│  │   │         TRM: Single Attempt Structure        │  │   │
│  │   │                                             │  │   │
│  │   │   State: (x, y, z)                          │  │   │
│  │   │   Recursion: n=6 per cycle, T=3 cycles      │  │   │
│  │   │   ACT: Simple binary halt                   │  │   │
│  │   │   Deep supervision: Learn every step        │  │   │
│  │   │                                             │  │   │
│  │   └─────────────────────────────────────────────┘  │   │
│  │                                                     │   │
│  │   Hindsight: Learn from failures                   │   │
│  │   Joint: Unified sampling + refinement              │   │
│  │   Iteration: Multi-round self-improvement           │   │
│  │                                                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  HRM Additions (where valuable):                            │
│  - Nested cycles for orchestrator/subagent                  │
│  - Dimensionality monitoring                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Part 3: Implementation for MechaCoder

### 3.1 Architecture Mapping

| SOAR Component | MechaCoder Equivalent |
|----------------|----------------------|
| LLM | Claude Code / FM subagent |
| Sample phase | Initial Gym attempts |
| Refine phase | Retry with execution feedback |
| Search traces | Archivist trajectory records |
| Fine-tuning | Skill extraction + memory updates |
| Model improvement | Pattern library growth |
| Hindsight relabeling | Synthetic task generation |
| Weighted majority voting | Solution ranking/selection |

### 3.2 Key Adaptations for No-Gradient Architecture

SOAR uses gradient-based fine-tuning. MechaCoder uses no-gradient learning. Adaptations:

| SOAR (Gradient) | MechaCoder (No-Gradient) |
|-----------------|--------------------------|
| LoRA fine-tuning | Skill library updates |
| Loss minimization | Success rate tracking |
| Weight updates | Pattern storage + retrieval |
| Model checkpoints | Library versioning |

**Hindsight Relabeling Without Gradients**:
```typescript
// SOAR: Fine-tune on synthetic (task, solution) pairs
// MechaCoder: Store synthetic patterns in skill library

interface SyntheticSkill {
  // Original task that failed
  originalTask: Task;
  originalOutput: string;  // What we produced

  // Hindsight-relabeled task
  syntheticTask: Task;  // Task where our output IS correct

  // The pattern
  pattern: SkillPattern;

  // Metadata
  source: "hindsight";
  confidence: number;  // Based on structural validity
}
```

### 3.3 Detailed Implementation Components

#### Component 1: Hindsight Relabeler

```typescript
// src/learning/soar-hindsight.ts

interface HindsightRelabeler {
  /**
   * Given a failed attempt, create a synthetic task where the
   * attempt's output is "correct"
   */
  relabel(
    originalTask: Task,
    attempt: AttemptRecord,
  ): SyntheticTaskSolution | null;

  /**
   * Validate that a synthetic task is structurally valid
   * (not degenerate, has meaningful transformation)
   */
  validateSynthetic(synthetic: SyntheticTaskSolution): boolean;

  /**
   * Convert synthetic task-solution to skill pattern
   */
  extractSkill(synthetic: SyntheticTaskSolution): SkillPattern;
}
```

**Key Insight**: For Terminal-Bench tasks, "hindsight relabeling" means:
- Original: "Write function that returns X" → Agent writes function returning Y
- Synthetic: "Write function that returns Y" → Same code is correct

This expands the skill library with patterns that "worked" for something.

#### Component 2: Greedy-Diverse Data Selector

```typescript
// src/learning/soar-selection.ts

interface DataSelector {
  /**
   * Select training data using greedy-diverse strategy
   *
   * @param attempts All attempts from search
   * @param maxPerTask Maximum examples per task
   * @returns Selected attempts for training
   */
  selectGreedyDiverse(
    attempts: AttemptRecord[],
    maxPerTask: number,
  ): AttemptRecord[];
}

// Implementation:
function selectGreedyDiverse(
  attempts: AttemptRecord[],
  maxPerTask: number = 50,
): AttemptRecord[] {
  const byTask = groupBy(attempts, a => a.taskId);
  const selected: AttemptRecord[] = [];

  for (const [taskId, taskAttempts] of byTask) {
    // Sort by accuracy
    const sorted = sortBy(taskAttempts, a => -a.accuracy);

    // Take top N/2 (greedy)
    const greedy = sorted.slice(0, maxPerTask / 2);

    // Take bottom N/2 (diversity)
    const diverse = sorted.slice(-(maxPerTask / 2));

    selected.push(...greedy, ...diverse);
  }

  return selected;
}
```

#### Component 3: Joint Sample+Refine Skill Library

```typescript
// src/learning/soar-unified-skills.ts

interface UnifiedSkillLibrary {
  /**
   * Skills are tagged with their source but stored together
   */
  skills: Map<string, UnifiedSkill>;

  /**
   * Retrieve skills for sampling (initial attempt)
   */
  getForSampling(context: TaskContext): UnifiedSkill[];

  /**
   * Retrieve skills for refinement (with feedback)
   */
  getForRefinement(context: RefinementContext): UnifiedSkill[];

  /**
   * Learn from both sampling and refinement outcomes
   */
  learnJointly(
    samplingOutcomes: Outcome[],
    refinementOutcomes: Outcome[],
  ): void;
}

interface UnifiedSkill {
  id: string;
  pattern: SkillPattern;

  // Track effectiveness for both uses
  samplingSuccessRate: EMA;
  refinementSuccessRate: EMA;

  // Joint confidence
  get confidence(): number {
    return (this.samplingSuccessRate.value + this.refinementSuccessRate.value) / 2;
  }
}
```

#### Component 4: Iterated Self-Improvement Loop

```typescript
// src/learning/soar-loop.ts

interface SOARLoop {
  /**
   * Run one complete SOAR iteration
   */
  runIteration(
    tasks: Task[],
    currentModel: SkillLibrary,
  ): IterationResult;

  /**
   * Run multiple iterations with model improvement
   */
  runMultipleIterations(
    tasks: Task[],
    numIterations: number,
  ): SOARResult;
}

async function runIteration(
  tasks: Task[],
  skillLibrary: SkillLibrary,
): Promise<IterationResult> {
  const searchTraces: SearchTrace[] = [];

  // Phase 1: Sample & Refine
  for (const task of tasks) {
    // Sample N candidates
    const samples = await sampleCandidates(task, skillLibrary, 3000);

    // Refine using REX
    const refined = await refineCandidates(task, samples, skillLibrary, 3000);

    // Record traces
    searchTraces.push({
      task,
      samples,
      refined,
      bestSolution: selectBest(refined),
    });
  }

  // Phase 2: Learning
  const selectedData = selectGreedyDiverse(searchTraces);
  const syntheticData = applyHindsightRelabeling(selectedData);

  // Update skill library
  updateSkillLibrary(skillLibrary, selectedData, syntheticData);

  return { searchTraces, skillLibrary };
}
```

#### Component 5: Test-Time Training Adapter

```typescript
// src/learning/soar-ttt.ts

interface TestTimeTrainer {
  /**
   * Improve on target tasks WITHOUT ground truth
   *
   * Uses training accuracy as proxy for solution quality
   */
  adaptToTargetTasks(
    targetTasks: Task[],
    skillLibrary: SkillLibrary,
    iterations: number,
  ): SkillLibrary;
}

async function adaptToTargetTasks(
  targetTasks: Task[],
  skillLibrary: SkillLibrary,
  iterations: number,
): Promise<SkillLibrary> {
  let currentLibrary = skillLibrary;

  for (let i = 0; i < iterations; i++) {
    const traces = await runSearchPhase(targetTasks, currentLibrary);

    // Select by TRAINING accuracy (not test accuracy)
    // This is the key for test-time training
    const selected = selectByTrainingAccuracy(traces);

    // Use majority voting to estimate "correct" solutions
    const pseudoLabels = estimateWithMajorityVoting(traces);

    // Apply hindsight relabeling
    const synthetic = applyHindsightRelabeling(selected);

    // Update library
    currentLibrary = updateLibrary(currentLibrary, selected, synthetic);
  }

  return currentLibrary;
}
```

#### Component 6: Weighted Majority Voting

```typescript
// src/learning/soar-voting.ts

interface MajorityVoter {
  /**
   * Select best solution using weighted majority voting
   *
   * Weight = count + c × training_accuracy
   * where c = 1000 (penalize low-accuracy patterns)
   */
  selectBest(
    candidates: Candidate[],
    numOutputs: number,
  ): Solution[];
}

function selectBest(
  candidates: Candidate[],
  numOutputs: number = 2,
  c: number = 1000,
): Solution[] {
  // Group by output
  const byOutput = groupBy(candidates, c => hashOutput(c.output));

  // Compute weighted votes
  const votes: Map<string, number> = new Map();
  for (const [outputHash, group] of byOutput) {
    const count = group.length;
    const avgAccuracy = mean(group.map(c => c.trainingAccuracy));
    const weight = count + c * avgAccuracy;
    votes.set(outputHash, weight);
  }

  // Return top N
  const sorted = sortBy([...votes.entries()], ([_, w]) => -w);
  return sorted.slice(0, numOutputs).map(([hash, _]) =>
    byOutput.get(hash)![0].solution
  );
}
```

---

## Part 4: Task Recommendations

### 4.1 New SOAR-Specific Tasks

Based on the analysis, these tasks should be added:

**Phase 1: Hindsight Learning Infrastructure (P1)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-001 | Implement hindsight relabeler | Create src/learning/soar-hindsight.ts to convert failed attempts into synthetic task-solution pairs |
| soar-002 | Add synthetic skill storage | Extend skill library to store hindsight-relabeled patterns with source tracking |
| soar-003 | Implement structural validation | Validate synthetic tasks are meaningful (not degenerate) |

**Phase 2: Data Selection (P1)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-004 | Implement greedy-diverse selection | Create src/learning/soar-selection.ts with greedy-diverse data selection |
| soar-005 | Add accuracy binning for refinement | Balance refinement training across accuracy bins (0%, 1-34%, 34-98%, 100%) |

**Phase 3: Unified Learning (P1-P2)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-006 | Create unified skill library | Merge sampling and refinement skills into single library |
| soar-007 | Implement joint learning | Update library from both sampling and refinement outcomes |
| soar-008 | Track dual success rates | Track samplingSuccessRate and refinementSuccessRate per skill |

**Phase 4: Iterated Self-Improvement (P2)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-009 | Implement SOAR iteration loop | Create src/learning/soar-loop.ts with full iteration |
| soar-010 | Add iteration tracking | Track which iteration produced each skill |
| soar-011 | Implement cross-iteration data pooling | Pool data across iterations for training |

**Phase 5: Test-Time Training (P2)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-012 | Implement test-time training | Adapt to target tasks without ground truth |
| soar-013 | Add training accuracy proxy | Select data by training accuracy when test unavailable |
| soar-014 | Implement pseudo-labeling | Use majority voting to estimate correct solutions |

**Phase 6: Voting and Ensembling (P2-P3)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-015 | Implement weighted majority voting | Select solutions using count + c×accuracy weighting |
| soar-016 | Add solution diversity tracking | Measure diversity using embedding distance |
| soar-017 | Implement cross-strategy pooling | Pool solutions from multiple subagent strategies |

**Phase 7: Monitoring and Analysis (P3)**

| Task ID | Title | Description |
|---------|-------|-------------|
| soar-018 | Add iteration progress dashboard | Visualize performance across iterations |
| soar-019 | Track diversity over iterations | Monitor solution diversity to detect mode collapse |
| soar-020 | Add scaling analysis | Track model-size and search-budget scaling curves |

### 4.2 Integration with Existing Tasks

**TRM Tasks That Complement SOAR:**
- oa-459168 (TRM training loop) → Integrate hindsight relabeling
- oa-1f1b7a (per-step learning) → Learn from every step for hindsight
- oa-6499e1 (EMA stability) → Apply EMA to SOAR success rates

**HRM Tasks That Complement SOAR:**
- oa-168328 (Archivist integration) → Store SOAR search traces
- oa-a85fe8 (segment extraction) → Extract skills from hindsight data

### 4.3 Recommended Task Updates

**Update trm-integration-tasks.md:**
- Add note about SOAR integration
- Reference hindsight learning for skill extraction

**Update hrm-integration-tasks.md:**
- Note that Archivist should store SOAR traces
- Reference cross-iteration data pooling

---

## Part 5: Implementation Priority

### 5.1 Combined Implementation Order

```
Week 1-2: Foundation
├── TRM State Schema (x, y, z)
├── Simple Halt Decision
├── Hindsight Relabeler (SOAR)
└── Greedy-Diverse Selection (SOAR)

Week 3-4: Core Learning
├── Deep Supervision (TRM/SOAR shared)
├── Unified Skill Library (SOAR)
├── Joint Sample+Refine Learning (SOAR)
└── EMA Stability (TRM)

Week 5-6: Iteration
├── SOAR Iteration Loop
├── Test-Time Training (SOAR)
├── Nested Cycles (HRM, if needed)
└── Depth Tracking (TRM)

Week 7-8: Refinement
├── Weighted Majority Voting (SOAR)
├── Diversity Tracking (SOAR)
├── A/B Testing Framework
└── Dashboards
```

### 5.2 Success Criteria

1. **Hindsight Learning Working**: Skill library grows from failed attempts
2. **Joint Learning Active**: Single library serves both sampling and refinement
3. **Iteration Improves Performance**: Each iteration lifts success rate
4. **Test-Time Training Works**: Can improve on target tasks without labels
5. **Diversity Maintained**: Don't collapse to single solution strategy

---

## Part 6: Key Takeaways

### For Architecture Design

1. **Learn from failures** - Hindsight relabeling is SOAR's secret weapon
2. **Unify sampling and refinement** - Joint learning beats specialized modules
3. **Iterate multiple times** - Each iteration lifts the scaling curve
4. **Maintain diversity** - Greedy-diverse prevents mode collapse
5. **Pool across variations** - Cross-model/cross-iteration pooling helps

### For MechaCoder Specifically

1. **Every Gym attempt is valuable** - Extract synthetic skills from failures
2. **Same skill library for initial and retry** - Don't specialize
3. **Run multiple SOAR iterations overnight** - Accumulate improvements
4. **Test-time training on Terminal-Bench** - Adapt without ground truth
5. **Track diversity** - Alert if solutions converge too quickly

### The Meta-Lesson

SOAR's core insight: **The search traces ARE the training data.**

Traditional approaches:
- Success → learn
- Failure → discard

SOAR approach:
- Success → learn
- Failure → hindsight relabel → learn

This 2x-3x multiplies the effective training data, enabling self-improvement from limited feedback.

---

## References

- SOAR Paper: "Self-Improving Language Models for Evolutionary Program Synthesis" - Inria/MIT
- GitHub: github.com/flowersteam/SOAR
- TRM Analysis: `docs/research/analysis/trm-vs-hrm-comparison.md`
- HRM Tasks: `docs/mechacoder/hrm-integration-tasks.md`
- TRM Tasks: `docs/mechacoder/trm-integration-tasks.md`
