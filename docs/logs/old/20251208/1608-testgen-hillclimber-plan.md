# TestGen HillClimber: Iterative Test Generation Evolution

## Overview

Convert the test generation service into an iterative HillClimber-style loop that:
1. **Runs** test generation sessions with configurable parameters
2. **Analyzes** results programmatically to extract quality metrics
3. **Meta-reasons** about what to change (prompts, parameters, strategies)
4. **Evolves** configs over time to improve test generation quality
5. **Persists** everything to SQLite for analysis and UI display

---

## Architecture Comparison

| Component | HillClimber (Tasks) | TestGen HillClimber |
|-----------|---------------------|---------------------|
| Runner | `runner.ts` | `testgen-runner.ts` (NEW) |
| Executor | `executor.ts` | `testgen-service.ts` (existing) |
| Store | `store.ts` | `testgen-store.ts` (NEW) |
| Meta-Reasoner | `meta-reasoner.ts` | `testgen-meta-reasoner.ts` (NEW) |
| Scoring | `scoring.ts` | `testgen-analyzer.ts` (NEW) |
| CLI | `cli.ts` | `testgen-cli.ts` (extend existing) |
| Config | `HillClimberConfig` | `TestGenConfig` (NEW) |

---

## Database Schema Extension

### New Tables (Migration: `005_testgen_evolution.sql`)

```sql
-- TestGen Configs (the "knobs" being tuned)
CREATE TABLE testgen_configs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  version TEXT NOT NULL,                    -- "1.0.0", "1.0.1", etc.

  -- Generation Parameters
  temperature REAL NOT NULL DEFAULT 0.3,
  max_tokens INTEGER NOT NULL DEFAULT 2048,
  min_tests_per_category INTEGER NOT NULL DEFAULT 2,
  max_tests_per_category INTEGER NOT NULL DEFAULT 5,
  max_rounds_per_category INTEGER NOT NULL DEFAULT 3,

  -- Strategy Weights (0-1)
  environment_weight REAL NOT NULL DEFAULT 0.7,
  anti_cheat_weight REAL NOT NULL DEFAULT 0.8,
  precision_weight REAL NOT NULL DEFAULT 0.6,

  -- Category Order (JSON array)
  category_order JSON NOT NULL DEFAULT '["anti_cheat","existence","correctness","boundary","integration"]',

  -- Prompt Templates (JSON)
  category_prompts JSON,                    -- Record<Category, string>
  anti_cheat_prompt TEXT,
  reflection_prompt TEXT,

  -- Model Selection
  primary_model TEXT NOT NULL DEFAULT 'local',  -- 'local' | 'claude'
  reflection_model TEXT NOT NULL DEFAULT 'local',

  -- Quality Thresholds
  min_comprehensiveness_score REAL NOT NULL DEFAULT 7.0,
  target_comprehensiveness_score REAL NOT NULL DEFAULT 8.5,

  -- Hash for deduplication
  config_hash TEXT NOT NULL,
  is_current INTEGER DEFAULT 0,

  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(config_hash)
);

-- TestGen Runs (record of every generation session)
CREATE TABLE testgen_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL UNIQUE,              -- "tg-YYYYMMDD-HHMMSS-random"
  session_id TEXT NOT NULL REFERENCES testgen_trajectories(session_id),
  config_id INTEGER NOT NULL REFERENCES testgen_configs(id),
  task_id TEXT NOT NULL,

  -- Results
  total_tests INTEGER NOT NULL,
  comprehensiveness_score REAL,
  duration_ms INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,

  -- Analysis Metrics (computed post-run)
  category_balance REAL,                    -- 0-1, how balanced across categories
  anti_cheat_coverage REAL,                 -- 0-1, coverage of prohibited tools
  parameter_discovery REAL,                 -- 0-1, coverage of discovered parameters
  reflection_effectiveness REAL,            -- 0-1, how much reflections improved tests
  token_efficiency REAL,                    -- comprehensiveness per 1k tokens

  -- Meta-reasoning
  meta_model TEXT,
  proposed_change TEXT,
  change_accepted INTEGER DEFAULT 0,

  -- Scoring
  score INTEGER NOT NULL,                   -- Computed quality score
  is_best INTEGER DEFAULT 0,

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- TestGen Best Configs (quick lookup per task type)
-- Uses "_global_" for global default, task-type names for overrides
CREATE TABLE testgen_best_configs (
  task_type TEXT PRIMARY KEY,               -- "_global_" | "conversion" | "implementation" | "debugging" | etc.
  config_id INTEGER NOT NULL REFERENCES testgen_configs(id),
  run_id INTEGER NOT NULL REFERENCES testgen_runs(id),
  score INTEGER NOT NULL,
  pass_count INTEGER DEFAULT 0,
  total_runs INTEGER DEFAULT 0,
  is_override INTEGER DEFAULT 0,            -- 1 if this beats global for this task type
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- TestGen Evolution History (track what changed and why)
CREATE TABLE testgen_evolution (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_config_id INTEGER REFERENCES testgen_configs(id),
  to_config_id INTEGER REFERENCES testgen_configs(id),

  changes JSON NOT NULL,                    -- What changed
  reasoning TEXT NOT NULL,                  -- Why (from meta-reasoner)
  expected_improvement TEXT,

  -- Results (filled after testing)
  actual_improvement REAL,
  quality_delta REAL,

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_tg_configs_current ON testgen_configs(is_current) WHERE is_current = 1;
CREATE INDEX idx_tg_runs_task ON testgen_runs(task_id);
CREATE INDEX idx_tg_runs_config ON testgen_runs(config_id);
CREATE INDEX idx_tg_runs_score ON testgen_runs(score);
CREATE INDEX idx_tg_runs_created ON testgen_runs(created_at);
```

