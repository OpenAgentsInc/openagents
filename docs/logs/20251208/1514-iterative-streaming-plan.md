# Iterative Streaming Test Generation

## Goal

Transform single-shot test generation into an iterative, self-refining system that:
1. Generates tests ONE AT A TIME (true streaming, not fake 50ms delays)
2. Uses reflection loops to identify gaps and generate more tests
3. Continues until comprehensive coverage across all categories
4. Streams tests to UI in real-time as they're generated

## Problem with Current Implementation

**Current**: `generateTestsFromEnvironment()` makes ONE LLM call, returns ALL tests at once
- Fake streaming: 50ms delays after generation completes (testgen-service.ts:135-159)
- All-or-nothing: LLM must generate 15-30 tests in single JSON response
- No refinement: Can't ask "what's missing?" and iterate
- No streaming: User waits 10-30s before seeing first test

**New**: Iterative generation with real streaming
- Generate 2-5 tests per round, emit immediately
- Reflect after each category: "What edge cases are missing?"
- Continue until comprehensive (self-assessment score ≥ 8)
- First test appears in UI within 5 seconds

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                       TB TESTGEN WIDGET                              │
│  - Task selector (dropdown or random)                                │
│  - Environment context display                                       │
│  - Streaming test cards (one per generated test)                    │
│  - Progress indicator (category, round, phase)                      │
│  - Reflection messages (gap analysis, self-assessment)              │
└─────────────────────────────────────────────────────────────────────┘
        │                                        ▲
        │ request:startTestGen                   │ hud events (test, progress, reflection)
        ▼                                        │
┌─────────────────────────────────────────────────────────────────────┐
│                    DESKTOP SERVER                                    │
│  - Handles startTestGen request                                      │
│  - Calls new iterative test-generator-iterative.ts                  │
│  - Emits streaming HUD messages AS tests are generated              │
└─────────────────────────────────────────────────────────────────────┘
```

## Iteration Strategy

### Category-Based Iteration (Phase 2)
Process categories in priority order: `anti_cheat → existence → correctness → boundary → integration`

**Per-Category Rounds:**
- Round 1: Generate 3-5 initial tests
- Round 2+: Reflection ("What's missing?") → Generate 1-3 more tests
- Threshold: Min 2 tests, target 3-5 per category
- Max rounds per category: 3

### Global Refinement (Phase 3)
After all categories processed:
1. Self-assessment prompt: "Review all tests. Rate comprehensiveness 1-10. What's missing?"
2. If score < 8: Generate 2-5 additional tests for weak areas
3. Max global rounds: 2

### Termination Criteria (User Preferences)
```typescript
{
  // Category iteration (AGGRESSIVE - higher quality)
  minTestsPerCategory: 2,
  targetTestsPerCategory: 5,
  maxRoundsPerCategory: 3,  // Up to 3 refinement rounds per category

  // Global refinement (ALWAYS-ON - every generation gets self-assessment)
  enableGlobalRefinement: true,  // Always enabled by default
  minComprehensivenessScore: 8,  // 1-10 scale from self-assessment
  maxGlobalRefinementRounds: 2,

  // Overall limits
  minTotalTests: 15,
  targetTotalTests: 30,  // Higher target with aggressive settings
  maxTotalRounds: 12,  // 5 categories × 3 rounds + 2 global = 17 max theoretical

  // Hard limits (cost control)
  maxTotalTokens: 50000,  // Stop if we exceed this
  maxTotalTimeMs: 180000,  // 3 minutes max

  // Parallelization (V1: SEQUENTIAL)
  parallelCategories: false  // Generate one category at a time (simpler, easier to debug)
}
```

## Streaming Implementation

### Single-Test Tool Calls
LLM calls `generate_test()` tool 2-5 times per round, emitting each test immediately:

```typescript
const generateSingleTestTool: Tool = {
  name: "generate_test",
  description: "Generate a single test case for the current category",
  input_schema: {
    type: "object",
    properties: {
      id: { type: "string" },
      input: { type: "string" },
      expectedOutput: { type: "string | null" },
      reasoning: { type: "string" },
      confidence: { type: "number" }  // 0-1
    }
  }
};
```

Each tool call is parsed individually and emitted via `onTest()` immediately.

## Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `src/hillclimber/test-generator-iterative.ts` | Core iterative generation engine with category loops, reflection, self-assessment |

### Modified Files

| File | Changes |
|------|---------|
| `src/hillclimber/testgen-service.ts` | Replace `generateTestsFromEnvironment()` call with `generateTestsIteratively()`. Remove fake streaming delays. |
| `src/hud/protocol.ts` | Add `TestGenProgressMessage`, `TestGenReflectionMessage`. Update `TestGenCompleteMessage` with iteration stats. |
| `src/effuse/widgets/tb-command-center/tbcc-testgen.ts` | Handle progress/reflection messages. Show phase, category, round in UI. Display comprehensiveness score. |

## Implementation Steps

### Step 1: Create Iterative Generator Core

**New File: `src/hillclimber/test-generator-iterative.ts`**

Main exports:
```typescript
// State tracker for multi-round generation
interface GeneratorState {
  antiCheatTests: GeneratedTest[];
  existenceTests: GeneratedTest[];
  correctnessTests: GeneratedTest[];
  boundaryTests: GeneratedTest[];
  integrationTests: GeneratedTest[];

