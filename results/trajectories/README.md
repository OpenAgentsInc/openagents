# ATIF Trajectories

This directory stores ATIF (Agent Trajectory Interchange Format) v1.4 trajectories from Terminal-Bench runs.

## Structure

```
results/trajectories/
├── {task-id}/
│   ├── {session-id}/
│   │   ├── trajectory.json    # Full ATIF trajectory
│   │   ├── events.jsonl       # Streaming events log
│   │   └── metrics.json       # Token usage, cost, timing
│   └── ...
└── ...
```

## Session ID Format

`YYYYMMDD-HHMMSS-{random-hex}` - e.g., `20251211-153500-a1b2c3d4`

## ATIF v1.4 Schema

```json
{
  "schema_version": "1.4",
  "session_id": "tbench-...",
  "agent": {
    "name": "claude-code",
    "version": "2.0.58",
    "model": "claude-sonnet-4-20250514",
    "provider": "anthropic"
  },
  "steps": [
    {
      "step_id": 1,
      "timestamp": "ISO8601",
      "source": "user|agent|system",
      "message": "...",
      "tool_calls": [...],
      "observation": {...}
    }
  ],
  "final_metrics": {
    "total_prompt_tokens": 5000,
    "total_completion_tokens": 2000,
    "total_cost_usd": 0.042,
    "total_steps": 12
  },
  "extra": {
    "instruction": "...",
    "start_time": "...",
    "end_time": "...",
    "success": true
  }
}
```

## Generating Trajectories

Use `tbench` CLI:

```bash
# With streaming (for UI)
tbench \
  --instruction "Task description" \
  --output-dir results/trajectories/regex-log/20251211-153500-abc123 \
  --stream

# Without streaming
tbench \
  --instruction "Task description" \
  --output-dir results/trajectories/regex-log/20251211-153500-abc123
```

Or use the convenience script:

```bash
./scripts/tb2-run.sh regex-log
```

## Git Policy

- `trajectory.json` and `metrics.json` are tracked
- Large `events.jsonl` files (>1MB) are ignored
- See `.gitignore` in this directory