---

## Implementation Plan

### Phase 1: Foundation (Core Types + Store)

**Files to Create:**

1. **`src/hillclimber/testgen-types.ts`** - Type definitions
   ```typescript
   export interface TestGenConfig {
     id: number;
     version: string;
     temperature: number;
     maxTokens: number;
     minTestsPerCategory: number;
     maxTestsPerCategory: number;
     maxRoundsPerCategory: number;
     environmentWeight: number;
     antiCheatWeight: number;
     precisionWeight: number;
     categoryOrder: TestCategory[];
     categoryPrompts?: Record<TestCategory, string>;
     antiCheatPrompt?: string;
     reflectionPrompt?: string;
     primaryModel: "local" | "claude";
     reflectionModel: "local" | "claude";
     minComprehensivenessScore: number;
     targetComprehensivenessScore: number;
     configHash: string;
     isCurrent: boolean;
     createdAt: string;
   }

   export interface TestGenRun {
     id: number;
     runId: string;
     sessionId: string;
     configId: number;
     taskId: string;
     totalTests: number;
     comprehensivenessScore: number | null;
     durationMs: number;
     totalTokens: number;
     categoryBalance: number | null;
     antiCheatCoverage: number | null;
     parameterDiscovery: number | null;
     reflectionEffectiveness: number | null;
     tokenEfficiency: number | null;
     metaModel: string | null;
     proposedChange: string | null;
     changeAccepted: boolean;
     score: number;
     isBest: boolean;
     createdAt: string;
   }

   export interface TestGenConfigChange {
     type: "keep" | "update_params" | "update_prompts" | "update_weights";
     changes?: Partial<TestGenConfig>;
     reasoning: string;
     model?: string;
   }
   ```

2. **`src/hillclimber/testgen-store.ts`** - SQLite persistence (Effect service)
   ```typescript
   export class TestGenStore extends Context.Tag("TestGenStore")<
     TestGenStore,
     {
       // Config operations
       saveConfig: (config: TestGenConfigInput) => Effect.Effect<TestGenConfig, DatabaseError>;
       getCurrentConfig: (taskType?: string) => Effect.Effect<TestGenConfig | null, DatabaseError>;
       setCurrentConfig: (configId: number) => Effect.Effect<void, DatabaseError>;
       ensureDefaultConfig: () => Effect.Effect<TestGenConfig, DatabaseError>;

       // Run operations
       saveRun: (run: TestGenRunInput) => Effect.Effect<TestGenRun, DatabaseError>;
       getRunHistory: (taskId: string, limit?: number) => Effect.Effect<TestGenRun[], DatabaseError>;
       getRecentRuns: (limit?: number) => Effect.Effect<TestGenRun[], DatabaseError>;

       // Best config operations
       getBestConfig: (taskType: string) => Effect.Effect<TestGenBestConfig | null, DatabaseError>;
       updateBestConfig: (...) => Effect.Effect<void, DatabaseError>;

       // Stats
       getStats: () => Effect.Effect<TestGenStats, DatabaseError>;
       getTaskStats: (taskId: string) => Effect.Effect<TestGenTaskStats, DatabaseError>;
     }
   >() {}
   ```

