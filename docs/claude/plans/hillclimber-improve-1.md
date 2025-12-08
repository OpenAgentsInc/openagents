# Iterative Streaming Test Generation Architecture

**Goal**: Transform single-shot test generation into an iterative, streaming system that continuously refines and expands until comprehensive coverage is achieved.

## Executive Summary

Current test generation makes ONE LLM call and returns ALL tests at once with fake streaming. This plan redesigns the system to:

1. **True Streaming**: Generate and emit tests ONE AT A TIME as the LLM produces them
2. **Iterative Refinement**: Use reflection loops to identify gaps and generate more tests
3. **Comprehensive Coverage**: Don't stop until we have thorough coverage across all categories
4. **Real-time UI Updates**: Each test appears in UI immediately after generation

## Current State Analysis

### Problems with Single-Shot Generation

**File: src/hillclimber/test-generator.ts (lines 925-962)**
```typescript
export async function generateTestsFromEnvironment(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  options: TestGeneratorOptions = {},
): Promise<EnvironmentAwareTestResult>
```
- Makes ONE LLM call via `generateTestsWithEnvironmentLocalFM()` or `generateTestsWithEnvironmentClaude()`
- Returns `EnvironmentAwareTestResult` with all 5 categories populated in single response
- Expects LLM to generate 2-5 tests per category in one shot

**File: src/hillclimber/testgen-service.ts (lines 135-159)**
```typescript
// Emit tests one at a time (by category) with streaming delay
await emitTestsForCategory(result.antiCheatTests, "anti_cheat");
await emitTestsForCategory(result.existenceTests, "existence");
// ...
await new Promise((resolve) => setTimeout(resolve, 50)); // FAKE STREAMING
```
- Tests emitted with 50ms delays AFTER generation completes
- No actual streaming during LLM generation
- All-or-nothing JSON parsing via `parseEnvironmentAwareResponse()`

### Existing Infrastructure (We Can Leverage)

1. **PartialToolArgsParser** (src/llm/partialToolArgs.ts)
   - Incremental JSON streaming parser
   - Accumulates chunks and attempts decode when valid JSON
   - Schema-based validation with Effect.Schema

2. **TestGenEmitter Interface** (src/hillclimber/testgen-service.ts:29-41)
   ```typescript
   export interface TestGenEmitter {
     onStart: (msg: TestGenStartMessage) => void;
     onTest: (msg: TestGenTestMessage) => void;    // ✅ Already supports one-at-a-time
     onComplete: (msg: TestGenCompleteMessage) => void;
     onError: (msg: TestGenErrorMessage) => void;
   }
   ```

3. **Environment Context** (src/hillclimber/environment-info.ts)
   - Platform, languages, tools, file previews
   - Prohibited tools for anti-cheat detection
   - File structure extraction (variables, functions, parameters)

4. **Test Evaluator** (src/hillclimber/test-gen-evaluator.ts)
   - Compares generated vs actual tests
   - Quality scoring: coverage, accuracy, edge case detection, category balance
   - Can be used for self-assessment

5. **FM Client with Guided Generation** (src/llm/foundation-models.ts)
   - Supports `responseFormat: { type: "json_schema", schema_type: "..." }`
   - Currently `stream: false` but infrastructure exists

## Architectural Design

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Phase 1: INITIALIZATION                                          │
├─────────────────────────────────────────────────────────────────┤
│ 1. Load task description + environment                           │
│ 2. Emit TestGenStartMessage                                      │
│ 3. Initialize GeneratorState (empty test arrays)                 │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ Phase 2: ITERATIVE CATEGORY GENERATION                           │
├─────────────────────────────────────────────────────────────────┤
│ For each category in priority order:                             │
│   Anti-Cheat → Existence → Correctness → Boundary → Integration │
│                                                                   │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │ Round 1: Initial Generation (2-5 tests)                  │   │
│   │ - Generate tests for this category only                  │   │
│   │ - Stream each test via onTest() immediately              │   │
│   │ - Accumulate in GeneratorState                           │   │
│   └─────────────────────────────────────────────────────────┘   │
│                         ↓                                         │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │ Round 2+: Refinement (if needed)                         │   │
│   │ - Reflection: "What's missing from this category?"       │   │
│   │ - Generate 1-3 additional tests                          │   │
│   │ - Stream immediately                                     │   │
│   │ - Repeat until category threshold met OR max rounds      │   │
│   └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ Phase 3: GLOBAL REFINEMENT (Optional)                            │
├─────────────────────────────────────────────────────────────────┤
│ 1. Self-Assessment Prompt:                                       │
│    "Review all generated tests. What critical scenarios are      │
│     missing? Rate comprehensiveness 1-10."                       │
│ 2. If score < 8: Generate 2-5 more tests for weak areas          │
│ 3. Repeat until score ≥ 8 OR max global rounds reached           │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ Phase 4: COMPLETION                                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. Emit TestGenCompleteMessage with final stats                  │
│ 2. Return EnvironmentAwareTestResult                             │
└─────────────────────────────────────────────────────────────────┘
```

### 1. Iteration Strategy: Hybrid Coverage + Quality

**Category-Based Iteration** (Phase 2):
- Process categories in priority order: `anti_cheat → existence → correctness → boundary → integration`
- Each category gets 1-3 generation rounds
- **Threshold per category**: Minimum 2 tests, target 3-5 tests
- **Max rounds per category**: 3 (prevents runaway on hard categories)

**Global Quality Iteration** (Phase 3):
- After all categories processed, run self-assessment
- Ask LLM to rate comprehensiveness 1-10 and identify gaps
- If score < 8, generate 2-5 additional tests for weak areas
- **Max global rounds**: 2 (prevents excessive cost)

**Termination Conditions**:
```typescript
interface TerminationCriteria {
  // Per-category
  minTestsPerCategory: 2;
  targetTestsPerCategory: 5;
  maxRoundsPerCategory: 3;
  
