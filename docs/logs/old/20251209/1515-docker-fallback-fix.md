# Docker Fallback Fix: Enable Local Pytest When Docker Unavailable

**Time:** 15:15 CT
**Date:** 2025-12-09
**Issue:** System kept trying Docker even when not available, resulting in 0/0 tests
**Fix:** Added local pytest fallback in evaluator

---

## Problem

During the clean validation run (`1505-clean-validation-run-standard.md`), the system:
1. Generated 28 tests successfully ✅
2. FM wrote regex ✅
3. **Every verification attempt failed** ❌ because Docker wasn't running
4. System returned `0/0 tests` → `0% progress`
5. FM got no feedback and wasted 10 turns

### Root Cause

`src/hillclimber/evaluator.ts` function `runVerificationWithDocker()` had no fallback mechanism. When Docker failed:
- Returned `0/0 tests` and `0% progress`
- FM couldn't iterate because no test feedback
- System continued wasting turns

---

## Solution

Added Docker availability detection and local pytest fallback.

### Changes to `src/hillclimber/evaluator.ts`

1. **`isDockerAvailable()` function:**
   - Checks `docker --version` (CLI exists)
   - Checks `docker info` (daemon is running)
   - Respects `OPENAGENTS_DOCKER_AVAILABLE` env var

2. **`runLocalPytest()` function:**
   - Runs `python3 -m pytest tests/ -v --tb=short` locally
   - Parses output using existing `parsePytestOutput()`
   - Returns same format as Docker runner

3. **Updated `runVerificationWithDocker()`:**
   - Checks Docker availability first
   - Falls back to local pytest if Docker unavailable
   - Catches Docker errors and falls back gracefully
   - Logs fallback decisions for debugging

### Code Pattern

```typescript
// Check Docker availability
const dockerAvailable = isDockerAvailable();

if (!dockerAvailable) {
  console.log(`[TB2] Docker not available, falling back to local pytest`);
  return await runLocalPytest(workspace);
}

try {
  // Try Docker
  const result = await runTB2InDocker(...);
  return result;
} catch (dockerError) {
  // Fall back on error
  console.log(`[TB2] Docker verification failed, falling back to local pytest`);
  return await runLocalPytest(workspace);
}
```

---

## Impact

**Before:**
- Docker unavailable → `0/0 tests` → `0% progress` → FM stuck

**After:**
- Docker unavailable → Local pytest → Real test results → FM can iterate

---

## Testing

To verify the fix works:

1. **Without Docker:**
   ```bash
   # Stop Docker
   # Run test
   bun scripts/test-progress-fix.ts --standard
   # Should see: "[TB2] Docker not available, falling back to local pytest"
   # Should get real test results (not 0/0)
   ```

2. **With Docker:**
   ```bash
   # Start Docker
   # Run test
   bun scripts/test-progress-fix.ts --standard
   # Should use Docker as before
   ```

---

## Next Steps

1. ✅ Fix applied
2. Re-run clean validation test to verify FM can now iterate
3. Document results in new log file

---

**Status:** Fix complete, ready for testing
