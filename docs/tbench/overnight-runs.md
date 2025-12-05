# Overnight Iteration Runs

This guide covers how to run Terminal-Bench 2.0 iterations overnight for continuous improvement tracking.

## Overview

The overnight iteration system runs Terminal-Bench tasks repeatedly, tracking episode metrics over time. This enables:

- **Performance tracking**: Monitor pass rates across iterations
- **Model comparison**: Compare Claude Code vs Ollama models
- **Regression detection**: Identify when changes hurt performance
- **Learning foundation**: Episode data for future Gym Trainer/Archivist integration

## Quick Start

```bash
# Run 5 iterations with Claude Code
bun src/cli/tbench-iterate.ts \
  --suite tasks/terminal-bench-2.json \
  --iterations 5

# Run with Ollama
bun src/cli/tbench-iterate.ts \
  --suite tasks/terminal-bench-2.json \
  --model ollama:codellama:34b \
  --iterations 10

# Resume an interrupted run
bun src/cli/tbench-iterate.ts \
  --resume ./results/20251205/state.json
```

## CLI Reference

```bash
bun src/cli/tbench-iterate.ts [options]
```

### Required Options

| Option | Description |
|--------|-------------|
| `-s, --suite <path>` | Path to Terminal-Bench suite JSON |

### Optional Options

| Option | Default | Description |
|--------|---------|-------------|
| `-o, --output <dir>` | `./results/YYYYMMDD` | Output directory |
| `-m, --model <spec>` | `claude-code` | Model to use (see Model Configuration) |
| `-i, --iterations <n>` | `10` | Number of iterations to run |
| `-t, --tasks <ids>` | all | Comma-separated task IDs |
| `--timeout <sec>` | `3600` | Per-task timeout in seconds |
| `--max-turns <n>` | `300` | Max agent turns per task |
| `--ollama-endpoint <url>` | `http://localhost:11434` | Ollama server endpoint |
| `--claude-validation-rate <0-1>` | `0` | Use Claude for N% of Ollama runs |
| `--resume <path>` | - | Resume from state file |

## Output Structure

Each run creates a timestamped output directory:

```
results/YYYYMMDD/
├── config.json           # Run configuration
├── state.json            # Resume state (updated each iteration)
├── summary.md            # Overall summary across iterations
├── episodes.json         # All episode records
└── iterations/
    ├── 001/
    │   ├── results.json      # Machine-readable results
    │   ├── report.md         # Human-readable summary
    │   └── <task-id>/
    │       ├── workspace/    # Task workspace with agent output
    │       ├── output.txt    # Agent conversation log
    │       ├── verification.txt  # Test output
    │       └── atif/         # ATIF trajectories (see below)
    │           └── YYYYMMDD/
    │               ├── <sessionId>.atif.jsonl
    │               └── <sessionId>.index.json
    ├── 002/
    │   └── ...
    └── ...
```

## ATIF Disk Persistence

Each task run saves its full ATIF trajectory to disk for future analysis and learning:

**Files saved:**
- `<sessionId>.atif.jsonl` - Append-only step log (one JSON line per step)
- `<sessionId>.index.json` - Metadata and checkpoint info

**ATIF step data includes:**
- Tool calls (function name, arguments)
- Observations (tool results)
- Timestamps for each step
- Agent source info

**Use cases:**
- Post-run analysis of agent behavior
- Training data for future learning systems (Gym Trainer)
- Debugging failed tasks
- Comparing agent strategies across iterations

**Path format:**
```
<output>/<task-id>/atif/<YYYYMMDD>/<sessionId>.atif.jsonl
```

## Episode Tracking

Episodes are stored in `.openagents/gym/episodes.jsonl` for cross-run analysis:

