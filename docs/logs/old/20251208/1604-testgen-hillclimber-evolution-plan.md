# Test Generation Service: Miniature HillClimber Evolution Plan

**Date:** 2025-12-08
**Time:** 16:15 CT
**Context:** Test generation is critical to HillClimber's success. We need a self-improving system.

## Executive Summary

Test generation is the **foundation** of HillClimber's blind verification architecture. Without high-quality, environment-aware test generation, the entire optimization loop fails. We've built the infrastructure (UI, streaming, persistence), but now need to evolve it into a **self-improving system** that learns from every generation to produce better tests over time.

**Vision:** A miniature HillClimber loop specifically for test generation - each generation run improves the next through programmatic analysis of what worked, what didn't, and why.

---

## HillClimber Context: Why Test Generation Matters

### HillClimber Architecture Overview

HillClimber is an overnight optimization system that:
1. **Runs tasks** with different configurations (hints, skills, turn limits)
2. **Scores results** (pass/fail + turn efficiency)
3. **Meta-reasons** about what to change (via OpenRouter LLM)
4. **Iterates** to find optimal configs
5. **Exports** learned hints when stable

**Critical Insight:** HillClimber's success depends on **blind verification** - the agent must pass real benchmark tests without seeing them. This requires:
- **Self-generated tests** that the agent can see and iterate on
- **Blind verification** that only reveals pass/fail at the end
- **Test quality** that matches or exceeds real benchmark tests

### The Test Generation Problem

From HillClimber v3 plan analysis:

**Current State:**
- Test generation only uses task descriptions (~40% alignment with real tests)
- Missing critical context: environment, prohibited tools, file structure
- Missing anti-cheat tests (e.g., "R should NOT be installed" for R→Python conversion)
- Missing parameter discovery (can't see R scripts to know all parameters)
- Generic tests instead of specific precision bounds

**Required State:**
- Environment-aware generation (container introspection)
- Anti-cheat reasoning ("what would a lazy implementation do?")
- Parameter discovery from file previews
- Specific precision bounds (not generic ranges)
- Category balance (existence, format, correctness, boundary, edge cases, integration)

**The Gap:** Without high-quality test generation, HillClimber agents:
- Pass self-tests but fail blind verification (test quality gap)
- Miss critical edge cases (anti-cheat, boundary conditions)
- Waste turns on false-positive tests
- Can't prove generalization (self-tests don't match real tests)

---

## What We've Built So Far

### 1. Environment-Aware Test Generation

**Files:**
- `src/hillclimber/environment-info.ts` - EnvironmentInfo types
- `src/hillclimber/environment-info.ts` - Environment introspection
- `src/hillclimber/test-generator.ts` - Environment-aware generation
- `src/hillclimber/test-generator-iterative.ts` - Multi-round iterative generation

**Features:**
- Container introspection (languages, packages, tools, files)
- Prohibited tool inference (anti-cheat detection)
- File preview extraction (parameter discovery)
- Category-based generation (anti_cheat, existence, correctness, boundary, integration)
- Self-reflection and gap analysis
- Comprehensiveness scoring (1-10 scale)

**Status:** ✅ Implemented and working

### 2. Streaming UI Integration

**Files:**
- `src/hillclimber/testgen-service.ts` - Streaming service wrapper
- `src/effuse/widgets/tb-command-center/tbcc-testgen.ts` - UI widget
- `src/hud/protocol.ts` - HUD message types
- `src/desktop/handlers.ts` - Desktop request handlers

**Features:**
- Real-time streaming of tests as they're generated
- Progress updates (phase, category, round)
- Reflection messages (gap analysis)
- ATIF-style chronological thread display
- Environment context panel
- Test cards with category badges, confidence bars
- Final stats (total tests, rounds, comprehensiveness, tokens)

**Status:** ✅ Implemented and working

### 3. Database Persistence

**Files:**
- `.openagents/migrations/004_trajectories.sql` - Migration
- `src/storage/database.ts` - DatabaseService with `insertTestGenTrajectory`
- `src/hillclimber/testgen-service.ts` - Auto-save on completion

**Features:**
- Every generation session saved to SQLite
- Dual-table design: `trajectories` (generic) + `testgen_trajectories` (specialized)
- Full data capture: tests, reflections, environment, uncertainties, tokens, scores
- Indexed for efficient querying (task, date, score, tokens)
- Full-text search on trajectory content

