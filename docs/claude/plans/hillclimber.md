# HillClimber Agent Implementation Plan

## Overview

Build a **TBHillClimber** - a Bun CLI script that iterates overnight on Terminal-Bench tasks, learning optimal configurations (hints, knobs) for each task through hill-climbing optimization.

**Key Insight**: FM (Apple Foundation Model) executes tasks, while OpenRouter (free model) provides meta-reasoning to propose config tweaks.

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
│ runSingleTB  │  │                                              │
│ TaskWithFM() │  │ proposeConfigChange():                       │
│              │  │   - Analyze StepSummary, pass/fail           │
│              │  │   - Suggest single hint tweak                │
└──────────────┘  └──────────────────────────────────────────────┘
```

## Storage: SQLite

**Database**: `.openagents/openagents.db` (shared with other systems)
**Migration**: `.openagents/migrations/003_hillclimber.sql`

### Tables

```sql
-- Task configurations (the "knobs" we're tuning)
CREATE TABLE hillclimber_configs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL,
  hint TEXT,                           -- Task-specific hint (main knob)
  use_skills INTEGER DEFAULT 0,        -- Boolean
  max_turns_override INTEGER DEFAULT 30,
  config_hash TEXT NOT NULL,           -- SHA256 of config for comparison
  is_current INTEGER DEFAULT 0,        -- Boolean - current config for this task
  created_at TEXT NOT NULL DEFAULT (datetime('now')),

  UNIQUE(task_id, config_hash)
);

-- Run history (every execution attempt)
CREATE TABLE hillclimber_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL UNIQUE,         -- "hc-{timestamp}-{random}"
  task_id TEXT NOT NULL,
  config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),

  -- Results
  passed INTEGER NOT NULL,             -- Boolean
  turns INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  step_summary TEXT,                   -- Last 3 StepSummary entries (JSON)
  error_message TEXT,

  -- Meta-reasoning
  meta_model TEXT,                     -- Model used for reasoning (e.g., "grok-4.1-fast:free")
  proposed_change TEXT,                -- What change was proposed
  change_accepted INTEGER DEFAULT 0,   -- Boolean - was the change applied

  -- Scoring
  score INTEGER NOT NULL,              -- Computed score for comparison
  is_best INTEGER DEFAULT 0,           -- Boolean - was this the best run for this task

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Best configs per task (for quick lookup and export)
CREATE TABLE hillclimber_best_configs (
  task_id TEXT PRIMARY KEY,
  config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),
  run_id INTEGER NOT NULL REFERENCES hillclimber_runs(id),
  score INTEGER NOT NULL,
  pass_count INTEGER DEFAULT 0,        -- How many times this config passed
  total_runs INTEGER DEFAULT 0,        -- Total runs with this config
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_hc_configs_task ON hillclimber_configs(task_id);
CREATE INDEX idx_hc_configs_current ON hillclimber_configs(task_id, is_current) WHERE is_current = 1;
CREATE INDEX idx_hc_runs_task ON hillclimber_runs(task_id);
CREATE INDEX idx_hc_runs_config ON hillclimber_runs(config_id);
CREATE INDEX idx_hc_runs_best ON hillclimber_runs(task_id, is_best) WHERE is_best = 1;
```

### TypeScript Types

```typescript
interface HillClimberConfig {
  id: number;
  taskId: string;
  hint: string | null;
  useSkills: boolean;
  maxTurnsOverride: number;
  configHash: string;
  isCurrent: boolean;
  createdAt: string;
}

interface HillClimberRun {
  id: number;
  runId: string;
  taskId: string;
  configId: number;
  passed: boolean;
  turns: number;
  durationMs: number;
  stepSummary: string[] | null;
  errorMessage: string | null;
  metaModel: string | null;
  proposedChange: string | null;
  changeAccepted: boolean;
  score: number;
  isBest: boolean;
  createdAt: string;
}

