# Terminal-Bench Integration Plan for MechaCoder

## Overview

Add Terminal-Bench 2.0 support to MechaCoder, enabling both internal evaluation and official leaderboard submission via the Harbor framework.

**Key Decisions:**
- Goal: Both internal evaluation AND leaderboard submission
- Packaging: Integrated into openagents repo (Python adapter + TypeScript core)
- Execution: Claude Code only (recommended mode for best results)

## Already Implemented

The following infrastructure is already complete:

### ATIF Module (`src/atif/`)
- **schema.ts** - Full ATIF v1.4 schema with Effect/Schema
- **validation.ts** - Trajectory validation service
- **collector.ts** - TrajectoryCollector for capturing interactions
- **service.ts** - TrajectoryService for storage/retrieval
- **adapter.ts** - Event-to-ATIF converters:
  - `orchestratorEventsToSteps()` - Convert orchestrator events to ATIF steps
  - `sessionEntriesToTrajectory()` - Convert session entries to full trajectory
  - `subagentResultToObservation()` - Handle subagent results with refs
  - Agent factories: `createMechaCoderAgent()`, `createClaudeCodeAgent()`, `createMinimalSubagent()`
- **integration.ts** - Integration helpers for agent loops

### Benchmarking Infrastructure (`src/bench/`)
- **metrics.ts** - Token usage, timing, tool call tracking
- **terminal-bench.ts** - Terminal-Bench task format adapter
- **harness.ts** - Benchmark execution harness
- **reporter.ts** - Comparison reporting

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Harbor CLI                                │
│   harbor run -d terminal-bench@2.0 --agent-import-path ...      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  MechaCoderAgent (Python)                        │
│   src/harbor/mechacoder_agent.py                                 │
│   - Inherits BaseInstalledAgent                                  │
│   - Implements setup(), run(), populate_context_post_run()       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ Executes via bash
┌─────────────────────────────────────────────────────────────────┐
│                  MechaCoder CLI Wrapper                          │
│   src/cli/tbench.ts                                              │
│   - Entry point for Harbor to invoke                             │
│   - Accepts: --instruction, --model, --output-dir                │
│   - Uses Claude Code subagent for execution                      │
│   - Outputs: JSONL events + trajectory.json + metrics.json       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Existing Infrastructure                         │
│   src/bench/terminal-bench.ts  (task conversion)                 │
│   src/bench/metrics.ts         (metrics collection)              │
│   src/atif/schema.ts           (ATIF format)                     │
│   src/agent/loop.ts            (agent execution)                 │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: CLI Wrapper for Harbor Invocation (P1)

Create a CLI entry point that Harbor can invoke inside Docker containers.

**Files to create:**
- `src/cli/tbench.ts` - Main entry point for Terminal-Bench runs

**Interface:**
```bash
bun src/cli/tbench.ts \
  --instruction "Task description from Terminal-Bench" \
  --model "anthropic/claude-sonnet-4-5" \
  --output-dir /logs/agent \
  --timeout 3600
```

**Output files (written to --output-dir):**
- `events.jsonl` - Streaming events during execution
- `trajectory.json` - ATIF-format trajectory with tool calls
- `metrics.json` - Token usage, cost, timing, tool stats
- `stdout.txt` / `stderr.txt` - Raw output capture

**Key behaviors:**
- Execute task using Claude Code subagent
- Stream events to JSONL for real-time monitoring
- Generate ATIF trajectory from agent turns
- Extract token usage from Claude Code responses
- Exit with status 0 on success, non-zero on failure

### Phase 2: ATIF Trajectory Generation (P1) ✅ DONE

**Already implemented in `src/atif/`:**
- Full ATIF v1.4 schema with Effect/Schema
- `orchestratorEventsToSteps()` - Converts all orchestrator events to ATIF steps
- `sessionEntriesToTrajectory()` - Full trajectory generation with metrics
- `subagentResultToObservation()` - Handles Claude Code subagent trajectories
- Agent factories for MechaCoder, Claude Code, and minimal subagent
- Validation, storage, and retrieval services

No additional work needed for ATIF generation.

### Phase 3: Harbor Python Adapter (P1)

Create the Python adapter that Harbor uses to launch MechaCoder.

**Files to create:**
- `src/harbor/mechacoder_agent.py` - Agent class extending BaseInstalledAgent
- `src/harbor/install-mechacoder.sh.j2` - Jinja2 template for container setup
- `src/harbor/__init__.py` - Package init
- `pyproject.toml` - Python package definition

**MechaCoderAgent implementation:**
```python
class MechaCoderAgent(BaseInstalledAgent):
    @staticmethod
    def name() -> str:
        return "mechacoder"

    @property
    def _install_agent_template_path(self) -> Path:
        return Path(__file__).parent / "install-mechacoder.sh.j2"

    def create_run_agent_commands(self, instruction: str) -> list[ExecInput]:
        # Returns commands to execute MechaCoder

    def populate_context_post_run(self, context: AgentContext) -> None:
        # Parse metrics.json and populate token counts
```

**Installation template:**
```bash
#!/bin/bash
# Install Bun
curl -fsSL https://bun.sh/install | bash
export PATH="$HOME/.bun/bin:$PATH"

# Clone and setup MechaCoder
git clone https://github.com/OpenAgentsInc/openagents.git /opt/mechacoder
cd /opt/mechacoder
bun install

# Verify installation
bun src/cli/tbench.ts --help
```

