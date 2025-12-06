# TRM Integration Tasks

This document explains each task created to integrate insights from the Tiny Recursive Model (TRM) paper into MechaCoder's learning system.

## Background

The TRM paper ("Less is More: Recursive Reasoning with Tiny Networks") demonstrates that:
- A **7M parameter model with 2 layers** outperforms HRM (27M params, 4 layers)
- Achieves **45% on ARC-AGI-1** (vs HRM's 40%), **87% on Sudoku** (vs HRM's 55%)
- Simplicity wins: no fixed-point theorems, no hierarchy, no biological arguments
- Single tiny network recursively refining its answer beats complex architectures

The key insight: **Less is More** - reducing complexity improves generalization.

---

## Phase 1: Simplified State Model

### oa-642def: Define TRM-style state schema (x, y, z)

**Description:** Create `src/learning/trm-state.ts` with TRMState interface containing x (task context), y (candidate solution), z (reasoning trace).

**Why:** TRM reinterprets HRM's "hierarchical" zH/zL as simply:
- **x** = input question (task description)
- **y** = current proposed solution (what HRM called zH)
- **z** = latent reasoning trace (what HRM called zL)

This is more intuitive and the paper proves it's optimal:
- Single feature (z only): 71.9% accuracy
- Multi-scale (n+1 features): 77.6% accuracy
- **Two features (y, z): 87.4% accuracy** - best!

The reasoning: without y, z must store both solution AND reasoning. Without z, we forget HOW we got to y (like losing chain-of-thought). Two is the minimum needed.

---

### oa-96e4d4: Migrate from HierarchicalState to TRMState

**Description:** Refactor HierarchicalState (zH, zL) to TRMState (x, y, z). Update all consumers.

**Why:** TRM's naming is cleaner and more intuitive:
- `y = solution` (previously zH)
- `z = reasoning` (previously zL)
- `x = input` (previously implicit)

This isn't just renaming - it's a conceptual shift from "hierarchical levels" to "solution + reasoning". The hierarchy interpretation was based on uncertain biological arguments; TRM shows it's unnecessary.

---

### oa-374b68: Update TrainingLoop to use TRM state

**Description:** Modify `src/learning/loop.ts` to use TRMState, track (y, z) across supervision steps, detach state between episodes.

**Why:** The training loop needs to:
1. Initialize y and z
2. Run recursions that update both
3. Evaluate y after each supervision step
4. Detach (y, z) before next step (prevents gradient flow)
5. Learn from every step (deep supervision)

This is simpler than HRM's two-network approach.

---

## Phase 2: Single Network Pattern

### oa-ce1808: Create unified TRM reasoning function

**Description:** Create `src/learning/trm-reasoning.ts` with trmRecursion function that handles both z-update and y-update in a single unified process.

**Why:** HRM uses two separate networks (fL for z, fH for y). TRM shows ONE network works better:
- Same network, different inputs
- `z = net(x, y, z)` - includes task context x
- `y = net(y, z)` - no x, focuses on refinement

The presence/absence of x determines the operation. This reduces parameters by half (10M → 5M) while improving accuracy (82.4% → 87.4%).

---

### oa-0ba88d: Implement TRM z-update and y-update functions

**Description:** Create `src/learning/trm-updates.ts` with updateReasoning(x,y,z) and refineSolution(y,z) functions.

**Why:** These are the two core operations:

1. **updateReasoning(x, y, z)**: Update the reasoning trace
   - Takes full context (task + current solution + current reasoning)
   - Produces updated reasoning (hypotheses, error patterns, next steps)
   - Called n times per cycle

2. **refineSolution(y, z)**: Update the solution
   - Takes current solution + reasoning (no task context)
   - Produces improved solution candidate
   - Called once per cycle

The key insight: same underlying FM, different prompts.

---

### oa-1ae055: Create TRM prompt templates

**Description:** Create `src/learning/trm-prompts.ts` with reasoning and solution prompts.

**Why:** TRM's single-network approach works because the prompts differentiate the task:

**Reasoning prompt (includes x):**
```
Task: ${x.taskDescription}
Current solution: ${y.candidate}
Reasoning so far: ${z.reasoningHistory}
Errors seen: ${z.errorPatterns}

Update your reasoning about how to solve this task.
```

**Solution prompt (no x):**
```
Current solution: ${y.candidate}
Latest reasoning: ${z.reasoningHistory.slice(-3)}

Refine the solution based on your reasoning.
```

The absence of x in the solution prompt forces focus on refinement rather than re-analyzing the task.

---

## Phase 3: Deep Supervision Integration

### oa-459168: Implement TRM training loop with deep supervision

**Description:** Create `src/learning/trm-loop.ts` with trmTrainingLoop that runs up to Nsup=16 supervision steps.

