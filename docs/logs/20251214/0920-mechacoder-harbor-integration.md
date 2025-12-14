# MechaCoder Harbor Integration Work Log

**Date:** 2024-12-14 09:20
**Goal:** Get MechaCoder doing a demo single-task ("regex-log") run via Harbor with testgen skill

## Session Summary

Re-adding Harbor integration that was previously deleted during codebase cleanup. Supporting both Claude Code CLI and Pi agents via `--agent-import-path` flag.

## Context

### Previous Success
- **2024-12-11**: Claude Code + Haiku + TestGen achieved 100% on regex-log
- 66 tests generated via testgen protocol
- Documented in `docs/logs/old/20251211/1745-testgen-v2-haiku-tb2-pass.md`

### Existing Assets
- `crates/claude-agent-sdk/` - Rust SDK for Claude Code CLI
- `crates/pi/` - Rust port of Pi coding agent
- `.claude/skills/testgen-protocol/` - TestGen skill
- `~/code/mechacoder-terminal-bench/` - Reference Harbor adapter (forked from pi-terminal-bench)

---

## Implementation Log

### Step 1: Create Harbor Package Structure
**Status:** Complete

Created `harbor/` Python package at repo root:

```
harbor/
├── pyproject.toml
├── src/
│   └── openagents_harbor/
│       ├── __init__.py
│       ├── claude_agent.py
│       ├── pi_agent.py
│       └── install-agent.sh.j2
```

### Step 2: Claude Code Agent Implementation
**Status:** Complete

File: `harbor/src/openagents_harbor/claude_agent.py`

Features:
- Testgen skill protocol prepended to all instructions
- Credential export from Mac Keychain (OAuth tokens work in containers)
- Model parsing from Harbor format (anthropic/model → model)
- Session JSONL parsing for token usage

Key components:
```python
class ClaudeCodeAgent(BaseInstalledAgent):
    # Spawns: claude --print --dangerously-skip-permissions --max-turns 50 ...
    # Prepends testgen skill to instruction
    # Exports Mac Keychain credentials if needed
```

### Step 3: Pi Agent Implementation
**Status:** Complete

File: `harbor/src/openagents_harbor/pi_agent.py`

Features:
- Multi-provider support (Anthropic, OpenAI, Gemini, Groq, etc.)
- JSON output mode for structured logging
- Provider/model parsing from Harbor format

Key components:
```python
class PiAgent(BaseInstalledAgent):
    # Spawns: pi --print --mode json --provider X --model Y ...
```

### Step 4: Installation Template
**Status:** Complete

File: `harbor/src/openagents_harbor/install-agent.sh.j2`

Installs:
- Node.js and npm
- Claude Code CLI (`@anthropic-ai/claude-code`)
- Pi agent (`@anthropic-ai/pi`)
- Python pytest for test generation

### Step 5: Demo Run Script
**Status:** Complete

File: `scripts/tbench-demo.sh`

Usage:
```bash
# Claude Code (default) with testgen
./scripts/tbench-demo.sh claude regex-log

# Pi agent
./scripts/tbench-demo.sh pi regex-log

# Custom model
./scripts/tbench-demo.sh claude regex-log anthropic/claude-sonnet-4-5-20250929
```

Features:
- Automatic Harbor patch application (upload_dir bug)
- Mac Keychain credential export
- Virtual environment setup
- Results display

---

## Credential Handling

OAuth tokens work in containers via credential export:

1. Extract from Mac Keychain: `security find-generic-password -s "Claude Code-credentials" -g`
2. Parse JSON from output: `password: "{\"claudeAiOauth\":{...}}"`
3. Write to `~/.claude/.credentials.json` with mode 600
4. Claude CLI finds credentials and authenticates

This approach documented in:
- `docs/logs/old/20251204/2347-claude-credentials-export-log.md`
- Commit `75b245318` (feat: add Claude Code OAuth credential injection)

---

## Files Created

| File | Purpose |
|------|---------|
| `harbor/pyproject.toml` | Python package config |
| `harbor/src/openagents_harbor/__init__.py` | Package exports |
| `harbor/src/openagents_harbor/claude_agent.py` | Claude Code adapter with testgen |
| `harbor/src/openagents_harbor/pi_agent.py` | Pi agent adapter |
| `harbor/src/openagents_harbor/install-agent.sh.j2` | Agent installation template |
| `scripts/tbench-demo.sh` | Demo run script |

---

## Usage

### Setup
```bash
# Install Harbor
uv tool install harbor

# Install our adapter
cd harbor
uv venv && source .venv/bin/activate
uv pip install -e ".[dev]"

# Set credentials (either method works)
export ANTHROPIC_API_KEY="sk-ant-..."
# OR let script export from Mac Keychain
```

### Run with Claude Code + TestGen
```bash
harbor run \
    -d terminal-bench@2.0 \
    --agent-import-path openagents_harbor:ClaudeCodeAgent \
    -m anthropic/claude-haiku-4-5-20251001 \
    -t regex-log \
    -o results/

# Or use the demo script
./scripts/tbench-demo.sh claude regex-log
```

### Run with Pi Agent
```bash
harbor run \
    -d terminal-bench@2.0 \
    --agent-import-path openagents_harbor:PiAgent \
    -m anthropic/claude-haiku-4-5-20251001 \
    -t regex-log \
    -o results/

# Or use the demo script
./scripts/tbench-demo.sh pi regex-log
```

---

## Issues Encountered

### Harbor upload_dir Bug
The demo script automatically patches Harbor's `upload_dir` function to fix the `/tests` directory copy issue. See `~/code/mechacoder-terminal-bench/README.md` for details.

---

## Test Results

### Setup Verification
- Package installation: PASS
- Harbor patch applied: PASS
- Imports working: PASS
  ```python
  from openagents_harbor import ClaudeCodeAgent, PiAgent
  ClaudeCodeAgent.name() # -> "claude-code"
  PiAgent.name()         # -> "pi"
  ```

### Demo Run Attempt
- **Status:** Docker daemon stopped during test
- **Error:** `Cannot connect to the Docker daemon at unix:///Users/christopherdavid/.docker/run/docker.sock`
- **Resolution:** Start Docker Desktop and re-run

The implementation is complete. To run the demo:

```bash
# Ensure Docker is running
docker ps

# Run the demo
cd harbor
source .venv/bin/activate
harbor run -d terminal-bench@2.0 --agent-import-path openagents_harbor:ClaudeCodeAgent -m anthropic/claude-haiku-4-5-20251001 -t regex-log -o results/
```

---

## Next Steps

1. Start Docker Desktop
2. Run demo: `./scripts/tbench-demo.sh claude regex-log`
3. Verify testgen skill generates 66+ tests
4. Verify regex-log passes (reward=1)
