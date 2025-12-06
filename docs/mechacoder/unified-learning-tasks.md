# Unified Learning Tasks: TRM + SOAR for Terminal-Bench

This document consolidates the learning system integration tasks, combining TRM architecture with SOAR's self-improvement loop.

## Decision Summary

After analyzing HRM, TRM, and SOAR papers:
- **Architecture**: TRM (simpler, better results)
- **Learning Loop**: SOAR (hindsight relabeling, test-time training)
- **HRM**: Superseded by TRM (15 tasks closed)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│              SOAR: Outer Self-Improvement Loop              │
│                                                             │
│   Hindsight Relabeling → Greedy-Diverse Selection →        │
│   Joint Learning → Iteration → Test-Time Training          │
│                                                             │
│   ┌─────────────────────────────────────────────────────┐  │
│   │         TRM: Single Attempt Structure               │  │
│   │                                                     │  │
│   │   State: {x, y, z}                                  │  │
│   │   - x = task context                                │  │
│   │   - y = candidate solution                          │  │
│   │   - z = reasoning trace                             │  │
│   │                                                     │  │
│   │   ACT: Simple binary halt                           │  │
│   │   Deep Supervision: Learn every step                │  │
│   │   EMA: Stable success rate tracking                 │  │
│   │                                                     │  │
│   └─────────────────────────────────────────────────────┘  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation (P0/P1)

### TRM State Schema
- **Task**: Create `src/learning/trm-state.ts` with {x, y, z} schema
- **TRM Task ID**: oa-642def
- **Why**: TRM's state is simpler and more intuitive than HRM's zH/zL

### Simple Halt Decision
- **Task**: Create `src/learning/trm-halt.ts` with binary halt
- **TRM Task ID**: oa-49128f
- **Why**: Works as well as Q-learning ACT with half the compute

### EMA Stability
- **Task**: Create `src/learning/trm-ema.ts` for EMA tracking
- **TRM Task ID**: oa-6499e1
- **Why**: Prevents oscillation, 87.4% vs 79.9% accuracy

### Deep Supervision
- **Task**: Create `src/learning/trm-supervision.ts` for per-step learning
- **TRM Task ID**: oa-1f1b7a (and shared with HRM oa-4249d4)
- **Why**: 2x accuracy improvement (19% → 39%)

---

## Phase 2: Hindsight Learning (P1)

### Hindsight Relabeler (KEY)
- **Task**: Create `src/learning/soar-hindsight.ts`
- **SOAR Task ID**: oa-34d337
- **Why**: SOAR's key innovation - every failure becomes training data

### Synthetic Skill Storage
- **Task**: Extend skill library with source tracking
- **SOAR Task ID**: oa-5a2c82
- **Why**: Distinguish hindsight vs direct skills

### Structural Validation
- **Task**: Create `src/learning/soar-validation.ts`
- **SOAR Task ID**: oa-541678
- **Why**: Filter degenerate synthetic tasks

---

## Phase 3: Data Selection (P1)

### Greedy-Diverse Selection
- **Task**: Create `src/learning/soar-selection.ts`
- **SOAR Task ID**: oa-94ca90
- **Why**: 36.46% vs 34.30% (greedy) vs 32.38% (uniform)

### Accuracy Binning
- **Task**: Balance refinement data across accuracy bins
- **SOAR Task ID**: oa-5b4f0d
- **Why**: Learn to fix code at all difficulty levels

---

## Phase 4: Unified Learning (P1-P2)

### Unified Skill Library
- **Task**: Create `src/learning/soar-unified.ts`
- **SOAR Task ID**: oa-e3d4a2
- **Why**: Joint training beats specialized (39.79% vs 36.46%)

### Joint Learning
- **Task**: Create `src/learning/soar-joint.ts`
- **SOAR Task ID**: oa-7fb8ac
- **Why**: Learn from both sampling and refinement outcomes

### Dual Success Rates
- **Task**: Add sampling/refinement rates to skills
- **SOAR Task ID**: oa-ff160d
- **Why**: Track skill effectiveness per context

---

## Phase 5: Iteration Loop (P1-P2)

### SOAR Iteration Loop
- **Task**: Create `src/learning/soar-loop.ts`
- **SOAR Task ID**: oa-f04075
- **Why**: Search → Learn → Repeat breaks scaling plateaus

### Iteration Tracking
- **Task**: Track which iteration produced each skill
- **SOAR Task ID**: oa-a6685f
- **Why**: Skills from different iterations have different quality

### Cross-Iteration Pooling
- **Task**: Pool data across all iterations
- **SOAR Task ID**: oa-13665d
- **Why**: 41.1% vs 34.6% with pooling

---

## Phase 6: Test-Time Training (P0/P1) - CRITICAL

