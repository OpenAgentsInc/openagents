# 2325 Work Log - Unified Learning Plan and Task Triage

## Summary

Consolidated HRM/TRM/SOAR research into a single unified path: **TRM (Architecture) + SOAR (Learning Loop)**.

## Key Decision

After deep analysis of all three ARC-AGI papers:
- **TRM wins architecturally**: Simpler (7M vs 27M params), better results (44.6% vs 40.3% ARC-AGI, 87.4% vs 55% Sudoku)
- **SOAR provides meta-learning**: Hindsight relabeling turns failures into training data
- **HRM is superseded**: TRM achieves HRM's goals with less complexity

## Task Triage

### Closed (15 HRM tasks)

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

### Priority Updates (P2 → P1)

| Task ID | Title |
|---------|-------|
| oa-6499e1 | Add EMA for success rate tracking |
| oa-34d337 | Implement hindsight relabeler |
| oa-5a2c82 | Add synthetic skill storage |
| oa-541678 | Implement structural validation |
| oa-87fdbe | Add training accuracy proxy |
| oa-723382 | Implement pseudo-labeling |
| oa-5679bd | Implement weighted majority voting |

## Documentation Changes

### Archived to `history/learning-research/`
- `hrm-integration-tasks.md`
- `trm-integration-tasks.md`
- `soar-integration-tasks.md`
- `trm-vs-hrm-comparison.md`
- `soar-self-improving-synthesis.md`

### Created
- `docs/mechacoder/unified-learning-tasks.md` - Consolidated task documentation

## Task Counts

| Before | After |
|--------|-------|
| 59 research tasks (18 HRM + 21 TRM + 20 SOAR) | ~31 focused tasks |
| Many overlapping/redundant | Clean TRM + SOAR path |

## Priority Structure

| Priority | Tasks | Description |
|----------|-------|-------------|
| P0/P1 | ~20 | Foundation (TRM) + TTT (SOAR) |
| P2 | ~8 | Other SOAR (joint learning, iteration) |
| P3 | ~3 | Dashboards, monitoring |

## Architecture Summary

```
SOAR Outer Loop (Learning)
├── Hindsight Relabeling (failures → training data)
├── Greedy-Diverse Selection
├── Joint Sample+Refine Learning
├── Iteration (Search → Learn → Repeat)
└── Test-Time Training (critical for TB)

TRM Inner Loop (Single Attempt)
├── State: {x, y, z}
├── Simple Binary Halt
├── Deep Supervision
└── EMA Stability
```

## Next Steps

1. Implement TRM state schema (`src/learning/trm-state.ts`)
2. Implement hindsight relabeler (`src/learning/soar-hindsight.ts`)
3. Implement test-time training (`src/learning/soar-ttt.ts`)
4. Wire to existing MechaCoder learning system
