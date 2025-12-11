# TestGen v2 + constraints-scheduling - TB2 FAIL

**Date:** 2024-12-11 17:55
**Status:** FAIL
**Model:** `claude-haiku-4-5-20251001`

## Summary

TestGen v2 failed on a more complex task requiring external input files.

| Metric | Value |
|--------|-------|
| TestGen Tests | 16 passed |
| TB2 Result | **FAIL (1/3 tests)** |
| Model | claude-haiku-4-5-20251001 |

## Root Cause

**Input file mismatch**: Claude generated tests against simplified calendar data it created, not the actual calendar files from the benchmark.

The scheduled meeting (Tuesday Jan 16, 10:00-11:00) conflicts with Alice's existing meeting:
- Alice has `alice-003`: 10:00-12:00 on Tuesday Jan 16

## Why This Happened

1. Claude didn't properly read the full calendar files
2. Created simplified test fixtures instead
3. TestGen tests passed against synthetic data
4. TB2 verification uses actual benchmark calendars

## Lesson Learned

TestGen works well for **self-contained** tasks (like regex-log) where the solution is validated against rules/patterns.

For tasks with **external input dependencies** (files that must be read correctly), TestGen needs to:
1. Parse actual input files before generating tests
2. Include tests that validate against real input data
3. Not create synthetic fixtures that mask real complexity

## Files

| File | Description |
|------|-------------|
| `/tmp/constraints-test/app/meeting_scheduled.ics` | Generated ICS (wrong time) |
| `/tmp/constraints-test/app/testgen_tests.py` | 16 tests (passed against wrong data) |

## Output

```ics
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Team Planning//EN
BEGIN:VEVENT
SUMMARY:Team Planning Meeting
DTSTART:20240116T100000Z  # WRONG - conflicts with Alice's calendar
DTEND:20240116T110000Z
...
END:VCALENDAR
```

## Recommendation

For TestGen to work on input-dependent tasks:
1. First step should be "PARSE INPUTS" to understand actual data
2. Generate tests that use real input file contents
3. Add validation that solution doesn't conflict with parsed data