  // Global
  minTotalTests: 15;
  targetTotalTests: 25;
  maxTotalRounds: 10;  // Sum of all category + global rounds
  
  // Quality-based (Phase 3)
  minComprehensivenessScore: 8;  // 1-10 scale
  maxGlobalRefinementRounds: 2;
  
  // Hard limits (cost control)
  maxTotalTokens: 50000;  // Stop if we exceed this
  maxTotalTimeMs: 180000;  // 3 minutes max
}
```

### 2. Streaming Approach: Single-Test Tool Calls

**Option Chosen**: One test per tool call with streaming JSON parsing

**Why**: 
- Most reliable for true streaming (no need to parse partial arrays)
- LLM can emit test immediately without waiting for category completion
- Natural feedback points (after each test, can reflect)
- Works with both Claude (tool calls) and FM (guided generation)

**Implementation**:

```typescript
// New tool definition for single-test generation
const generateSingleTestTool: Tool = {
  name: "generate_test",
  description: "Generate a single test case for the current category",
  input_schema: {
    type: "object",
    properties: {
      id: { type: "string", description: "Unique test identifier" },
      input: { type: "string", description: "Test input" },
      expectedOutput: { type: "string | null", description: "Expected output or null" },
      reasoning: { type: "string", description: "Why this test matters" },
      confidence: { type: "number", description: "Confidence 0-1" }
    },
    required: ["id", "input", "reasoning", "confidence"]
  }
};

// Prompt structure for category round
async function generateTestsForCategoryRound(
  category: TestCategory,
  existingTests: GeneratedTest[],
  environment: EnvironmentInfo,
  roundNumber: number
): Promise<GeneratedTest[]> {
  const prompt = buildCategoryPrompt(category, existingTests, environment, roundNumber);
  
  // Request: Generate 2-5 tests via tool calls
  const request: ChatRequest = {
    messages: [{ role: "user", content: prompt }],
    tools: [generateSingleTestTool],
    toolChoice: "required",  // Must call tool
    temperature: 0.3
  };
  
  const tests: GeneratedTest[] = [];
  const response = await fm.chat(request);
  
  // Parse each tool call as a test
  for (const toolCall of response.choices[0].message.tool_calls ?? []) {
    const test = JSON.parse(toolCall.arguments);
    test.category = category;  // Ensure category is set
    
    // Emit immediately via TestGenEmitter
    emitter.onTest({
      type: "testgen_test",
      sessionId,
      test
    });
    
    tests.push(test);
  }
  
  return tests;
}
```

**Alternative (for FM with guided generation)**:
```typescript
// FM can use guided generation with single-test schema
const response = await fm.chat({
  messages: [{ role: "user", content: prompt }],
  responseFormat: {
    type: "json_schema",
    schema_type: "single_test_generation"  // New schema type
  }
});
// Repeat N times for N tests
```

### 3. Feedback Mechanism: Show-Previous-Ask-Next Pattern

**Reflection Between Rounds**:

```typescript
// After Round 1 of "correctness" category
const reflectionPrompt = `
You generated these correctness tests:
${JSON.stringify(existingTests, null, 2)}

Review these tests and identify:
1. What edge cases are missing?
2. What parameter combinations aren't covered?
3. What boundary conditions should we test?

Based on this analysis, generate 1-3 additional tests to fill the gaps.
Focus on scenarios that would catch bugs the existing tests would miss.
`;
```

**Self-Assessment for Global Refinement** (Phase 3):
```typescript
const assessmentPrompt = `
You have generated these tests for task "${taskId}":

ANTI-CHEAT (${antiCheatTests.length} tests):
${summarizeTests(antiCheatTests)}

EXISTENCE (${existenceTests.length} tests):
${summarizeTests(existenceTests)}

CORRECTNESS (${correctnessTests.length} tests):
${summarizeTests(correctnessTests)}

BOUNDARY (${boundaryTests.length} tests):
${summarizeTests(boundaryTests)}

INTEGRATION (${integrationTests.length} tests):
${summarizeTests(integrationTests)}

TASK DESCRIPTION:
${taskDescription}

ENVIRONMENT CONSTRAINTS:
${formatEnvironment(environment)}

Questions for self-assessment:
1. Do these tests cover ALL requirements from the task description? (List any missing)
2. Do these tests verify ALL parameters visible in file previews? (List any missing)
3. What critical failure scenarios are NOT covered? (List)
4. Rate the comprehensiveness of this test suite: 1-10
5. What 2-3 additional tests would most improve coverage?

Respond in JSON:
{
  "comprehensivenessScore": 7,
  "missingRequirements": ["...", "..."],
  "missingParameters": ["alpha", "sigma"],
  "uncoveredScenarios": ["...", "..."],
  "recommendations": ["Generate test for X", "Add boundary test for Y"]
}
`;
```

### 4. State Management: Generator State Tracker

**New State Type**:

```typescript
/**
 * Tracks progress of iterative test generation.
 */