interface BestConfig {
  taskId: string;
  configId: number;
  runId: number;
  score: number;
  passCount: number;
  totalRuns: number;
  updatedAt: string;
}
```

## Files to Create

### 1. `.openagents/migrations/003_hillclimber.sql`
SQLite migration for HillClimber tables (schema shown above).

### 2. `src/hillclimber/types.ts`
Type definitions for HillClimber configs, runs, best configs.

### 3. `src/hillclimber/store.ts`
SQLite-backed storage service (Effect-based, like InferenceStore):
- `HillClimberStore` - Context.Tag service
- `saveConfig(config)` - Create/update config
- `getCurrentConfig(taskId)` - Get current config for task
- `setCurrentConfig(taskId, configId)` - Mark config as current
- `saveRun(run)` - Record run result
- `updateBestConfig(taskId)` - Update best config if new run is better
- `getRunHistory(taskId, limit?)` - Get recent runs
- `getStats()` - Aggregate stats across all tasks
- `getBestConfigs()` - Get best config for each task
- `hashConfig(config)` - SHA256 hash for config comparison

### 4. `src/hillclimber/executor.ts`
FM task execution wrapper:
- `runSingleTBTask(taskId, config)` - Run one TB task with FM
- Reuses existing `createModelRunner({ type: "foundation-models" })`
- Extracts StepSummary from result output
- Returns structured result with pass/fail, turns, summary

### 5. `src/hillclimber/meta-reasoner.ts`
OpenRouter-based meta-reasoning:
- `proposeConfigChange(taskId, config, runResult)` - Suggest one tweak
- Uses `x-ai/grok-4.1-fast:free` by default (free, unlimited)
- Uses `openrouter/auto` every 10th run for deeper analysis
- Automatic tracking via InferenceStore
- Returns new config or null (keep current)

### 6. `src/hillclimber/scoring.ts`
Scoring and comparison:
- `scoreResult(passed, turns)` - Numeric score (higher = better)
- `isBetterThan(newScore, oldScore)` - Compare scores
- Scoring: passed > failed, fewer turns = better

### 7. `src/hillclimber/exporter.ts`
Export learned hints to the hints system:
- `exportToHints(taskId)` - Export best config's hint to hints.ts
- Creates a task-specific hint entry in the hints system
- Only exports configs with >= 3 consecutive passes

### 8. `src/hillclimber/runner.ts`
Main loop:
- `runHillClimber(options)` - Main entry point
- Options: task list, max runs, sleep interval
- Handles Ctrl+C gracefully (save state before exit)
- Round-robin task selection

### 9. `src/hillclimber/cli.ts`
CLI interface:
```bash
bun run hillclimber                    # Run on all configured tasks
bun run hillclimber --task regex-log   # Single task mode
bun run hillclimber --max-runs 100     # Limit runs
bun run hillclimber --sleep 30000      # 30s between runs (default)
bun run hillclimber --dry-run          # Show what would happen
bun run hillclimber --stats            # Show current stats
bun run hillclimber --export           # Export best hints to hints.ts
```

## Implementation Details

### Scoring Algorithm

```typescript
function scoreResult(passed: boolean, turns: number): number {
  const passBonus = passed ? 1000 : 0;
  const turnsScore = Math.max(0, 100 - turns); // Fewer turns = higher score
  return passBonus + turnsScore;
}

function isBetterThan(newScore: Score, oldScore: Score | null): boolean {
  if (!oldScore) return true;
  if (newScore.passed && !oldScore.passed) return true;
  if (newScore.passed === oldScore.passed && newScore.turns < oldScore.turns) return true;
  return false;
}
```

### Meta-Reasoner Prompt (Free Model)

```
You are a tiny tuning agent for a coding benchmark.

Task: {taskDescription}
Current hint: {currentHint || "none"}
Last run: {passed ? "PASSED" : "FAILED"} in {turns} turns
Step summary: {stepSummary}

Based on this, suggest ONE small change to the hint that might improve performance.
Reply with ONLY the new hint text (1-2 sentences max), or "KEEP" if no change needed.

Examples:
- "Use Python regex with lookbehind to capture dates; write directly to /app/regex.txt."
- "Read the log file first to understand the format."
- "KEEP"
```

### Heuristic Fallback (When Not Using Meta-Reasoner)

For deterministic fallback when OpenRouter is unavailable:

```typescript
function proposeHeuristicChange(taskId: string, config: TaskConfig, result: RunResult): TaskConfig {
  // If never passed and hint is empty, add a basic hint
  if (!result.passed && !config.hint) {
    return { ...config, hint: getDefaultHintForTask(taskId) };
  }

  // If passed but took many turns, suggest efficiency hint
  if (result.passed && result.turns > 20) {
    return { ...config, hint: config.hint + " Be direct and efficient." };
  }

  // If failed with specific error patterns, adjust hint
  const errorPatterns = extractErrorPatterns(result.stepSummary);
  if (errorPatterns.includes("file_not_found")) {
    return { ...config, hint: (config.hint || "") + " Check file paths carefully." };
  }

  // No change
  return config;
}
```

### Integration with Existing Systems

**FM Runner** (reuse existing):
```typescript
import { createModelRunner } from "../bench/model-adapter.js";
import { loadTerminalBenchSuite } from "../bench/terminal-bench.js";

const runner = createModelRunner({ type: "foundation-models", useMicroTask: true });
const suite = await loadTerminalBenchSuite(suitePath);
const task = suite.tasks.find(t => t.id === taskId);
const result = await runner.runTask(task, { workspace, timeout, maxTurns });
```

**OpenRouter Inference** (reuse existing):
```typescript
import { OpenRouterInference, OpenRouterInferenceLive } from "../llm/openrouter-inference.js";

