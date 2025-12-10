# TestGen Analysis: regex-log Task (Post-Formatter Fix)

**Date:** 2024-12-10 11:09
**Task:** `regex-log` from Terminal-Bench 2
**Tests Generated:** 25
**Duration:** 48.5 seconds
**Tokens Used:** 5,083

---

## What Changed Since Last Analysis

The smart pytest formatter from Phase 1 is now deployed. The previous analysis (1006) identified that ALL tests were stubs with `pass # TODO`. This run uses the new formatter that generates real assertions based on test data.

---

## Test Generation Output

```
=== TB2 Task: regex-log ===
[category_generation anti_cheat round 1] Generating anti_cheat tests
  + anti_cheat_1: 192.168.1.1 2023-01-15 12345 -> ['2023-01-15']
  + anti_cheat_2: 10.0.0.1 2023-01-15 2023-02-28 56789 -> ['2023-02-28']
  + anti_cheat_3: 172.16.0.1 2023-02-29 2023-03-01 98765 -> ['2023-02-29']
  + anti_cheat_4: 192.168.0.1 2023-01-01 2023-01-15 111222 -> ['2023-01-15']
  + anti_cheat_5: 8.8.8.8 2023-02-29 2023-03-01 222333 -> ['2023-02-29']

[category_generation existence round 1] Generating existence tests
  + existence_1: 192.168.1.1 2023-01-15 2023-02-28 -> ['2023-01-15', '2023-02-28']
  + existence_2: 10.0.0.1 2023-03-10 2023-03-11 2023-03-12 -> ['2023-03-12']
  + existence_3: 172.16.0.1 2023-04-01 2023-04-02 2023-04-03 2023-04-04 -> ['2023-04-04']
  + existence_4: 198.51.100.1 2023-05-10 ... 2023-05-14 -> ['2023-05-14']
  + existence_5: 8.8.8.8 2023-06-15 ... 2023-06-19 -> ['2023-06-19']

[category_generation correctness round 1] Generating correctness tests
  + correctness_1: 192.168.1.1 2023-01-15 -> ['2023-01-15']
  + correctness_2: 10.0.0.1 2023-02-28 2023-03-01 -> ['2023-02-28']
  + correctness_3: 8.8.8.8 2023-02-29 2023-03-01 2023-03-02 -> ['2023-02-29']
  + correctness_4: 172.16.0.1 2023-01-15 2023-02-28 2023-03... -> ['2023-02-28']
  + correctness_5: 192.168.0.1 2023-02-29 2023-03-01 2023-0... -> ['2023-02-29']

[category_generation boundary round 1] Generating boundary tests
  + boundary_1: 192.168.1.1 2023-01-15 -> ['2023-01-15']
  + boundary_2: 192.168.1.1 2023-01-15 2023-02-29 -> ['2023-01-15']  # WRONG!
  + boundary_3: 192.168.1.1 2023-02-28 2023-03-01 -> ['2023-02-28']  # WRONG!
  + boundary_4: 192.168.1.1 2023-03-01 2023-03-31 2023-0... -> ['2023-03-30']
  + boundary_5: (empty) -> []

[category_generation integration round 1] Generating integration tests
  + integration_1: 192.168.1.1 2023-01-15 2023-02-28 -> ['2023-02-28']
  + integration_2: 192.168.1.1 2023-01-15 2023-02-29 2023-0... -> ['2023-02-29']
  + integration_3: 192.168.1.1 2023-01-15 2023-02-28 2023-0... -> ['2023-03-02']
  + integration_4: 192.168.1.1 2023-01-15 2023-02-28 2023-0... -> ['2023-03-03']
  + integration_5: 192.168.1.1 2023-01-15 2023-02-28 2023-0... -> ['2023-03-04']

=== Generation Complete ===
Total tests: 25
Total rounds: 5
Duration: 48521ms
```

---

## Critical Finding: File Path Gap

### The Problem

The new formatter correctly refuses to hardcode `/app/regex.txt` (per anti-cheating policy). Instead, it looks for file paths in the test data (input/reasoning fields). When no path is found, tests now generate `pytest.skip("Missing regex file path in test data")`.

