# Terminal-Bench Integration Plan for MechaCoder

## Overview

Add Terminal-Bench 2.0 support to MechaCoder, enabling both internal evaluation and official leaderboard submission via the Harbor framework.

**Key Decisions:**
- Goal: Both internal evaluation AND leaderboard submission
- Packaging: Integrated into openagents repo (Python adapter + TypeScript core)
- Execution: Claude Code only (recommended mode for best results)

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

### Phase 2: ATIF Trajectory Generation (P1)

Convert MechaCoder's internal events to ATIF format for Harbor/leaderboard compatibility.

**Files to create/modify:**
- `src/atif/generator.ts` - Convert TaskRunEvent[] → ATIFDocument
- `src/atif/schema.ts` - Extend if needed for Harbor compatibility

**ATIF structure (per Harbor expectations):**
```typescript
interface ATIFDocument {
  version: "1.4";
  agent: { name: "mechacoder"; version: string };
  model: { provider: string; name: string };
  task: { id: string; instruction: string };
  steps: ATIFStep[];
  summary: {
    totalTokens: { input: number; output: number };
    totalCost: number;
    duration: number;
    outcome: "success" | "failure" | "timeout";
  };
}

interface ATIFStep {
  type: "thought" | "tool_call" | "tool_result" | "observation";
  content: string;
  timestamp: string;
  metadata?: {
    tool?: string;
    args?: Record<string, unknown>;
    tokens?: { input: number; output: number };
  };
}
```

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
oa-tbench-01: Create tbench CLI wrapper entry point
oa-tbench-02: Implement ATIF trajectory generator from agent events
oa-tbench-03: Create Harbor Python adapter (MechaCoderAgent class)
oa-tbench-04: Create install-mechacoder.sh.j2 template
oa-tbench-05: Add token extraction from Claude Code output
oa-tbench-06: Create pyproject.toml for Harbor package
```

### Phase 2 Tasks (P2 - Local Evaluation)

```
oa-tbench-07: Implement tbench-local CLI for internal runs
oa-tbench-08: Add TB-specific reporting to reporter.ts
oa-tbench-09: Complete Terminal-Bench result conversion
oa-tbench-10: Add e2e test for Harbor adapter
```

### Phase 3 Tasks (P3 - Polish)

```
oa-tbench-11: Document Terminal-Bench integration in docs/
oa-tbench-12: Add CI workflow for TB smoke tests
oa-tbench-13: Create results visualization dashboard
```

## Critical Files to Modify

| File | Purpose |
|------|---------|
| `src/cli/tbench.ts` | **NEW** - Harbor invocation entry point |
| `src/atif/generator.ts` | **NEW** - Convert events to ATIF |
| `src/harbor/mechacoder_agent.py` | **NEW** - Harbor adapter |
| `src/harbor/install-mechacoder.sh.j2` | **NEW** - Container setup |
| `pyproject.toml` | **NEW** - Python package for Harbor |
| `src/agent/claude-code.ts` | Modify - Add token extraction |
| `src/bench/terminal-bench.ts` | Modify - Complete result conversion |
| `src/bench/reporter.ts` | Modify - Add TB report format |

## Testing Strategy

1. **Unit tests** for ATIF generator
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