**Status:** ✅ Implemented and working

### 4. Iterative Generation with Reflection

**Files:**
- `src/hillclimber/test-generator-iterative.ts` - Core iterative logic

**Features:**
- Multi-round generation per category
- Self-reflection after each round ("what gaps remain?")
- Global refinement phase
- Comprehensiveness scoring
- Category-specific round limits
- Token budget management

**Status:** ✅ Implemented and working

---

## The Evolution Vision: Miniature HillClimber for Test Generation

### Core Concept

Just as HillClimber optimizes task execution configs (hints, skills, turns), we need a **TestGen HillClimber** that optimizes test generation itself:

```
┌─────────────────────────────────────────────────────────────────┐
│              TEST GENERATION HILLCLIMBER LOOP                    │
│                                                                  │
│  1. Generate tests (with current "testgen config")              │
│  2. Evaluate quality (programmatic analysis)                    │
│  3. Meta-reason about improvements (what worked? what didn't?) │
│  4. Update testgen config (prompts, parameters, strategies)    │
│  5. Iterate to improve quality over time                        │
└─────────────────────────────────────────────────────────────────┘
```

### What Gets Optimized

**TestGen Config Knobs:**
1. **Prompt templates** - Category-specific prompts, anti-cheat reasoning, parameter discovery
2. **Generation parameters** - Temperature, max tokens, min/max tests per category
3. **Reflection strategies** - When to reflect, what to ask, how many rounds
4. **Category weights** - Balance between existence, correctness, boundary, etc.
5. **Environment usage** - How much to rely on environment vs description
6. **Model selection** - Claude vs local FM for different phases

**Quality Metrics:**
1. **Alignment with real tests** - % overlap with actual TB2 tests (when available for comparison)
2. **Comprehensiveness score** - LLM self-assessment (1-10)
3. **Category coverage** - All categories represented?
4. **Anti-cheat coverage** - Critical for conversion tasks
5. **Parameter discovery** - All parameters identified?
6. **Precision accuracy** - Specific bounds vs generic ranges
7. **False positive rate** - Tests that don't exist in real suite
8. **Token efficiency** - Quality per token spent

---

## Implementation Plan

### Phase 1: Programmatic Analysis Engine

**Goal:** Automatically analyze every test generation session to extract insights.

**Files to Create:**
- `src/hillclimber/testgen-analyzer.ts` - Analysis engine
- `src/hillclimber/testgen-metrics.ts` - Quality metrics calculation

**Analysis Capabilities:**

1. **Category Distribution Analysis**
   ```typescript
   analyzeCategoryDistribution(tests: GeneratedTest[]): {
     distribution: Record<TestCategory, number>;
     balance: number; // 0-1, how balanced
     missing: TestCategory[]; // Categories not represented
   }
   ```

2. **Anti-Cheat Coverage Analysis**
   ```typescript
   analyzeAntiCheatCoverage(
     tests: GeneratedTest[],
     environment: EnvironmentInfo,
     taskDescription: string
   ): {
     hasAntiCheat: boolean;
     prohibitedTools: string[];
     coverage: number; // 0-1
   }
   ```

3. **Parameter Discovery Analysis**
   ```typescript
   analyzeParameterDiscovery(
     tests: GeneratedTest[],
     environment: EnvironmentInfo
   ): {
     discoveredParams: string[];
     missingParams: string[];
     coverage: number; // 0-1
   }
   ```

4. **Reflection Effectiveness Analysis**
   ```typescript
   analyzeReflections(
     reflections: Reflection[],
     testsBefore: GeneratedTest[],
     testsAfter: GeneratedTest[]
   ): {
     gapsIdentified: string[];
     testsAdded: number;
     qualityImprovement: number; // 0-1
   }
   ```

5. **Token Efficiency Analysis**
   ```typescript
   analyzeTokenEfficiency(
     totalTokens: number,
     tests: GeneratedTest[],
     comprehensivenessScore: number
   ): {
     tokensPerTest: number;
     qualityPerToken: number; // comprehensiveness / tokens
     efficiency: number; // 0-1
   }
   ```

