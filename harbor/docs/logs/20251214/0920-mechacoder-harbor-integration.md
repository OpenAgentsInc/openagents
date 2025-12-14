# MechaCoder Harbor Integration Work Log

**Date:** 2025-12-14
**Goal:** Get Claude Code CLI running Terminal-Bench evaluations via Harbor with testgen skill

## Summary

Successfully created Harbor adapter package for Claude Code CLI with testgen skill integration. Achieved **reward=1.0** on regex-log task with Claude Sonnet.

## Implementation

### Package Structure

Created `harbor/` Python package with:
- `pyproject.toml` - Package config (Python 3.12+, Harbor dependency)
- `src/openagents_harbor/__init__.py` - Package exports
- `src/openagents_harbor/claude_agent.py` - Claude Code CLI adapter
- `src/openagents_harbor/pi_agent.py` - Pi agent adapter (stub)
- `src/openagents_harbor/install-agent.sh.j2` - Jinja template for container setup

### Key Technical Challenges Solved

1. **Root User Issue**: Claude CLI refuses `--dangerously-skip-permissions` when run as root
   - **Solution**: Create non-root `claude` user in container, run CLI via `su - claude`

2. **Shell Quoting**: Complex instruction escaping broke with `su -c '...'`
   - **Solution**: Write instruction to file, pipe to claude via `cat instruction.txt | claude -p -`

3. **OAuth Credentials**: Claude needs `~/.claude/.credentials.json` for OAuth auth
   - **Solution**: Read local credentials and inject into container for claude user

4. **Node.js Installation**: Container didn't have Node.js
   - **Solution**: Install nvm + Node.js 22 as claude user in install script

### Install Script Flow

```bash
1. apt-get install curl sudo
2. Create claude user with sudo access
3. chown -R claude:claude /app
4. As claude user:
   - Install nvm
   - Install Node.js 22
   - npm install -g @anthropic-ai/claude-code
   - npm install -g @mariozechner/pi-coding-agent
5. pip install pytest
```

### Agent Command Flow

```bash
1. mkdir -p /logs/agent
2. Write instruction to /logs/agent/instruction.txt (heredoc)
3. Create /home/claude/.claude/.credentials.json from local credentials
4. su - claude -c 'source nvm && cat instruction.txt | claude --print --dangerously-skip-permissions --max-turns 50 --model <model> -p -'
```

## Test Results

### regex-log Task

| Model | Result | Time | Notes |
|-------|--------|------|-------|
| claude-haiku-4-5-20251001 | 0.0 | 3:19 | Misunderstood task - limited all days to 29 |
| claude-haiku-4-5-20251001 | 0.0 | 5:15 | Same issue |
| claude-sonnet-4-20250514 | **1.0** | 4:46 | **PASSED** |

### Additional Tasks

| Task | Model | Result | Notes |
|------|-------|--------|-------|
| chess-best-move | Sonnet 4 | 0.0 | Challenging task |
| filter-js-from-html | Sonnet 4 | Running | - |
| gcode-to-text | Sonnet 4 | Running | - |

Note: Some tasks are harder than others. The infrastructure is validated - regex-log passes with Sonnet.

## Issues Encountered

1. **Python 3.10 too old** - Harbor requires Python 3.12+
   - Fixed: Updated `requires-python = ">=3.12"`, recreated venv

2. **Missing README.md** - Hatchling build failed
   - Fixed: Created harbor/README.md

3. **curl not found in container** - Install script tried curl before installing it
   - Fixed: apt-get install curl first

4. **`--task-ids` wrong flag** - Harbor uses `-t` or `--task-name`
   - Fixed: Updated demo script

5. **`--session` flag unknown** - Claude CLI doesn't support `--session`
   - Fixed: Removed from command

6. **Root privilege error** - `--dangerously-skip-permissions` rejected as root
   - Fixed: Run as non-root claude user

7. **Quoting issues** - Complex shell escaping broke with `su -c`
   - Fixed: Write instruction to file, pipe to claude

## Files Created/Modified

- `harbor/pyproject.toml`
- `harbor/README.md`
- `harbor/src/openagents_harbor/__init__.py`
- `harbor/src/openagents_harbor/claude_agent.py`
- `harbor/src/openagents_harbor/pi_agent.py`
- `harbor/src/openagents_harbor/install-agent.sh.j2`
- `scripts/tbench-demo.sh`

## Usage

```bash
# Setup
cd harbor
uv venv --python python3.12
source .venv/bin/activate
uv pip install -e ".[dev]"

# Run demo
harbor run \
  -d terminal-bench@2.0 \
  --agent-import-path openagents_harbor:ClaudeCodeAgent \
  -m anthropic/claude-sonnet-4-20250514 \
  -t regex-log \
  -o /tmp/tbench-results \
  -n 1
```

## Next Steps

1. Test more tasks to validate infrastructure
2. Improve Haiku's task understanding (maybe tune testgen prompt)
3. Add token tracking from Claude CLI output
4. Complete Pi agent implementation
