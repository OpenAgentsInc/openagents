# Fix: Python String Literal Bug

**Date:** 2025-12-09
**Time:** 1218 CT

---

## Bug Description

The pytest "0/0 tests" issue was caused by Python syntax errors in generated test files. The LLM outputs `expectedOutput` with quotes already included (e.g., `"2023-10-01"`), and then `pythonStringLiteral()` wraps it in more quotes, creating invalid Python:

```python
# BROKEN (4 quotes)
expected = ""2023-10-01""

# EVEN WORSE (8 quotes for empty string)
expected = """"""
```

This causes a Python syntax error that prevents pytest from loading the file at all.

---

## Root Cause

1. LLM outputs: `"2023-10-01"` (string value with embedded quotes)
2. `pythonStringLiteral()` wraps it: `""2023-10-01""`
3. Result: Python syntax error → pytest can't load file → "0/0 tests"

---

## Fix Applied

**File:** `src/hillclimber/testgen-to-pytest.ts`

Added quote stripping before calling `pythonStringLiteral()`:

```typescript
// CRITICAL: Strip any leading/trailing quotes that LLM may have added
let expectedValue = test.expectedOutput;
if (typeof expectedValue === "string") {
  // Remove surrounding quotes if LLM added them (e.g., '"2023-10-01"' -> '2023-10-01')
  expectedValue = expectedValue.replace(/^["']+|["']+$/g, "");
}
lines.push(`    expected = ${pythonStringLiteral(expectedValue)}`);
```

---

## Validation

After fix, generated Python should look like:

```python
# CORRECT
expected = "2023-10-01"

# For null/no-match cases:
assert len(matches) == 0, f"Expected no match, but got {matches}"
```

---

## Status

✅ Fix implemented
⏳ Need to test with new test generation