**Result:** 15 out of 25 tests are skipped:
- All 5 correctness tests: SKIPPED
- All 5 boundary tests: SKIPPED
- All 5 integration tests: SKIPPED
- 5 existence tests: PASS (but stubs - no path data)
- 5 anti-cheat tests: PASS (conceptual checks only)

### Why This Happens

The FM generates test inputs like:
```
"input": "192.168.1.1 2023-01-15"
"reasoning": "Tests regex with single date and IPv4"
```

But the formatter needs:
```
"input": "192.168.1.1 2023-01-15"
"reasoning": "Tests regex from /app/regex.txt with single date and IPv4"
```

The task description DOES mention `/app/regex.txt` on line 9:
> Save your regex in /app/regex.txt

But the FM isn't propagating this context into the test data fields. The formatter can't find it.

### This Is NOT an Anti-Cheating Violation

The file path `/app/regex.txt` comes from the task description itself, not TB2 knowledge. It's legitimate for the FM to:
1. Read the task description
2. Extract "output goes to /app/regex.txt"
3. Include this in test reasoning
4. Formatter then uses this to generate assertions

The problem is the FM prompt doesn't instruct the FM to do this.

---

## What the Formatter Produced

### Anti-Cheat Tests (5/5 PASS - Conceptual)

```python
def test_anti_cheat_1():
    """This test checks if the regex correctly identifies the last date..."""
    # Anti-cheat check: 192.168.1.1 2023-01-15 12345
    # Verify no prohibited tools/patterns were used
    pass  # Anti-cheat validation happens at runtime
```

These are intentionally pass stubs. Anti-cheat tests validate the PROCESS, not the output. They pass unless the evaluator detects cheating.

### Existence Tests (5/5 PASS - Weak)

```python
def test_existence_1():
    """This test ensures the regex correctly identifies the last date..."""
    # Input: 192.168.1.1 2023-01-15 2023-02-28
    # This test verifies existence of expected outputs
    # Unable to determine specific file path from test data
    pass  # TODO: Implement specific check
```

Fallback to pass stub because no file path was found. These don't actually test anything.

### Correctness/Boundary/Integration Tests (15/15 SKIPPED)

```python
def test_correctness_1():
    """This test ensures the regex correctly identifies the last date..."""
    # Test: This test ensures the regex correctly identifies...
    # Unable to determine regex file path from test data
    pytest.skip("Missing regex file path in test data")
```

Correctly skipped rather than hardcoding `/app/regex.txt`. This is the anti-cheating fix working as intended.

---

## Comparison: Before vs After Formatter Fix

| Aspect | Before (1006) | After (1109) |
|--------|---------------|--------------|
| Test bodies | All `pass # TODO` | Mixed: skip/pass based on data |
| File path handling | Hardcoded `/app/regex.txt` | Data-driven, skip if missing |
| Correctness tests | Always pass (wrong!) | Skip (correct behavior) |
| Anti-cheat policy | Violated | Compliant |
| Useful feedback | None | None (blocked by prompt gap) |

### Progress

The formatter now correctly implements anti-cheating. But usefulness is blocked by the FM prompt not extracting output file paths from task descriptions.

---

## Root Cause: Prompt Engineering Gap

The FM prompt in `generator.rs` tells the FM:
1. Generate test inputs (concrete data)
2. Generate expected outputs
3. Include reasoning

But it does NOT tell the FM:
1. Extract output file location from task description
2. Include file path in reasoning field
3. Understand that tests need to EXECUTE against a file

### The Fix

Update FM prompt to include:
```
## CRITICAL: Output File Context
- Read the task description for WHERE the solution should be saved
- Extract phrases like "Save to /app/...", "Write to /path/..."
- Include this path in your test reasoning
- Example: "Tests regex from /app/regex.txt with multiple dates"

Without the output file path, tests cannot execute properly.
```