### Test-Time Training
- **Task**: Create `src/learning/soar-ttt.ts`
- **SOAR Task ID**: oa-c559bd
- **Why**: +3-5% improvement without ground truth

### Training Accuracy Proxy
- **Task**: Select data by training accuracy
- **SOAR Task ID**: oa-87fdbe
- **Why**: Proxy for solution quality when test labels unavailable

### Pseudo-Labeling
- **Task**: Majority voting for "correct" estimates
- **SOAR Task ID**: oa-723382
- **Why**: Estimate labels without ground truth

### Weighted Majority Voting
- **Task**: Create `src/learning/soar-voting.ts`
- **SOAR Task ID**: oa-5679bd
- **Why**: weight = count + 1000 × training_accuracy

---

## Phase 7: Monitoring (P2-P3)

### Diversity Tracking
- **Task**: Create `src/dashboard/soar-diversity.ts`
- **SOAR Task ID**: oa-c7c037
- **Why**: Detect mode collapse early

### Iteration Progress Dashboard
- **Task**: Create `src/dashboard/soar-progress.ts`
- **SOAR Task ID**: oa-8d2ce3
- **Why**: Visualize improvement across iterations

### Cross-Strategy Pooling
- **Task**: Pool solutions across subagent strategies
- **SOAR Task ID**: oa-fa9395
- **Why**: 52% vs 45% with pooling

---

## Files to Create

| File | Purpose | Priority |
|------|---------|----------|
| `src/learning/trm-state.ts` | TRM state {x, y, z} | P0 |
| `src/learning/trm-halt.ts` | Simple binary halt | P1 |
| `src/learning/trm-ema.ts` | EMA stability | P1 |
| `src/learning/trm-supervision.ts` | Per-step learning | P1 |
| `src/learning/soar-hindsight.ts` | Hindsight relabeler | P1 |
| `src/learning/soar-validation.ts` | Synthetic validation | P1 |
| `src/learning/soar-selection.ts` | Greedy-diverse | P1 |
| `src/learning/soar-unified.ts` | Unified library | P1 |
| `src/learning/soar-joint.ts` | Joint learning | P2 |
| `src/learning/soar-loop.ts` | Iteration loop | P1 |
| `src/learning/soar-ttt.ts` | Test-time training | P0 |
| `src/learning/soar-voting.ts` | Majority voting | P1 |
| `src/dashboard/soar-diversity.ts` | Diversity tracking | P3 |
| `src/dashboard/soar-progress.ts` | Progress dashboard | P3 |

---

## Success Criteria

1. **TRM State Working**: Training loop uses {x, y, z}
2. **Hindsight Active**: Skill library grows from failures
3. **Joint Learning**: Single library for sampling + refinement
4. **Iteration Improves**: Each loop increases success rate
5. **TTT Works**: Improves on TB without ground truth
6. **Diversity Maintained**: No mode collapse on unsolved tasks

---

## Closed HRM Tasks (15 total)

These tasks were closed as "superseded by TRM":

| Task ID | Title | Reason |
|---------|-------|--------|
| oa-ee0011 | HierarchicalState schema | Use TRM state |
| oa-98ebd8 | TrainingLoop state | Use TRM state |
| oa-3960ce | Gym hierarchical state | Use TRM state |
| oa-0becba | Nested cycle structure | TRM simpler |
| oa-42d7d2 | Convergence detection | Not needed |
| oa-3880bd | L-reset on H-update | TRM simpler |
| oa-8090ef | Q-head for ACT | Simple halt |
| oa-1e3eec | TrainerService ACT | Simple halt |
| oa-ea7259 | Q-learning update | Simple halt |
| oa-b4324f | Dimensionality metrics | Low priority |
| oa-e1141a | Intermediate viz | Low priority |
| oa-18efc5 | Convergence monitoring | Not needed |
| oa-c2b296 | Dimensionality tracking | Low priority |
| oa-3b4a5a | Unified state interface | TRM only |
| oa-c3e40f | A/B testing framework | TRM only |

---

## Remaining Active Tasks

### TRM Tasks (11 remaining)
- State schema, prompts, training loop
- Simple halt, progress detection
- EMA stability
- Depth tracking

### SOAR Tasks (17 remaining)
- Hindsight relabeling (3)
- Data selection (2)
- Unified learning (3)
- Iteration loop (3)
- Test-time training (3)
- Voting/diversity (3)

### HRM Tasks (3 remaining, valuable)
- oa-4249d4: Deep supervision (shared with TRM)
- oa-168328: Archivist segments (useful for SOAR)
- oa-a85fe8: Segment skill extraction (useful for hindsight)

---

## References

- TRM Paper: "Less is More: Recursive Reasoning with Tiny Networks"
- SOAR Paper: "Self-Improving Language Models for Evolutionary Program Synthesis"
- Archived docs: `history/learning-research/`