interface GeneratorState {
  sessionId: string;
  taskId: string;
  startedAt: number;
  
  // Test accumulation
  antiCheatTests: GeneratedTest[];
  existenceTests: GeneratedTest[];
  correctnessTests: GeneratedTest[];
  boundaryTests: GeneratedTest[];
  integrationTests: GeneratedTest[];
  
  // Iteration tracking
  currentPhase: "initialization" | "category_generation" | "global_refinement" | "complete";
  currentCategory: TestCategory | null;
  categoryRoundNumber: Record<TestCategory, number>;
  globalRoundNumber: number;
  
  // Termination tracking
  totalRounds: number;
  totalTokensUsed: number;
  totalTimeMs: number;
  
  // Quality tracking
  comprehensivenessScore: number | null;  // 1-10 from self-assessment
  uncertainties: string[];
  
  // Requirements tracking
  descriptionRequirements: string[];
  environmentRequirements: string[];
  coveredRequirements: Set<string>;
}

/**
 * Check if category is complete.
 */
function isCategoryComplete(
  category: TestCategory,
  state: GeneratorState,
  criteria: TerminationCriteria
): boolean {
  const tests = state[`${category}Tests`];
  const rounds = state.categoryRoundNumber[category];
  
  return (
    tests.length >= criteria.targetTestsPerCategory ||
    (tests.length >= criteria.minTestsPerCategory && rounds >= criteria.maxRoundsPerCategory)
  );
}

/**
 * Check if global generation is complete.
 */
function isGenerationComplete(
  state: GeneratorState,
  criteria: TerminationCriteria
): boolean {
  const totalTests = 
    state.antiCheatTests.length +
    state.existenceTests.length +
    state.correctnessTests.length +
    state.boundaryTests.length +
    state.integrationTests.length;
  
  return (
    // All categories met minimum
    state.antiCheatTests.length >= criteria.minTestsPerCategory &&
    state.existenceTests.length >= criteria.minTestsPerCategory &&
    state.correctnessTests.length >= criteria.minTestsPerCategory &&
    state.boundaryTests.length >= criteria.minTestsPerCategory &&
    state.integrationTests.length >= criteria.minTestsPerCategory &&
    
    // Total tests sufficient
    totalTests >= criteria.minTotalTests &&
    
    // Quality threshold met (if assessed)
    (state.comprehensivenessScore === null || state.comprehensivenessScore >= criteria.minComprehensivenessScore) &&
    
    // OR hard limits reached
    state.totalRounds >= criteria.maxTotalRounds ||
    state.totalTokensUsed >= criteria.maxTotalTokens ||
    state.totalTimeMs >= criteria.maxTotalTimeMs
  );
}
```

### 5. Prompt Structure Evolution

**Round 1 (Initial)**: Focus on category basics
```typescript
function buildCategoryPromptRound1(
  category: TestCategory,
  environment: EnvironmentInfo
): string {
  return `
Generate 3-5 ${category} tests for this task.

TASK DESCRIPTION:
${taskDescription}

ENVIRONMENT:
${formatEnvironment(environment)}

CATEGORY FOCUS: ${category}
${getCategoryGuidance(category)}

Generate tests as tool calls using generate_test().
Each test should be distinct and cover different aspects.
  `;
}
```

**Round 2+ (Refinement)**: Show existing, ask for gaps
```typescript
function buildCategoryPromptRoundN(
  category: TestCategory,
  existingTests: GeneratedTest[],
  environment: EnvironmentInfo,
  roundNumber: number
): string {
  return `
You previously generated these ${category} tests:
${JSON.stringify(existingTests, null, 2)}

REFLECTION:
- What scenarios are NOT covered by these tests?
- What edge cases are missing?
- What would a naive implementation get wrong that these tests don't catch?

Generate 1-3 ADDITIONAL ${category} tests that fill critical gaps.
Focus on what's missing, not duplicating existing coverage.

Use generate_test() tool calls.
  `;
}
```

**Global Assessment Prompt**:
```typescript
function buildGlobalAssessmentPrompt(
  state: GeneratorState,
  environment: EnvironmentInfo
): string {
  return `
Review the complete test suite you've generated:

ANTI-CHEAT: ${state.antiCheatTests.length} tests
${summarizeTests(state.antiCheatTests)}

EXISTENCE: ${state.existenceTests.length} tests
${summarizeTests(state.existenceTests)}

CORRECTNESS: ${state.correctnessTests.length} tests
${summarizeTests(state.correctnessTests)}

BOUNDARY: ${state.boundaryTests.length} tests
${summarizeTests(state.boundaryTests)}

INTEGRATION: ${state.integrationTests.length} tests
${summarizeTests(state.integrationTests)}

ORIGINAL TASK:
${taskDescription}

ENVIRONMENT CONSTRAINTS:
${formatEnvironment(environment)}

Self-Assessment Questions:
1. Comprehensiveness score (1-10): How well do these tests verify the task?
2. Missing requirements: What task requirements aren't tested?
3. Missing parameters: What file preview parameters aren't verified?
4. Uncovered scenarios: What failure modes aren't caught?
5. Top recommendations: What 2-3 tests would most improve the suite?

Respond with JSON:
{
  "comprehensivenessScore": <1-10>,
  "missingRequirements": ["..."],
  "missingParameters": ["..."],
  "uncoveredScenarios": ["..."],
  "recommendations": ["...", "...", "..."]
}
  `;
}
```

### 6. Protocol Changes

**Add Progress Messages**:

```typescript
// New message type for iteration progress
export interface TestGenProgressMessage {
  type: "testgen_progress";
  sessionId: string;
  phase: "category_generation" | "global_refinement";
  currentCategory?: TestCategory;
  roundNumber: number;
  totalTests: number;
  status: string;  // Human-readable status like "Generating correctness tests (round 2)..."
}
```

**Add Reflection Messages** (for debugging/transparency):

```typescript
export interface TestGenReflectionMessage {
  type: "testgen_reflection";
  sessionId: string;
  category?: TestCategory;
  reflectionText: string;  // What the LLM identified as gaps
  action: "refining" | "assessing" | "complete";
}
```

**Updated TestGenCompleteMessage**:

```typescript
export interface TestGenCompleteMessage {
  type: "testgen_complete";
  sessionId: string;
  totalTests: number;
  durationMs: number;
  uncertainties: string[];
  