const program = Effect.gen(function* () {
  const inference = yield* OpenRouterInference;
  const response = yield* inference.send(
    "x-ai/grok-4.1-fast:free",
    [{ role: "user", content: metaPrompt }],
    { temperature: 0.3 }
  );
  return response.choices[0]?.message?.content ?? "KEEP";
});
```

**Verification** (reuse existing):
```typescript
import { runTaskVerification } from "../bench/terminal-bench.js";
const verifyResult = await Effect.runPromise(runTaskVerification(task));
```

### Overnight Run Configuration

Add to `package.json`:
```json
{
  "scripts": {
    "hillclimber": "bun src/hillclimber/cli.ts",
    "hillclimber:overnight": "bun src/hillclimber/cli.ts --max-runs 500 --sleep 30000"
  }
}
```

## Starting Task Set (3-5 Easy Tasks)

Begin with easy/medium tasks to validate the system. Need to identify actual task IDs from the TB2 suite.

**Candidate tasks** (verify against actual suite):
1. **regex-log** (medium) - Write regex to file, conceptually simple
2. **word-count** or similar (easy) - Count words in file
3. **file-copy** or similar (easy) - Basic file operations
4. **text-transform** or similar (easy) - String manipulation
5. **json-parse** or similar (easy) - Parse/extract JSON data

**Initial config** (inserted via SQL on first run):
```sql
-- For each starting task, create initial config with no hint
INSERT INTO hillclimber_configs (task_id, hint, use_skills, max_turns_override, config_hash, is_current)
VALUES
  ('regex-log', NULL, 0, 30, '<computed>', 1),
  ('word-count', NULL, 0, 30, '<computed>', 1),
  ('file-copy', NULL, 0, 30, '<computed>', 1);
```

The CLI will auto-create default configs for specified tasks if they don't exist.

## Cost Management

- **Primary**: `x-ai/grok-4.1-fast:free` - Free, unlimited
- **Backup**: `openrouter/auto` - Use sparingly, rate limit to 1/10 calls
- **Tracking**: All calls logged via existing InferenceStore

```typescript
const shouldUseAutoModel = (runCount: number): boolean => {
  // Only use auto model every 10th run for complex analysis
  return runCount % 10 === 0;
};
```

## Logging & Observability

**Structured Logging**:
```
[HillClimber] Starting run #42 for task: regex-log
[HillClimber] Config: {"hint":"Use Python regex...","useSkills":false}
[FM] Running task with micro-task supervisor...
[FM] Turn 1: write_file(/app/regex.txt) - success
[FM] Turn 2: task_complete - signaled
[Verification] Running verification script...
[Verification] PASSED
[HillClimber] Result: PASSED in 2 turns (best so far!)
[MetaReasoner] Proposing config change...
[MetaReasoner] Suggestion: KEEP (already optimal)
[HillClimber] Saved state. Sleeping 30s...
```

**Work Log**:
Save periodic summaries to `docs/logs/YYYYMMDD/HHMM-hillclimber-log.md`

## Implementation Order

1. **Migration** (`003_hillclimber.sql`)
   - Create SQLite tables
   - Add indexes

2. **Types** (`types.ts`)
   - Define TypeScript interfaces
   - Row-to-object converters

3. **Store** (`store.ts`)
   - Effect-based SQLite service
   - CRUD operations for configs/runs
   - Best config tracking

4. **Scoring** (`scoring.ts`)
   - Score calculation
   - Comparison logic

5. **Executor** (`executor.ts`)
   - Wrap FM runner
   - Extract StepSummary
   - Handle verification

6. **Meta-Reasoner** (`meta-reasoner.ts`)
   - OpenRouter integration (free + auto)
   - Prompt construction
   - Change proposal parsing

7. **Exporter** (`exporter.ts`)
   - Export best hints to hints.ts
   - Threshold check (3+ passes)

8. **Runner** (`runner.ts`)
   - Main loop
   - Graceful shutdown
   - Round-robin task selection

9. **CLI** (`cli.ts`)
   - Parse args
   - Initialize Effect layers
   - Launch runner

## Success Criteria

After overnight run (~100+ iterations):
- At least one task should achieve consistent passes
- Config should converge (same hint repeated)
- SQLite tables should have clean run history
- Best configs should be tracked accurately
- No crashes or data corruption

## Files Modified (Existing)

- `src/fm/hints.ts` - Add exported hints from HillClimber
- `package.json` - Add `hillclimber` script

## Not in Initial Implementation

1. **Profile evolution** - Mutate entire config profiles (temperature, etc.)
2. **Baseline comparison** - Compare against episode baselines
3. **Dashboard** - Web UI for monitoring overnight runs
4. **Skill creation** - Turn configs into full Skills (beyond hints)
