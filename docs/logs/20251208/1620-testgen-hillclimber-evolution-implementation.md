# TestGen HillClimber Evolution System - Full Implementation

**Date:** 2025-12-08
**Time:** 16:20 CT
**Task:** Implement complete TestGen HillClimber evolution system per plan

## Overview

Fully implemented the TestGen HillClimber evolution system as specified in `docs/logs/20251208/1608-testgen-hillclimber-plan.md`. This creates a miniature HillClimber loop specifically for optimizing test generation quality over time through iterative evolution.

**Core Concept:** Just as HillClimber optimizes task execution configs (hints, skills, turns), TestGen HillClimber optimizes test generation itself:
1. **Runs** test generation sessions with configurable parameters
2. **Analyzes** results programmatically to extract quality metrics
3. **Meta-reasons** about what to change (prompts, parameters, strategies)
4. **Evolves** configs over time to improve test generation quality
5. **Persists** everything to SQLite for analysis and UI display

---

## Implementation Summary

### Phase 1: Foundation (Database + Types + Store) ✅

**Files Created:**
- `.openagents/migrations/005_testgen_evolution.sql` - Database schema
- `src/hillclimber/testgen-types.ts` - Type definitions
- `src/hillclimber/testgen-store.ts` - SQLite persistence (Effect service)

**Database Schema:**
- `testgen_configs` - Stores all test generation configurations (the "knobs" being tuned)
- `testgen_runs` - Records every generation session with analysis metrics
- `testgen_best_configs` - Quick lookup for best performing config per task type
- `testgen_evolution` - Tracks config changes and their impact

**Key Features:**
- Config deduplication via `config_hash` (SHA256)
- Current config tracking via `is_current` flag
- Hybrid config: global default (`_global_`) + task-type-specific overrides
- Quality scoring: 0-1000 scale (comprehensiveness + balance + coverage + efficiency)

### Phase 2: Analysis Engine ✅

**Files Created:**
- `src/hillclimber/testgen-analyzer.ts` - Programmatic analysis functions
- `src/hillclimber/testgen-scoring.ts` - Scoring formula (0-1000 scale)

**Analysis Capabilities:**
1. **Category Distribution Analysis** - Counts tests by category, calculates balance (0-1)
2. **Anti-Cheat Coverage Analysis** - Checks if prohibited tools have anti-cheat tests (0-1)
3. **Parameter Discovery Analysis** - Verifies tests use parameters from file previews (0-1)
4. **Reflection Effectiveness Analysis** - Measures if reflections led to new tests (0-1)
5. **Token Efficiency Analysis** - Computes comprehensiveness per 1k tokens (0-1)

**Scoring Formula (0-1000 points):**
- Comprehensiveness (1-10) → 0-400 points (40 points per point)
- Category balance (0-1) → 0-200 points
- Anti-cheat coverage (0-1) → 0-200 points
- Token efficiency (0-1) → 0-200 points

### Phase 3: Meta-Reasoner ✅

**Files Created:**
- `src/hillclimber/testgen-meta-reasoner.ts` - LLM-based config proposals

**Features:**
- Uses OpenRouter free models (same pattern as HillClimber meta-reasoner)
- Analyzes recent runs and proposes config changes
- Change types: `keep`, `update_params`, `update_prompts`, `update_weights`
- Validates and sanitizes proposed changes
- Version incrementing for config evolution

**Meta-Prompt Structure:**
- Current config parameters
- Recent performance (last 5 runs with scores and metrics)
- Patterns observed (low balance, low coverage, etc.)
- Task type context
- Returns JSON with change proposal and reasoning

### Phase 4: Runner Loop ✅

**Files Created:**
- `src/hillclimber/testgen-runner.ts` - Main evolution loop

**Evolution Loop Steps:**
1. Get current config (global or task-type-specific)
2. Pick task (random or specified)
3. Run test generation (silent emitter for evolution mode)
4. Get trajectory from database
5. Analyze results (category balance, anti-cheat, efficiency, etc.)
6. Score the run (0-1000)
7. Save run with analysis metrics
8. Meta-reason about improvements
9. Apply config change if proposed
10. Update best config if this is better
11. Repeat

