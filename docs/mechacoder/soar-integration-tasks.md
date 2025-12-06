# SOAR Integration Tasks

This document explains each task created to integrate insights from the SOAR paper (Self-Improving Language Models for Evolutionary Program Synthesis) into MechaCoder's learning system.

## Background

The SOAR paper demonstrates that:
- A **self-improving loop** can lift a 7B model from 14.25% to 36.25% on ARC-AGI (2.5x improvement)
- **Hindsight relabeling** turns failures into training data (2.4M examples from 400 tasks)
- **Joint learning** of sampling and refinement beats specialized models
- **Test-time training** improves performance without ground truth
- Achieved **52% on ARC-AGI** (state-of-the-art for open-source)

The key insight: **Learn from ALL attempts, not just successes.** Failures are correct for *some* task.

---

## Phase 1: Hindsight Learning Infrastructure

### oa-soar01: Implement hindsight relabeler

**Description:** Create `src/learning/soar-hindsight.ts` with HindsightRelabeler that converts failed attempts into synthetic task-solution pairs.

**Why:** Hindsight relabeling is SOAR's key innovation. When an agent attempts task T and produces output O (instead of correct output C), the attempt is a "failure." But that same code is *correct* for the synthetic task T' where O is the expected output.

This transforms learning:
- Traditional: Learn from successes only (limited data)
- SOAR: Learn from all attempts via hindsight (2.4M examples from 400 tasks)

For MechaCoder:
```
Original: "Write function returning fibonacci(10)"
Agent output: Function returning factorial(10)
Result: FAILURE

Hindsight relabeled:
Synthetic task: "Write function returning factorial(10)"
Agent output: Same code
Result: SUCCESS (by construction)
```

Every failed Gym attempt becomes a valid skill pattern for *something*.

---

### oa-soar02: Add synthetic skill storage

**Description:** Extend skill library schema to store hindsight-relabeled patterns with source tracking (`source: "hindsight" | "direct"`).

**Why:** Synthetic skills from hindsight relabeling need to be distinguished:
- They're correct by construction for their synthetic task
- They may not generalize as well as direct successes
- We need to track their effectiveness separately

Schema addition:
```typescript
interface Skill {
  // ... existing fields
  source: "hindsight" | "direct" | "synthetic";
  originalTaskId?: string;  // What task it failed on
  syntheticTaskDesc?: string;  // What task it's correct for
}
```

---

### oa-soar03: Implement structural validation

**Description:** Create `src/learning/soar-validation.ts` to validate synthetic tasks are meaningful (not degenerate, have real transformation).

**Why:** Not all synthetic tasks are useful. Degenerate cases:
- Identity transformation (input = output)
- Trivial output (all zeros, empty)
- Non-deterministic (random output)

Validation criteria:
1. Input ≠ Output (non-trivial transformation)
2. Output is structurally valid (parses, type-checks)
3. Transformation is consistent across examples
4. Not a hardcoded lookup table

---

## Phase 2: Data Selection

### oa-soar04: Implement greedy-diverse selection

**Description:** Create `src/learning/soar-selection.ts` with greedy-diverse data selection strategy.

**Why:** SOAR's ablation shows greedy-diverse beats both pure greedy and uniform:

| Strategy | Accuracy |
|----------|----------|
| uniform | 32.38% |
| greedy | 34.30% |
| **greedy-diverse** | **36.46%** |

Algorithm:
1. For each task, collect all attempts
2. Sort by accuracy
3. Take top 25 (greedy - learn from best)
4. Take bottom 25 (diversity - maintain exploration)
5. Apply hindsight relabeling to all

Pure greedy leads to mode collapse on easy solutions. Diversity maintains capability on hard problems.

---

### oa-soar05: Add accuracy binning for refinement data

**Description:** Implement balanced sampling across accuracy bins (0%, 1-34%, 34-98%, 100%) for refinement training data.

**Why:** Learning to refine requires examples at all difficulty levels:
- 0% accuracy parents: Learn to fix completely broken code
- 1-34%: Learn to fix major bugs
- 34-98%: Learn to fix edge cases
- 100% (wrong test): Learn final polish

Uniform sampling over-represents easy refinements. Balanced binning ensures coverage across difficulty spectrum.

---

## Phase 3: Unified Learning

### oa-soar06: Create unified skill library

**Description:** Refactor skill library to serve both sampling (initial attempts) and refinement (with feedback) from a single unified store.

**Why:** SOAR's key finding: joint training outperforms specialized models.

| Configuration | Sample Acc | Search Acc |
|---------------|------------|------------|
| separate models | 36.46% | 43.88% |
| **unified model** | **39.79%** | **44.42%** |

The same skill patterns are useful for both:
- Initial attempt: "Here's how to solve this type of problem"
- Refinement: "Here's how to fix this type of error"

