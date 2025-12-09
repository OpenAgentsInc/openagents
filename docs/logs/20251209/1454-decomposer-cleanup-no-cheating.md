# 1454 Decomposer Cleanup: Removing Hardcoded Solutions

**Time:** 14:54 CT
**Date:** 2025-12-09
**Task:** Clean up decomposer to remove cheating and maintain clean separation

---

## Summary

Rewrote `src/hillclimber/decomposer.ts` REGEX_LOG_DECOMPOSITION to remove hardcoded solutions and replace them with legitimate domain knowledge.

**The user correctly identified that my earlier suggestions were crossing the line into cheating.**

---

## What Was Wrong (Before)

The old decomposer (lines 67-79) contained:

```typescript
EXAMPLE REGEX (copy this exactly):
(?=.*\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}).*(\\d{4}-\\d{2}-\\d{2})

TEST CASES (your regex must handle these):
✓ MATCH: "192.168.1.1 login on 2024-01-15" → captures "2024-01-15"
✗ NO MATCH: "Error on 2024-01-15" (no IPv4)
...
```

**This is CHEATING because:**
1. "EXAMPLE REGEX (copy this exactly)" = hardcoded solution
2. Specific TEST CASES with expected outputs = leaking TB2 test information

Per `docs/logs/20251208/1219-benchmark-gaming-analysis.md`, this falls on the "cheating" end of the spectrum.

---

## The Philosophy

The entire point of the HillClimber architecture is:

1. **TestGen** bootstraps comprehensive tests from the task DESCRIPTION (not TB2 tests)
2. **FM** DISCOVERS the solution through iteration against TestGen tests
3. **If TestGen is good**, the discovered solution will pass TB2 too
4. **Curve 3** validates: "Do our internal metrics correlate with benchmark performance?"

**Giving FM the answer defeats the entire purpose.**

---

## What Changed (After)

The new decomposer provides DOMAIN KNOWLEDGE, not solutions:

### Subtask 1: write-conditional-regex

**Old goal (cheating):**
```
EXAMPLE REGEX (copy this exactly):
(?=.*\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}).*(\\d{4}-\\d{2}-\\d{2})
```

**New goal (legitimate):**
```
Write a regex to /app/regex.txt that matches dates ONLY on lines meeting certain conditions.

Read the task description carefully to understand:
1. What condition must the line satisfy? (e.g., contain a specific pattern)
2. What date format should be captured?
3. If multiple dates exist, which one should be captured?
```

**New hints (domain knowledge):**
- "Lookahead (?=...) checks a condition WITHOUT consuming characters"
- "Positive lookahead (?=.*pattern) ensures pattern exists somewhere on the line"
- "Greedy .* matches as much as possible - useful for capturing the LAST match"

### Subtask 2: add-boundary-handling

**Old goal (too specific):**
```
Add non-alphanumeric boundary checks:
- Before date: (?:^|[^0-9A-Za-z])
- After date: (?:[^0-9A-Za-z]|$)
```

**New goal (concept-based):**
```
Improve regex to prevent false positives by adding boundary handling.

False positives occur when:
- The pattern accidentally matches part of a larger token
- Invalid values slip through (e.g., out-of-range numbers)
```

**New hints:**
- "\\b word boundary works for alphanumeric boundaries"
- "For numeric boundaries, use negative lookahead/lookbehind"
- "Character class [^0-9A-Za-z] matches non-alphanumeric characters"

---

## Why This Is Legitimate

Per the spectrum from `benchmark-gaming-analysis.md`:

| Type | Old Decomposer | New Decomposer |
|------|----------------|----------------|
| Domain knowledge | N/A | Regex syntax concepts |
| Process knowledge | Minimal | TDD iteration strategy |
| Expected output leakage | YES (test cases) | NO |
| Hardcoded solution | YES (example regex) | NO |

The new decomposer teaches:
- **What lookahead IS** (concept) — not **which lookahead to use** (solution)
- **What boundaries DO** (concept) — not **which boundary pattern** (solution)
- **How to iterate** (process) — not **what the answer is** (solution)

---

## TestGen's Role

TestGen already has robust edge case extraction from task descriptions:

```typescript
// From test-generator-iterative.ts
export function extractTaskEdgeCases(taskDescription: string, taskId: string): TaskEdgeCases {
  // Parses task DESCRIPTION to find IPv4, date, regex patterns
  // Generates edge cases WITHOUT looking at TB2 tests
}
```

This generates:
- Invalid IPv4 tests (256.x.x.x)
- Invalid date tests (month 13, day 32)
- Boundary condition tests

FM should discover the solution by iterating against these TestGen-generated tests.

---

## Files Changed

| File | Change |
|------|--------|
| `src/hillclimber/decomposer.ts` | Complete rewrite of REGEX_LOG_DECOMPOSITION |

**Lines changed:** ~100 lines (replaced old with new)

---

## Tests

```bash
bun test src/hillclimber/map-orchestrator.test.ts
# 16 pass, 0 fail
```

---

## Impact

**Before:** FM could "cheat" by copying the example regex → inflated 89.5% result

**After:** FM must DISCOVER the solution through iteration → true validation of architecture

This is a cleaner test of the thesis: "Architecture beats model size."

---

## Design Philosophy Comment

Added a comment block to the decomposer explaining the philosophy:

```typescript
// ============================================================================
// REGEX-LOG DECOMPOSITION
//
// DESIGN PHILOSOPHY: This decomposition provides DOMAIN KNOWLEDGE, not solutions.
// - FM must DISCOVER the correct regex through iteration
// - TestGen generates comprehensive tests from task description
// - Hints teach regex CONCEPTS (lookahead, boundaries) not specific patterns
//
// See docs/logs/20251208/1219-benchmark-gaming-analysis.md for the spectrum
// of legitimate optimization vs cheating.
// ============================================================================
```

---

## Next Steps

1. Run standard mode test (10 turns) to see if FM can discover the solution
2. Compare results to the 89.5% achieved with hardcoded hints
3. If FM stalls, improve TestGen edge case extraction (legitimate)
4. Document the trajectory

---

**Status:** Complete - Decomposer cleaned up, tests passing