3. **`.openagents/migrations/005_testgen_evolution.sql`** - Database migration

---

### Phase 2: Analysis Engine

**Files to Create:**

1. **`src/hillclimber/testgen-analyzer.ts`** - Programmatic analysis
   ```typescript
   export interface TestGenAnalysis {
     categoryDistribution: Record<TestCategory, number>;
     categoryBalance: number;              // 0-1
     antiCheatCoverage: number;            // 0-1
     parameterDiscovery: number;           // 0-1
     reflectionEffectiveness: number;      // 0-1
     tokenEfficiency: number;              // comprehensiveness per 1k tokens
     overallScore: number;                 // Composite score (0-1000)
   }

   // Analysis functions
   export const analyzeTestGenRun: (
     trajectory: TestGenTrajectory,
     environment: EnvironmentInfo,
   ) => TestGenAnalysis;

   export const analyzeCategoryDistribution: (tests: GeneratedTest[]) => {...};
   export const analyzeAntiCheatCoverage: (tests: GeneratedTest[], env: EnvironmentInfo) => {...};
   export const analyzeReflectionEffectiveness: (reflections: Reflection[], tests: GeneratedTest[]) => {...};
   export const analyzeTokenEfficiency: (tokens: number, score: number) => {...};
   ```

2. **`src/hillclimber/testgen-scoring.ts`** - Scoring functions
   ```typescript
   // Score formula (0-1000 scale, matching HillClimber)
   // - Comprehensiveness (1-10) → 0-400 points
   // - Category balance (0-1) → 0-200 points
   // - Anti-cheat coverage (0-1) → 0-200 points
   // - Token efficiency (0-1) → 0-200 points

   export const scoreTestGenRun = (analysis: TestGenAnalysis): number => {
     const comprehensivenessScore = (analysis.comprehensivenessScore ?? 5) * 40;
     const balanceScore = analysis.categoryBalance * 200;
     const antiCheatScore = analysis.antiCheatCoverage * 200;
     const efficiencyScore = analysis.tokenEfficiency * 200;
     return Math.round(comprehensivenessScore + balanceScore + antiCheatScore + efficiencyScore);
   };
   ```

---

### Phase 3: Meta-Reasoner

**Files to Create:**

1. **`src/hillclimber/testgen-meta-reasoner.ts`** - LLM-based config proposals
   ```typescript
   export const proposeTestGenConfigChange = (
     config: TestGenConfig,
     recentRuns: TestGenRun[],
     lastAnalysis: TestGenAnalysis,
     taskType: string,
   ): Effect.Effect<TestGenConfigChange, InferenceError> =>
     Effect.gen(function* () {
       const history = aggregateRunHistory(recentRuns);
       const prompt = buildTestGenMetaPrompt(config, history, lastAnalysis, taskType);

       const inference = yield* OpenRouterInference;
       const response = yield* inference.send(FREE_MODEL, [{ role: "user", content: prompt }]);

       return parseTestGenConfigChange(response);
     });
   ```

   **Meta-prompt structure:**
   ```
   You are optimizing a test generation system.

   Current config:
   - Temperature: 0.3
   - Min tests per category: 2
   - Anti-cheat weight: 0.8

   Recent performance (last 5 runs):
   - Run 1: Score 650, balance=0.7, anti-cheat=0.9, efficiency=0.5
   - Run 2: Score 720, balance=0.8, anti-cheat=0.8, efficiency=0.6
   ...

   Patterns observed:
   - Anti-cheat coverage declining
   - Token efficiency improving
   - Category balance inconsistent

   What should we change to improve overall quality?
   Return JSON: { "type": "...", "changes": {...}, "reasoning": "..." }
   ```