Separating them misses transfer learning between tasks.

---

### oa-soar07: Implement joint learning

**Description:** Create `src/learning/soar-joint.ts` to update skill library from both sampling and refinement outcomes simultaneously.

**Why:** SOAR finds positive synergy between sampling and refinement learning:
- Learning to refine teaches what makes programs better
- This knowledge transfers to better initial sampling
- Combined effect exceeds sum of parts

Implementation:
```typescript
function learnJointly(
  samplingOutcomes: Outcome[],
  refinementOutcomes: Outcome[],
  library: SkillLibrary,
): void {
  // Extract patterns from both
  const samplingPatterns = extractPatterns(samplingOutcomes);
  const refinementPatterns = extractPatterns(refinementOutcomes);

  // Update library with both
  for (const pattern of [...samplingPatterns, ...refinementPatterns]) {
    library.update(pattern);
  }
}
```

---

### oa-soar08: Track dual success rates

**Description:** Add `samplingSuccessRate` and `refinementSuccessRate` fields to skills, using EMA for stability.

**Why:** Skills may be effective for sampling but not refinement (or vice versa). Tracking both:
1. Enables intelligent skill selection based on context
2. Identifies skills that transfer well
3. Detects skills that overfit to one task type

```typescript
interface UnifiedSkill {
  samplingSuccessRate: EMA;  // How well does this work for initial attempts?
  refinementSuccessRate: EMA;  // How well does this work for refinements?

  // Joint confidence
  get jointConfidence(): number {
    return (this.samplingSuccessRate.value + this.refinementSuccessRate.value) / 2;
  }
}
```

---

## Phase 4: Iterated Self-Improvement

### oa-soar09: Implement SOAR iteration loop

**Description:** Create `src/learning/soar-loop.ts` with the full SOAR self-improvement loop (search phase + learning phase).

**Why:** SOAR's power comes from iteration. Each iteration:
1. Uses current skill library for search
2. Collects all search traces
3. Applies hindsight relabeling
4. Updates skill library
5. Repeat

Results compound:
| Iteration | 7B Model | 14B Model |
|-----------|----------|-----------|
| 0 | ~30% | ~35% |
| 1 | ~42% | ~47% |
| 2 | ~50% | ~55% |
| 3 | ~57% | ~59% |

---

### oa-soar10: Add iteration tracking

**Description:** Track which SOAR iteration produced each skill in the library.

**Why:** Skills from different iterations may have different quality:
- Early iterations: Broader but potentially noisier
- Later iterations: More refined but potentially overfit

Tracking enables:
- Analysis of skill evolution
- Rollback if later iterations degrade
- Weighting by iteration quality

---

### oa-soar11: Implement cross-iteration data pooling

**Description:** Pool training data across all iterations when fine-tuning base model.

**Why:** SOAR shows that training on pooled data from all iterations outperforms training on single iteration:

| Training Data | Test Accuracy |
|---------------|---------------|
| Single iteration | 19.9-34.6% |
| **All iterations pooled** | **33.0-41.1%** |

Different iterations solve different problems. Pooling captures the union of capabilities.

---

## Phase 5: Test-Time Training

### oa-soar12: Implement test-time training

**Description:** Create `src/learning/soar-ttt.ts` to adapt skill library to target tasks WITHOUT ground truth.

**Why:** During actual deployment, we don't have ground truth labels. But we can still improve:
1. Run search on target tasks
2. Select data by training accuracy (test proxy)
3. Apply hindsight relabeling
4. Update skills
5. Repeat

SOAR achieves +3-5% additional improvement through test-time training alone.

---

### oa-soar13: Add training accuracy proxy

**Description:** Implement data selection using training accuracy when test accuracy unavailable.

**Why:** For Terminal-Bench tasks:
- Test accuracy = does output match expected?
- Training accuracy = do intermediate examples pass?

When we don't know expected output, training accuracy serves as proxy:
- High training accuracy → likely correct approach
- Low training accuracy → likely wrong approach

Selection strategy:
```typescript
function selectByTrainingAccuracy(attempts: Attempt[]): Attempt[] {
  // Sort by training accuracy (not test)
  const sorted = sortBy(attempts, a => -a.trainingAccuracy);
  // Apply greedy-diverse
  return greedyDiverse(sorted);
}
```

---

### oa-soar14: Implement pseudo-labeling with majority voting

**Description:** Use weighted majority voting to estimate "correct" solutions for test-time training.

**Why:** Without ground truth, we need to estimate which solutions are correct. SOAR uses weighted majority voting:

```
weight = count + c × training_accuracy
```

Where:
- `count` = how many attempts produced this output
- `training_accuracy` = how well those attempts did on examples
- `c` = scaling factor (1000 in SOAR)

Multiple attempts producing the same output with high training accuracy → likely correct.

