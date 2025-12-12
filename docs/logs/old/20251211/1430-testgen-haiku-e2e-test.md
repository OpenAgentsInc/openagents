# TestGen E2E Test with Haiku 4.5

**Date:** 2024-12-11 14:30
**Status:** PARTIAL SUCCESS - TestGen workflow executed correctly, TB2 verification FAILED
**Model:** `claude-haiku-4-5-20251001`

## Summary

First successful end-to-end test of the TestGen protocol with Claude Code. Claude Haiku 4.5 correctly followed the DESCRIBE → WRITE TESTS → ITERATE workflow, but the solution failed TB2 verification due to edge cases not covered by self-generated tests.

## Test Configuration

```bash
cd /tmp/regex-log-test/app
timeout 300 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "[TestGen-wrapped regex-log instruction]" \
  --max-turns 30 \
  --output-format stream-json
```

## Results

### TestGen Protocol Compliance ✅

| Criterion | Status | Details |
|-----------|--------|---------|
| DESCRIBE section output | ✅ | Full task analysis with acceptance criteria |
| testgen_tests.py created | ✅ | 334 lines, 28 test cases |
| Tests ran before solution | ✅ | TDD workflow followed |
| Tests passed | ✅ | 28/28 passed |

### Files Created

| File | Size | Description |
|------|------|-------------|
| `testgen_tests.py` | 13KB (334 lines) | 28 pytest tests across 10 test classes |
| `regex.txt` | 493 bytes | Complex regex with lookahead assertions |

### Claude's Regex Solution

```regex
^(?=.*(?<!\d)(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])(?!\d)).*(?<!\w)((?:\d{4}-(?:(?:0[13579]|1[02])-(?:0[1-9]|[12][0-9]|3[01])|(?:04|06|09|11)-(?:0[1-9]|[12][0-9]|30)|02-(?:0[1-9]|1[0-9]|2[0-9]))))(?!\w)(?!.*(?<!\w)\d{4}-(?:(?:0[13579]|1[02])-(?:0[1-9]|[12][0-9]|3[01])|(?:04|06|09|11)-(?:0[1-9]|[12][0-9]|30)|02-(?:0[1-9]|1[0-9]|2[0-9]))(?!\w))
```

### TestGen Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.13.7, pytest-8.4.2

testgen_tests.py::TestDateValidation::test_valid_date_basic PASSED
testgen_tests.py::TestDateValidation::test_valid_date_various_months PASSED
testgen_tests.py::TestDateValidation::test_valid_date_february_29 PASSED
testgen_tests.py::TestIPV4Detection::test_date_without_ipv4 PASSED
testgen_tests.py::TestIPV4Detection::test_date_with_valid_ipv4 PASSED
testgen_tests.py::TestIPV4Detection::test_ipv4_at_start_of_line PASSED
testgen_tests.py::TestIPV4Detection::test_ipv4_with_leading_zero_not_valid PASSED
testgen_tests.py::TestMultipleDates::test_last_date_only_with_ipv4 PASSED
testgen_tests.py::TestMultipleDates::test_multiple_dates_all_invalid_except_last PASSED
testgen_tests.py::TestWordBoundaries::test_date_preceded_by_alphanumeric PASSED
testgen_tests.py::TestWordBoundaries::test_date_followed_by_alphanumeric PASSED
testgen_tests.py::TestWordBoundaries::test_date_with_valid_boundaries PASSED
testgen_tests.py::TestWordBoundaries::test_date_with_special_char_boundary PASSED
testgen_tests.py::TestDateValidityRanges::test_invalid_month_13 PASSED
testgen_tests.py::TestDateValidityRanges::test_invalid_month_00 PASSED
testgen_tests.py::TestDateValidityRanges::test_invalid_day_32 PASSED
testgen_tests.py::TestDateValidityRanges::test_invalid_day_00 PASSED
testgen_tests.py::TestDateValidityRanges::test_february_30_invalid PASSED
testgen_tests.py::TestDateValidityRanges::test_valid_day_ranges PASSED
testgen_tests.py::TestFalsePositives::test_user_id_like_text PASSED
testgen_tests.py::TestFalsePositives::test_similar_pattern_not_date PASSED
testgen_tests.py::TestIPV4Validation::test_ipv4_octet_ranges PASSED
testgen_tests.py::TestIPV4Validation::test_ipv4_invalid_octet_256 PASSED
testgen_tests.py::TestNoIPV4NoMatch::test_no_ipv4_no_match PASSED
testgen_tests.py::TestNoIPV4NoMatch::test_multiline_some_with_ipv4_some_without PASSED
testgen_tests.py::TestComplexScenarios::test_multiple_ips_one_line PASSED
testgen_tests.py::TestComplexScenarios::test_realistic_log_line PASSED
testgen_tests.py::TestComplexScenarios::test_dates_next_to_each_other PASSED

