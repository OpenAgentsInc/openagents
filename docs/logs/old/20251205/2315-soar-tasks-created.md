# 2315 Work Log - SOAR Tasks Created

## Summary

Added 20 SOAR (Self-Improving Language Models for Evolutionary Program Synthesis) integration tasks to `.openagents/tasks.jsonl`.

## Tasks Created

### Phase 1: Hindsight Learning Infrastructure (P2)
- `oa-34d337` - Implement hindsight relabeler
- `oa-5a2c82` - Add synthetic skill storage
- `oa-541678` - Implement structural validation

### Phase 2: Data Selection (P1-P2)
- `oa-94ca90` - Implement greedy-diverse selection (P1)
- `oa-5b4f0d` - Add accuracy binning for refinement data (P2)

### Phase 3: Unified Learning (P1-P2)
- `oa-e3d4a2` - Create unified skill library (P1)
- `oa-7fb8ac` - Implement joint learning (P1)
- `oa-ff160d` - Track dual success rates (P2)

### Phase 4: Iterated Self-Improvement (P1-P2)
- `oa-f04075` - Implement SOAR iteration loop (P1)
- `oa-a6685f` - Add iteration tracking (P2)
- `oa-13665d` - Implement cross-iteration data pooling (P2)

### Phase 5: Test-Time Training (P1-P2)
- `oa-c559bd` - Implement test-time training (P1)
- `oa-87fdbe` - Add training accuracy proxy (P2)
- `oa-723382` - Implement pseudo-labeling with majority voting (P2)

### Phase 6: Voting and Ensembling (P2)
- `oa-5679bd` - Implement weighted majority voting
- `oa-c7c037` - Add solution diversity tracking
- `oa-fa9395` - Implement cross-strategy pooling

### Phase 7: Monitoring and Analysis (P3)
- `oa-8d2ce3` - Add iteration progress dashboard
- `oa-04f44d` - Track diversity over iterations
- `oa-aed3f9` - Add scaling analysis tools

## Priority Breakdown
- **P1 (High):** 5 tasks - Core SOAR loop, joint learning, TTT
- **P2 (Medium):** 12 tasks - Infrastructure, data selection, ensembling
- **P3 (Low):** 3 tasks - Dashboard and monitoring

## Key SOAR Concepts

| Concept | Implementation |
|---------|----------------|
| Hindsight relabeling | Turn failures into valid (synthetic task, solution) pairs |
| Greedy-diverse selection | Top 25 + bottom 25 attempts per task |
| Joint learning | Single skill library for sampling + refinement |
| Iteration loop | Search → Learn → Repeat |
| Test-time training | Improve without ground truth |
| Weighted voting | weight = count + 1000 × training_accuracy |

## Combined Task Totals

| Source | Tasks |
|--------|-------|
| HRM | 18 |
| TRM | 21 |
| SOAR | 20 |
| **Total** | **59** |

## Documentation

- SOAR analysis: `docs/research/analysis/soar-self-improving-synthesis.md`
- SOAR tasks: `docs/mechacoder/soar-integration-tasks.md`
- TRM vs HRM comparison: `docs/research/analysis/trm-vs-hrm-comparison.md`

## How They Complement Each Other

| Approach | Focus |
|----------|-------|
| TRM/HRM | Structure single reasoning attempts |
| SOAR | Learn and improve across multiple attempts |

TRM/HRM define HOW to reason on a single task.
SOAR defines HOW to learn from ALL attempts (successes AND failures).

## Next Steps

1. Implement shared infrastructure first (EMA, deep supervision)
2. Start with TRM-style simplified state (x, y, z)
3. Add SOAR hindsight relabeling to convert failures to training data
4. Build iteration loop that compounds improvements
