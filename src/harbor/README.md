# OpenAgents Harbor Adapter

Harbor agent adapter for running MechaCoder in Terminal-Bench evaluations.

## Installation

```bash
pip install openagents-harbor
```

Or install from source:

```bash
cd src/harbor
pip install -e .
```

## Usage

Run MechaCoder on Terminal-Bench tasks via Harbor:

```bash
harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path openagents.harbor:MechaCoderAgent \
  --model anthropic/claude-sonnet-4-5 \
  --k 5 \
  --jobs-dir ./results
```

## How It Works

1. Harbor calls `MechaCoderAgent.setup()` to install MechaCoder in the container
2. For each task, Harbor calls `create_run_agent_commands()` with the instruction
3. MechaCoder runs via `bun src/cli/tbench.ts` using Claude Code as its subagent
4. After completion, `populate_context_post_run()` extracts metrics from the output

## Output Files

MechaCoder produces these files in the output directory:

- `events.jsonl` - Streaming events during execution
- `trajectory.json` - ATIF v1.4 format trajectory
- `metrics.json` - Token usage, cost, timing, tool stats

## Requirements

- Python 3.11+
- Harbor framework
- Anthropic API key (for Claude Code)