**Features:**
- Graceful shutdown (SIGINT/SIGTERM)
- Error recovery (continues to next iteration on failure)
- Configurable sleep between runs
- Dry-run mode for preview

### Phase 5: CLI & API ✅

**Files Modified:**
- `src/hillclimber/test-gen-cli.ts` - Extended with evolution commands

**Files Created:**
- `src/hillclimber/testgen-api.ts` - Programmatic API

**CLI Commands:**
- `bun run src/hillclimber/test-gen-cli.ts:evolve` - Run evolution loop
- `bun run src/hillclimber/test-gen-cli.ts:stats` - Show statistics
- `bun run src/hillclimber/test-gen-cli.ts --task <id>` - Generate tests (existing)

**API Functions:**
- `generateTests(options)` - Simple API, returns all tests when complete
- `generateTestsWithCallbacks(options)` - Advanced API with streaming callbacks
- `getCurrentConfig(taskType?)` - Get current config
- `ensureDefaultConfig()` - Ensure default config exists

---

## Testing via CLI

### 1. Basic Test Generation (Existing Functionality)

```bash
# Generate tests for a specific task
bun run src/hillclimber/test-gen-cli.ts --task regex-log

# Use local model
bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local

# Verbose output
bun run src/hillclimber/test-gen-cli.ts --task regex-log --verbose
```

### 2. View Statistics

```bash
# Show overall stats
bun run src/hillclimber/test-gen-cli.ts:stats

# JSON output
bun run src/hillclimber/test-gen-cli.ts:stats --json
```

**Expected Output:**
```
=== TestGen Evolution Stats ===
Total runs: 0
Total configs: 0
Average score: 0/1000
Best score: 0/1000
Average comprehensiveness: 0.0
Average token efficiency: 0.00
Config evolutions: 0
```

### 3. Run Evolution Loop

```bash
# Basic evolution (10 runs, 10s sleep)
bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 10 --sleep 10000

# Specific task
bun run src/hillclimber/test-gen-cli.ts:evolve --task regex-log --max-runs 5

# Task type filter
bun run src/hillclimber/test-gen-cli.ts:evolve --task-type conversion --max-runs 20

# Longer run (overnight)
bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 100 --sleep 30000

# Dry run (preview without executing)
bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 5 --dry-run
```

**What Happens:**
1. Creates default config if none exists
2. Picks a task (random or specified)
3. Runs test generation (saves to `testgen_trajectories`)
4. Analyzes results (category balance, anti-cheat, efficiency)
5. Scores the run (0-1000)
6. Saves run to `testgen_runs` with all metrics
7. Meta-reasons about improvements (LLM call)
8. Creates new config if change proposed
9. Updates best config if score is higher
10. Sleeps and repeats

**Console Output:**
```
[TestGenRunner] Starting evolution loop (max runs: 10)
[TestGenRunner] Starting run 1/10 (tg-20251208-162000-abc123)
[TestGenRunner] Using config v1.0.0 (id: 1)
[TestGenRunner] Selected task: regex-log
[TestGenRunner] Analysis: score=650, balance=0.70, anti-cheat=0.85, efficiency=0.55
[TestGenRunner] Meta-reasoner proposed: update_params
[TestGenRunner] Created new config v1.0.1 (id: 2)
[TestGenRunner] Run 1 complete: score=650, change=update_params
[TestGenRunner] Sleeping 10000ms before next run...
...
[TestGenRunner] Evolution complete (10 runs)
```

### 4. Verify Database State

```bash
# Check SQLite database
sqlite3 .openagents/openagents.db

# View configs
SELECT id, version, temperature, min_tests_per_category, max_tests_per_category, is_current FROM testgen_configs;

# View runs
SELECT run_id, task_id, score, comprehensiveness_score, category_balance, anti_cheat_coverage FROM testgen_runs ORDER BY created_at DESC LIMIT 10;

# View best configs
SELECT task_type, config_id, score, total_runs FROM testgen_best_configs;

# View evolution history
SELECT from_config_id, to_config_id, reasoning FROM testgen_evolution ORDER BY created_at DESC LIMIT 5;
```

### 5. Test Programmatic API

