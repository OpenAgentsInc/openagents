# Continuation: Debugging Pytest Discovery Issue

**Date:** 2025-12-09
**Time:** 11:58 CT
**Session:** Continuing from 1141-testgen-edge-case-extraction.md

---

## Current Status

### Completed Work
- ✅ TestGen edge case extraction implemented
- ✅ Combined IP+Date boundary tests
- ✅ "null" string → null value parsing fix
- ✅ 12 unit tests passing
- ✅ TestGen generates 31 tests (up from 15)

### Issue Identified
Integration test shows "0/0 tests" despite generating 31 valid tests. This suggests pytest isn't discovering the test file in the Docker container.

---

## Investigation Plan

### Hypothesis 1: Test File Location
- TestGen writes to `workspace/tests/test_outputs.py`
- Docker runner copies workspace to `/app/` in container
- Pytest runs `python3 -m pytest tests/ -v` from `/app/`
- **Expected:** Tests should be at `/app/tests/test_outputs.py`
- **Check:** Verify file exists in container

### Hypothesis 2: Test File Format
- Generated tests use `def test_*()` functions
- Pytest should discover these automatically
- **Check:** Verify no syntax errors in generated file

### Hypothesis 3: Pytest Discovery
- Pytest might need explicit path or different invocation
- **Check:** Try `pytest tests/test_outputs.py` explicitly

### Hypothesis 4: Working Directory
- Pytest might be running from wrong directory
- **Check:** Ensure pytest runs from `/app/` (already set with `-w /app`)

---

## Fixes Implemented

### 1. Explicit Test File Path
**File:** `src/bench/tb2-docker-runner.ts`

Changed pytest invocation to try explicit file path first:
```bash
# Before
python3 -m pytest tests/ -v 2>&1

# After
if [ -f tests/test_outputs.py ]; then
  python3 -m pytest tests/test_outputs.py -v 2>&1
else
  python3 -m pytest tests/ -v 2>&1
fi
```

### 2. Enhanced Debug Logging
Added debug output to see:
- Test file location check (`ls -la tests/`)
- Which pytest command is being used
- Full output when no tests discovered (0/0)

### 3. Better Error Reporting
When `parsed.total === 0`, now logs full pytest output instead of just first 500 chars.

---

## Next Steps

1. ✅ **Added better error logging** - Done
2. ⏳ **Run test to verify fix** - Next
3. ⏳ **Fix any remaining discovery issues** - If needed
4. ⏳ **Run full integration test** - After pytest works

---

## Files Modified

- `src/bench/tb2-docker-runner.ts` - Enhanced pytest discovery and debugging

---

## Summary

**Status:** Pytest discovery improvements implemented, ready for testing

**Changes:**
1. ✅ Try explicit test file path (`tests/test_outputs.py`) before directory
2. ✅ Add debug logging to see test file location
3. ✅ Log full output when no tests discovered (0/0)

**Next:** Run quick test to verify pytest discovery works, then proceed with full integration test.

---

**Session End:** 11:58 CT
