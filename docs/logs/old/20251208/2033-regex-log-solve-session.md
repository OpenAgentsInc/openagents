# regex-log Solve Session

- **Date:** 2025-12-08
- **Time:** 20:33 CT
- **Goal:** Get regex-log task solving (9/9 tests) in < 15 turns

---

## Key Insight from User

**Cannot hardcode `/app/` → workspace replacement in evaluator.**

Per docs/logs/20251208/1219-benchmark-gaming-analysis.md and 1224-blind-verification-redesign.md:
- That would be gaming the benchmark
- The architecture requires:
  1. TestGen generates tests from description ONLY (blind to real tests)
  2. FM iterates on SELF-GENERATED tests
  3. Blind verification at end returns ONLY pass/fail

**Problem:** Real TB2 tests require Docker (they expect `/app/` to exist).

---

## Solution: Docker-Based Verification

TB2 provides Dockerfile per task. We need:
1. Build Docker image from TB2 task Dockerfile
2. Mount workspace to `/app/` inside container
3. Run `pytest tests/ -v` inside container
4. Return pass/fail only (blind verification)

---

## Step 1: Create Docker-Based TB2 Runner

Creating `src/bench/tb2-docker-runner.ts`...

