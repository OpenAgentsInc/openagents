# HillClimber Analysis - Current Status

## What's Happening

HillClimber is running an optimization loop to find better hints/configs for Terminal Bench tasks. Currently in **early exploration phase** - all runs are failing.

## Current Status (from stats)

- **Total runs**: 13
- **Total passes**: 0 (0% pass rate)
- **Tasks being optimized**: 5 tasks (path-tracing, model-extraction-relu-logits, video-processing, dna-assembly, regex-log)
- **Best scores**: 84-89 (all failing - no pass bonus)

## Scoring System

Scores are calculated as:
- **PASS bonus**: +1000 points
- **Turn efficiency**: 100 - turns (capped at 0)
- **Total score**: passBonus + turnScore

**Examples:**
- Pass in 5 turns = 1000 + 95 = **1095** ✅
- Pass in 50 turns = 1000 + 50 = **1050** ✅
- Fail in 5 turns = 0 + 95 = **95** ❌
- Fail in 11 turns = 0 + 89 = **89** ❌

**Current scores (84-89)** mean:
- All runs are **failing** (no 1000 bonus)
- Using 11-16 turns per run
- System is exploring baseline performance

## Data Storage

All runs are stored in SQLite database:
- **Location**: `.openagents/openagents.db`
- **Tables**:
  - `hillclimber_runs` - Every execution attempt
  - `hillclimber_configs` - Configurations (hints, skills, maxTurns)
  - `hillclimber_best_configs` - Best config per task

## How to Inspect

### 1. View Statistics

```bash
bun run hillclimber --stats
```

Shows:
- Total runs, passes, pass rate
- Per-task breakdown
- Best configs found so far

### 2. Query Database Directly

```bash
# View recent runs
sqlite3 .openagents/openagents.db "
SELECT
  run_id,
  task_id,
  passed,
  turns,
  score,
  proposed_change,
  change_accepted,
  created_at
FROM hillclimber_runs
ORDER BY created_at DESC
LIMIT 20;
"

# View all configs tried
sqlite3 .openagents/openagents.db "
SELECT
  id,
  task_id,
  hint,
  use_skills,
  max_turns_override,
  is_current,
  created_at
FROM hillclimber_configs
ORDER BY created_at DESC;
"

# View best configs per task
sqlite3 .openagents/openagents.db "
SELECT
  task_id,
  score,
  pass_count,
  total_runs,
  updated_at
FROM hillclimber_best_configs
ORDER BY score DESC;
"
```

### 3. View Detailed Run Info

```bash
# Get step summaries and error messages for a specific run
sqlite3 .openagents/openagents.db "
SELECT
  run_id,
  task_id,
  step_summary,
  error_message,
  score,
  turns
FROM hillclimber_runs
WHERE task_id = 'path-tracing'
ORDER BY created_at DESC
LIMIT 5;
"
```

## Issue: Meta-Reasoner Not Proposing Changes

**Problem**: All runs show `proposed_change = "Empty response"` and `change_accepted = 0`

**Root cause**: The meta-reasoner (Grok model via OpenRouter) is returning empty responses. This could be:
1. API rate limiting
2. Model returning empty content
3. Network/connection issues
4. Prompt format issues

**Location**: `src/hillclimber/meta-reasoner.ts` lines 194-228

**Current behavior**: When response is empty, it defaults to `type: "keep"` with reasoning "Empty response"

## What's Valuable

Even though all runs are failing, the system is:
1. **Collecting baseline data** - Understanding how tasks fail without hints
2. **Tracking turn counts** - Measuring efficiency (11-16 turns per run)
3. **Building history** - Each run adds to the database for future meta-reasoning
4. **Testing infrastructure** - Verifying the optimization loop works

## Next Steps

1. **Fix meta-reasoner**: Investigate why Grok is returning empty responses
2. **Add fallback hints**: Use heuristic hints when meta-reasoner fails (see `proposeHeuristicChange`)
3. **Wait for passes**: Once a task passes, the system can learn from successful configs
4. **Monitor progress**: Use `--stats` to track improvements over time

## Export Learned Hints

Once tasks start passing consistently:

```bash
# Export best hints to a file
bun run hillclimber --export

# Generate TypeScript code for hints
bun run hillclimber --export-code
```

## Useful Commands

```bash
# Run optimization loop
bun run hillclimber --max-runs 500 --sleep 30000

# Optimize specific task
bun run hillclimber --task regex-log --max-runs 50

# Check progress (non-blocking)
bun run hillclimber --stats

# Preview what would happen
bun run hillclimber --dry-run
```