```json
{
  "id": "tbrun-20251205-083000-a1b2-001",
  "runId": "tbrun-20251205-083000-a1b2",
  "iteration": 1,
  "model": "claude-code",
  "suiteVersion": "2.0.0",
  "startedAt": "2025-12-05T08:30:00.000Z",
  "finishedAt": "2025-12-05T09:15:00.000Z",
  "status": "success",
  "summary": {
    "total": 89,
    "passed": 72,
    "failed": 15,
    "timeout": 2,
    "error": 0,
    "passRate": 0.809,
    "avgTurns": 18,
    "avgTokens": 12500,
    "totalDurationMs": 2700000
  },
  "resultsPath": "./results/20251205/iterations/001"
}
```

### Episode Status Values

| Status | Meaning |
|--------|---------|
| `success` | Pass rate >= 80% |
| `partial` | Pass rate 30-79% |
| `failed` | Pass rate < 30% |
| `timeout` | One or more tasks timed out |
| `error` | System error during run |

## Resume Capability

Runs can be interrupted and resumed:

```bash
# Start a long run
bun src/cli/tbench-iterate.ts --suite tasks/terminal-bench-2.json --iterations 50

# (Interrupt with Ctrl+C)

# Resume from where it left off
bun src/cli/tbench-iterate.ts --resume ./results/20251205/state.json
```

The `state.json` file tracks:
- Current iteration number
- Completed iterations
- Run configuration
- Episode IDs

## Mixed Model Runs

Use `--claude-validation-rate` to validate Ollama results with Claude:

```bash
# 90% Ollama, 10% Claude validation
bun src/cli/tbench-iterate.ts \
  --suite tasks/terminal-bench-2.json \
  --model ollama:codellama:34b \
  --claude-validation-rate 0.1 \
  --iterations 20
```

This is useful for:
- Verifying Ollama model quality against Claude baseline
- Catching model-specific failures
- Building comparison datasets

## HUD Integration

The overnight runner integrates with the Electrobun HUD for real-time monitoring:

1. Start the HUD: `bun run dev` in the desktop directory
2. Start a run with HUD: The runner auto-connects to WebSocket port 4242
3. Monitor progress: See pass/fail counts, timing, task details live

If the HUD shows "Disconnected":
- Ensure port 4242 is free
- Restart Electrobun
- Check for stale `.worktrees/` directories

## Best Practices

### Overnight Runs

1. **Use a stable network**: Claude Code requires internet
2. **Check disk space**: Each iteration can use 100MB+
3. **Set appropriate timeouts**: Hard tasks may need 30+ minutes
4. **Use `--tasks` for targeted runs**: Debug specific failures
5. **Monitor with HUD**: Watch for early failures

### Iteration Strategy

1. **Start small**: Run 1-2 iterations first to verify setup
2. **Increase gradually**: Scale to 5, 10, then 50 iterations
3. **Compare baselines**: Use episode data to track trends
4. **Focus on failures**: Use `--tasks` to retry failed tasks

### Resource Management

```bash
# Limit parallelism for resource-constrained systems
# (Currently sequential; parallelism planned for Phase 2)

# For Ollama, ensure model is loaded:
ollama run codellama:34b "hello"  # Preload model
```

## Troubleshooting

### "Model not available"

```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Pull the model if needed
ollama pull codellama:34b
```

### "Claude Code CLI not found"

```bash
# Ensure Claude Code is installed
which claude

# Or use Ollama instead
--model ollama:codellama:34b
```

### High failure rate

1. Check `verification.txt` for specific failures
2. Run failed tasks individually for debugging
3. Increase timeout if tasks are timing out
4. Check workspace for correct file structure

### Resume not working

1. Ensure `state.json` exists and is valid JSON
2. Check `completedIterations` array in state
3. Verify output directory still exists

## Future Phases

The overnight iteration system is Phase 1 of a larger learning system:

- **Phase 2**: Parallel execution with worktrees
- **Phase 3**: Gym Trainer integration for pattern learning
- **Phase 4**: Archivist integration for cross-run knowledge

See the main [README.md](./README.md) for architecture overview.
