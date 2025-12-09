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

