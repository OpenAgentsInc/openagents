# Claude TestGen Operations Guide

## Overview

This guide explains how to run Claude Code on Terminal-Bench 2 (TB2) tasks using the TestGen protocol. The TestGen protocol requires Claude to generate its own tests before solving tasks, ensuring it understands requirements before implementing.

## Models

**ALWAYS use 4.5 model versions. Older versions are deprecated.**

| Model | Model ID | Use Case |
|-------|----------|----------|
| Haiku 4.5 | `claude-haiku-4-5-20251001` | Fast testing, iteration |
| Sonnet 4.5 | `claude-sonnet-4-5-20251022` | Balanced speed/quality |
| Opus 4.5 | `claude-opus-4-5-20251101` | Best quality, slowest |

**NEVER use these deprecated IDs:**
- ❌ `claude-sonnet-4-20250514`
- ❌ `claude-haiku-4-20250514`
- ❌ Any model ID without "4-5" in the name

## Directory Structure

For each test run, create this structure:

```
/tmp/<task-name>-test/
├── app/                    # Working directory (maps to /app in Docker)
│   ├── testgen_tests.py    # Claude-generated tests (TestGen creates this)
│   └── regex.txt           # Claude's solution (or whatever output)
└── logs/
    ├── agent/
    │   └── claude-code.txt # Full Claude output
    └── verifier/
        ├── reward.txt      # TB2 result: "1" = pass, "0" = fail
        └── ctrf.json       # Detailed test results
```

## Running Claude with TestGen

### Step 1: Setup Directories

```bash
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/app
mkdir -p /tmp/regex-log-test/logs/agent
mkdir -p /tmp/regex-log-test/logs/verifier
```

### Step 2: Run Claude

```bash
cd /tmp/regex-log-test/app

claude --verbose \
  --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "[TESTGEN WRAPPED INSTRUCTION]" \
  --max-turns 30 \
  --output-format stream-json \
  2>&1 | tee /tmp/regex-log-test/logs/agent/claude-code.txt
```

### Required Parameters

| Parameter | Value | Why |
|-----------|-------|-----|
| `--verbose` | (flag) | Required for stream-json output |
| `--dangerously-skip-permissions` | (flag) | Allows file writes without prompts |
| `--model` | `claude-haiku-4-5-20251001` | Use 4.5 models only |
| `-p` | TestGen-wrapped instruction | The task with TestGen protocol |
| `--max-turns` | 30 | Enough iterations for TestGen loop |
| `--output-format` | `stream-json` | Structured output for parsing |

### Optional Parameters

| Parameter | Default | Notes |
|-----------|---------|-------|
| `--allowedTools` | All tools | Can restrict to specific tools |
| `timeout` | None | Use `timeout 300` wrapper for 5min limit |

## TestGen Instruction Format

The instruction must be wrapped with TestGen protocol. The wrapper adds:

```
# TestGen Protocol Active

This task requires Test-Driven Development. You MUST generate your own tests
from the task description before writing the solution.

## YOUR TASK

[Original task instruction here]

## REQUIRED WORKFLOW: TestGen Protocol

### Step 1: DESCRIBE (Output this section first)
[Task analysis with acceptance criteria]

### Step 2: WRITE TESTS (Create /app/testgen_tests.py)
[Pytest tests for each criterion]

### Step 3: ITERATE (Loop until tests pass)
1. Write initial solution
2. Run: pytest /app/testgen_tests.py -v
3. If FAIL: fix solution, go to step 2
4. If PASS: you're done

## CRITICAL RULES
- NEVER read /tests/* - Those are for final verification only
- Derive tests from description ONLY
```

## Running TB2 Verification

After Claude completes, run the official TB2 tests:

```bash
# Build TB2 image if not exists
cd ~/code/terminal-bench-2/regex-log/environment
docker build -t alexgshaw/regex-log:20251031 .

# Run verification
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v ~/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh
```

### Check Results

```bash
# Check pass/fail (1 = pass, 0 = fail)
cat /tmp/regex-log-test/logs/verifier/reward.txt

# Check if testgen tests were created
ls -la /tmp/regex-log-test/app/testgen_tests.py

# Check solution
cat /tmp/regex-log-test/app/regex.txt
```

## Example: Full regex-log Test

```bash
# 1. Setup
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/{app,logs/agent,logs/verifier}

# 2. Run Claude with Haiku 4.5
cd /tmp/regex-log-test/app
timeout 180 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "# TestGen Protocol Active
...
[full TestGen-wrapped instruction]
...
" \
  --max-turns 20 \
  --output-format stream-json \
  2>&1 | tee /tmp/regex-log-test/logs/agent/claude-code.txt

# 3. Check TestGen compliance
ls /tmp/regex-log-test/app/testgen_tests.py  # Should exist
cat /tmp/regex-log-test/app/regex.txt        # Solution

# 4. Run TB2 verification
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v /home/christopherdavid/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh

# 5. Check result
cat /tmp/regex-log-test/logs/verifier/reward.txt
```

## Anti-Cheating Rules

**Claude MUST NOT:**
- Read `/tests/*` directory (TB2 official tests)
- Read `test_outputs.py`
- Hardcode solutions based on task ID
- Use knowledge from benchmark test files

**Claude MUST:**
- Derive tests from task description ONLY
- Follow DESCRIBE → WRITE TESTS → ITERATE workflow
- Create `/app/testgen_tests.py` before solution
- Iterate until self-generated tests pass

## Troubleshooting

### "model not found" or API errors
- Check model ID is exactly `claude-haiku-4-5-20251001`
- Verify ANTHROPIC_API_KEY is set

### Claude doesn't create testgen_tests.py
- Instruction wrapper may be missing
- Check TestGen protocol is in the prompt

### Timeout
- Increase timeout (default 180s may not be enough)
- Use faster model (Haiku)
- Reduce max-turns

### Docker image not found
- Build locally: `cd ~/code/terminal-bench-2/regex-log/environment && docker build -t alexgshaw/regex-log:20251031 .`
- TB2 images are NOT on Docker Hub

## File Locations

| File | Purpose |
|------|---------|
| `crates/gym/src/mechacoder/testgen_wrapper.rs` | Wraps instructions |
| `crates/gym/src/mechacoder/testgen_validator.rs` | Validates protocol |
| `.claude/skills/testgen-protocol/SKILL.md` | TestGen methodology |
| `~/code/terminal-bench-2/` | TB2 tasks directory |

## Cost Estimates

| Model | ~Cost per regex-log run |
|-------|------------------------|
| Haiku 4.5 | $0.01 - $0.05 |
| Sonnet 4.5 | $0.10 - $0.30 |
| Opus 4.5 | $0.50 - $2.00 |

Start with Haiku for testing, use Sonnet/Opus for production runs.

## WARNING: Deprecated Models in Codebase

The codebase has hardcoded `claude-sonnet-4-20250514` in many places:

```
crates/llm/src/anthropic.rs:15      - DEFAULT_MODEL
crates/cli/src/mechacoder_cmd.rs:26 - CLI default
crates/llm/src/models.rs:149        - Anthropic default
crates/harbor/src/lib.rs:47         - Harbor config
crates/gym/src/tbcc/types.rs:224    - TBCC types
crates/orchestrator/src/session.rs  - Session default
```

When running manually, ALWAYS specify `--model claude-haiku-4-5-20251001` explicitly to override these defaults.

TODO: Update all hardcoded models to 4.5 versions.