---

### Phase 4: Runner Loop

**Files to Create:**

1. **`src/hillclimber/testgen-runner.ts`** - Main evolution loop
   ```typescript
   export interface TestGenRunnerOptions {
     taskId?: string;                // Specific task, or random
     taskType?: string;              // "conversion", "implementation", etc.
     maxRuns: number;
     sleepMs: number;
     suitePath: string;
     modelOverride?: string;
     dryRun?: boolean;
   }

   export const runTestGenEvolution = async (options: TestGenRunnerOptions): Promise<void> => {
     const state: RunnerState = { totalRuns: 0, running: true };

     while (state.running && state.totalRuns < options.maxRuns) {
       await runSingleIteration(options, state);
       await sleep(options.sleepMs);
       state.totalRuns++;
     }
   };

   const runSingleIteration = async (options, state): Promise<void> => {
     // 1. Get current config
     const config = await Effect.runPromise(
       store.ensureDefaultConfig().pipe(Effect.provide(TestGenStoreLive))
     );

     // 2. Pick task (random or specified)
     const taskId = options.taskId ?? pickRandomTask(options.suitePath);

     // 3. Run test generation
     const sessionId = generateSessionId();
     await runTestGenWithStreaming(suitePath, taskId, sessionId, silentEmitter, {
       model: config.primaryModel,
       temperature: config.temperature,
       // ... pass config parameters
     });

     // 4. Analyze results
     const trajectory = await getTrajectory(sessionId);
     const analysis = analyzeTestGenRun(trajectory, trajectory.environment);
     const score = scoreTestGenRun(analysis);

     // 5. Save run with analysis
     const run = await store.saveRun({
       runId: generateRunId(),
       sessionId,
       configId: config.id,
       taskId,
       ...analysis,
       score,
     });

     // 6. Meta-reason about improvements
     const change = await proposeTestGenConfigChange(config, recentRuns, analysis, taskType);

     // 7. Apply config change if proposed
     if (change.type !== "keep") {
       const newConfig = applyConfigChange(config, change);
       await store.saveConfig(newConfig);
       await store.setCurrentConfig(newConfig.id);
     }

     // 8. Update best config if this is better
     await updateBestIfBetter(taskType, config.id, run, score);

     // 9. Log progress
     log(`Run ${state.totalRuns}: score=${score}, change=${change.type}`);
   };
   ```

---

### Phase 5: CLI Extension

**Files to Modify:**

1. **`src/hillclimber/test-gen-cli.ts`** - Extend with evolution commands
   ```typescript
   // Existing commands:
   // bun run testgen --task regex-log
   // bun run testgen --random

   // New commands:
   // bun run testgen:evolve --max-runs 50 --sleep 30000
   // bun run testgen:evolve --task-type conversion --max-runs 20
   // bun run testgen:stats
   // bun run testgen:export
   // bun run testgen:config --show
   // bun run testgen:config --reset
   ```

   **CLI Options:**
   ```
   testgen:evolve
     --max-runs <n>      Maximum evolution runs (default: 100)
     --sleep <ms>        Sleep between runs (default: 10000)
     --task <id>         Specific task ID
     --task-type <type>  Task type filter (conversion, implementation, etc.)
     --model <name>      Model override
     --dry-run           Preview without executing

   testgen:stats
     --json              Output as JSON
     --task <id>         Stats for specific task
     --since <date>      Stats since date

   testgen:export
     --format <fmt>      Export format (json, csv)
     --output <path>     Output file path
   ```

---

### Phase 6: UI Integration

**Files to Create/Modify:**

1. **`src/effuse/widgets/tb-command-center/tbcc-testgen-evolution.ts`** - Evolution dashboard widget
   - Quality trends chart (score over time)
   - Config comparison view
   - Run history with analysis metrics
   - Manual override controls

2. **`src/hud/protocol.ts`** - Add evolution message types
   ```typescript
   export interface TestGenEvolutionProgressMessage {
     type: "testgen_evolution_progress";
     runNumber: number;
     totalRuns: number;
     currentScore: number;
     bestScore: number;
     lastChange: TestGenConfigChange;
   }
   ```