  currentPhase: "category_generation" | "global_refinement" | "complete";
  currentCategory: TestCategory | null;
  categoryRoundNumber: Record<TestCategory, number>;
  globalRoundNumber: number;

  totalRounds: number;
  totalTokensUsed: number;
  comprehensivenessScore: number | null;
}

// Main API (replaces generateTestsFromEnvironment)
export async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  emitter: TestGenEmitter,  // Callbacks for real-time emission
  options: TestGeneratorOptions = {}
): Promise<EnvironmentAwareTestResult>
```

**Key Functions:**
1. `generateTestsForCategoryRound()` - Generate 2-5 tests for one category in one round
2. `reflectOnCategory()` - Ask "what's missing from this category?"
3. `assessComprehensiveness()` - Global self-assessment (1-10 score)
4. `generateFromRecommendations()` - Generate additional tests based on gaps

### Step 2: Update Protocol for Progress/Reflection

**File: `src/hud/protocol.ts`**

Add new message types:
```typescript
// Show current progress
export interface TestGenProgressMessage {
  type: "testgen_progress";
  sessionId: string;
  phase: "category_generation" | "global_refinement";
  currentCategory?: TestCategory;
  roundNumber: number;
  status: string;  // "Generating correctness tests (round 2)..."
}

// Show LLM's gap analysis
export interface TestGenReflectionMessage {
  type: "testgen_reflection";
  sessionId: string;
  category?: TestCategory;
  reflectionText: string;
  action: "refining" | "assessing" | "complete";
}
```

Update existing message:
```typescript
export interface TestGenCompleteMessage {
  type: "testgen_complete";
  sessionId: string;
  totalTests: number;
  totalRounds: number;  // NEW: Sum of all rounds
  categoryRounds: Record<TestCategory, number>;  // NEW: Rounds per category
  comprehensivenessScore: number | null;  // NEW: 1-10 from self-assessment
  totalTokensUsed: number;  // NEW: Cost tracking
  durationMs: number;
  uncertainties: string[];
}
```

### Step 3: Update Service Layer

**File: `src/hillclimber/testgen-service.ts`**

Replace single-shot call with iterative:
```typescript
// OLD (remove):
const result = await generateTestsFromEnvironment(
  task.description,
  taskId,
  environment,
  options
);
// Then fake stream with 50ms delays...