Create a test file `test-testgen-api.ts`:

```typescript
import { generateTests, getCurrentConfig } from "./src/hillclimber/testgen-api.js";

// Simple API
const result = await generateTests({
  taskId: "regex-log",
  model: "local",
});

console.log(`Generated ${result.totalTests} tests`);
console.log(`Comprehensiveness: ${result.comprehensivenessScore}`);
console.log(`Tokens used: ${result.totalTokensUsed}`);

// With callbacks
await generateTestsWithCallbacks({
  taskId: "regex-log",
  onTest: (test) => console.log(`Test: ${test.id}`),
  onProgress: (progress) => console.log(`Progress: ${progress.status}`),
  onComplete: (result) => console.log(`Complete: ${result.totalTests} tests`),
});

// Get config
const config = await getCurrentConfig();
console.log(`Current config: v${config?.version}`);
```

Run it:
```bash
bun run test-testgen-api.ts
```

### 6. Integration Testing

**Test the full evolution loop:**
```bash
# Run 3 iterations to see evolution in action
bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 3 --sleep 5000

# Check that configs evolved
sqlite3 .openagents/openagents.db "SELECT id, version, temperature, is_current FROM testgen_configs;"

# Check that runs were saved
sqlite3 .openagents/openagents.db "SELECT COUNT(*) as total_runs FROM testgen_runs;"

# Check that best config was updated
sqlite3 .openagents/openagents.db "SELECT * FROM testgen_best_configs;"
```

**Expected Results:**
- At least 1 config created (default)
- 3 runs saved to `testgen_runs`
- Analysis metrics populated (category_balance, anti_cheat_coverage, etc.)
- Scores calculated (0-1000)
- Best config updated if any run scored higher
- Evolution history recorded if config changed

### 7. Error Handling Tests

```bash
# Test with invalid task (should handle gracefully)
bun run src/hillclimber/test-gen-cli.ts:evolve --task invalid-task --max-runs 1

# Test with very short sleep (stress test)
bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 5 --sleep 1000

# Test interruption (Ctrl+C should shutdown gracefully)
bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 100 --sleep 5000
# Press Ctrl+C after a few runs
```

### 8. Performance Testing

```bash
# Measure time per iteration
time bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 1 --sleep 0

# Check database size
ls -lh .openagents/openagents.db

# Check trajectory size
sqlite3 .openagents/openagents.db "SELECT session_id, total_tests, total_tokens_used FROM testgen_trajectories ORDER BY created_at DESC LIMIT 1;"
```

---

## Database Schema Details

### testgen_configs
Stores all configurations tried during evolution:
- `id` - Primary key
- `version` - Semantic version (1.0.0, 1.0.1, etc.)
- `temperature`, `max_tokens`, `min_tests_per_category`, etc. - Generation parameters
- `environment_weight`, `anti_cheat_weight`, `precision_weight` - Strategy weights
- `category_order` - JSON array of category priority
- `config_hash` - SHA256 hash for deduplication
- `is_current` - Flag for current active config

### testgen_runs
Records every generation session:
- `run_id` - Unique run identifier
- `session_id` - Links to `testgen_trajectories` table
- `config_id` - Config used for this run
- `task_id` - Task that was tested
- `total_tests`, `comprehensiveness_score`, `total_tokens` - Results
- `category_balance`, `anti_cheat_coverage`, etc. - Analysis metrics
- `score` - Computed quality score (0-1000)
- `is_best` - Flag for best run

### testgen_best_configs
Quick lookup for best config per task type:
- `task_type` - "_global_" or specific type (conversion, implementation, etc.)
- `config_id` - Best performing config
- `run_id` - Run that achieved best score
- `score` - Best score achieved
- `total_runs` - Number of runs with this config
- `is_override` - Whether this beats global default

### testgen_evolution
Tracks config changes:
- `from_config_id` - Previous config
- `to_config_id` - New config
- `changes` - JSON object of what changed
- `reasoning` - Why the change was made (from meta-reasoner)
- `actual_improvement` - Filled after testing (quality delta)

---

## Integration Points

### With Existing Systems

1. **TestGen Service** (`testgen-service.ts`)
   - Already saves trajectories to `testgen_trajectories`
   - Runner reads from this table for analysis

