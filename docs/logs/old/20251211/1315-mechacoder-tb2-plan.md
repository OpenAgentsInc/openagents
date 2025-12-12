# MechaCoder TB2 Docker Integration Plan

## Goal
Switch MechaCoder Gym screen to run Claude Code inside Docker containers matching exact Terminal-Bench 2 environment, ready for official leaderboard submission via Harbor.

## Current Problems
1. MechaCoder uses `std::env::current_dir()` (openagents dir, not TB2 environment)
2. Claude CLI path broken (`~/.claude/local/claude` doesn't exist on Linux)
3. Tasks hardcode `/app/regex.txt` but agent runs elsewhere
4. No Docker container setup

## Architecture

```
MechaCoder Screen
       |
       v
TB2 Task Loader  ----> Load from ~/code/terminal-bench-2/
       |
       v
Docker Runner    ----> sandbox::DockerBackend (existing)
       |
       v
Verification     ----> Run tests/test.sh, parse reward.txt
       |
       v
ATIF Export      ----> Harbor-compatible trajectory.json
```

## Files to Create

### 1. `crates/gym/src/mechacoder/tb2_loader.rs` (~250 lines)
Parse TB2 tasks from `~/code/terminal-bench-2/`:
- `TaskToml` struct - parse task.toml (docker_image, timeouts, metadata)
- `TB2Task` struct - complete task with instruction, paths
- `TB2TaskLoader` - discover and load tasks

### 2. `crates/gym/src/mechacoder/docker_runner.rs` (~400 lines)
Execute Claude Code in TB2 containers:
- Use existing `sandbox::DockerBackend`
- `DockerRunConfig` - task, workspace_dir, logs_dir, timeout
- `DockerEvent` - ContainerStarted, ClaudeOutput, AssistantMessage, etc.
- Stream Claude `--output-format stream-json` output to UI

### 3. `crates/gym/src/mechacoder/verifier.rs` (~150 lines)
Run TB2 verification:
- Execute `tests/test.sh` in container
- Parse `/logs/verifier/reward.txt` (1=pass, 0=fail)
- Parse `/logs/verifier/ctrf.json` for test details

### 4. `crates/gym/src/mechacoder/trajectory_exporter.rs` (~150 lines)
Export Harbor-compatible ATIF:
- Convert Claude session JSONL to ATIF steps
- Write trajectory.json, events.jsonl, metrics.json

## Files to Modify

### `crates/gym/src/mechacoder/mod.rs`
- Add module imports for new files
- Replace `on_start()` to use DockerRunner instead of Claude SDK directly
- Line 424: Replace `std::env::current_dir()` with temp workspace
- Lines 374-758: Replace `run_cc_query()` with Docker-based execution

### `crates/gym/src/mechacoder/types.rs`
- Add TB2 fields to `MechaTask`: `docker_image`, `timeout_sec`, `task_dir`
- Add `From<TB2Task>` conversion
- Update `tasks` module to load from TB2TaskLoader

### `crates/gym/src/mechacoder/task_panel.rs`
- Add task selector dropdown for TB2 tasks
- Show CWD as "/app (Docker)" when using Docker mode
- Display task metadata (difficulty, category)

### `crates/gym/Cargo.toml`
Add dependencies:
```toml
sandbox = { path = "../sandbox" }
toml = "0.8"
tempfile = "3"
```

## Container Setup

Directory structure inside container:
```
/app/                    # Working directory, agent produces solutions here
/logs/agent/sessions/    # Claude session JSONL files
/logs/verifier/          # reward.txt, ctrf.json
/tests/                  # test.sh, test_outputs.py
```

Claude command (matching Harbor):
```bash
claude --verbose --output-format stream-json \
  -p "$INSTRUCTION" \
  --allowedTools Bash,Edit,Write,Read,Glob,Grep,LS,WebFetch,NotebookEdit,NotebookRead,TodoRead,TodoWrite,Agent \
  2>&1 | tee /logs/agent/claude-code.txt
```

Environment variables:
- `ANTHROPIC_API_KEY` - from host environment
- `CLAUDE_CONFIG_DIR=/logs/agent/sessions`
- `FORCE_AUTO_BACKGROUND_TASKS=1`

## Data Flow

1. User selects task from dropdown (loaded from `~/code/terminal-bench-2/`)
2. Click Start -> create temp workspace + logs dirs
3. DockerRunner pulls/verifies TB2 docker_image from task.toml
4. Start container with mounts: workspace->/app, logs->/logs, tests->/tests
5. Run Claude CLI, stream JSON output to UI via mpsc channel
6. On Claude completion, VerificationRunner runs test.sh
7. Parse reward.txt, update UI with pass/fail
8. Save ATIF trajectory to store

## Implementation Order

1. **TB2 Task Loader** - Parse task.toml, load instruction.md
2. **Docker Runner** - Use sandbox::DockerBackend, stream Claude output
3. **Verifier** - Run tests, parse results
4. **UI Integration** - Wire up to MechaCoder screen
5. **Trajectory Export** - Harbor-compatible output

## Critical Files Reference

| File | Purpose |
|------|---------|
| `crates/gym/src/mechacoder/mod.rs:374-758` | Replace with Docker runner |
| `crates/sandbox/src/docker.rs` | Existing Docker backend to reuse |
| `~/code/harbor/src/harbor/agents/installed/claude_code.py:719-762` | Reference for Claude command |
| `~/code/terminal-bench-2/regex-log/task.toml` | Example task config |


