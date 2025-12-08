# 1224 Blind Verification Redesign

## The Problem

The current HillClimber v2 implementation is fundamentally flawed:

```
CURRENT FLOW (Gaming):
1. FM reads task description
2. FM writes solution
3. Run REAL benchmark tests
4. FM sees: "expected ['2025-01-09'], got ['2025-01-09', '2020-01-01']"
5. FM adjusts to produce exact expected output
6. Repeat until tests pass
```

This proves NOTHING. Any system can pass tests it can see and iterate on. The whole point of stakes.md is to prove:

> **Architecture + local inference > model scale**

If we game the benchmark, we don't prove this. We just prove "iteration eventually works."

## The Real Capability We Need to Demonstrate

The thesis is that a small local model with the right architecture can beat large cloud models. This requires demonstrating GENERALIZATION:

1. The system must work on tasks it hasn't seen during development
2. The system must understand requirements deeply, not just pattern-match to test cases
3. The improvement must come from architecture, not from knowing the answers

## New Architecture: Self-Generated Verification

### Core Principle

**The agent must be BLIND to the actual benchmark tests.**

Instead, the agent:
1. Reads the task description
2. Reasons about what would make a correct solution
3. Generates its OWN test cases
4. Implements and iterates against its own tests
5. Only runs real benchmark tests as final validation (with minimal feedback)

### Phase 1: Understanding & Test Generation

```
┌─────────────────────────────────────────────────────────────┐
│                    TEST GENERATOR                            │
│                                                              │
│  Input: Task description (ONLY - no access to real tests)   │
│                                                              │
│  Process:                                                    │
│  1. Parse requirements from description                      │
│  2. Identify explicit constraints                            │
│  3. Reason about implicit edge cases                         │
│  4. Generate diverse test cases covering:                    │
│     - Happy path                                             │
│     - Boundary conditions                                    │
│     - Error cases                                            │
│     - Edge cases the description hints at                    │
│                                                              │
│  Output: GeneratedTest[]                                     │
└─────────────────────────────────────────────────────────────┘
```

For regex-log, the test generator would reason:
- "The description says YYYY-MM-DD format" → generate valid dates
- "The description says valid IPv4" → generate IPs with 0-255 octets
- "The description mentions false matches like 1134-12-1234" → generate similar traps
- "The description says LAST date" → generate lines with multiple dates

The key: the generator doesn't KNOW the actual test cases. It REASONS about what tests would be needed.

### Phase 2: Implementation with Self-Tests

```
┌─────────────────────────────────────────────────────────────┐
│                 SELF-VERIFICATION LOOP                       │
│                                                              │
│  1. FM implements initial solution                           │
│  2. Run SELF-GENERATED tests (not benchmark tests)           │
│  3. FM sees results of its OWN tests                         │
│  4. FM iterates until self-tests pass                        │
│                                                              │
│  The agent is optimizing for its UNDERSTANDING of the        │
│  requirements, not for known correct answers.                │
└─────────────────────────────────────────────────────────────┘
```

### Phase 3: Blind Benchmark Verification

```
┌─────────────────────────────────────────────────────────────┐
│                  BLIND VERIFICATION                          │
│                                                              │
│  When agent thinks it's done:                                │
│  1. Run REAL benchmark tests                                 │
│  2. Return ONLY: passed: true/false                          │
│  3. NO expected values                                       │
│  4. NO specific failure details that reveal answers          │
│                                                              │
│  Optional feedback (generic only):                           │
│  - "Some edge cases failing"                                 │
│  - "Input validation issues"                                 │
│  - NOT: "Expected X, got Y"                                  │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Plan

### 1. Test Generator Module

**File**: `src/hillclimber/test-generator.ts`

```typescript
export interface GeneratedTest {
  id: string;
  /** Test input (e.g., log line for regex-log) */
  input: string;
  /** Expected output (e.g., matched date or null) */
  expectedOutput: string | null;
  /** Why this test exists */
  reasoning: string;
  /** What category: happy_path, edge_case, boundary, error */
  category: "happy_path" | "edge_case" | "boundary" | "error";
}

export interface TestGenerationResult {
  tests: GeneratedTest[];
  requirements: string[];      // Parsed requirements from description
  assumptions: string[];       // Assumptions the generator made
  uncertainties: string[];     // Things the generator wasn't sure about
}

/**
 * Generate test cases from ONLY the task description.
 * Must NOT have access to real benchmark tests.
 */