2. **Database Service** (`database.ts`)
   - Migration system runs `005_testgen_evolution.sql` automatically
   - TestGen store uses same database file

3. **HillClimber**
   - Can use evolved testgen configs for blind verification
   - TestGen quality improvements → better HillClimber pass rates

4. **UI (Future)**
   - Evolution dashboard widget can query `testgen_runs` and `testgen_configs`
   - Display quality trends, config comparisons, run history

---

## Success Metrics

### Short-Term (Immediate)
- ✅ All generations automatically analyzed
- ✅ Quality metrics calculated and stored
- ✅ Meta-reasoner proposes config changes
- ✅ Config evolution tracked in database

### Medium-Term (After Running)
- ⏳ Comprehensiveness scores improve over time
- ⏳ Category coverage becomes more balanced
- ⏳ Anti-cheat coverage increases for conversion tasks
- ⏳ Token efficiency improves (same quality, fewer tokens)

### Long-Term (Goal)
- ⏳ Test generation quality matches or exceeds real TB2 tests
- ⏳ HillClimber pass rates improve (better self-tests → better blind verification)
- ⏳ System generalizes across task types
- ⏳ Programmatic API enables integration with other systems

---

## Known Limitations & Future Work

### Current Limitations
1. **No UI Dashboard** - Phase 4 UI widget not yet implemented (optional per plan)
2. **No Comparison with Real Tests** - Can't validate against actual TB2 tests (would break blindness)
3. **Meta-Reasoning Cost** - Uses free models but still has token costs
4. **Single Database** - All runs in one SQLite file (could get large over time)

### Future Enhancements
1. **UI Dashboard** (`tbcc-testgen-evolution.ts`)
   - Quality trends chart
   - Config comparison view
   - Run history with analysis metrics
   - Manual override controls

2. **Advanced Analysis**
   - Comparison with real TB2 tests (development mode only)
   - Overfitting detection
   - Holdout task validation

3. **Optimization**
   - Batch analysis (analyze multiple sessions together)
   - Cache analysis results
   - Periodic evolution (not every generation)

4. **Integration**
   - HillClimber can call testgen API for blind verification setup
   - CI/CD can generate tests for new tasks
   - Research can batch-generate tests for analysis

---

## Files Created/Modified

### New Files (8)
1. `.openagents/migrations/005_testgen_evolution.sql` - Database schema
2. `src/hillclimber/testgen-types.ts` - Type definitions
3. `src/hillclimber/testgen-store.ts` - SQLite persistence
4. `src/hillclimber/testgen-analyzer.ts` - Analysis engine
5. `src/hillclimber/testgen-scoring.ts` - Scoring functions
6. `src/hillclimber/testgen-meta-reasoner.ts` - Meta-reasoning
7. `src/hillclimber/testgen-runner.ts` - Evolution loop
8. `src/hillclimber/testgen-api.ts` - Programmatic API

### Modified Files (1)
1. `src/hillclimber/test-gen-cli.ts` - Extended with evolution commands

---

## Next Steps

1. **Run Initial Evolution** - Start with 10-20 runs to establish baseline
2. **Monitor Quality Trends** - Check if scores improve over time
3. **Tune Meta-Prompts** - Refine meta-reasoner prompts based on results
4. **Build UI Dashboard** - Create evolution dashboard widget (Phase 4)
5. **Integrate with HillClimber** - Use evolved configs in blind verification

---

## Related Documentation

- `docs/logs/20251208/1608-testgen-hillclimber-plan.md` - Original implementation plan
- `docs/logs/20251208/1600-trajectory-persistence-implementation.md` - Trajectory persistence
- `docs/logs/20251208/1325-hillclimber-v3-plan.md` - HillClimber v3 architecture
- `src/hillclimber/meta-reasoner.ts` - HillClimber meta-reasoning (reference)
- `src/hillclimber/store.ts` - HillClimber store (reference pattern)

---

**Status:** ✅ Implementation Complete - Ready for Testing
**Commit:** `fa5646580` - "Implement TestGen HillClimber evolution system"
**Lines Added:** ~2,450 insertions across 13 files