This is legitimate because:
1. File path comes from task description (not TB2 knowledge)
2. FM naturally knows this context but isn't propagating it
3. Tests genuinely need to know where to read the solution

---

## FM Test Quality Analysis

### Good: Concrete Test Data

The FM now generates concrete inputs, not descriptions:

**Before (1006):**
```
"input": "test input with multiple dates and IPv4"
```

**After (1109):**
```
"input": "192.168.1.1 2023-01-15 2023-02-28"
```

This is the Phase 2 prompt engineering working correctly.

### Good: Correct Expected Outputs

Most expected outputs are properly formatted JSON arrays:
```
["2023-01-15"]
["2023-02-28"]
```

### Bad: Boundary Test Logic Errors

Some boundary tests have wrong expected outputs:

```
boundary_2: "192.168.1.1 2023-01-15 2023-02-29" -> ['2023-01-15']
# WRONG! Should be ['2023-02-29'] (last date)

boundary_3: "192.168.1.1 2023-02-28 2023-03-01" -> ['2023-02-28']
# WRONG! Should be ['2023-03-01'] (last date)
```

The FM is confusing "last date" with "first date" in some cases. This may indicate the FM doesn't fully understand the requirement.

### Neutral: Existence Test Outputs

```
existence_1: "192.168.1.1 2023-01-15 2023-02-28" -> ['2023-01-15', '2023-02-28']
```

This expected output is WRONG for the regex task (should only match last date), but existence tests don't actually run assertions, so it doesn't matter. The test category semantics are confused.

---

## Required Fixes

### Fix 1: FM Prompt - Output File Extraction (Priority 1)

Update `crates/testgen/src/generator.rs` prompts to instruct FM to:
1. Extract output file path from task description
2. Include path in reasoning field
3. Understand test execution context

Example addition to prompt:
```
## Output File Context
The task description specifies where the solution will be saved. Look for:
- "Save your ... in /app/..."
- "Write to /path/..."
- File paths mentioned in example code

Include this path in your test reasoning. Example:
  "reasoning": "Tests regex from /app/regex.txt with single date and IPv4"

Tests need this information to know where to read the solution.
```

### Fix 2: Boundary Test Category Semantics (Priority 2)

The FM is generating boundary tests with wrong expected outputs. Add clarifying prompt:
```
## Boundary Tests
- Test EDGE CASES of the requirement
- For "match only the last date" - test lines with many dates
- Expected output MUST follow the same rules as correctness tests
- Don't confuse "boundary" with "different behavior"
```

### Fix 3: Existence Test Category (Priority 3)

Existence tests are generating regex match expectations instead of file existence checks. Consider:
- Renaming category to "Setup" or "Prerequisites"
- Adjusting prompt to focus on file/environment existence
- Not generating expected regex matches for existence category

---

## Metrics

| Metric | Value |
|--------|-------|
| Tests generated | 25 |
| Tests executable | 0 (all skip or pass) |
| Tests with assertions | 0 |
| Time | 48.5s |
| Tokens | 5,083 |
| Correct expected outputs | ~18/25 (72%) |
| File path extracted | 0/25 (0%) |

---

## Verdict

**The formatter fix is working correctly.** It no longer hardcodes TB2 paths, which is the right behavior per anti-cheating policy.

**But a new gap emerged.** The FM prompt doesn't tell the FM to extract and propagate output file paths from the task description. This causes 60% of tests to skip because the formatter can't find where to read the solution.

**The fix is a prompt update**, not a formatter change. This is legitimate because:
1. The file path (`/app/regex.txt`) comes from the task description
2. The FM naturally knows this context but isn't outputting it
3. We just need to tell the FM to include it in reasoning

**Priority:** Update FM prompt to extract output file context. This should unblock 15 tests that are currently skipping.

---

## Raw Generated Test File

See `/Users/christopherdavid/code/terminal-bench-2/regex-log/test_generated.py`

Key patterns:
- Lines 96-124: All correctness tests skip
- Lines 131-159: All boundary tests skip
- Lines 166-194: All integration tests skip
- Anti-cheat and existence pass but don't test anything useful