============================== 28 passed in 0.01s ==============================
```

### TB2 Verification Result

**Result: FAILED (reward.txt = 0)**

```
Expected: ['2025-01-09', '2024-11-01', '2020-01-01', '2019-06-15', '2018-06-06', '2017-03-03', '2016-12-31', '2024-11-11', '1999-03-05']

Got: ['2025-01-09', '2024-11-01', '2020-01-01', '2019-06-15', '2018-06-06', '2017-03-03', '2016-12-31', '2023-01-01', '2024-11-11', '1999-03-05', '2021-12-01', '1999-07-20']
```

### Root Cause Analysis

Claude's regex incorrectly matched 3 extra dates from lines where the **IP address** had alphanumeric boundaries (not the date):

| Extra Match | Line | Issue |
|-------------|------|-------|
| `2023-01-01` | `Error for user with code abc192.168.1.1 on 2023-01-01` | IP preceded by `abc` |
| `2021-12-01` | `User 192.168.1.1abc on 2021-12-01` | IP followed by `abc` |
| `1999-07-20` | `abc10.0.0.5 accessed system on 1999-07-20` | IP preceded by `abc` |

**The Bug**: Claude's regex uses `(?<!\d)` and `(?!\d)` for IP boundaries, but these only check for digits. The requirement says IPv4 addresses must not be "immediately preceded or followed by **alphanumeric** characters" - meaning `\w` boundaries are needed, not just `\d`.

### Gap Analysis: TestGen vs TB2

Claude's testgen tests correctly tested:
- Date word boundaries (alphanumeric before/after date)
- IPv4 octet value ranges (0-255)
- IPv4 leading zeros

Claude's testgen tests **missed**:
- IPv4 alphanumeric word boundaries (e.g., `abc192.168.1.1`)

This is a **TestGen quality gap** - Claude read the instruction which says both dates AND IPs need word boundaries, but only wrote tests for date word boundaries.

## Learnings

### What Worked

1. **TestGen protocol executed correctly** - DESCRIBE → TESTS → ITERATE workflow followed
2. **Comprehensive test generation** - 28 tests across 10 categories
3. **Self-iteration worked** - Claude achieved 28/28 pass rate on its own tests
4. **Complex regex generated** - Sophisticated lookahead/lookbehind assertions

### What Failed

1. **Incomplete requirement parsing** - Missed IP word boundary requirement
2. **Test coverage gap** - Didn't test IPs adjacent to alphanumerics
3. **Assumption in regex** - Used `(?<!\d)` instead of `(?<!\w)` for IP boundaries

### Improvement Ideas

1. **More explicit prompting** - TestGen wrapper could emphasize "test ALL boundary conditions"
2. **Requirement extraction step** - Add explicit step to list every constraint before testing
3. **Iteration with TB2-like scenarios** - If we had a "hint" system, could suggest edge cases

## Directory Contents After Test

```
/tmp/regex-log-test/
├── app/
│   ├── regex.txt           # Claude's solution (493 bytes)
│   ├── testgen_tests.py    # Claude's tests (334 lines, 28 tests)
│   ├── .pytest_cache/
│   └── __pycache__/
└── logs/
    ├── agent/
    │   └── claude-code.txt # Claude output (70 lines - truncated by stream)
    └── verifier/
        ├── reward.txt      # "0" (FAIL)
        └── ctrf.json       # Test results
```

## Cost

~$0.02 (Haiku 4.5 is very cheap)

## Next Steps

1. **Try with Sonnet 4.5** - May generate better tests with larger model
2. **Improve TestGen wrapper** - Add emphasis on boundary conditions for ALL entities
3. **Consider multi-iteration** - Let Claude see TB2-style edge cases and iterate

## Command to Reproduce

```bash
# 1. Setup
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/{app,logs/agent,logs/verifier}

# 2. Run Claude
cd /tmp/regex-log-test/app
timeout 300 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "[TestGen wrapped instruction - see docs/hillclimber/claude-testgen-operations.md]" \
  --max-turns 30 \
  --output-format stream-json \
  2>&1 | tee ../logs/agent/claude-code.txt

# 3. Run TB2 verification
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v ~/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh

# 4. Check result
cat /tmp/regex-log-test/logs/verifier/reward.txt
```
