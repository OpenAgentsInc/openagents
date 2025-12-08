# 0236 HillClimber Agent Implementation

## Overview

Implemented the **TBHillClimber** agent - an overnight optimization loop for Terminal-Bench tasks that learns optimal configurations (hints, knobs) through hill-climbing optimization.

**Key Architecture:**
- Apple FM (Foundation Model) executes tasks
- OpenRouter (free model) provides meta-reasoning to propose config tweaks
- SQLite stores all state (configs, runs, best configs)

## Files Created

### SQLite Migration

**`.openagents/migrations/003_hillclimber.sql`**

Created three tables:
- `hillclimber_configs` - Task configurations (hint, use_skills, max_turns_override)
- `hillclimber_runs` - Every execution attempt with results and meta-reasoning
- `hillclimber_best_configs` - Quick lookup for best config per task

Key features:
- Config deduplication via SHA256 hash
- Current config tracking via `is_current` flag
- Best run tracking via `is_best` flag
- Pass rate tracking via `pass_count`/`total_runs`

### Core Module (`src/hillclimber/`)

**`types.ts`** - TypeScript interfaces
- `HillClimberConfig` - Task configuration
- `HillClimberRun` - Run record with results
- `BestConfig` - Best config per task
- `TaskRunResult` - Execution result
- `ConfigChange` - Proposed changes from meta-reasoner
- `HillClimberStats` - Aggregate statistics
- Row converters for SQLite
- `generateRunId()` helper

**`store.ts`** - Effect-based SQLite service
- `HillClimberStore` Context.Tag service
- Config operations: `saveConfig`, `getCurrentConfig`, `setCurrentConfig`, `getConfigById`, `getConfigByHash`
- Run operations: `saveRun`, `getRunHistory`, `getBestRun`
- Best config operations: `updateBestConfig`, `getBestConfigs`, `getBestConfigForTask`
- Stats: `getStats`, `getTaskStats`
- Utilities: `hashConfig`, `ensureDefaultConfig`
- Auto-migration on first connect

**`scoring.ts`** - Score calculation
- `scoreResult(passed, turns)` - Higher = better (pass bonus 1000 + 100-turns efficiency)
- `isBetterScore(new, old)` - Compare scores
- `isBetterResult(newPassed, newTurns, oldPassed, oldTurns)` - Compare results
- `isStableForExport(passCount, totalRuns, score)` - Check if stable for export (3+ passes, 50%+ pass rate)
- Format helpers: `formatScore`, `formatRunSummary`

**`executor.ts`** - FM task execution wrapper
- `runTask(taskId, config, options)` - Run single TB task with FM
- Suite caching to avoid repeated file reads
- Workspace management
- Hint injection into task description
- Verification via `runTaskVerification`
- Helpers: `getAvailableTasks`, `getTask`, `getEasyTasks`

**`meta-reasoner.ts`** - OpenRouter integration
- `proposeConfigChange(task, config, result, runNumber)` - Suggest config tweaks
- Uses `x-ai/grok-4.1-fast:free` by default (free, unlimited)
- Uses `openrouter/auto` every 10th run for deeper analysis
- `applyConfigChange(config, change)` - Apply proposed change
- `proposeHeuristicChange(task, config, result)` - Fallback when OpenRouter unavailable

**`exporter.ts`** - Export learned hints
- `getExportableHints()` - Get hints stable enough for export
- `exportHints(outputPath)` - Export to JSON file
- `exportTaskHint(taskId, outputPath)` - Export single task
- `loadLearnedHints(inputPath)` - Load from JSON
- `getLearnedHint(taskId)` - Get hint for task
- `generateHintsCode()` - Generate TypeScript code
- Output: `.openagents/learned-hints.json`

**`runner.ts`** - Main optimization loop
- `runHillClimber(options)` - Main entry point
- Round-robin task selection
- Graceful shutdown on SIGINT/SIGTERM
- `runSingleIteration(taskId, runNumber, options, workspaceBase)` - Single iteration
- `showStats()` - Display current statistics
- `dryRun(options)` - Preview without executing

**`cli.ts`** - CLI interface
```bash
bun run hillclimber                    # Run on default tasks
bun run hillclimber --task regex-log   # Single task mode
bun run hillclimber --max-runs 100     # Limit runs
bun run hillclimber --sleep 30000      # 30s between runs
bun run hillclimber --stats            # Show stats and exit
bun run hillclimber --export           # Export learned hints
bun run hillclimber --export-code      # Generate TypeScript code
bun run hillclimber --dry-run          # Preview without executing
bun run hillclimber --help             # Show help
```

**`index.ts`** - Module exports

### Package Scripts Added

