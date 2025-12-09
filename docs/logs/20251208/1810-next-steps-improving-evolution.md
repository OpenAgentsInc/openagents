# Next Steps: Improving Evolution System

- **Date:** 2025-12-08
- **Time:** 18:10 CT
- **Context:** After 20-iteration experiment, evolution is working but needs refinement

---

## Current Status

**What's Working:**
- ✅ Evolution system functional (configs can evolve)
- ✅ Guardrails preventing degenerate behavior
- ✅ Token tracking and analysis working
- ✅ System stable and reproducible

**What Needs Improvement:**
- ⚠️ Meta-reasoner proposes changes that violate guardrails
- ⚠️ System stuck at v1.0.2 (can't evolve further)
- ⚠️ No clear score improvement trend observed

---

## Action Plan

### Step 1: Improve Meta-Reasoner Prompts

**Problem:** Meta-reasoner keeps proposing changes like "min_tests: 2 → 4" which violates the ±1 guardrail.

**Solution:** Update prompt to explicitly instruct guardrail compliance.

**Implementation:**
- Add guardrail constraints to meta-reasoner prompt
- Provide examples of valid incremental changes
- Emphasize small, incremental improvements

**File:** `src/hillclimber/testgen-meta-reasoner.ts` - `buildTestGenMetaPrompt`

---

### Step 2: Expand Change Types

**Problem:** Meta-reasoner only proposes changes to test counts, which hit guardrails.

**Solution:** Allow meta-reasoner to propose changes to other parameters.

**New Change Types:**
- Environment weight (0-1)
- Anti-cheat weight (0-1)
- Precision weight (0-1)
- Category order (reordering)
- Temperature (within ±0.1)

**Implementation:**
- Update `TestGenConfigChange` type to include more change types
- Update meta-reasoner prompt to suggest these changes
- Update `applyConfigChange` to handle new change types

---

### Step 3: Test with Different Tasks

**Problem:** Only tested on `regex-log` task.

**Solution:** Run evolution on diverse tasks to see if patterns emerge.

**Tasks to Test:**
- Different task types (conversion, implementation, debugging)
- Tasks with different complexity levels
- Tasks with different environment requirements

---

### Step 4: Add Adaptive Guardrails (Optional)

**Problem:** Guardrails may be too restrictive for current meta-reasoner.

**Solution:** Implement adaptive guardrails that relax slightly if too many changes are blocked.

**Approach:**
- Track guardrail violation rate
- If violation rate > threshold, temporarily allow slightly larger changes
- Or use "credit system" where occasional violations are allowed

---

## Immediate Next Steps

1. **Improve meta-reasoner prompt** - Add guardrail constraints and examples
2. **Test improved prompt** - Run 10 iterations to see if evolution continues
3. **Expand change types** - Allow weight and temperature changes
4. **Test on different tasks** - Run evolution on 2-3 different tasks

---

## Implementation Priority

**High Priority:**
1. Improve meta-reasoner prompts (quick win)
2. Expand change types (enables more evolution paths)

**Medium Priority:**
3. Test with different tasks (validates generalization)

**Low Priority:**
4. Adaptive guardrails (complex, may not be needed)

---

Starting with Step 1: Improving meta-reasoner prompts...