  // New fields for iteration stats
  totalRounds: number;
  categoryRounds: Record<TestCategory, number>;
  globalRefinementRounds: number;
  comprehensivenessScore: number | null;
  totalTokensUsed: number;
}
```

### 7. File Modifications

**Primary Changes**:

#### 1. src/hillclimber/test-generator-iterative.ts (NEW)
Core iterative generation engine. Exports:
```typescript
export async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  emitter: TestGenEmitter,
  options: IterativeTestGenOptions
): Promise<EnvironmentAwareTestResult>

export interface IterativeTestGenOptions extends TestGeneratorOptions {
  criteria: TerminationCriteria;
  enableGlobalRefinement: boolean;
  enableReflection: boolean;
}

export interface TerminationCriteria {
  minTestsPerCategory: number;
  targetTestsPerCategory: number;
  maxRoundsPerCategory: number;
  minTotalTests: number;
  targetTotalTests: number;
  maxTotalRounds: number;
  minComprehensivenessScore: number;
  maxGlobalRefinementRounds: number;
  maxTotalTokens: number;
  maxTotalTimeMs: number;
}
```

Implementation structure:
```typescript
async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  emitter: TestGenEmitter,
  options: IterativeTestGenOptions
): Promise<EnvironmentAwareTestResult> {
  // Phase 1: Initialization
  const state = initializeGeneratorState(taskId);
  emitter.onStart(createStartMessage(state, environment));
  
  // Phase 2: Category-based generation
  for (const category of CATEGORY_ORDER) {
    while (!isCategoryComplete(category, state, options.criteria)) {
      const roundNumber = state.categoryRoundNumber[category] + 1;
      
      emitter.onProgress({
        phase: "category_generation",
        currentCategory: category,
        roundNumber,
        status: `Generating ${category} tests (round ${roundNumber})...`
      });
      
      const newTests = await generateTestsForCategoryRound(
        category,
        state[`${category}Tests`],
        environment,
        roundNumber,
        emitter,
        options
      );
      
      state[`${category}Tests`].push(...newTests);
      state.categoryRoundNumber[category] = roundNumber;
      state.totalRounds++;
      
      if (options.enableReflection && roundNumber < options.criteria.maxRoundsPerCategory) {
        const reflection = await reflectOnCategory(category, state, environment, options);
        emitter.onReflection({
          category,
          reflectionText: reflection,
          action: "refining"
        });
      }
    }
  }
  
  // Phase 3: Global refinement (optional)
  if (options.enableGlobalRefinement) {
    let refinementRound = 0;
    while (
      refinementRound < options.criteria.maxGlobalRefinementRounds &&
      !isGenerationComplete(state, options.criteria)
    ) {
      refinementRound++;
      
      emitter.onProgress({
        phase: "global_refinement",
        roundNumber: refinementRound,
        status: "Assessing overall comprehensiveness..."
      });
      
      const assessment = await assessComprehensiveness(state, environment, options);
      state.comprehensivenessScore = assessment.score;
      
      emitter.onReflection({
        reflectionText: JSON.stringify(assessment, null, 2),
        action: "assessing"
      });
      
      if (assessment.score >= options.criteria.minComprehensivenessScore) {
        break;
      }
      
      // Generate additional tests based on recommendations
      const additionalTests = await generateFromRecommendations(
        assessment,
        state,
        environment,
        emitter,
        options
      );
      
      // Distribute to appropriate categories
      for (const test of additionalTests) {
        state[`${test.category}Tests`].push(test);
      }
    }
  }
  
  // Phase 4: Completion
  const result = buildFinalResult(state, options);
  emitter.onComplete(createCompleteMessage(state, result));
  
  return result;
}
```

#### 2. src/hillclimber/testgen-service.ts (MODIFY)
Update to use iterative generator:
```typescript
export async function runTestGenWithStreaming(
  suitePath: string,
  taskId: string | undefined,
  sessionId: string,
  emitter: TestGenEmitter,
  options: TestGenOptions,
): Promise<void> {
  // ... load suite, pick task, build environment ...
  
  // REPLACE single-shot call with iterative generator
  const result = await generateTestsIteratively(
    task.description,
    task.id,
    env,
    emitter,
    {
      model: options.model,
      verbose: false,
      criteria: {
        minTestsPerCategory: 2,
        targetTestsPerCategory: 5,
        maxRoundsPerCategory: 3,
        minTotalTests: 15,
        targetTotalTests: 25,
        maxTotalRounds: 10,
        minComprehensivenessScore: 8,
        maxGlobalRefinementRounds: 2,
        maxTotalTokens: 50000,
        maxTotalTimeMs: 180000,
      },
      enableGlobalRefinement: true,
      enableReflection: true,
    }
  );
  
  // Tests already emitted via emitter.onTest() during generation
  // Just emit completion (already done by generateTestsIteratively)
}
```

#### 3. src/hud/protocol.ts (MODIFY)
Add new message types:
- `TestGenProgressMessage`
- `TestGenReflectionMessage`
- Update `TestGenCompleteMessage` with iteration stats

#### 4. src/effuse/widgets/tb-command-center/tbcc-testgen.ts (MODIFY)
Handle new message types:
```typescript
// In subscriptions
if (msg.type === "testgen_progress") {
  const data = msg as TestGenProgressMessage;
  yield* ctx.state.update((s) => ({
    ...s,
    currentPhase: data.phase,
    currentCategory: data.currentCategory,
    currentStatus: data.status,
  }));
} else if (msg.type === "testgen_reflection") {
  // Optional: show reflection in UI for transparency
  const data = msg as TestGenReflectionMessage;
  yield* ctx.state.update((s) => ({
    ...s,
    lastReflection: data.reflectionText,
  }));
}
```

Add UI elements to show progress:
- Current phase indicator
- Current category being worked on
- Round number
- Estimated completion (based on criteria)

#### 5. src/llm/foundation-models.ts (OPTIONAL ENHANCEMENT)
Add streaming support for future optimization:
```typescript
export async function chatStreaming(
  request: ChatRequest,
  onChunk: (chunk: string) => void
): Promise<ChatResponse> {
  // Set stream: true in API request
  // Call onChunk for each SSE chunk
  // Use PartialToolArgsParser to extract complete tool calls as they arrive
}
```

### 8. Error Handling

**Graceful Degradation**:

```typescript
async function generateTestsForCategoryRound(
  category: TestCategory,
  existingTests: GeneratedTest[],
  environment: EnvironmentInfo,
  roundNumber: number,
  emitter: TestGenEmitter,
  options: IterativeTestGenOptions
): Promise<GeneratedTest[]> {
  try {
    const tests = await attemptGeneration(category, existingTests, environment, options);
    return tests;
  } catch (error) {
    console.error(`[TestGen] Error in ${category} round ${roundNumber}:`, error);
    
    // If this is the first round for this category, fail hard
    if (roundNumber === 1) {
      throw error;
    }
    
    // Otherwise, continue with what we have
    emitter.onReflection({
      category,
      reflectionText: `Failed to generate additional tests: ${error.message}`,
      action: "complete"
    });
    
    return [];  // Return empty array, move on to next category
  }
}
```

**Timeout Handling**:

```typescript
async function generateWithTimeout<T>(
  fn: () => Promise<T>,
  timeoutMs: number,
  timeoutMessage: string
): Promise<T> {
  return Promise.race([
    fn(),
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(timeoutMessage)), timeoutMs)
    ),
  ]);
}

