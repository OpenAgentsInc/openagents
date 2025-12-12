# Clean Validation Run: Standard Mode (10 turns)

**Time:** 15:05 CT
**Date:** 2025-12-09
**Mission:** Achieve 100% on regex-log with CLEAN validation (no hardcoded solutions)
**Mode:** Standard (10 turns, 15 min timeout)

---

## Context

The decomposer has been cleaned up (see `1454-decomposer-cleanup-no-cheating.md`). Previous 89.5% result is INVALID because FM was given the answer. This run tests whether FM can DISCOVER the correct regex through iteration against TestGen-generated tests.

**This is the true test of "architecture beats model size."**

---

## Test Configuration

- **Command:** `bun scripts/test-progress-fix.ts --standard`
- **Turns:** 10
- **Timeout:** 900s (15 min)
- **Task:** regex-log
- **Decomposer:** Clean (no hardcoded solutions, only domain knowledge)

---

## Execution

Starting test run...

---

## CRITICAL ISSUE DISCOVERED: Docker Not Available

**Problem:** The test run failed because Docker is not running, but the system kept trying to use Docker anyway, resulting in 0/0 tests and 0% progress.

### What Happened

1. **TestGen generated 28 tests successfully** ✅
2. **FM wrote initial regex** ✅ (`r'(?<=.*\d{1,3}-\d{2}-\d{4})\d{1,3}-\d{2}-\d{4}'`)
3. **Every verification attempt failed** ❌:
   - Docker daemon not running
   - System tried Docker → failed → tried fallback image → still tried Docker → failed
   - Result: `0/0 tests` → `0.0% progress`
4. **FM kept iterating** but couldn't get feedback because no tests ran
5. **Wasted 10 turns** with no progress

### Root Cause

The `evaluator.ts` function `runVerificationWithDocker` has NO fallback mechanism. When Docker isn't available:
- It tries Docker → fails with exit code 125
- Returns `0/0 tests` and `0% progress`
- FM gets no feedback about what's wrong with the regex
- System continues wasting turns

### Evidence from Log

```
[TB2] Cannot connect to the Docker daemon at unix:///Users/christopherdavid/.docker/run/docker.sock. Is the docker daemon running?
[TB2] Using fallback image: python:3.11-slim
[TB2] Docker exitCode: 125
[TB2] WARNING: No tests discovered! Full output:
docker: Cannot connect to the Docker daemon...
[MAP] Progress: 0.0% (0/0 tests)
```

This pattern repeated **4 times** during the run (turns 2, 4, 8, 10).

### Solution Needed

Add a fallback to run tests locally using `python3 -m pytest` when Docker is not available. The codebase already has this pattern in `src/cli/tbench-local.ts`:

```typescript
const proc = Bun.spawn(["python3", "-m", "pytest", "tests/", "-v", "--tb=short"], {
  cwd: workspaceDir,
  stdout: "pipe",
  stderr: "pipe",
  env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
});
```

### Next Steps

1. ✅ Fix `evaluator.ts` to detect Docker availability
2. ✅ Fall back to local pytest when Docker unavailable
3. Re-run the test with the fix applied

---

## Fix Applied

**File:** `src/hillclimber/evaluator.ts`

**Changes:**
1. Added `isDockerAvailable()` function to check if Docker daemon is running
2. Added `runLocalPytest()` function to run tests locally using `python3 -m pytest`
3. Modified `runVerificationWithDocker()` to:
   - Check Docker availability first
   - Fall back to local pytest if Docker unavailable
   - Catch Docker errors and fall back to local pytest

**Code added:**
- `isDockerAvailable()` - Checks `docker --version` and `docker info` to verify daemon is running
- `runLocalPytest()` - Runs `python3 -m pytest tests/ -v --tb=short` locally
- Error handling in `runVerificationWithDocker()` to catch Docker failures and fall back

**Result:** System will now run tests locally when Docker is not available, giving FM proper feedback to iterate on the regex.
