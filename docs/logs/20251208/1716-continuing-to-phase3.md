# Continuing to Phase 3: Evolution Experiment

- **Date:** 2025-12-08
- **Time:** 17:16 CT
- **Status:** Committed and pushed, continuing with Phase 3

---

## Commit Summary

**Commit:** Fixed TestGen HillClimber bugs and added guardrails

**Changes:**
- Phase 1: All bug fixes (token efficiency, trajectory timing, rate limits, token tracking)
- Phase 2: All guardrails (delta caps, hard minimums, token limits)

**Status:** ✅ Committed and pushed successfully

---

## Phase 3: First Evolution Experiment

**Goal:** Run 50-iteration evolution and capture data to answer Q1: "Does evolution improve TestGen scores over time?"

**Command:**
```bash
bun run src/hillclimber/test-gen-cli.ts --evolve \
  --max-runs 50 \
  --task regex-log \
  --sleep 5000
```

**What to capture:**
- Score vs run index (plot)
- Comprehensiveness vs run index (plot)
- Config changes over time
- Guardrail violations (if any)
- Token efficiency trends

**Success Criteria:**
- Clear upward trajectory in score over 50 runs
- Non-trivial config changes
- Guardrails prevent degenerate behavior

**Failure Criteria:**
- Scores bounce around noise level
- Configs oscillate
- Evolution has no effect

---

## Next Actions

1. Run the 50-iteration evolution experiment
2. Capture and analyze the data
3. Create research document with results
4. Determine if evolution is working or needs adjustment

---

Starting evolution experiment now...

---

## Initial Test Run (5 iterations)

**Command Executed:**
```bash
bun run src/hillclimber/test-gen-cli.ts --evolve --max-runs 5 --task regex-log --sleep 3000
```

**Results:**
- All 5 runs completed successfully
- Scores: 527, 527, 527, 527, 526 (very stable)
- Guardrails working: Caught violations "Min tests per category change too large: 2 > 1"
- Configs created: v1.0.1, v1.0.2 (guardrails preventing some changes)

**Observations:**
- Scores are stable around 526-527 (good - system is consistent)
- Guardrails are preventing overly aggressive changes
- Meta-reasoner is proposing changes but guardrails are filtering them
- Token efficiency: ~0.14 (working correctly)

**Next Steps:**
- Run longer experiment (50 iterations) to see if trends emerge
- Check if guardrails are too restrictive (preventing all changes)
- Analyze config evolution history to see what changes are being proposed

---

## Database Query Results

**Recent Runs:**
```
Run 1: score=527, comprehensiveness=8.0, balance=0.89, efficiency=0.14, tokens=~5,200
Run 2: score=527, comprehensiveness=8.0, balance=0.89, efficiency=0.14, tokens=~5,200
Run 3: score=527, comprehensiveness=8.0, balance=0.89, efficiency=0.14, tokens=~5,200
Run 4: score=527, comprehensiveness=8.0, balance=0.89, efficiency=0.14, tokens=~5,200
Run 5: score=526, comprehensiveness=8.0, balance=0.89, efficiency=0.14, tokens=~5,200
```

**Configs:**
- v1.0.0 (id: 1) - Original default config
- v1.0.1 (id: 2) - First evolution attempt
- v1.0.2 (id: 3) - Second evolution attempt (guardrails preventing changes)

**Analysis:**
- Scores are very stable (526-527) - this is actually good (consistent system)
- Guardrails are working but may be too restrictive
- Meta-reasoner is trying to make changes but guardrails are blocking them
- Need to check if guardrails are preventing ALL changes or just overly aggressive ones

---

## Continuing...

System is working correctly. Ready to run longer experiment or adjust guardrails if needed.

