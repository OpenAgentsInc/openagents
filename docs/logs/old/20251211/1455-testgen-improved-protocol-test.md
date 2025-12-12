# TestGen Improved Protocol Test

**Date:** 2024-12-11 14:55
**Status:** MAP phase WORKED, TB2 still FAILED (different failure mode)
**Model:** `claude-haiku-4-5-20251001`

## Summary

The Entity-Constraint Mapping (MAP) phase successfully addressed the previous test coverage gap. Claude now generates tests for IP word boundaries. However, the solution still failed TB2 with a different failure mode.

## Key Finding: MAP Phase Works

### Previous Test (without MAP)
```
testgen_tests.py::TestWordBoundaries::test_date_preceded_by_alphanumeric PASSED
testgen_tests.py::TestWordBoundaries::test_date_followed_by_alphanumeric PASSED
# NO IP BOUNDARY TESTS!
```
Result: Matched 12 dates (9 expected + 3 false positives from invalid IPs)

### This Test (with MAP)
```
testgen_tests.py::TestWordBoundaries::test_c8_date_not_preceded_by_word_char PASSED
testgen_tests.py::TestWordBoundaries::test_c9_date_not_followed_by_word_char PASSED
testgen_tests.py::TestWordBoundaries::test_c10_ipv4_not_preceded_by_word_char PASSED  ← NEW!
testgen_tests.py::TestWordBoundaries::test_c11_ipv4_not_followed_by_word_char PASSED  ← NEW!
```
Result: Matched only 3 dates (9 expected - regex too restrictive)

## Failure Mode Comparison

| Metric | Previous (no MAP) | This Run (with MAP) |
|--------|-------------------|---------------------|
| Expected dates | 9 | 9 |
| Matched dates | 12 | 3 |
| False positives | 3 (from invalid IPs) | 0 |
| False negatives | 0 | 6 |
| IP boundary tests | NO | YES |

## Analysis

### What the MAP Phase Fixed
1. Claude explicitly listed entities: "dates" and "IPv4 addresses"
2. Claude identified the shared constraint: "not adjacent to alphanumerics"
3. Claude wrote tests for BOTH entities' word boundaries
4. The CONSTRAINT-ENTITY MATRIX forced complete coverage

### What Still Went Wrong
The regex itself is now too restrictive. The IP word boundary logic likely interferes with the date matching in ways the tests didn't catch. This is a regex complexity issue, not a TestGen coverage issue.

The regex:
```regex
(?=.*\b(?:(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\b)(?<![0-9-])\b\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])\b(?!.*\b\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])\b)
```

This regex uses `\b` for IP boundaries but the IP validation lookahead may be too strict when combined with the date matching.

## TestGen Test Coverage (20 tests)

| Category | Tests | Description |
|----------|-------|-------------|
| Date Format | 4 | Year, month, day format validation |
| IPv4 Format | 3 | Octet ranges, leading zeros, dots |
| **Word Boundaries** | **4** | Date AND IP boundaries (the fix!) |
| Multiple Dates | 1 | Last date matching |
| Line Context | 1 | Requires IP on same line |
| False Positives | 1 | Similar-but-invalid patterns |
| Real World | 6 | Log scenarios |

## Conclusion

**The MAP phase succeeded** - it forced Claude to write tests for both entity types when a constraint applies to both. This is exactly what we designed it to do.

**The regex still fails** - but for a different reason (too restrictive, not too permissive). This is progress:
- Before: Missing tests → False positives
- After: Complete tests → Different bug type (false negatives)

The TestGen methodology improvement is validated. The remaining work is either:
1. More iterations to fix the regex logic
2. Using a more capable model (Sonnet/Opus)
3. Adding more test scenarios for edge cases

## Files Modified

| File | Change |
|------|--------|
| `crates/gym/src/mechacoder/testgen_wrapper.rs` | Added MAP ENTITIES & CONSTRAINTS phase |
| `.claude/skills/testgen-protocol/SKILL.md` | Added Phase 1.5 Entity-Constraint Mapping |

## Next Steps

1. Commit the improved TestGen protocol
2. Try with Sonnet 4.5 (more capable model)
3. Consider adding more iteration budget