export async function generateTestsFromDescription(
  taskDescription: string,
  taskId: string,
  options?: {
    minTests?: number;
    maxTests?: number;
    reasonerModel?: string;
  }
): Promise<TestGenerationResult>;
```

The test generator uses a reasoner (FM or a reasoning model) with a prompt like:

```
You are a QA engineer. Based ONLY on this task description, generate test cases.

Task Description:
{description}

Generate tests that cover:
1. The explicit requirements stated in the description
2. Edge cases hinted at in the description
3. Common pitfalls for this type of task
4. Boundary conditions

For each test, explain WHY it's important.

You do NOT have access to the actual test suite. You must REASON about what tests would be needed.
```

### 2. Self-Verification Runner

**File**: `src/hillclimber/self-verifier.ts`

```typescript
export interface SelfVerificationResult {
  passed: boolean;
  testsRun: number;
  testsPassed: number;
  failures: Array<{
    testId: string;
    input: string;
    expected: string;
    actual: string;
    category: string;
  }>;
}

/**
 * Run the agent's self-generated tests.
 * This is NOT the benchmark - these are tests the agent created.
 */
export async function runSelfVerification(
  task: TerminalBenchTask,
  workspace: string,
  generatedTests: GeneratedTest[]
): Promise<SelfVerificationResult>;
```

For regex-log, this would:
1. Read the regex from `/app/regex.txt`
2. Run it against each generated test input
3. Compare output to expected
4. Return detailed results (agent CAN see these - they're its own tests)

### 3. Blind Benchmark Runner

**File**: `src/hillclimber/blind-verifier.ts`

```typescript
export interface BlindVerificationResult {
  /** Did all benchmark tests pass? */
  passed: boolean;
  /** Generic progress indicator (0-1) */
  progress: number;
  /** Optional generic feedback (NO expected values) */
  feedback?: string;
}

/**
 * Run the REAL benchmark tests but return minimal information.
 *
 * CRITICAL: This must NOT reveal expected values or specific failures
 * that would allow the agent to reverse-engineer the answers.
 */
export async function runBlindVerification(
  task: TerminalBenchTask,
  workspace: string
): Promise<BlindVerificationResult>;
```

The blind verifier:
1. Runs real benchmark tests
2. Returns ONLY pass/fail and a rough progress score
3. Optionally returns GENERIC feedback:
   - "Input validation issues" (not "expected 0-255, got 256")
   - "Edge case failures" (not "expected no match on line 7")
   - "Format issues" (not "expected PPM P6, got P3")

### 4. Redesigned Orchestrator

**File**: `src/hillclimber/blind-orchestrator.ts`

```typescript
/**
 * Orchestrator that maintains blindness to benchmark tests.
 */
export async function runBlindMAP(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  options: BlindMAPOptions
): Promise<BlindMAPResult> {

  // Phase 1: Generate tests from description ONLY
  const testGen = await generateTestsFromDescription(
    task.description,
    task.id
  );

  log(`Generated ${testGen.tests.length} self-tests`);
  log(`Requirements identified: ${testGen.requirements.join(', ')}`);
  log(`Uncertainties: ${testGen.uncertainties.join(', ')}`);

  // Phase 2: Implement with self-verification
  let selfVerifyResult: SelfVerificationResult;
  let attempts = 0;

  while (attempts < maxAttempts) {
    // FM implements/iterates
    await runFMImplementation(task, workspace, testGen);

    // Run SELF-GENERATED tests (agent can see these results)
    selfVerifyResult = await runSelfVerification(task, workspace, testGen.tests);

    if (selfVerifyResult.passed) {
      log(`Self-tests passing, running blind verification...`);
      break;
    }

    // FM can iterate based on its OWN test failures
    attempts++;
  }

  // Phase 3: Blind benchmark verification
  const blindResult = await runBlindVerification(task, workspace);

  if (blindResult.passed) {
    return { passed: true, ... };
  }

  // If blind verification fails, agent gets GENERIC feedback only
  // It must reason about what it might have missed
  log(`Blind verification failed. Generic feedback: ${blindResult.feedback}`);

  // Agent can try to generate MORE self-tests based on the generic feedback
  // But it still can't see the actual expected values
}
```

### 5. Skill Library Cleanup

**Remove task-specific patterns from `tb2-skills.ts`**

Current (problematic):
```typescript
// This is basically giving away the answer
"IPv4 pattern: (?:25[0-5]|2[0-4]\\d|1?\\d?\\d\\.){3}..."
"Date pattern with boundary: (?:^|[^0-9A-Za-z])\\d{4}-..."
```

New (general knowledge only):
```typescript
// General regex knowledge - NOT specific to this task
"Regex character classes: \\d matches digits, \\w matches word chars"
"Regex quantifiers: {n} exact, {n,m} range, * zero-or-more"
"Regex lookahead: (?=...) positive, (?!...) negative"

