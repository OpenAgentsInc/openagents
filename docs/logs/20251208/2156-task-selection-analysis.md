# TB2 Task Selection Analysis

**Date:** 2025-12-08
**Time:** 21:56 CT
**Goal:** Find appropriate task for validating HillClimber solving capability

---

## Problem Statement

**regex-log is too hard:**
- Requires 383-char expert-level regex
- IPv4 + date validation in single pattern  
- No FM can solve without massive help

**Need:** Task that's genuinely solvable but still meaningful for validation

---

## Analyzing TB2 Task Difficulties


## Task Comparison

### regex-log (medium)
- **Solution:** 1 line, 383-char monster regex
- **Requires:** Expert regex with IPv4/date validation
- **Solvability:** Nearly impossible for FM without massive help

### openssl-selfsigned-cert (medium)
- **Solution:** 98 lines of bash + Python
- **Requires:** Standard openssl commands, file creation, Python script
- **Solvability:** Much more achievable - procedural steps

### log-summary-date-ranges (medium)
- **Solution:** 71 lines of bash/Python
- **Requires:** File parsing, date logic, CSV output
- **Solvability:** Moderate - clear data processing task

## Recommendation

**Switch to `openssl-selfsigned-cert` for HillClimber validation:**

1. Clear, procedural steps
2. Standard tools (openssl, cat, chmod)
3. Verifiable outputs (files exist, permissions correct)
4. Reference solution is straightforward
5. Much more realistic for FM to solve with decomposition

**Next Steps:**
1. Create e2e test for openssl-selfsigned-cert
2. Run HillClimber with MAP orchestrator
3. Validate solving capability
4. If successful, move to harder tasks

---

## Infrastructure Status: VALIDATED ✅

Docker support is **100% working**:
- Task-specific images ✅
- Python/pytest execution ✅
- Test result parsing ✅
- Blind verification ✅

The problem was task selection, not infrastructure.

