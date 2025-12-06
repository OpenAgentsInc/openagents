# Unified Learning System Plan: TRM + SOAR for Terminal-Bench

## Executive Summary

After deep analysis of all three ARC-AGI papers (HRM, TRM, SOAR), the recommended path forward is:

**TRM for Architecture** + **SOAR for Learning Loop**

- **TRM wins architecturally**: Simpler (7M vs 27M params), better results (44.6% vs 40.3% ARC-AGI)
- **SOAR provides the meta-learning**: Hindsight relabeling turns failures into training data
- **HRM is largely superseded**: TRM achieves HRM's goals with less complexity

This plan consolidates 59 research tasks (18 HRM + 21 TRM + 20 SOAR) into ~25 focused tasks.

---

## Key Insight: Why TRM > HRM

| Aspect | HRM | TRM | Winner |
|--------|-----|-----|--------|
| Parameters | 27M | 7M | **TRM** (4x smaller) |
| Layers | 4 | 2 | **TRM** (simpler) |
| Networks | 2 (fH, fL) | 1 | **TRM** (unified) |
| State | zH + zL (hierarchy) | x, y, z (intuitive) | **TRM** |
| ACT | Q-learning (2 passes) | Binary halt (1 pass) | **TRM** |
| ARC-AGI-1 | 40.3% | 44.6% | **TRM** |
| Sudoku | 55.0% | 87.4% | **TRM** |

**Bottom Line**: HRM's "hierarchical" interpretation was unnecessary complexity. TRM proves simpler is better.

---

## Architecture Decision: TRM + SOAR Synthesis