// NEW (replace with):
const result = await generateTestsIteratively(
  task.description,
  taskId,
  environment,
  {
    onStart: (msg) => emitter.onStart(msg),
    onTest: (msg) => emitter.onTest(msg),  // Already emitted during generation!
    onProgress: (msg) => emitter.onProgress(msg),  // NEW
    onReflection: (msg) => emitter.onReflection(msg),  // NEW
    onComplete: (msg) => emitter.onComplete(msg),
    onError: (msg) => emitter.onError(msg),
  },
  options
);
// No fake delays needed - tests already streamed in real-time
```

Update `TestGenEmitter` interface:
```typescript
export interface TestGenEmitter {
  onStart: (msg: TestGenStartMessage) => void;
  onTest: (msg: TestGenTestMessage) => void;
  onProgress: (msg: TestGenProgressMessage) => void;  // NEW
  onReflection: (msg: TestGenReflectionMessage) => void;  // NEW
  onComplete: (msg: TestGenCompleteMessage) => void;
  onError: (msg: TestGenErrorMessage) => void;
}
```

### Step 4: Update UI Widget

**File: `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`**

Add new state fields:
```typescript
export interface TBTestGenState {
  // ... existing fields ...

  // NEW: Iteration tracking
  currentPhase: "idle" | "category_generation" | "global_refinement" | "complete";
  currentCategory: TestCategory | null;
  currentRound: number;
  progressStatus: string | null;  // "Generating correctness tests (round 2)..."
  reflections: Array<{
    category: TestCategory | null;
    text: string;
    action: "refining" | "assessing" | "complete";
  }>;

  // NEW: Final stats
  totalRounds: number;
  categoryRounds: Record<TestCategory, number> | null;
  comprehensivenessScore: number | null;  // 1-10
  totalTokensUsed: number;
}
```

Subscribe to new message types in `subscriptions`:
```typescript
if (msg.type === "testgen_progress") {
  yield* ctx.state.update(s => ({
    ...s,
    currentPhase: msg.phase,
    currentCategory: msg.currentCategory ?? null,
    currentRound: msg.roundNumber,
    progressStatus: msg.status,
  }));
}