### Phase 4: Token Extraction from Claude Code (P1)

Extract token usage from Claude Code's streaming JSON output.

**Files to modify:**
- `src/agent/claude-code.ts` - Capture usage from stream events

**Claude Code outputs usage in stream events:**
```json
{"type": "result", "usage": {"input_tokens": 1234, "output_tokens": 567}}
```

**Integration points:**
- Parse Claude Code's JSONL output for usage events
- Aggregate token counts across all turns
- Include in metrics.json output

### Phase 5: Internal Evaluation Mode (P2)

Enable running Terminal-Bench tasks locally without Harbor for internal testing.

**Files to create:**
- `src/cli/tbench-local.ts` - Run TB tasks locally against MechaCoder

**Features:**
- Load Terminal-Bench task definitions (JSON format)
- Run selected tasks or full suite
- Generate comparison reports vs baseline
- No Docker/Harbor dependency

**Usage:**
```bash
# Run specific tasks
bun src/cli/tbench-local.ts --tasks kernel-build,git-setup --model anthropic/claude-sonnet-4-5

# Run full suite with comparison
bun src/cli/tbench-local.ts --suite terminal-bench-2.0.json --baseline ./baseline-results.json
```

### Phase 6: Results & Reporting (P2)

Enhance reporting for Terminal-Bench specific metrics.

**Files to modify:**
- `src/bench/reporter.ts` - Add TB-specific report format
- `src/bench/terminal-bench.ts` - Complete toBenchmarkResults()

**Report outputs:**
- Leaderboard-compatible JSON (for submission)
- Markdown summary with per-category breakdown
- Comparison with previous runs / baseline

## Task Breakdown for .openagents/tasks.jsonl

### Phase 1 Tasks (P1 - Core Infrastructure)

```
oa-c8b48c: Create tbench CLI wrapper entry point
oa-d0d651: Implement ATIF trajectory generator ✅ CLOSED (already in src/atif/)
oa-a1dd75: Create Harbor Python adapter (MechaCoderAgent class)
oa-349e6d: Create install-mechacoder.sh.j2 template
oa-070738: Add token extraction from Claude Code output
oa-75fa2c: Create pyproject.toml for Harbor package
```

### Phase 2 Tasks (P2 - Local Evaluation)

```
oa-8dcda0: Implement tbench-local CLI for internal runs
oa-12486d: Add TB-specific reporting to reporter.ts
oa-143507: Complete Terminal-Bench result conversion
oa-570dd6: Add e2e test for Harbor adapter
```

### Phase 3 Tasks (P3 - Polish)

```
oa-61fff4: Document Terminal-Bench integration in docs/
oa-cdbf40: Add CI workflow for TB smoke tests
oa-01061b: Create results visualization dashboard
```

## Critical Files to Modify

| File | Purpose | Status |
|------|---------|--------|
| `src/cli/tbench.ts` | **NEW** - Harbor invocation entry point | TODO |
| `src/atif/adapter.ts` | Event-to-ATIF conversion | ✅ DONE |
| `src/harbor/mechacoder_agent.py` | **NEW** - Harbor adapter | TODO |
| `src/harbor/install-mechacoder.sh.j2` | **NEW** - Container setup | TODO |
| `pyproject.toml` | **NEW** - Python package for Harbor | TODO |
| `src/agent/claude-code.ts` | Modify - Add token extraction | TODO |
| `src/bench/terminal-bench.ts` | Modify - Complete result conversion | TODO |
| `src/bench/reporter.ts` | Modify - Add TB report format | TODO |

## Testing Strategy

1. **Unit tests** for ATIF ✅ DONE (`src/atif/__tests__/`)
2. **Integration test** running tbench CLI with mock task
3. **Harbor smoke test** with `oracle` agent first, then MechaCoder
4. **Local TB suite** run with sample tasks

## Usage After Implementation

**Leaderboard submission:**
```bash
export ANTHROPIC_API_KEY=...
harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path openagents.harbor:MechaCoderAgent \
  --model anthropic/claude-sonnet-4-5 \
  --k 5 \
  --jobs-dir ./mechacoder-tbench-results
```

**Internal evaluation:**
```bash
bun src/cli/tbench-local.ts \
  --suite ./tasks/terminal-bench-2.0.json \
  --model anthropic/claude-sonnet-4-5 \
  --output ./results/$(date +%Y%m%d)
```

## Dependencies

**Python (for Harbor adapter):**
- harbor (pip install harbor)
- Jinja2

**TypeScript (already present):**
- Effect
- bun:test

## Risk Considerations

1. **Claude Code availability** - Harbor runs in Docker; need to verify Claude Code CLI works in containers
2. **Token extraction** - Claude Code stream format may vary; need robust parsing
3. **Container setup time** - Installing Bun + deps may slow evaluation; consider caching
4. **Cost** - Full TB 2.0 suite (88 tasks × 5 attempts) can be expensive

## Success Metrics

- [ ] Can run `harbor run -d terminal-bench@2.0 --agent-import-path openagents.harbor:MechaCoderAgent`
- [ ] Metrics (tokens, cost, success rate) appear in Harbor output
- [ ] ATIF trajectory generated for each task
- [ ] Can submit results to Terminal-Bench leaderboard
- [ ] Internal evaluation mode works without Docker/Harbor