```
┌─────────────────────────────────────────────────────────────┐
│                    MECHACODER UNIFIED                        │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              SOAR: Outer Self-Improvement           │   │
│  │                                                     │   │
│  │   Hindsight Relabeling: Learn from ALL attempts    │   │
│  │   Greedy-Diverse Selection: Balance quality/explore│   │
│  │   Joint Learning: Unified sampling + refinement     │   │
│  │   Iteration: Multi-round self-improvement           │   │
│  │   TTT: Improve without ground truth                 │   │
│  │                                                     │   │
│  │   ┌─────────────────────────────────────────────┐  │   │
│  │   │         TRM: Single Attempt Structure        │  │   │
│  │   │                                             │  │   │
│  │   │   State: {x, y, z}                          │  │   │
│  │   │   - x = task context                        │  │   │
│  │   │   - y = candidate solution                  │  │   │
│  │   │   - z = reasoning trace                     │  │   │
│  │   │                                             │  │   │
│  │   │   ACT: Simple binary halt                   │  │   │
│  │   │   Deep Supervision: Learn every step        │  │   │
│  │   │   EMA: Stable success rate tracking         │  │   │
│  │   │                                             │  │   │
│  │   └─────────────────────────────────────────────┘  │   │
│  │                                                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  HRM Additions (minimal, where valuable):                   │
│  - Orchestrator/subagent as soft hierarchy                  │
│  - Diversity monitoring                                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Task Triage

### KEEP (Core Path - ~25 tasks)

#### From TRM (Architecture):
1. **TRM State Schema** (x, y, z) - Foundation
2. **Simple Halt Decision** - Replace Q-learning ACT
3. **Deep Supervision** - Learn every step
4. **EMA Stability** - Prevent oscillation
5. **Depth Tracking** - Know optimal recursion per task type
6. **Per-Step Learning** - Extract signal from every attempt

#### From SOAR (Learning Loop):
7. **Hindsight Relabeler** - KEY INNOVATION: failures become training data
8. **Synthetic Skill Storage** - Store hindsight patterns
9. **Structural Validation** - Ensure synthetic tasks are meaningful
10. **Greedy-Diverse Selection** - Balance quality and exploration
11. **Unified Skill Library** - Same library for sampling + refinement
12. **Joint Learning** - Update from both outcomes simultaneously
13. **Dual Success Rates** - Track sampling vs refinement effectiveness
14. **SOAR Iteration Loop** - Search → Learn → Repeat
15. **Iteration Tracking** - Know which iteration produced each skill
16. **Cross-Iteration Pooling** - Pool data across all iterations
17. **Test-Time Training** - Improve on TB without ground truth
18. **Training Accuracy Proxy** - Select by training accuracy
19. **Pseudo-Labeling** - Majority voting for "correct" estimates
20. **Weighted Majority Voting** - Solution selection
21. **Solution Diversity Tracking** - Detect mode collapse
22. **Cross-Strategy Pooling** - Pool across subagent variants
23. **Iteration Progress Dashboard** - Visualize improvement

### CLOSE (HRM-Specific, Superseded by TRM):

| Task ID | Title | Reason |
|---------|-------|--------|
| oa-ee0011 | HierarchicalState schema | → Use TRM state instead |
| oa-0becba | Nested cycle structure | → TRM recursion is simpler |
| oa-42d7d2 | Convergence detection | → Not needed with TRM approach |
| oa-3880bd | L-reset on H-update | → TRM detachment simpler |
| oa-8090ef | Q-head for ACT | → Use TRM simple halt |
| oa-ea7259 | Q-learning update | → Removed with Q-head |
| oa-b4324f | Dimensionality metrics | → Low priority, defer |
| oa-e1141a | Intermediate prediction viz | → Lower priority |
| oa-18efc5 | Convergence monitoring | → Not needed |
| oa-c2b296 | Dimensionality tracking | → Low priority |

### MERGE (Similar Tasks from TRM/HRM/SOAR):

| Merged Task | Source Tasks | Consolidation |
|-------------|--------------|---------------|
| Unified State Schema | TRM x,y,z + HRM zH,zL | Use TRM naming only |
| Deep Supervision | TRM + HRM both have this | Single implementation |
| EMA Stability | TRM + SOAR both use EMA | Single module |
| Skill Extraction | HRM segment-level + SOAR hindsight | Unified extractor |

---

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
**Goal**: TRM-style state + SOAR hindsight basics

1. Create `src/learning/trm-state.ts` - {x, y, z} schema
2. Create `src/learning/trm-halt.ts` - Simple binary halt
3. Create `src/learning/trm-ema.ts` - EMA for stability
4. Create `src/learning/soar-hindsight.ts` - Hindsight relabeler
5. Create `src/learning/soar-validation.ts` - Synthetic task validation
6. Update existing TrainingLoop to use TRM state

### Phase 2: Learning Infrastructure (Week 3-4)
**Goal**: Unified skill library with joint learning

7. Extend skill library schema with source tracking (direct/hindsight)
8. Create `src/learning/soar-selection.ts` - Greedy-diverse selection
9. Create `src/learning/soar-unified.ts` - Unified skill retrieval
10. Create `src/learning/soar-joint.ts` - Joint learning
11. Add `samplingSuccessRate` and `refinementSuccessRate` to skills
12. Implement deep supervision in training loop

### Phase 3: Iteration Loop (Week 5-6)
**Goal**: Self-improving SOAR loop

13. Create `src/learning/soar-loop.ts` - Full iteration loop
14. Add iteration tracking to skill metadata
15. Implement cross-iteration data pooling
16. Create `src/learning/trm-depth.ts` - Depth tracking

### Phase 4: Test-Time Training (Week 7)
**Goal**: Improve on Terminal-Bench without ground truth

17. Create `src/learning/soar-ttt.ts` - Test-time training
18. Implement training accuracy proxy for data selection
19. Implement pseudo-labeling with majority voting
20. Create `src/learning/soar-voting.ts` - Weighted majority voting

### Phase 5: Monitoring & Polish (Week 8)
**Goal**: Dashboards and diversity tracking

21. Create `src/dashboard/soar-progress.ts` - Iteration progress
22. Create `src/dashboard/soar-diversity.ts` - Diversity tracking
23. Add cross-strategy pooling for solution selection

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/learning/trm-state.ts` | TRM state schema {x, y, z} |
| `src/learning/trm-halt.ts` | Simple binary halt |
| `src/learning/trm-ema.ts` | EMA for stability |
| `src/learning/trm-depth.ts` | Depth tracking per task type |
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

## Success Criteria

1. **TRM State Working**: Training loop uses {x, y, z} representation
2. **Hindsight Learning Active**: Skill library grows from failed attempts
3. **Joint Learning Working**: Single library serves both sampling and refinement
4. **Iteration Improves**: Each SOAR iteration increases success rate
5. **TTT Works**: Improvement on Terminal-Bench without ground truth
6. **Diversity Maintained**: Solution diversity stays high on unsolved tasks
7. **Terminal-Bench Performance**: Measurable improvement in success rate

---

## Key Rationale

