# OpenAgents Harbor Adapter

Harbor agent adapter for running MechaCoder in Terminal-Bench evaluations.

## Installation

```bash
uv pip install openagents-harbor
```

Or install from source:

```bash
cd crates/harbor/python
uv pip install -e .
```

## Usage

Run MechaCoder on Terminal-Bench tasks via Harbor:

```bash
harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path openagents_harbor:MechaCoderAgent \
  --k 5 \
  --jobs-dir ./results
```

**Note:** MechaCoder uses Claude Code as its subagent, which handles model selection
internally via your Anthropic subscription. The `--model` flag from Harbor is ignored.

## How It Works

1. Harbor calls `MechaCoderAgent.setup()` to install MechaCoder in the container
2. For each task, Harbor calls `create_run_agent_commands()` with the instruction
3. MechaCoder runs via the `tbench` Rust binary using Claude Code as its subagent
4. After completion, `populate_context_post_run()` extracts metrics from the output

## Output Files

MechaCoder produces these files in the output directory:

- `events.jsonl` - Streaming events during execution
- `trajectory.json` - ATIF v1.4 format trajectory
- `metrics.json` - Token usage, cost, timing, tool stats

## Requirements

- Python 3.12+
- Harbor framework
- Anthropic API key (for Claude Code)

## Development

The Rust `tbench` binary is in `crates/harbor/`. Build it with:

```bash
cargo build --release -p harbor
```