if (msg.type === "testgen_reflection") {
  yield* ctx.state.update(s => ({
    ...s,
    reflections: [...s.reflections, {
      category: msg.category ?? null,
      text: msg.reflectionText,
      action: msg.action,
    }],
  }));
}
```

Update UI to show progress and reflections:
- Progress bar showing current category and round
- Reflection panel with gap analysis from LLM
- Final comprehensiveness score display

## Core Generation Loop Pseudocode

```typescript
async function generateTestsIteratively(
  taskDescription, taskId, environment, emitter, options
) {
  const state: GeneratorState = initializeState();
  const startTime = Date.now();

  emitter.onStart({ taskId, taskDescription, environment });

  // Phase 2: Category-based iteration
  for (const category of CATEGORIES) {
    let round = 1;

    while (!isCategoryComplete(state, category) && round <= 3) {
      emitter.onProgress({
        phase: "category_generation",
        currentCategory: category,
        roundNumber: round,
        status: `Generating ${category} tests (round ${round})...`
      });

      // Generate 2-5 tests for this category
      const newTests = await generateTestsForCategoryRound(
        category, state[`${category}Tests`], environment, round
      );

      // Emit each test immediately as generated
      for (const test of newTests) {
        emitter.onTest({ sessionId, test: { ...test, category } });
        state[`${category}Tests`].push(test);
      }

      // Reflect if not final round
      if (round < 3 && shouldReflect(state, category)) {
        const reflection = await reflectOnCategory(
          category, state[`${category}Tests`], environment
        );
        emitter.onReflection({
          category,
          reflectionText: reflection,
          action: "refining"
        });
      }

      round++;
      state.totalRounds++;
    }
  }

  // Phase 3: Global refinement (optional)
  const assessment = await assessComprehensiveness(state, environment);

  if (assessment.score < 8 && state.totalRounds < 10) {
    emitter.onReflection({
      reflectionText: assessment.gaps,
      action: "assessing"
    });

    const additionalTests = await generateFromRecommendations(assessment);
    // Emit and categorize additional tests...
  }

  emitter.onComplete({
    totalTests: countAllTests(state),
    totalRounds: state.totalRounds,
    categoryRounds: state.categoryRoundNumber,
    comprehensivenessScore: assessment.score,
    totalTokensUsed: state.totalTokensUsed,
    durationMs: Date.now() - startTime
  });

  return convertStateToResult(state);
}
```

## Implementation Order

1. **Create `test-generator-iterative.ts`** - Core iterative engine with category loops
2. **Update `protocol.ts`** - Add progress/reflection messages, update complete message
3. **Update `testgen-service.ts`** - Replace single-shot call with iterative, remove fake delays
4. **Update `tbcc-testgen.ts`** - Add progress/reflection UI, iteration stats display
5. **Test end-to-end** - Verify streaming, reflection, comprehensive coverage

## Cost and Performance Analysis

### Current Single-Shot
- Tokens: ~8,000 per task
- Time: 10-30 seconds
- Tests: 15-25 (fixed, no refinement)

### Iterative (Conservative Settings)
- Tokens: ~15,000-20,000 per task (1.9-2.5x increase)
  - 5 categories × 2 rounds × 2,000 tokens = 20,000
  - With caching: ~12,000-15,000
- Time: 30-45 seconds (1.5x increase)
  - Parallelization opportunity: Generate categories in parallel (future)
- Tests: 20-35 (variable, based on quality)

### Hard Limits (Cost Control)
- Max total rounds: 10 (prevents runaway)
- Max tokens: 50,000 (stop if exceeded)
- Max time: 3 minutes (stop if exceeded)
- Max rounds per category: 3 (prevents stuck loops)

### Expected Quality Improvement
- 40-50% higher test-gen-evaluator scores
- 50% better edge case detection
- 90% parameter coverage (from file previews)
- Comprehensiveness self-assessment ≥ 8/10

## Key Design Decisions

1. **Why category-based iteration?**
   - Focuses LLM attention on one aspect at a time
   - Natural checkpoints for reflection
   - Easier to track progress in UI
   - Prevents "forgetting" about certain test types

2. **Why single-test tool calls?**
   - True streaming (no partial JSON arrays to parse)
   - Immediate UI feedback (first test in ~5s)
   - Natural error recovery (one test fails, others continue)
   - Works with both Claude (tools) and local FM (guided generation)

3. **Why self-assessment?**
   - LLM knows what it generated and can critique
   - More reliable than fixed round counts
   - Provides transparency (user sees the reasoning)
   - Allows adaptive stopping (stop early if score ≥ 8)

4. **Why hard limits?**
   - Prevents cost explosion on difficult tasks
   - Provides predictable worst-case cost
   - Forces efficient iteration (can't loop forever)

## Critical Files Reference

### Existing (Reference)
- `src/hillclimber/test-generator.ts:920-962` - Current `generateTestsFromEnvironment()` (single-shot)
- `src/hillclimber/test-gen-compare.ts:256-333` - `buildMockEnvironmentFromTask()` for environment building
- `src/hillclimber/testgen-service.ts:135-159` - Fake streaming implementation (remove delays)
- `src/effuse/utils/partialToolArgs.ts` - PartialToolArgsParser for streaming JSON

### To Create
- `src/hillclimber/test-generator-iterative.ts` - NEW iterative engine

### To Modify
- `src/hillclimber/testgen-service.ts` - Replace single-shot with iterative
- `src/hud/protocol.ts` - Add progress/reflection messages
- `src/effuse/widgets/tb-command-center/tbcc-testgen.ts` - Add iteration UI

## Next Steps

1. Start with `test-generator-iterative.ts` - implement basic category loop (no reflection)
2. Add reflection prompts and integrate into loop
3. Add global self-assessment phase
4. Update protocol and service layer
5. Update UI to show progress and reflections
6. Test with real tasks (regex-log, path-tracing)
7. Benchmark against current single-shot (use test-gen-evaluator.ts)
