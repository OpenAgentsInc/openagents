# TestGen v2 E2E Test - Context Exhaustion

**Date:** 2024-12-11 15:21
**Status:** INTERRUPTED - Claude Code ran out of context during 4th review iteration
**Model:** `claude-haiku-4-5-20251001`

## Summary

First E2E test of TestGen v2 (deterministic expansion + subagent review loops). The protocol executed correctly through 4 review iterations before the parent Claude Code process exhausted its context window and terminated.

**Key Finding:** The subagent review loop works - each iteration found real gaps that were addressed.

## What's New in v2

| Feature | v1 | v2 |
|---------|----|----|
| Test generation | LLM interprets each time | Deterministic `expand_tests.py` |
| Coverage review | Single pass | Fresh-context subagent loop |
| Protocol length | ~300 lines | ~70 lines |
| Review iterations | 1 | Until `thorough_enough=true` (max 5) |

## Test Configuration

```bash
cd /tmp/regex-log-test/app
timeout 600 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "[TestGen v2 wrapped instruction]" \
  --max-turns 50 \
  --output-format stream-json
```

## Protocol Execution

### Step 1: ANALYZE ✅

Claude correctly produced structured analysis:

```markdown
### ENTITIES
- date: YYYY-MM-DD format, year 4 digits, month 01-12, day 01-31 (Feb max 29)
- ipv4: Four octets 0-255, dot separated, no leading zeros

### CONSTRAINTS
- c1_format: Valid date/IP format
- c2_line_context: IP must be on same line as date
- c3_last_date: Only match last date when multiple present
- c4_word_boundary: Not preceded/followed by alphanumerics (includes underscore)

### MATRIX
| Constraint | Date | IPv4 |
|------------|------|------|
| c1_format | ✓ | ✓ |
| c2_line_context | ✓ | ✓ |
| c3_last_date | ✓ | - |
| c4_word_boundary | ✓ | ✓ |
```

### Step 2: EXPAND ✅

Ran `expand_tests.py` with the matrix:

```bash
python3 .claude/skills/testgen-protocol/expand_tests.py << 'EOF'
| Constraint | Date | IPv4 |
|------------|------|------|
| c1_format | ✓ | ✓ |
| c2_line_context | ✓ | ✓ |
| c3_last_date | ✓ | - |
| c4_word_boundary | ✓ | ✓ |
EOF
```

Generated scaffold with 14 test functions (7 entity-constraint combinations × 2 fixture types).

### Step 3: REVIEW (Loop) - 4 Iterations ⏳

| Iteration | thorough_enough | Tests | Key Gaps Found |
|-----------|-----------------|-------|----------------|
| 1 | ❌ false | 14→30 | Month/day ranges, IPv4 octets, underscore boundaries |
| 2 | ❌ false | 30→44 | Single-digit rejection, IPv4 wrong octet count |
| 3 | ❌ false | 44→50 | Day range 01-29 should be 01-31 for most months |
| 4 | ⏳ interrupted | 50+ | Context exhaustion before response |

### Review Loop Details

**Review 1 Gaps:**
- Missing month range validation (01-12)
- Missing day range validation (01-31, Feb 29)
- Missing IPv4 octet boundary tests (256, negative)
- Missing underscore as word boundary character

**Review 2 Gaps:**
- No tests for rejecting single-digit months (1 vs 01)
- No tests for IPv4 with wrong octet count (3 or 5 octets)
- Incomplete underscore boundary tests

**Review 3 Gaps:**
- Day range tests used 01-29, but most months have 30-31 days
- Missing edge year tests (0000, 9999)
- Missing wrong separator tests (slashes, dots in dates)

**Review 4:** Context exhausted before subagent could respond.

## Test Evolution

```
Initial scaffold:    14 tests (bare entity-constraint matrix)
After Review 1:      30 tests (+month/day ranges, IPv4 edges)
After Review 2:      44 tests (+underscore boundaries, octet count)
After Review 3:      50+ tests (+day 30/31, edge years, wrong separators)
```

### Final Test Categories

| Category | Count | Description |
|----------|-------|-------------|
| Date Format | 8 | Year digits, month 01-12, day 01-31, separators |
| IPv4 Format | 8 | Octet 0-255, leading zeros, dots, octet count |
| Word Boundaries | 8 | Date ±alphanumeric, IP ±alphanumeric, underscores |
| Line Context | 4 | IP required same line, IP on different line |
| Position | 4 | Last date when multiple present |
| Edge Cases | 6 | Year 0000/9999, Feb 29, day 31 |
| False Positives | 12 | Similar-but-invalid patterns |

## What Worked Well

1. **Subagent Review Loop** - Fresh-context reviewers consistently found gaps
2. **Iterative Improvement** - Tests grew from 14 to 50+ with real coverage gains
3. **Specific Feedback** - Each review provided actionable gap lists
4. **Protocol Compliance** - Claude followed v2 workflow exactly
5. **Entity-Constraint Matrix** - Forced systematic coverage thinking

## Issues Discovered

### Day Range Bug
- Initial tests rejected day 30 (2024-01-30) which is valid for most months
- **Fix:** Changed from 01-29 to 01-31 range validation

### Context Exhaustion
- Parent Claude Code process ran out of context during 4th review
- The protocol's multi-iteration nature + verbose test file = high token usage
- **Mitigation:** Need larger context budget or smarter summarization

### Missing Before Interruption
- Regex solution never written (Step 4: IMPLEMENT not reached)
- TB2 verification not run
- Unknown if 50+ tests would be sufficient

## Files Created

| File | Size | Status |
|------|------|--------|
| `/tmp/regex-log-test/app/testgen_tests.py` | ~400 lines | Created, 50+ tests |
| `/tmp/regex-log-test/app/regex.txt` | - | NOT CREATED (interrupted) |

## Cost

~$0.05 (Haiku 4.5, 4 review iterations before termination)

## Conclusion

**TestGen v2 architecture is validated:**
- Deterministic expansion works
- Subagent reviews find real gaps
- Iterative improvement produces comprehensive tests

**Operational issue:**
- Context exhaustion is a real risk with multi-iteration protocols
- Need either: larger context, better summarization, or iteration caps

## Root Cause of Interruption

Claude Code (the parent process running the test) exhausted its context window. This is different from the Haiku agent running out of context - the orchestrating process itself ran out of space due to:
1. Verbose test file content (400+ lines repeated in each review prompt)
2. Four full review iterations with gap analysis
3. Multiple edits to grow the test file

## Next Steps

1. Run again with fresh context
2. Consider summarizing test file in review prompts instead of full content
3. If successful, run TB2 verification
4. Compare with previous runs that used v1 protocol

## Command to Reproduce

```bash
# Fresh start
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/{app,logs/agent,logs/verifier}

# Run with extended timeout and turns
cd /tmp/regex-log-test/app
timeout 600 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "[TestGen v2 wrapped instruction]" \
  --max-turns 50 \
  --output-format stream-json \
  2>&1 | tee ../logs/agent/claude-code.txt

# After completion, run TB2 verification
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v ~/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh

cat /tmp/regex-log-test/logs/verifier/reward.txt
```