### Why TRM over HRM?
- **Empirical**: TRM achieves better results (44.6% vs 40.3% on ARC-AGI)
- **Simplicity**: 4x fewer parameters, single network vs two
- **Practical**: Binary halt is easier than Q-learning, works just as well

### Why SOAR is essential?
- **Hindsight Relabeling**: This is the key innovation. Every failed attempt becomes valid training data for a synthetic task. From 400 tasks, SOAR generates 2.4M training examples.
- **Self-Improvement**: Breaks scaling plateaus. A 7B model improved 2.5x (14% → 36%) through self-improvement alone.
- **Test-Time Training**: Critical for Terminal-Bench where we don't have ground truth labels.

### What to take from HRM?
- **Minimal**: The orchestrator/subagent split naturally creates a soft hierarchy
- **Monitoring**: Diversity tracking is valuable to prevent mode collapse
- **Otherwise**: TRM achieves HRM's goals with less machinery

---

## Existing Implementation Compatibility

The existing MechaCoder learning system (Phases 1-9) is **compatible**:

| Existing Component | Status | Notes |
|-------------------|--------|-------|
| `src/skills/` | Keep | Add source tracking, dual success rates |
| `src/memory/` | Keep | Use for SOAR iteration tracking |
| `src/reflexion/` | Keep | Complements hindsight learning |
| `src/archivist/` | Keep | Store SOAR search traces |
| `src/trainer/` | Keep | Gym becomes SOAR search environment |
| `src/learning/loop.ts` | Extend | Add TRM state, SOAR iteration |
| `src/learning/orchestrator.ts` | Extend | Add SOAR loop control |

**Key Changes**:
1. Add TRM state tracking to loop
2. Add hindsight relabeler for failed attempts
3. Wrap existing loop in SOAR iteration
4. Add test-time training for Terminal-Bench

---

## Task Operations Summary

**To Execute After Plan Approval:**

1. **Close 10 HRM tasks** (superseded by TRM):
   - Q-learning ACT tasks
   - Convergence detection
   - Dimensionality metrics
   - Some visualization tasks

2. **Keep 11 TRM tasks** (core architecture):
   - State schema
   - Simple halt
   - Deep supervision
   - EMA
   - Depth tracking

3. **Keep 17 SOAR tasks** (learning loop):
   - Hindsight relabeling (3)
   - Data selection (2)
   - Unified learning (3)
   - Iteration (3)
   - TTT (3)
   - Voting/diversity (3)

4. **Update task priorities**:
   - P1: Foundation tasks (TRM state, hindsight relabeler)
   - P1: Core SOAR loop
   - P2: Test-time training
   - P3: Monitoring dashboards

**Net Result**: 59 tasks → ~28 focused tasks

---

## User Decisions (from planning session)

1. **HRM Tasks**: Close completely with reason "superseded by TRM"
2. **Implementation Priority**: Foundation + TTT (TRM state schema AND test-time training)
3. **Documentation**: Archive old docs to `history/`, create new unified doc

---

## Execution Plan

### Step 1: Close HRM Tasks (10 tasks)
Close these tasks with reason "superseded by TRM - simpler architecture achieves better results":
- oa-ee0011 (HierarchicalState schema)
- oa-0becba (Nested cycle structure)
- oa-42d7d2 (Convergence detection)
- oa-3880bd (L-reset on H-update)
- oa-8090ef (Q-head for ACT)
- oa-ea7259 (Q-learning update)
- oa-b4324f (Dimensionality metrics)
- oa-e1141a (Intermediate prediction viz)
- oa-18efc5 (Convergence monitoring)
- oa-c2b296 (Dimensionality tracking)

### Step 2: Archive Documentation
Move to `history/learning-research/`:
- `docs/mechacoder/hrm-integration-tasks.md`
- `docs/mechacoder/trm-integration-tasks.md`
- `docs/mechacoder/soar-integration-tasks.md`

### Step 3: Create Unified Documentation
Create `docs/mechacoder/unified-learning-tasks.md` with the consolidated TRM+SOAR approach.

### Step 4: Update Task Priorities
- P0/P1: Foundation tasks (TRM state, simple halt, EMA)
- P0/P1: TTT tasks (test-time training, training accuracy proxy, majority voting)
- P2: Other SOAR tasks (hindsight, joint learning, iteration)
- P3: Dashboard/monitoring tasks

### Step 5: Create Work Log
Log the task triage decisions in `docs/logs/YYYYMMDD/HHMM-unified-learning-plan.md`