---

## Phase 6: Voting and Ensembling

### oa-soar15: Implement weighted majority voting

**Description:** Create `src/learning/soar-voting.ts` with weighted majority voting for solution selection.

**Why:** Simple majority voting treats all votes equally. Weighted voting favors:
- Outputs from high-accuracy attempts
- Outputs produced by many attempts

This is more robust than:
- Best single attempt (may be lucky)
- Unweighted majority (ignores quality signals)

---

### oa-soar16: Add solution diversity tracking

**Description:** Track diversity of solutions using embedding distance (cosine distance in embedding space).

**Why:** SOAR finds that diversity drops after self-improvement iterations. On solved problems, this is fine (converging on solution). On unsolved problems, this is bad (mode collapse).

Tracking diversity enables:
- Early warning of mode collapse
- Diversity-aware data selection
- Intervention when diversity drops too fast

Metric: Average pairwise cosine distance in embedding space.

---

### oa-soar17: Implement cross-strategy pooling

**Description:** Pool solutions from multiple subagent strategies/configurations.

**Why:** SOAR finds that different model sizes solve different problems:
- 7B solves some that 72B misses
- Pooling 5 model sizes: 52% (vs 45% best single)
- Oracle: 57.25% (9.5% gap indicates room for better ensembling)

For MechaCoder, pool across:
- Different subagent prompts
- Different temperature settings
- Different skill subsets
- Different retry strategies

---

## Phase 7: Monitoring and Analysis

### oa-soar18: Add iteration progress dashboard

**Description:** Create `src/dashboard/soar-progress.ts` to visualize performance across SOAR iterations.

**Why:** Need to see:
- Performance improvement per iteration
- When gains slow down
- Whether iterations are still helping

Key metrics:
- Success rate by iteration
- Skill library size growth
- Average skill confidence
- Tasks solved/unsolved breakdown

---

### oa-soar19: Track diversity over iterations

**Description:** Monitor solution diversity to detect mode collapse.

**Why:** SOAR Figure 6 shows:
- Solved problems: Diversity drops (convergence - good)
- Unsolved problems: Diversity should stay high (exploration needed)

If diversity drops on unsolved problems, we're losing exploration capacity and may need:
- More diverse data selection
- Explicit diversity optimization
- Quality-diversity methods

---

### oa-soar20: Add scaling analysis tools

**Description:** Track model-size and search-budget scaling curves to detect plateaus.

**Why:** SOAR shows that:
- Fixed models plateau with increased search budget (~5.2k)
- Fixed search plateaus with increased model size
- Self-improvement breaks both plateaus

Tracking scaling curves helps:
- Identify when to iterate vs scale search
- Detect when self-improvement is saturating
- Guide resource allocation

---

## Success Criteria

1. **Hindsight Learning Active**: Skill library grows from failed attempts
2. **Joint Learning Working**: Single library serves both sampling and refinement
3. **Iteration Improves**: Each iteration increases success rate
4. **TTT Works**: Improvement on target tasks without ground truth
5. **Diversity Maintained**: Solution diversity on unsolved tasks stays high
6. **Plateaus Broken**: Performance continues improving beyond search-only limit

---

## Key Differences: SOAR vs TRM vs HRM

| Aspect | TRM | HRM | SOAR |
|--------|-----|-----|------|
| Focus | Single attempt | Single attempt | Learning across attempts |
| State | (x, y, z) | (zH, zL) | Search traces |
| Learning | Deep supervision | Deep supervision | Hindsight relabeling |
| Key Insight | Simplicity wins | Two timescales | Learn from failures |

**These are complementary:**
- TRM/HRM: How to structure a single reasoning attempt
- SOAR: How to improve from attempt to attempt

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/learning/soar-hindsight.ts` | Hindsight relabeler |
| `src/learning/soar-validation.ts` | Synthetic task validation |
| `src/learning/soar-selection.ts` | Greedy-diverse data selection |
| `src/learning/soar-unified.ts` | Unified skill library |
| `src/learning/soar-joint.ts` | Joint learning |
| `src/learning/soar-loop.ts` | SOAR iteration loop |
| `src/learning/soar-ttt.ts` | Test-time training |
| `src/learning/soar-voting.ts` | Weighted majority voting |
| `src/dashboard/soar-progress.ts` | Iteration progress |
| `src/dashboard/soar-diversity.ts` | Diversity tracking |

---

## References

- SOAR Paper: "Self-Improving Language Models for Evolutionary Program Synthesis" - Inria/MIT
- GitHub: github.com/flowersteam/SOAR
- Analysis: `docs/research/analysis/soar-self-improving-synthesis.md`
- TRM Integration: `docs/mechacoder/trm-integration-tasks.md`
- HRM Integration: `docs/mechacoder/hrm-integration-tasks.md`