**Why:** Deep supervision is the **PRIMARY driver of performance**. The ARC Prize Foundation analysis showed:
- Single-step supervision: 19% accuracy
- Deep supervision (16 steps): 39% accuracy
- That's a **2x improvement** just from learning at multiple points!

The loop structure:
```
for step in range(Nsup):
    (y, z) = trmRecursion(x, y, z)
    loss = evaluate(y, ground_truth)
    learn_from_step(loss)
    (y, z) = detach(y, z)  # No gradient across steps
    if should_halt(y): break
```

---

### oa-1f1b7a: Implement TRM per-step learning

**Description:** Create `src/learning/trm-supervision.ts` to learn from each step, not just final outcome.

**Why:** Every step is a learning opportunity:
- **Successful step**: Extract skill, record pattern
- **Failed step**: Record failure pattern, update error context
- **Partial progress**: Note what worked, what didn't

Traditional training only learns from final success/failure. Deep supervision learns from the JOURNEY, not just the destination. This is especially valuable for hard tasks that require many attempts.

---

### oa-8bee11: Implement TRM state detachment between steps

**Description:** Create `src/learning/trm-detach.ts` to clear execution-specific context between steps.

**Why:** In neural networks, `detach()` prevents gradient flow. In MechaCoder (no gradients), detachment means:
- Clear transient execution details
- Preserve learned insights
- Reset for fresh attempt

Without detachment, each step would carry all the baggage of previous attempts. With detachment, each step gets a clean slate with accumulated KNOWLEDGE (not noise).

---

## Phase 4: Simplified ACT

### oa-49128f: Implement TRM simple halt decision

**Description:** Create `src/learning/trm-halt.ts` with simple shouldHalt function.

**Why:** HRM's ACT uses Q-learning with two forward passes:
1. Compute Q(halt) and Q(continue)
2. Extra forward pass for continue target
3. Binary cross-entropy loss

TRM shows this is unnecessary. Simple rules work just as well:
- Max steps reached? → halt
- Tests passed? → halt
- High confidence? → halt
- Stuck (same errors)? → halt

Result: Same accuracy (86.1% vs 87.4%), half the compute (1 pass vs 2).

---

### oa-ee4402: Remove complex Q-learning ACT from HRM tasks

**Description:** Simplify ACT by removing Q-head, Q-values, and continue loss.

**Why:** The TRM paper proves Q-learning ACT is over-engineered:
- HRM with Q-learning ACT: 86.1%
- TRM with simple halt: 87.4%

The complex machinery adds no value. This task updates the HRM integration to use TRM-style simple ACT, reducing complexity while maintaining (or improving) performance.

---

### oa-29f935: Add TRM progress and stuck detection

**Description:** Create `src/learning/trm-progress.ts` to detect progress and stuck states.

**Why:** Good halt decisions need progress signals:

**Progress detection:**
- Is the solution improving? (fewer errors, more tests passing)
- Is reasoning advancing? (new hypotheses, ruled out approaches)

**Stuck detection:**
- Same error 3+ times in a row
- No change in solution for N steps
- Oscillating between two states

These signals inform halt decisions without complex Q-learning.

---

## Phase 5: EMA and Stability

### oa-6499e1: Add EMA for success rate tracking

**Description:** Create `src/learning/trm-ema.ts` with Exponential Moving Average for success rates.

**Why:** TRM uses EMA (decay=0.999) to prevent overfitting and divergence:
- Without EMA: 79.9% accuracy, prone to collapse
- With EMA: 87.4% accuracy, stable training

For MechaCoder, EMA smooths noisy feedback:
```typescript
emaSuccessRate = 0.999 * emaSuccessRate + 0.001 * (succeeded ? 1 : 0)
```

This prevents wild swings from individual successes/failures.

---

### oa-d328fa: Stabilize skill confidence with EMA

**Description:** Apply EMA to skill success rates in the skill library.

**Why:** Skill confidence can be noisy:
- Skill works 5 times → 100% confidence
- Skill fails once → drops to 83%
- That's a 17% swing from one failure!

With EMA:
- 5 successes → ~99.5% confidence
- 1 failure → ~99.4% confidence
- Much more stable

This prevents skills from being incorrectly deprecated after rare failures.

---

### oa-558003: Add TRM divergence detection

**Description:** Create `src/learning/trm-stability.ts` to detect training divergence.

**Why:** Small data + deep recursion can cause divergence (TRM paper notes HRM "tends to overfit quickly and then diverge"). Early detection allows intervention:
- Success rate dropping sharply? → alert
- EMA trending down? → adjust parameters
- Skill library quality declining? → investigate

---

## Phase 6: Recursion Depth Optimization

### oa-d06c97: Track optimal recursion depth per task type

**Description:** Create `src/learning/trm-depth.ts` to track which depths work for which tasks.

**Why:** TRM tested various depths:
- T=2, n=2 (12 recursions): 73.7%
- T=3, n=6 (42 recursions): 87.4% ← optimal
- T=4, n=4 (80 recursions): 84.2%