---

## Data Flow Summary

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      TESTGEN EVOLUTION LOOP                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. GET CONFIG                                                          │
│     testgen_configs → current config (params, prompts, weights)        │
│                                                                         │
│  2. RUN GENERATION                                                      │
│     testgen-service.ts → generate tests with config                    │
│     → testgen_trajectories (saved automatically)                        │
│                                                                         │
│  3. ANALYZE                                                             │
│     testgen-analyzer.ts → compute metrics                               │
│     (category_balance, anti_cheat_coverage, token_efficiency, etc.)    │
│                                                                         │
│  4. SCORE                                                               │
│     testgen-scoring.ts → compute composite score (0-1000)              │
│                                                                         │
│  5. SAVE RUN                                                            │
│     testgen_runs → run record with analysis + score                    │
│                                                                         │
│  6. META-REASON                                                         │
│     testgen-meta-reasoner.ts → propose config change                   │
│     (LLM analyzes history, proposes: update_params | update_prompts)   │
│                                                                         │
│  7. APPLY CHANGE                                                        │
│     testgen_configs → save new config if changed                       │
│     testgen_evolution → record change + reasoning                      │
│                                                                         │
│  8. UPDATE BEST                                                         │
│     testgen_best_configs → update if score is higher                   │
│                                                                         │
│  9. REPEAT                                                              │
│     Sleep → next iteration                                              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Files to Create/Modify

### New Files (8)
1. `.openagents/migrations/005_testgen_evolution.sql` - Database schema
2. `src/hillclimber/testgen-types.ts` - Type definitions
3. `src/hillclimber/testgen-store.ts` - SQLite persistence (Effect)
4. `src/hillclimber/testgen-analyzer.ts` - Analysis engine
5. `src/hillclimber/testgen-scoring.ts` - Scoring functions
6. `src/hillclimber/testgen-meta-reasoner.ts` - LLM-based proposals
7. `src/hillclimber/testgen-runner.ts` - Main evolution loop
8. `src/effuse/widgets/tb-command-center/tbcc-testgen-evolution.ts` - UI widget

### Modify Files (3)
1. `src/hillclimber/test-gen-cli.ts` - Add evolution commands
2. `src/hillclimber/testgen-service.ts` - Accept config parameters
3. `src/hud/protocol.ts` - Add evolution message types

---

## Implementation Order

1. **Database + Types** (Phase 1)
   - Migration file
   - Type definitions
   - Store service

2. **Analysis + Scoring** (Phase 2)
   - Analyzer functions
   - Scoring formula
   - Integrate with existing persistence

3. **Meta-Reasoner** (Phase 3)
   - Prompt design
   - Response parsing
   - Config change application

4. **Runner Loop** (Phase 4)
   - Main loop logic
   - Signal handling
   - Logging

5. **CLI** (Phase 5)
   - Command parsing
   - Stats display
   - Export functionality

6. **UI** (Phase 6)
   - Evolution dashboard widget
   - HUD protocol messages
   - Charts and controls

---

## Design Decisions (User Confirmed)

1. **Config Scope: Hybrid**
   - Global default config optimized across all tasks
   - Task-type-specific overrides (conversion, implementation, debugging) when they outperform global
   - `testgen_best_configs` table has both `_global_` entry and task-type entries

2. **Implementation Priority: CLI First**
   - Phase 1-5: CLI fully working (`testgen:evolve`, `testgen:stats`, etc.)
   - Phase 6: UI dashboard (can start in parallel once CLI foundation is stable)

3. **Execution Mode: Both**
   - Manual: `bun run testgen:evolve --max-runs 10`
   - Background: `bun run testgen:evolve --daemon` (persistent process like HillClimber overnight)

---

## Success Criteria

- [ ] `bun run testgen:evolve --max-runs 10` completes without error
- [ ] All runs saved to `testgen_runs` table with analysis metrics
- [ ] Meta-reasoner proposes meaningful config changes
- [ ] Hybrid config: global default + task-type overrides
- [ ] Stats command shows quality trends (`testgen:stats`)
- [ ] Background daemon mode works (`testgen:evolve --daemon`)
- [ ] UI displays evolution progress in real-time
