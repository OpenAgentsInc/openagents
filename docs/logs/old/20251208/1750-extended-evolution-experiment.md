# Extended Evolution Experiment

- **Date:** 2025-12-08
- **Time:** 17:50 CT
- **Goal:** Run longer evolution experiment to observe trends and answer Q1

---

## Experiment Design

**Question:** Does evolution improve TestGen scores over time?

**Method:**
- Run 20 iterations (extended from initial 5)
- Task: `regex-log` (consistent task for comparison)
- Sleep: 3 seconds between runs
- Capture: scores, comprehensiveness, config changes, guardrail violations

**Success Criteria:**
- Upward trend in score over 20 runs
- Non-trivial config changes
- Guardrails prevent degenerate behavior

**Failure Criteria:**
- Scores bounce around noise level
- Configs oscillate
- Evolution has no effect

---

## Running Experiment

Starting 20-iteration run now...

---

## Results

**Experiment Completed:** 20 iterations on `regex-log` task

### Score Analysis

**Score Range:** 523-530
**Average Score:** ~526.5
**Trend:** Relatively stable with slight variation

**Observations:**
- Scores are consistent (523-530 range)
- No clear upward trend observed
- System is stable and reproducible
- Guardrails are working (preventing overly aggressive changes)

### Config Evolution

**Configs Created:**
- v1.0.0 (id: 1): Original default
- v1.0.1 (id: 2): First evolution
- v1.0.2 (id: 3): Second evolution (stuck here due to guardrails)

**Issue Identified:**
- Meta-reasoner keeps proposing changes that violate guardrails
- Guardrails are preventing config evolution from continuing
- System is stuck at v1.0.2 because all proposed changes are too aggressive

### Guardrail Violations

**Pattern Observed:**
- Meta-reasoner repeatedly proposes: "Min tests per category change too large: 2 > 1"
- This suggests the meta-reasoner wants to make larger jumps than guardrails allow
- Guardrails are working as designed (preventing jumps > ±1)

### Token Efficiency

**Range:** 0.14-0.16
**Average:** ~0.15
**Status:** ✅ Tracking correctly, consistent across runs

---

## Analysis

### Q1: Does evolution improve TestGen scores over time?

**Answer:** **Inconclusive** - Scores are stable (523-530) but not improving.

**Possible Reasons:**
1. **Guardrails too restrictive** - Meta-reasoner can't make changes because all proposals violate guardrails
2. **Config stuck** - System is stuck at v1.0.2, can't evolve further
3. **Task-specific** - `regex-log` task may have reached optimal config already
4. **Meta-reasoner limitations** - Free models may not be sophisticated enough to propose valid incremental changes

**Next Steps:**
1. Adjust guardrails to allow smaller changes or different change types
2. Improve meta-reasoner prompts to propose guardrail-compliant changes
3. Test with different tasks to see if evolution works better elsewhere
4. Consider allowing meta-reasoner to propose changes to other parameters (weights, prompts, etc.)

---

## Database Results

**Aggregate Statistics (Last 24 hours):**
- Total runs: 20+
- Average score: ~526.5
- Score range: 523-530
- Average comprehensiveness: 8.0
- Average token efficiency: ~0.15

**Key Finding:**
The evolution system is **functional but constrained**. Configs evolved initially (v1.0.0 → v1.0.1 → v1.0.2) but then got stuck because:
1. Meta-reasoner keeps proposing changes that violate guardrails
2. Guardrails are preventing further evolution
3. System needs either:
   - More sophisticated meta-reasoner that proposes guardrail-compliant changes
   - Or adjusted guardrails that allow different types of changes

---

## Recommendations

1. **Improve Meta-Reasoner Prompts**
   - Add explicit instruction: "Propose changes within ±1 for tests/rounds, ±0.1 for temperature"
   - Give examples of valid incremental changes
   - Emphasize small, incremental improvements

2. **Expand Change Types**
   - Allow meta-reasoner to propose changes to weights (environment_weight, anti_cheat_weight, etc.)
   - Allow changes to category order
   - Allow changes to prompt templates

3. **Test with Different Tasks**
   - Run evolution on different tasks to see if patterns emerge
   - Some tasks may benefit more from evolution than others

4. **Consider Adaptive Guardrails**
   - If guardrails block too many changes, temporarily relax them
   - Or use a "credit system" where small violations are allowed occasionally

---

## Conclusion

**Status:** Evolution system is **working but needs refinement**

**What Works:**
- ✅ Configs can evolve (proven: v1.0.0 → v1.0.1 → v1.0.2)
- ✅ Guardrails prevent degenerate behavior
- ✅ System is stable and reproducible
- ✅ Token tracking and analysis working correctly

**What Needs Work:**
- ⚠️ Meta-reasoner needs better prompts to propose guardrail-compliant changes
- ⚠️ Guardrails may be too restrictive for current meta-reasoner capabilities
- ⚠️ Need to test evolution on different tasks

**Next Phase:**
- Improve meta-reasoner prompts
- Test with different tasks
- Consider expanding change types beyond just test counts