6. **Uncertainty Resolution Analysis**
   ```typescript
   analyzeUncertainties(
     initialUncertainties: string[],
     finalUncertainties: string[],
     tests: GeneratedTest[]
   ): {
     resolved: string[];
     remaining: string[];
     resolutionRate: number; // 0-1
   }
   ```

**Database Schema Addition:**
```sql
CREATE TABLE testgen_analyses (
  session_id TEXT PRIMARY KEY REFERENCES testgen_trajectories(session_id),
  category_distribution JSON,
  anti_cheat_coverage REAL,
  parameter_discovery_coverage REAL,
  reflection_effectiveness REAL,
  token_efficiency REAL,
  uncertainty_resolution REAL,
  overall_quality_score REAL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Phase 2: Meta-Reasoner for Test Generation

**Goal:** LLM analyzes past generations to propose improvements.

**Files to Create:**
- `src/hillclimber/testgen-meta-reasoner.ts` - Meta-reasoning for testgen configs

**Meta-Reasoning Capabilities:**

1. **Prompt Template Optimization**
   - Analyze which prompt patterns led to better results
   - Propose category-specific prompt improvements
   - Suggest anti-cheat reasoning enhancements

2. **Parameter Tuning**
   - Analyze temperature vs quality correlation
   - Optimize min/max tests per category
   - Tune reflection round limits

3. **Strategy Selection**
   - When to use environment-heavy vs description-heavy
   - When to prioritize anti-cheat vs correctness
   - When to do more reflection rounds

**Example Meta-Prompt:**
```
You are optimizing a test generation system.

Recent generations:
- Session A: 85% comprehensiveness, good anti-cheat, but missed 2 parameters
- Session B: 90% comprehensiveness, all parameters, but weak boundary tests
- Session C: 70% comprehensiveness, excellent boundary tests, but too many false positives

Current config:
- Temperature: 0.3
- Min tests per category: 2
- Max rounds per category: 3
- Anti-cheat emphasis: medium

What should we change to improve overall quality?
```

### Phase 3: Config Evolution System

**Goal:** Track and evolve testgen configs over time.

**Files to Create:**
- `src/hillclimber/testgen-config.ts` - TestGen config types
- `src/hillclimber/testgen-evolution.ts` - Evolution loop

**Config Schema:**
```typescript
interface TestGenConfig {
  id: number;
  version: string;

  // Prompt templates
  categoryPrompts: Record<TestCategory, string>;
  antiCheatPrompt: string;
  reflectionPrompt: string;

  // Generation parameters
  temperature: number;
  maxTokens: number;
  minTestsPerCategory: number;
  maxTestsPerCategory: number;
  maxRoundsPerCategory: number;

  // Strategy weights
  environmentWeight: number; // 0-1, how much to rely on environment
  antiCheatWeight: number; // 0-1, emphasis on anti-cheat tests
  precisionWeight: number; // 0-1, emphasis on specific bounds

  // Model selection
  primaryModel: "claude" | "local";
  reflectionModel: "claude" | "local";

  // Quality thresholds
  minComprehensivenessScore: number;
  targetComprehensivenessScore: number;

  createdAt: string;
  isCurrent: boolean;
}
```

**Evolution Loop:**
```typescript
async function evolveTestGenConfig(
  currentConfig: TestGenConfig,
  recentAnalyses: TestGenAnalysis[]
): Promise<TestGenConfig> {
  // 1. Analyze recent performance
  const performance = aggregateAnalyses(recentAnalyses);

  // 2. Meta-reason about improvements
  const proposal = await proposeConfigChange(
    currentConfig,
    performance
  );

  // 3. Create new config
  const newConfig = applyConfigChange(currentConfig, proposal);

  // 4. Save to database
  await saveTestGenConfig(newConfig);

  return newConfig;
}
```

### Phase 4: UI for TestGen Evolution

**Goal:** Visualize evolution progress and allow manual intervention.

**Files to Create:**
- `src/effuse/widgets/tb-command-center/tbcc-testgen-evolution.ts` - Evolution dashboard

**UI Features:**

1. **Quality Trends**
   - Chart showing comprehensiveness scores over time
   - Category coverage trends
   - Token efficiency trends

2. **Config Comparison**
   - Side-by-side comparison of config versions
   - Highlight what changed and why
   - Show performance impact of changes

3. **Analysis Dashboard**
   - Aggregate statistics across all generations
   - Top-performing configs
   - Common failure patterns

4. **Manual Override**
   - Allow manual config tweaks
   - A/B test different configs
   - Export/import configs

### Phase 5: Programmatic API

**Goal:** Enable programmatic test generation for integration with other systems.

**Files to Create:**
- `src/hillclimber/testgen-api.ts` - Programmatic API

**API Design:**
```typescript
// Simple API
const result = await generateTests({
  taskId: "regex-log",
  taskDescription: "...",
  environment: envInfo,
  config?: TestGenConfig, // Optional, uses current if not provided
});