// Usage
const tests = await generateWithTimeout(
  () => generateTestsForCategoryRound(...),
  30000,  // 30 second timeout per round
  `Timeout generating ${category} tests (round ${roundNumber})`
);
```

**Partial Success Handling**:

```typescript
// If LLM returns fewer tests than requested, that's OK
function validateRoundResults(
  tests: GeneratedTest[],
  category: TestCategory,
  roundNumber: number
): GeneratedTest[] {
  if (tests.length === 0 && roundNumber === 1) {
    throw new Error(`No tests generated for ${category} in initial round`);
  }
  
  // Filter out invalid tests
  return tests.filter(t => 
    t.id && 
    t.input && 
    t.category === category &&
    t.confidence >= 0 && t.confidence <= 1
  );
}
```

## Implementation Phases

### Phase 1: Core Iterative Engine (MVP)
**Goal**: Replace single-shot with category-based iteration (no global refinement yet)

1. Create `test-generator-iterative.ts` with:
   - `GeneratorState` type
   - `TerminationCriteria` type
   - `generateTestsIteratively()` function
   - Category loop with round-based generation
   - Basic prompt building (Round 1 only)

2. Update `testgen-service.ts`:
   - Replace `generateTestsFromEnvironment()` call with `generateTestsIteratively()`
   - Set conservative criteria (2-5 tests per category, max 3 rounds each)

3. Test with local FM on single task

**Success Criteria**:
- Generates tests category by category
- Emits tests one at a time via onTest()
- UI shows tests appearing incrementally
- Respects termination criteria

### Phase 2: Reflection and Refinement
**Goal**: Add round 2+ prompts that show existing tests and ask for gaps

1. Implement `buildCategoryPromptRoundN()`:
   - Show existing tests
   - Ask "what's missing?"
   - Request 1-3 additional tests

2. Add `reflectOnCategory()` function:
   - Optional analysis step between rounds
   - Emits TestGenReflectionMessage for transparency

3. Test that rounds 2-3 actually fill gaps (manual inspection)

**Success Criteria**:
- Round 2 generates different tests than Round 1
- Reflection messages show up in logs
- Coverage improves across rounds

### Phase 3: Global Assessment and Refinement
**Goal**: Add self-assessment after all categories processed

1. Implement `assessComprehensiveness()`:
   - Build global assessment prompt
   - Parse JSON response with comprehensiveness score
   - Extract recommendations

2. Implement `generateFromRecommendations()`:
   - Generate 2-5 tests based on assessment recommendations
   - Distribute to appropriate categories

3. Add TestGenProgressMessage for "global_refinement" phase

**Success Criteria**:
- Global assessment runs after category generation
- Comprehensiveness score appears in completion message
- Additional tests generated when score < 8

### Phase 4: UI Enhancements
**Goal**: Show iteration progress in real-time

1. Update `tbcc-testgen.ts` widget:
   - Add current phase/category/round to state
   - Show progress indicator (e.g., "Generating correctness tests, round 2 of 3")
   - Display comprehensiveness score when available

2. Add reflection panel (optional, for debugging):
   - Show LLM's gap analysis
   - Show assessment results

**Success Criteria**:
- User sees which category is being worked on
- User sees round numbers incrementing
- User sees final comprehensiveness score

### Phase 5: Optimization (Future)
**Goal**: Reduce latency and token costs

1. Add true streaming (FM client modification):
   - Enable `stream: true` in FM API calls
   - Use PartialToolArgsParser to extract tool calls as they arrive
   - Emit tests DURING LLM generation, not after

2. Implement parallel category generation:
   - Generate existence + anti-cheat in parallel (independent)
   - Wait for dependencies (correctness needs existence results)

3. Add caching:
   - Cache environment introspection results
   - Cache initial requirements extraction

**Success Criteria**:
- First test appears within 5 seconds (vs current 10-30s)
- Total token usage reduced by 20% (better prompts)
- Total time reduced by 30% (parallelization + streaming)

## Cost and Performance Analysis

### Token Estimates (per task)

**Current Single-Shot**:
- 1 LLM call
- Prompt: ~3000 tokens (task desc + full environment)
- Response: ~5000 tokens (all 25 tests)
- **Total: ~8000 tokens**

**Iterative (Conservative Estimate)**:
- Category generation: 5 categories × 2 rounds avg = 10 calls
  - Round 1 prompt: ~3000 tokens (full context)
  - Round 1 response: ~1000 tokens (3-5 tests)
  - Round 2 prompt: ~2000 tokens (existing tests + reflection)
  - Round 2 response: ~500 tokens (1-3 tests)
  - Per category: ~6500 tokens
  - Total category: 6500 × 5 = **32,500 tokens**
  
- Global refinement: 2 rounds
  - Assessment prompt: ~4000 tokens (all tests + task)
  - Assessment response: ~500 tokens (JSON)
  - Refinement prompt: ~3000 tokens
  - Refinement response: ~500 tokens
  - Total refinement: **8000 tokens**

**Total Iterative: ~40,000 tokens (5x increase)**

### Mitigation Strategies

1. **Aggressive Termination**:
   - Default to `maxRoundsPerCategory: 2` instead of 3
   - Lower `targetTestsPerCategory` to 3 instead of 5
   - Disable global refinement by default (opt-in)
   - **Reduces to ~20,000 tokens (2.5x increase)**

2. **Prompt Compression**:
   - Don't repeat full environment in round 2+ prompts
   - Use test IDs instead of full test objects for reflection
   - Summarize instead of showing complete tests
   - **Saves ~30% per round**

3. **Smart Caching** (FM with prompt caching):
   - Cache task description + environment (static across rounds)
   - Only dynamic part is "existing tests"
   - **Effective token cost: ~15,000 tokens**

4. **Streaming Optimization**:
   - Don't wait for full response before next prompt
   - Pipeline category generation (start existence while finishing anti-cheat)
   - **Reduces wall-clock time by 40%**

### Performance Targets

**MVP (Phase 1-2)**:
- Wall-clock time: 60-90 seconds (vs current 10-30s)
- Token usage: ~20,000 tokens per task
- Quality improvement: 25-30% better coverage (measured via evaluator)

**Optimized (Phase 5)**:
- Wall-clock time: 30-45 seconds (streaming + parallel)
- Token usage: ~15,000 tokens (prompt caching)
- Quality improvement: 40-50% better coverage

**Cost per 100 Tasks** (local FM: free, Claude: ~$0.03/1K tokens):
- Current: Free (local FM) or $24 (Claude)
- Iterative (unoptimized): Free or $120 (5x)
- Iterative (optimized): Free or $45 (1.9x)

## Success Metrics

### Quantitative

1. **Coverage Improvement**:
   - Baseline: Current single-shot generator
   - Target: 40% higher test_gen_evaluator scores
   - Measure: Run evaluator on TB2 tasks, compare overall scores

2. **Test Quality**:
   - Edge case detection: +50% (measured by evaluator.edgeCaseDetection)
   - Parameter coverage: 90% of file preview parameters tested (manual audit)
   - Anti-cheat effectiveness: 95% of prohibited tools tested (automated check)

3. **User Experience**:
   - Time to first test: < 5 seconds (vs current 10-30s)
   - Total generation time: < 60 seconds for 25 tests
   - UI responsiveness: Tests appear smoothly, no "all at once" jump

4. **Reliability**:
   - Success rate: > 95% (complete without errors)
   - Minimum quality: All runs meet minTestsPerCategory for all categories
   - Consistency: Variance in test count < 20% across runs

### Qualitative

1. **Comprehensiveness**:
   - Manual review of 10 generated suites: "Would these tests catch a naive implementation?"
   - Expert assessment: "Are there obvious gaps?"

2. **Diversity**:
   - Do iterative rounds add genuinely new scenarios?
   - Or do they duplicate existing tests?

3. **Transparency**:
   - Can users understand why each test was generated?
   - Do reflection messages add value?

## Rollout Plan

### Week 1: Core Implementation
- Day 1-2: Create test-generator-iterative.ts with Phase 2 loop
- Day 3: Update testgen-service.ts integration
- Day 4: Add termination criteria and state management
- Day 5: Test on 5 TB2 tasks, debug issues

### Week 2: Refinement Features
- Day 1-2: Implement reflection prompts (Round 2+)
- Day 3: Add TestGenReflectionMessage and emission
- Day 4-5: Test refinement quality on 10 tasks

### Week 3: Global Assessment
- Day 1-2: Implement assessComprehensiveness()
- Day 3: Implement generateFromRecommendations()
- Day 4-5: End-to-end testing, tune comprehensiveness threshold

### Week 4: UI and Polish
- Day 1-2: Update tbcc-testgen widget with progress
- Day 3: Add reflection display (optional)
- Day 4: Performance profiling and optimization
- Day 5: Documentation and demo

### Week 5: Optimization (Optional)
- Streaming implementation
- Prompt caching
- Parallel generation
- Benchmark against baseline

## Risks and Mitigations

### Risk 1: Token Cost Explosion
**Impact**: High (5x token increase)
**Mitigation**:
- Default to aggressive termination criteria
- Make global refinement opt-in
- Monitor token usage, add hard limits
- Use local FM primarily (free)

### Risk 2: Quality Doesn't Improve
**Impact**: High (invalidates entire approach)
**Mitigation**:
- Run test-gen-evaluator on baseline first (establish ground truth)
- Measure incrementally (does Round 2 beat Round 1?)
- Manual review of failing cases
- Fallback: Keep single-shot as option

### Risk 3: Generation Too Slow
**Impact**: Medium (user frustration)
**Mitigation**:
- Phase 5 streaming optimizations
- Parallel category generation
- Reduce round counts if needed
- Show progress so users know it's working

### Risk 4: LLM Reflection is Poor
**Impact**: Medium (wasted rounds)
**Mitigation**:
- Test reflection quality separately (can LLM identify gaps?)
- Tune prompts based on actual reflection outputs
- Skip reflection if it's not helpful (benchmark with/without)

### Risk 5: Complexity
**Impact**: Low-Medium (harder to maintain)
**Mitigation**:
- Keep single-shot generator as backup
- Document state machine clearly
- Add extensive logging for debugging
- Unit test each phase independently

## Appendices

### A. Category-Specific Guidance

```typescript
function getCategoryGuidance(category: TestCategory): string {
  switch (category) {
    case "anti_cheat":
      return `
ANTI-CHEAT TESTS verify that prohibited tools/approaches are NOT used.
- If task says "convert R to Python", check that R is NOT installed
- If task says "implement from scratch", check no pre-built libraries exist
- Think: "What would a lazy implementation do that we should catch?"
Examples:
- which R 2>/dev/null || echo 'not found'  →  expect: 'not found'
- pip list | grep scikit-learn  →  expect: empty (if task forbids sklearn)
      `;
      
    case "existence":
      return `
EXISTENCE TESTS verify required outputs are created.
- File exists at correct path
- File is non-empty
- File has correct permissions (if relevant)
Examples:
- test -f output.txt && echo 'exists'  →  expect: 'exists'
- test -s output.txt && echo 'non-empty'  →  expect: 'non-empty'
      `;
      
    case "correctness":
      return `
CORRECTNESS TESTS verify basic functionality works.
- Happy path: valid input → expected output
- All parameters from file previews are tested
- Verify output format matches spec
Examples:
- python solution.py --input valid.txt  →  expect: correct result
- Check all parameters: alpha, sigma, beta (from file previews)
      `;
      
    case "boundary":
      return `
BOUNDARY TESTS verify limits and ranges.
- Minimum valid value
- Maximum valid value
- Just below minimum (should fail/reject)
- Just above maximum (should fail/reject)
Examples:
- Input: 0 (minimum)  →  expect: handled correctly
- Input: MAX_VALUE + 1  →  expect: error or null
      `;
      
    case "integration":
      return `
INTEGRATION TESTS verify system-level behavior.
- Multi-step workflows
- Interaction with existing files
- End-to-end verification
Examples:
- Run full pipeline: preprocess → analyze → output
- Verify integration with provided data files
      `;
  }
}
```

### B. Example Round Progression

**Task**: "Convert R simulation to Python. Preserve all parameters: alpha, sigma, rho."

**Round 1 - Anti-Cheat**:
```
Generated:
1. which R → expect: not found
2. Rscript --version → expect: error
```

**Round 2 - Anti-Cheat** (reflection: "check R packages too"):
```
Generated:
3. R -e "library(dplyr)" → expect: error
```

**Round 1 - Existence**:
```
Generated:
1. test -f simulation.py → expect: exists
2. test -s simulation.py → expect: non-empty
3. python -m py_compile simulation.py → expect: success
```

**Round 2 - Existence** (reflection: "check output files"):
```
Generated:
4. python simulation.py && test -f results.csv → expect: exists
```

**Round 1 - Correctness**:
```
Generated:
1. python simulation.py --alpha 0.5 → expect: numeric output
2. python simulation.py --sigma 1.0 → expect: numeric output
3. python simulation.py --alpha 0.5 --sigma 1.0 → expect: valid result
```

**Round 2 - Correctness** (reflection: "missing rho parameter!"):
```
Generated:
4. python simulation.py --rho 0.8 → expect: numeric output
5. python simulation.py --alpha 0.5 --sigma 1.0 --rho 0.8 → expect: full result
```

**Global Assessment**:
```
Comprehensiveness Score: 7
Missing: "What about invalid parameter values?"
Recommendations: 
- Test alpha < 0 (should error)
- Test sigma = 0 (boundary case)
```

**Global Refinement**:
```
Generated:
1. python simulation.py --alpha -1 → expect: error
2. python simulation.py --sigma 0 → expect: error or special handling
```

**Final Suite**: 15 tests across 5 categories, comprehensiveness score: 9

### C. Testing Checklist

Before merging:

- [ ] Unit test `isCategoryComplete()`
- [ ] Unit test `isGenerationComplete()`
- [ ] Unit test prompt builders (Round 1 vs Round N)
- [ ] Integration test: Full run on regex-log task
- [ ] Integration test: Full run on path-tracing task
- [ ] Integration test: Termination at max rounds
- [ ] Integration test: Termination at max tokens
- [ ] Integration test: Termination at target quality
- [ ] Error handling: LLM timeout during round
- [ ] Error handling: LLM returns 0 tests
- [ ] Error handling: Invalid JSON in response
- [ ] Performance: Measure total time on 10 tasks
- [ ] Performance: Measure token usage on 10 tasks
- [ ] Quality: Run evaluator, compare to baseline
- [ ] UI: Tests appear one at a time
- [ ] UI: Progress messages shown correctly
- [ ] UI: Reflection messages shown (if enabled)

---

## Critical Files for Implementation

- **/Users/christopherdavid/code/openagents/src/hillclimber/test-generator-iterative.ts** - NEW: Core iterative generation engine with state management, category loops, reflection, and assessment
  
- **/Users/christopherdavid/code/openagents/src/hillclimber/testgen-service.ts** - MODIFY: Replace single-shot call with iterative generator, update to use new termination criteria
  
- **/Users/christopherdavid/code/openagents/src/hud/protocol.ts** - MODIFY: Add TestGenProgressMessage, TestGenReflectionMessage, update TestGenCompleteMessage with iteration stats
  
- **/Users/christopherdavid/code/openagents/src/effuse/widgets/tb-command-center/tbcc-testgen.ts** - MODIFY: Handle progress/reflection messages, show current phase/category/round in UI
  
- **/Users/christopherdavid/code/openagents/src/hillclimber/test-generator.ts** - REFERENCE: Existing prompt templates, parsing logic, environment formatting to adapt for iterative approach
