# TestGen v2 + Haiku - TB2 PASS

**Date:** 2024-12-11 17:45
**Status:** SUCCESS
**Model:** `claude-haiku-4-5-20251001`

## Summary

Second successful TestGen v2 run confirming the protocol works reliably.

| Metric | Value |
|--------|-------|
| TestGen Tests | 66 passed |
| TB2 Result | **PASS (reward=1)** |
| Model | claude-haiku-4-5-20251001 |

## The Winning Regex

```regex
(?=.*(?<![a-zA-Z0-9.])(?:\d|[1-9]\d|1\d{2}|2[0-4]\d|25[0-5])\.(?:\d|[1-9]\d|1\d{2}|2[0-4]\d|25[0-5])\.(?:\d|[1-9]\d|1\d{2}|2[0-4]\d|25[0-5])\.(?:\d|[1-9]\d|1\d{2}|2[0-4]\d|25[0-5])(?![a-zA-Z0-9.])).*\b(\d{4}-(?:01|03|05|07|08|10|12)-(?:0[1-9]|[12]\d|3[01])|\d{4}-(?:04|06|09|11)-(?:0[1-9]|[12]\d|30)|\d{4}-02-(?:0[1-9]|1\d|2[0-9]))\b(?!.*\d{4}-\d{2}-\d{2})
```

Key features:
- IPv4 lookahead (IPv4 can appear anywhere on line, not just before date)
- Per-month day validation (31/30/29 day months)
- Word boundaries with `[a-zA-Z0-9.]` (includes dot for IP boundary)
- Last-date negative lookahead

## Test Coverage (66 Tests)

Categories:
- Month/day validation
- IPv4 octet ranges (0-255, no leading zeros)
- Boundary conditions (alphanumeric before/after)
- Multiline behavior
- Multiple dates per line (last match only)

## Comparison with 1616 (No TestGen)

| Run | TestGen | TB2 Result | Why |
|-----|---------|------------|-----|
| 1616 | No | FAIL (6/9) | Regex assumed IP before date |
| **1745** | **Yes** | **PASS** | Lookahead allows IP anywhere |

**Critical difference:** Without TestGen, Haiku assumed IP must appear before the date. With TestGen, the self-generated tests caught this edge case.

## Files

| File | Size |
|------|------|
| `/tmp/regex-log-test/app/regex.txt` | 356 bytes |
| `/tmp/regex-log-test/app/testgen_tests.py` | 19KB |

## Validation

This run confirms:
1. TestGen v2 protocol is reproducible (2nd successful run)
2. Haiku needs TestGen scaffolding for complex regex tasks
3. Self-generated tests discover edge cases the model would otherwise miss

## Command to Reproduce

```bash
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/{app,logs/agent,logs/verifier}

cd /tmp/regex-log-test/app
cat /tmp/testgen_instruction.txt | timeout 600 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  --max-turns 50 \
  2>&1 | tee ../logs/agent/claude-code.txt

docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v ~/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh

cat /tmp/regex-log-test/logs/verifier/reward.txt  # Should be "1"
```