```json
{
  "hillclimber": "bun src/hillclimber/cli.ts",
  "hillclimber:overnight": "bun src/hillclimber/cli.ts --max-runs 500 --sleep 30000",
  "hillclimber:stats": "bun src/hillclimber/cli.ts --stats",
  "hillclimber:export": "bun src/hillclimber/cli.ts --export"
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      TBHillClimber CLI                          │
│  bun run hillclimber [--task regex-log] [--max-runs 100]       │
└──────────────┬──────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    HillClimber Core Loop                        │
│  1. Load current config from SQLite (hillclimber_configs)       │
│  2. Pick task (round-robin or single-task mode)                 │
│  3. Get current config for task                                 │
│  4. Run task via FM micro-task runner                           │
│  5. Score result (passed, turns)                                │
│  6. Propose config change (via meta-reasoner)                   │
│  7. Save run to SQLite, update best if improved                 │
│  8. Sleep briefly, repeat                                       │
└──────────────┬──────────────────────────────────────────────────┘
               │
      ┌────────┴────────┐
      ▼                 ▼
┌──────────────┐  ┌──────────────────────────────────────────────┐
│ FM Executor  │  │ Meta-Reasoner (OpenRouter)                   │
│ (Apple FM)   │  │ • Free model: x-ai/grok-4.1-fast:free        │
│              │  │ • Auto model: openrouter/auto (sparingly)    │
│ runTask()    │  │                                              │
│              │  │ proposeConfigChange():                       │
│              │  │   - Analyze StepSummary, pass/fail           │
│              │  │   - Suggest single hint tweak                │
└──────────────┘  └──────────────────────────────────────────────┘
```

## Scoring Algorithm

```typescript
function scoreResult(passed: boolean, turns: number): number {
  const passBonus = passed ? 1000 : 0;
  const turnsScore = Math.max(0, 100 - turns);
  return passBonus + turnsScore;
}

// Examples:
// Pass in 5 turns:  1095 (1000 + 95)
// Pass in 50 turns: 1050 (1000 + 50)
// Fail in 5 turns:  95 (0 + 95)
// Fail in 50 turns: 50 (0 + 50)
```

## Meta-Reasoner Prompt

```
You are a tiny tuning agent for a coding benchmark.

Task ID: {taskId}
Task description: {description}

Current hint: {currentHint || "none"}
Last run: {passed ? "PASSED" : "FAILED"} in {turns} turns
{errorMessage}

Step summary:
{stepSummary}

Based on this, suggest ONE small change to the hint that might improve performance.
Reply with ONLY the new hint text (1-2 sentences max), or "KEEP" if no change needed.
```

## Cost Management

- **Primary**: Uses `DEFAULT_FREE_MODEL` from `openrouter-inference.ts` (currently `arcee-ai/trinity-mini:free`) - Free, unlimited
- **Backup**: `openrouter/auto` - Used every 10th run for deeper analysis
- **Tracking**: All calls logged via existing InferenceStore

## Export Criteria

A config is exported when:
- `passCount >= 3` (at least 3 passes)
- `score >= 1000` (must have passed)
- `passRate >= 50%` (decent success rate)

## Testing

```bash
# Verify CLI works
bun run hillclimber --help

# Check stats (initializes DB)
bun run hillclimber:stats

# Output:
# ============================================================
# HillClimber Statistics
# ============================================================
# Total runs: 0
# Total passes: 0
# Overall pass rate: 0.0%
# Unique tasks: 0
# Unique configs: 0
# ============================================================
```

## Usage for Overnight Runs

```bash
# Start overnight run (500 iterations, 30s sleep between runs)
bun run hillclimber:overnight

# Or customize:
bun run hillclimber --task regex-log --max-runs 200 --sleep 60000

# Monitor progress (in another terminal):
bun run hillclimber:stats

# Export learned hints when stable:
bun run hillclimber:export
```

## TypeScript Fixes Applied

1. **executor.ts** - Changed suite cache type to use `readonly` arrays
2. **executor.ts** - Fixed `runTaskSetup` and `runTaskVerification` to change to workspace directory first
3. **meta-reasoner.ts** - Fixed layer composition with `Layer.provideMerge` chain
4. **runner.ts** - Converted `await` inside `Effect.gen` to `yield* Effect.promise()`
5. **exporter.ts** - Removed unused imports

## Files Summary

| File | Lines | Purpose |
|------|-------|---------|
| `003_hillclimber.sql` | 72 | SQLite migration |
| `types.ts` | 163 | TypeScript interfaces |
| `store.ts` | 402 | SQLite service |
| `scoring.ts` | 121 | Score calculation |
| `executor.ts` | 256 | FM execution |
| `meta-reasoner.ts` | 231 | OpenRouter integration |
| `exporter.ts` | 203 | Hint export |
| `runner.ts` | 249 | Main loop |
| `cli.ts` | 122 | CLI interface |
| `index.ts` | 59 | Module exports |
| **Total** | **~1878** | |

## Next Steps

1. Run overnight: `bun run hillclimber:overnight`
2. Monitor: `bun run hillclimber:stats`
3. Export: `bun run hillclimber:export`
4. Review learned hints in `.openagents/learned-hints.json`