// Advanced API with callbacks
await generateTestsWithCallbacks({
  taskId: "regex-log",
  taskDescription: "...",
  environment: envInfo,
  onTest: (test) => console.log("Test:", test),
  onProgress: (progress) => console.log("Progress:", progress),
  onReflection: (reflection) => console.log("Reflection:", reflection),
  onComplete: (result) => console.log("Complete:", result),
});
```

**Integration Points:**
- HillClimber can call this API for blind verification setup
- CI/CD can generate tests for new tasks
- Research can batch-generate tests for analysis

---

## Database Schema Extensions

### TestGen Configs Table
```sql
CREATE TABLE testgen_configs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  version TEXT NOT NULL,

  -- Prompt templates (JSON)
  category_prompts JSON NOT NULL,
  anti_cheat_prompt TEXT NOT NULL,
  reflection_prompt TEXT NOT NULL,

  -- Generation parameters
  temperature REAL NOT NULL,
  max_tokens INTEGER NOT NULL,
  min_tests_per_category INTEGER NOT NULL,
  max_tests_per_category INTEGER NOT NULL,
  max_rounds_per_category INTEGER NOT NULL,

  -- Strategy weights
  environment_weight REAL NOT NULL,
  anti_cheat_weight REAL NOT NULL,
  precision_weight REAL NOT NULL,

  -- Model selection
  primary_model TEXT NOT NULL,
  reflection_model TEXT NOT NULL,

  -- Quality thresholds
  min_comprehensiveness_score REAL NOT NULL,
  target_comprehensiveness_score REAL NOT NULL,

  -- Metadata
  is_current INTEGER DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### TestGen Analyses Table
