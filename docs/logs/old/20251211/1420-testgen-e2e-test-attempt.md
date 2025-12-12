# TestGen E2E Test Attempt

**Date:** 2024-12-11 14:20
**Status:** FAILED - Wrong model, user interrupted

## What Happened

### Test Setup
1. Created test directories at `/tmp/regex-log-test/`
2. Read regex-log instruction from `~/code/terminal-bench-2/regex-log/instruction.md`
3. Prepared to run Claude with TestGen-wrapped instruction

### The Failure

**Attempted command:**
```bash
claude --verbose --dangerously-skip-permissions \
  --model claude-sonnet-4-20250514 \
  -p "[TestGen wrapped instruction]" \
  --max-turns 30 --output-format stream-json
```

**Problem:** Used deprecated model `claude-sonnet-4-20250514`

### User Feedback

User interrupted test with:
> "sonnet 4 was deprecated, everything is on 4.5 now. i want to try with haiku first can try sonnet later"

### Correct Models

| Model | ID |
|-------|-----|
| Haiku 4.5 | `claude-haiku-4-5-20251001` |
| Sonnet 4.5 | `claude-sonnet-4-5-20251022` |
| Opus 4.5 | `claude-opus-4-5-20251101` |

### Result

- Test directories created but empty
- No Claude output captured
- No testgen_tests.py created
- No regex.txt created
- Test aborted before execution completed

### Next Steps

Re-run test with correct model: `claude-haiku-4-5-20251001`

## Lesson Learned

**Always use 4.5 model versions:**
- Haiku: `claude-haiku-4-5-20251001`
- Sonnet: `claude-sonnet-4-5-20251022`
- Opus: `claude-opus-4-5-20251101`

The older model IDs like `claude-sonnet-4-20250514` are deprecated.
