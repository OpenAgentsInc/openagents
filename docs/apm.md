# APM: Actions Per Minute

> Inspired by StarCraft 2's APM metric for measuring mechanical skill in competitive play.

## Overview

APM (Actions Per Minute) measures how rapidly agents interact with the codebase through messages and tool calls. It serves as the primary metric of agent velocity in OpenAgents.

**Formula:** `APM = (messages + tool_calls) / duration_minutes`

## Data Sources

APM is calculated from Claude Code conversation logs at:
```
~/.claude/projects/<project-name>/*.jsonl
```

### Source Classification

| Source | Identification | Description |
|--------|---------------|-------------|
| **Claude Code** | `userType !== "external"` | Direct interactive usage |
| **MechaCoder** | `userType === "external"` + subtask patterns | Autonomous agent runs |

MechaCoder sessions are identified by:
- `userType: "external"` in JSONL entries
- Message content containing `## Subtask: oa-*` patterns

## Time Windows

| Window | Duration | Minutes | Use Case |
|--------|----------|---------|----------|
| 1h | 1 hour | 60 | Current session activity |
| 6h | 6 hours | 360 | Recent work period |
| 1d | 24 hours | 1,440 | Daily productivity |
| 1w | 7 days | 10,080 | Weekly trends |
| 1m | 30 days | 43,200 | Monthly baseline |
| Lifetime | All time | wall-clock | Historical comparison |

### Window Calculation

For time windows (1h through 1m):
```
APM = (actions_in_window) / window_minutes
```

For lifetime:
```
APM = (total_actions) / (last_session_time - first_session_time)
```

This uses wall-clock time (calendar time) rather than active time.

## Tool Categories

Tools are categorized for analysis:

| Category | Tools |
|----------|-------|
| **Code Generation** | Edit, MultiEdit, Write, NotebookEdit |
| **File Operations** | Read, LS, Glob |
| **System Operations** | Bash, BashOutput, KillShell |
| **Search** | Grep, WebSearch, WebFetch |
| **Planning** | TodoWrite, TodoRead, Task |
| **Other** | All other tools |

## CLI Usage

```bash
# Show all APM stats
bun src/cli/apm.ts

# Show APM for current project
bun src/cli/apm.ts --project .

# Output as JSON
bun src/cli/apm.ts --json

# Help
bun src/cli/apm.ts --help
```

### Sample Output

```
# APM Statistics
══════════════════════════════════════════════════

## Comparison: MechaCoder vs Claude Code
══════════════════════════════════════════════════

  Claude Code APM:   4.495
  MechaCoder APM:    18.974
  Delta:             +14.480
  Efficiency:        4.221x (+322.1%)

  Claude Code sessions: 1179
  MechaCoder sessions:  73
```

## API Usage

```typescript
import { parseClaudeConversations } from "../agent/apm-parser.js";
import { APMCollector } from "../agent/apm.js";

// Get historical APM stats
const stats = await Effect.runPromise(
  parseClaudeConversations.pipe(Effect.provide(BunContext.layer))
);

console.log(stats.combined.apmLifetime);  // Overall APM
console.log(stats.comparison.efficiencyRatio);  // MechaCoder vs Claude Code

// Real-time tracking during a session
const collector = new APMCollector("session-123", "openagents");
collector.recordAction("message");
collector.recordAction("tool_call", "Edit");
console.log(collector.getSessionAPM());
```

## Baseline & Goals

| Metric | Value | Notes |
|--------|-------|-------|
| **Video Baseline (July 2025)** | 2.3 APM | 277 sessions over 30 days |
| **Current Combined** | ~5 APM | 1252 sessions |
| **Claude Code Direct** | ~4.5 APM | Interactive usage |
| **MechaCoder** | ~19 APM | Autonomous runs |

**Goal:** Maximize APM while maintaining code quality.

## Files

| File | Purpose |
|------|---------|
| `src/agent/apm.ts` | Core types and APMCollector class |
| `src/agent/apm-parser.ts` | Claude Code JSONL parser |
| `src/cli/apm.ts` | CLI command |
| `docs/apm.md` | This specification |

## Related

- [Episode 186: Actions Per Minute](docs/transcripts/oa-186-actions-per-minute.md) - Original video context
- [MechaCoder Golden Loop](docs/mechacoder/GOLDEN-LOOP-v2.md) - Autonomous agent spec
- [Benchmark Metrics](src/bench/metrics.ts) - Related metrics infrastructure