```sql
CREATE TABLE testgen_analyses (
  session_id TEXT PRIMARY KEY REFERENCES testgen_trajectories(session_id),
  config_id INTEGER REFERENCES testgen_configs(id),

  -- Quality metrics
  category_distribution JSON,
  anti_cheat_coverage REAL,
  parameter_discovery_coverage REAL,
  reflection_effectiveness REAL,
  token_efficiency REAL,
  uncertainty_resolution REAL,
  overall_quality_score REAL,

  -- Insights (JSON)
  strengths JSON, -- What worked well
  weaknesses JSON, -- What didn't work
  recommendations JSON, -- Suggested improvements

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### TestGen Evolution History
```sql
CREATE TABLE testgen_evolution (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_config_id INTEGER REFERENCES testgen_configs(id),
  to_config_id INTEGER REFERENCES testgen_configs(id),

  -- Change details
  changes JSON NOT NULL, -- What changed
  reasoning TEXT NOT NULL, -- Why it changed
  expected_improvement TEXT, -- What we expect to improve

  -- Results (filled after testing)
  actual_improvement REAL, -- Did it actually improve?
  quality_delta REAL, -- Change in quality score

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

## Success Metrics

### Short-Term (Phase 1-2)
- ✅ All generations automatically analyzed
- ✅ Quality metrics calculated and stored
- ✅ Meta-reasoner proposes config changes
- ✅ Config evolution tracked in database

### Medium-Term (Phase 3-4)
- ✅ Comprehensiveness scores improve over time (baseline → target)
- ✅ Category coverage becomes more balanced
- ✅ Anti-cheat coverage increases for conversion tasks
- ✅ Token efficiency improves (same quality, fewer tokens)

### Long-Term (Phase 5)
- ✅ Test generation quality matches or exceeds real TB2 tests
- ✅ HillClimber pass rates improve (better self-tests → better blind verification)
- ✅ System generalizes across task types
- ✅ Programmatic API enables integration with other systems

---

## Implementation Order

### Step 1: Analysis Engine (Week 1)
1. Create `testgen-analyzer.ts` with all analysis functions
2. Create `testgen-metrics.ts` for metric calculations
3. Add analysis table to database migration
4. Integrate analysis into `testgen-service.ts` (run after completion)
5. Test on existing trajectory data

### Step 2: Meta-Reasoner (Week 1-2)
1. Create `testgen-meta-reasoner.ts`
2. Design meta-reasoning prompts
3. Integrate with existing OpenRouter infrastructure
4. Test on historical data

### Step 3: Config System (Week 2)
1. Create `testgen-config.ts` with types
2. Add config table to database
3. Create config management functions
4. Initialize default config
5. Test config loading/saving

### Step 4: Evolution Loop (Week 2-3)
1. Create `testgen-evolution.ts`
2. Implement evolution logic
3. Add evolution history table
4. Test evolution on sample data
5. Integrate with testgen-service

### Step 5: UI Dashboard (Week 3)
1. Create `tbcc-testgen-evolution.ts` widget
2. Add charts for trends
3. Add config comparison view
4. Add manual override controls
5. Test UI with real data

### Step 6: Programmatic API (Week 3-4)
1. Create `testgen-api.ts`
2. Design clean API surface
3. Add documentation
4. Test with sample integrations
5. Document usage patterns

---

## Key Considerations

### 1. Comparison with Real Tests

**Challenge:** We can't always compare with real TB2 tests (that would break blindness).

**Solution:**
- Use comparison only for **development/analysis** (not during HillClimber runs)
- Separate "development mode" vs "production mode"
- In production, rely on comprehensiveness scores and self-reflection
- Periodic validation runs where we compare (but don't use results in evolution)

### 2. Overfitting Risk

**Challenge:** Optimizing for specific tasks might hurt generalization.

**Solution:**
- Track performance across multiple task types
- Use holdout tasks (never seen during evolution)
- Monitor for overfitting (good on training tasks, bad on holdout)
- Regularize config changes (prefer general improvements over task-specific)

### 3. Token Cost

**Challenge:** More analysis and meta-reasoning = more tokens.

**Solution:**
- Use free models for meta-reasoning (OpenRouter free tier)
- Batch analysis (analyze multiple sessions together)
- Cache analysis results
- Only evolve configs periodically (not every generation)

### 4. Human Oversight

**Challenge:** Fully automated evolution might go wrong.

**Solution:**
- UI dashboard for monitoring
- Manual override capabilities
- Config versioning (can rollback)
- Approval workflow for major changes

### 5. Integration with HillClimber

**Challenge:** TestGen evolution should improve HillClimber, not just testgen itself.

**Solution:**
- Track HillClimber pass rates alongside testgen quality
- Correlate testgen improvements with HillClimber improvements
- Use HillClimber feedback to guide testgen evolution
- Joint optimization (testgen + HillClimber configs together)

---

## Next Steps (Immediate)

1. **Create analysis engine** - Start with category distribution and anti-cheat coverage
2. **Add analysis to database** - Extend trajectories migration with analysis table
3. **Run analysis on existing data** - Analyze all past generations to establish baseline
4. **Design meta-reasoner prompts** - How should LLM reason about testgen improvements?
5. **Create config system** - Start with simple config (temperature, min/max tests)
6. **Build evolution loop** - Simple version: analyze → propose → test → adopt

---

## Related Documentation

- `docs/logs/20251208/1325-hillclimber-v3-plan.md` - HillClimber v3 architecture
- `docs/logs/20251208/1318-test-gen-analysis.md` - Test generation gap analysis
- `docs/logs/20251208/1313-test-generation-benchmark-status.md` - Benchmark status
- `docs/logs/20251208/1600-trajectory-persistence-implementation.md` - Persistence implementation
- `src/hillclimber/test-generator-iterative.ts` - Iterative generation logic
- `src/hillclimber/meta-reasoner.ts` - HillClimber meta-reasoning (reference)

---

**Status:** 📋 Planning Complete - Ready for Implementation
**Priority:** High - Test generation quality is critical to HillClimber success
**Timeline:** 3-4 weeks for full implementation