// The agent must FIGURE OUT that it needs boundary assertions
// We don't tell it "use (?:^|[^0-9A-Za-z])"
```

### 6. Decomposer Cleanup

**Remove task-specific process knowledge**

Current (problematic):
```typescript
// regex-log decomposition that basically solves the problem
subtasks: [
  { goal: "Write regex with IPv4 boundary assertions" },  // Giving away the approach
  { goal: "Add date validation with month/day limits" },  // Giving away what's needed
]
```

New (generic process only):
```typescript
// Generic decomposition - agent must figure out specifics
subtasks: [
  { goal: "Understand requirements from description" },
  { goal: "Design test cases covering requirements" },
  { goal: "Implement initial solution" },
  { goal: "Iterate until self-tests pass" },
  { goal: "Verify against benchmark" },
]
```

## Validation: How Do We Know This Works?

### The Generalization Test

1. **Development**: Build and tune on TB2 tasks
2. **Holdout**: Keep some TB2 tasks completely unseen during development
3. **Test**: Run on holdout tasks
4. **Measure**: Does performance transfer?

If performance transfers to unseen tasks, we've proven generalization.
If it doesn't, we were still gaming (just more subtly).

### The Cross-Benchmark Test

1. **Build** on Terminal-Bench 2
2. **Test** on Terminal-Bench 3 (or SWE-Bench, or other coding benchmarks)
3. **Measure**: Does the architecture help on completely different tasks?

### Success Metrics

| Metric | Gaming (Bad) | Generalizing (Good) |
|--------|--------------|---------------------|
| TB2 seen tasks | High | High |
| TB2 holdout tasks | Low | Similar to seen |
| TB3/other benchmarks | Low | Reasonable |
| Novel tasks | Fails | Works |

## Why This Proves the Thesis

If HillClimber can:
1. Generate its own tests from requirements
2. Implement solutions that pass both self-tests AND unseen benchmark tests
3. Do this with a small local model

Then we've proven:
- **Understanding > memorization**: The model understands requirements, not test cases
- **Architecture matters**: The test generation + iteration loop is what enables success
- **Local can win**: Low latency enables more reasoning iterations

This is what stakes.md is about. Not gaming a benchmark, but proving a fundamental insight about how agents should work.

## Implementation Order

### Step 1: Test Generator (Priority: Critical)
- Build test generation from description only
- Validate that generated tests cover similar ground to real tests (without seeing them)
- This is the hardest and most important part

### Step 2: Self-Verification Runner
- Run agent's own tests
- Full feedback to agent on its own tests
- This enables the iteration loop

### Step 3: Blind Benchmark Runner
- Run real tests with minimal feedback
- Only pass/fail + generic hints
- No expected value leakage

### Step 4: Skill/Decomposer Cleanup
- Remove task-specific knowledge
- Keep only general domain patterns
- Let the agent figure out what's needed

### Step 5: Holdout Validation
- Reserve some tasks for testing
- Measure generalization
- Iterate until transfer works

## Open Questions

1. **How generic should feedback be?**
   - Too generic: Agent can't improve
   - Too specific: Back to gaming
   - Maybe: "You have edge case failures in input validation" without saying WHICH cases

2. **How good does test generation need to be?**
   - If agent generates tests that miss what benchmark tests, it will fail blind verification
   - Need the reasoner to be good at anticipating edge cases
   - This is actually the core capability we're building

3. **How many blind verification attempts?**
   - 1 attempt: Very hard, but truest test
   - N attempts with generic feedback: Allows some iteration
   - Need to balance difficulty with practicality

4. **What if test generation uses a cloud model?**
   - If test generation requires GPT-4 but implementation uses local FM
   - Is that still "local beats cloud"?
   - Maybe: Test generation is one-time, implementation is the repeated part

---

## Summary

The current implementation proves nothing because FM can see and iterate on real test cases.

The new architecture:
1. **Test Generator**: Reasons about requirements, generates tests WITHOUT seeing real tests
2. **Self-Verification**: Agent iterates on its OWN tests
3. **Blind Verification**: Real tests with minimal feedback

This proves actual capability: understanding requirements well enough to anticipate edge cases.

If this works on unseen tasks, we've proven the stakes.md thesis.
If it only works on seen tasks, we were gaming, just more subtly.

The goal is GENERALIZATION, not benchmark scores.

---

*Plan by Claude, 2025-12-08*
