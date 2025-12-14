# Haiku regex-log Pass with ATIF Trajectory Support

**Date:** 2025-12-14 11:55
**Task:** regex-log (Terminal-Bench 2.0)
**Model:** claude-haiku-4-5-20251001
**Result:** PASS (reward=1.0)
**Time:** 9:56

## Summary

Successfully ran Claude Code + Haiku with testgen skill on regex-log task via Harbor, with full ATIF trajectory capture. This validates both the Harbor adapter and the ATIF integration.

## Preparation Work

### 1. Initial Implementation (Earlier Session)

Created `harbor/` Python package with:
- `ClaudeCodeAgent` extending `BaseInstalledAgent`
- TestGen skill protocol prepended to instructions
- Non-root user setup (Claude CLI security restriction)
- OAuth credential injection from Mac Keychain

**Initial Issues:**
- `--dangerously-skip-permissions` rejected when running as root
- Complex shell quoting broke with `su -c '...'`
- `--print` mode gave plain text only, no structured data

### 2. ATIF Trajectory Support

User requested ATIF trajectory capture. Investigated Harbor's built-in `ClaudeCode` agent at:
```
~/code/harbor/src/harbor/agents/installed/claude_code.py
```

**Key Findings:**
- Harbor uses `--output-format stream-json` (not `--print`)
- Sets `CLAUDE_CONFIG_DIR` to capture session JSONL files
- Converts session files to ATIF format in `populate_context_post_run()`
- Trajectory saved to `agent/trajectory.json`

### 3. Refactored Agent

Changed `ClaudeCodeAgent` to extend `ClaudeCode` (Harbor's built-in agent):

```python
from harbor.agents.installed.claude_code import ClaudeCode

class ClaudeCodeAgent(ClaudeCode):
    """Extends Harbor's ClaudeCode with testgen skill."""

    def create_run_agent_commands(self, instruction: str) -> list[ExecInput]:
        # Prepend testgen skill
        full_instruction = TESTGEN_SKILL + "\n\n# TASK:\n\n" + instruction
        return self._create_testgen_commands(full_instruction)
```

**Key Changes:**
- Inherit trajectory conversion from `ClaudeCode`
- Use `--output-format stream-json` instead of `--print`
- Set `CLAUDE_CONFIG_DIR` for session capture
- Use `--allowedTools` instead of `--dangerously-skip-permissions`
- Simplified install template (install as root, like Harbor's)

### 4. Install Template

Simplified to match Harbor's approach:

```bash
#!/bin/bash
apt-get update && apt-get install -y curl
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.2/install.sh | bash
source "$HOME/.nvm/nvm.sh"
nvm install 22
npm install -g @anthropic-ai/claude-code@latest
```

### 5. Command Structure

Final command:
```bash
source $HOME/.nvm/nvm.sh && \
claude --verbose --output-format stream-json \
  -p '<testgen + task instruction>' \
  --allowedTools Bash Edit Write Read Glob Grep ... \
  2>&1 </dev/null | tee /logs/agent/claude-code.txt
```

## Run Details

### Command
```bash
harbor run \
  -d terminal-bench@2.0 \
  --agent-import-path openagents_harbor:ClaudeCodeAgent \
  -m anthropic/claude-haiku-4-5-20251001 \
  -t regex-log \
  -o /tmp/tbench-haiku \
  -n 1
```

### Output
```
Wrote Claude Code trajectory to /tmp/tbench-haiku/2025-12-14__11-44-54/regex-log__ARRKjid/agent/trajectory.json
  1/1 Mean: 1.000 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 0:09:56 0:00:00

    claude-code-testgen (claude-haiku-4-5-20251001) on terminal-bench
┏━━━━━━━━━━━━━━━━━━━━━┳━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ Metric              ┃ Value                                           ┃
┡━━━━━━━━━━━━━━━━━━━━━╇━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┩
│ Agent               │ claude-code-testgen (claude-haiku-4-5-20251001) │
│ Dataset             │ terminal-bench                                  │
│ Trials              │ 1                                               │
│ Errors              │ 0                                               │
│ Mean                │ 1.000                                           │
│ Reward Distribution │                                                 │
│   reward = 1.0      │ 1                                               │
└─────────────────────┴─────────────────────────────────────────────────┘
```

### ATIF Trajectory

**Location:** `/tmp/tbench-haiku/2025-12-14__11-44-54/regex-log__ARRKjid/agent/trajectory.json`

**Size:** 276KB

**Schema:** ATIF-v1.2

**Contents:**
```json
{
  "schema_version": "ATIF-v1.2",
  "session_id": "<uuid>",
  "agent": {
    "name": "claude-code",
    "version": "2.0.69",
    "model_name": "claude-haiku-4-5-20251001",
    "extra": { "cwds": ["/app"], "agent_ids": [...] }
  },
  "steps": [
    // Full conversation: user messages, assistant responses, tool calls
    // Each step has: step_id, timestamp, source, message, tool_calls, observation
    // Agent steps include: model_name, reasoning_content, metrics
  ],
  "final_metrics": {
    "total_prompt_tokens": <number>,
    "total_completion_tokens": <number>,
    "total_cached_tokens": <number>,
    "total_steps": <number>
  }
}
```

### Trial Directory Structure
```
/tmp/tbench-haiku/2025-12-14__11-44-54/regex-log__ARRKjid/
├── agent/
│   ├── claude-code.txt          # Raw stream-json output
│   ├── trajectory.json          # ATIF trajectory (276KB)
│   ├── sessions/                # Claude session files
│   │   └── projects/-app/*.jsonl
│   ├── command-0/               # mkdir setup
│   ├── command-1/               # credential injection
│   └── command-2/               # claude execution
├── config.json
├── result.json
└── verifier/
    ├── reward.txt               # "1"
    └── test-stdout.txt
```

## Why Haiku Passed This Time

Previous Haiku runs failed because:
1. Task misinterpretation (limited all days to 29, not just February)
2. Using `--print` mode which may have different behavior

This run succeeded likely because:
1. `--output-format stream-json` gives Claude more structured feedback
2. `--allowedTools` explicit list may affect behavior
3. Different session/state handling with `CLAUDE_CONFIG_DIR`
4. Or simply model variance (Haiku occasionally gets it right)

## Files Modified

| File | Change |
|------|--------|
| `harbor/src/openagents_harbor/claude_agent.py` | Extend ClaudeCode, use stream-json |
| `harbor/src/openagents_harbor/install-agent.sh.j2` | Simplify to root install |

## Commits

- `7d05cbae2` - feat(harbor): Add ATIF trajectory support via ClaudeCode inheritance

## Validation

- [x] Haiku passes regex-log (reward=1.0)
- [x] ATIF trajectory saved (276KB)
- [x] Token metrics captured
- [x] Full conversation history preserved
- [x] Testgen skill prepended to instruction