Different task types may have different optima. Track a histogram of {depth → success rate} per task type to learn optimal depth dynamically.

---

### oa-550a84: Implement TRM adaptive depth selection

**Description:** Create `src/learning/trm-adaptive.ts` to select depth based on task type statistics.

**Why:** Not all tasks need the same depth:
- Easy tasks: Halt at depth 5 (save compute)
- Medium tasks: Run to depth 20
- Hard tasks: Full 42 recursions

By learning optimal depth per task type, we can:
- Save compute on easy tasks (like TRM's ACT)
- Invest more compute on hard tasks
- Improve overall throughput

---

### oa-5c515a: Add depth vs success analysis dashboard

**Description:** Create `src/dashboard/depth-analysis.ts` to visualize depth/success curves.

**Why:** Understanding depth dynamics helps tune the system:
- Plot depth on X-axis, success rate on Y-axis
- Different curves per task type
- Identify optimal depth visually
- Track how optimal depth changes over training

---

## Phase 7: TRM vs HRM Comparison

### oa-3b4a5a: Create unified state interface for TRM and HRM

**Description:** Create `src/learning/state-unified.ts` to abstract over both approaches.

**Why:** We're implementing both HRM and TRM insights. A unified interface allows:
- Switching between approaches per task
- Fair A/B comparison
- Gradual migration from one to another

```typescript
interface UnifiedState {
  asTRM(): TRMState;  // { x, y, z }
  asHRM(): HierarchicalState;  // { zH, zL }
}
```

---

### oa-c3e40f: Create A/B testing framework for TRM vs HRM

**Description:** Create `src/learning/ab-test.ts` to empirically compare approaches.

**Why:** The papers show TRM > HRM on ARC-AGI benchmarks. But MechaCoder's tasks are different (Terminal-Bench, coding). We need empirical comparison:
- Run same tasks with TRM approach
- Run same tasks with HRM approach
- Track which wins on which task types
- Auto-select best approach

---

### oa-6c04ea: Document TRM vs HRM empirical findings

**Description:** Create `docs/research/trm-vs-hrm-findings.md` with MechaCoder-specific results.

**Why:** Academic benchmarks (ARC-AGI, Sudoku) differ from Terminal-Bench. Document:
- Which approach works better for which task types
- Where TRM's simplicity helps vs where HRM's hierarchy helps
- Recommendations for future development

---

## Key Differences: TRM vs HRM

| Aspect | HRM | TRM | Winner |
|--------|-----|-----|--------|
| Parameters | 27M | 7M | TRM (4x smaller) |
| Layers | 4 | 2 | TRM (simpler) |
| Networks | 2 (fH, fL) | 1 | TRM (unified) |
| State | zH + zL (hierarchy) | y + z (simple) | TRM (intuitive) |
| Gradient | 1-step approximation | Full backprop | TRM (no assumptions) |
| ACT | Q-learning (2 passes) | Binary halt (1 pass) | TRM (simpler) |
| ARC-AGI-1 | 40.3% | 44.6% | TRM |
| Sudoku-Extreme | 55.0% | 87.4% | TRM |

**Bottom line:** TRM achieves better results with a simpler architecture. The "hierarchical" interpretation was unnecessary complexity.

---

## Success Criteria

1. **TRM State Working**: Training loop uses {x, y, z} representation
2. **Single Network**: One FM prompt template adapts for both operations
3. **Deep Supervision**: Every step produces learning signal
4. **Simple Halt**: Binary decision without Q-learning
5. **EMA Stability**: Success rates smoothed, no divergence
6. **Depth Tracking**: Know optimal recursion depth per task type
7. **Comparison Complete**: TRM vs HRM empirically compared on TB tasks

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/learning/trm-state.ts` | TRM state schema (x, y, z) |
| `src/learning/trm-reasoning.ts` | Unified recursion function |
| `src/learning/trm-updates.ts` | z-update and y-update |
| `src/learning/trm-prompts.ts` | TRM prompt templates |
| `src/learning/trm-loop.ts` | TRM training loop |
| `src/learning/trm-supervision.ts` | Per-step learning |
| `src/learning/trm-detach.ts` | State detachment |
| `src/learning/trm-halt.ts` | Simple halt decision |
| `src/learning/trm-progress.ts` | Progress/stuck detection |
| `src/learning/trm-ema.ts` | EMA for stability |
| `src/learning/trm-depth.ts` | Depth tracking |
| `src/learning/trm-adaptive.ts` | Adaptive depth selection |

---

## References

- TRM Paper: "Less is More: Recursive Reasoning with Tiny Networks" - Samsung SAIL Montreal
- Plan file: `/Users/christopherdavid/.claude/plans/trm-integration-plan.md`
- HRM integration: `docs/mechacoder/hrm-integration-tasks.md`
