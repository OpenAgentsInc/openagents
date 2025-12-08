# HillClimber v3: Blind Verification Architecture

## Problem Statement

The current HillClimber implementation "games" the benchmark by showing the agent actual test results including expected values. This proves nothing about generalization - any system can pass tests it can iterate on with full feedback.

**The goal**: Prove that `architecture + local inference > model scale` by demonstrating the system works on tasks where the agent **never sees the actual test cases**.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    BLIND ORCHESTRATOR                            │
│  1. Read task description                                        │
│  2. Generate self-tests from description (BLIND to real tests)  │
│  3. Implement + iterate against self-tests                       │
│  4. Final blind verification (pass/fail only)                   │
└─────────────────────────────────────────────────────────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│    TEST      │    │    SELF      │    │    BLIND     │
│  GENERATOR   │    │  VERIFIER    │    │  VERIFIER    │
│              │    │              │    │              │
│ Task desc →  │    │ Run agent's  │    │ Run REAL     │
│ Generate     │    │ own tests    │    │ benchmark    │
│ test cases   │    │ Full detail  │    │ Pass/fail    │
│              │    │ allowed      │    │ only         │
└──────────────┘    └──────────────┘    └──────────────┘
```

---

## TB2 Test Pattern Analysis (95 Tasks)

### Distilled Test Principles

From analyzing all TB2 test files, tests follow a **layered difficulty pattern**:

| Layer | What It Tests | Examples |
|-------|---------------|----------|
| **1. Existence** | File/output created | `assert Path('/app/regex.txt').exists()` |
| **2. Format** | Structure valid | JSON parses, CSV headers match, PPM magic bytes |
| **3. Correctness** | Happy path works | Valid IP → date extraction works |
| **4. Boundary** | Limits honored | Month 1-12, octet 0-255, length 15-45 |
| **5. Edge Cases** | Traps avoided | 256.x.x.x invalid, alphanumeric boundaries |
| **6. Integration** | System works E2E | Multi-step deployment, timing constraints |

### Test Categories Found Across TB2

1. **Happy Path** (100% of tasks) - Basic correct behavior
2. **Boundary Conditions** (90%+) - Min/max values, limits
3. **Invalid Inputs** (80%+) - What should fail/be rejected
4. **Edge Cases** (70%+) - Tricky scenarios hinted in description
5. **Precision/Tolerance** (30%+) - Numerical similarity thresholds
6. **Timing/Order** (20%+) - Concurrency, sequence dependencies
7. **Anti-Cheat** (10%+) - Hide reference files, verify from scratch

### Gap Analysis: Description vs Actual Tests

| Gap Type | Example | Frequency |
|----------|---------|-----------|
| Hidden implementation details | BsaI primer structure not in description | 60%+ |
| Implicit edge cases | IP 0.0.0.0 and 255.255.255.255 are valid | 80%+ |
| Domain expertise required | Biology (Tm calc), security (XSS bypass) | 40%+ |
| System integration knowledge | Git hooks, Nginx config, SSH setup | 30%+ |
| Exact expected values | Tests have hard-coded answers | 90%+ |

---

## Implementation Plan

### Phase 1: Test Generator (Priority: CRITICAL)

**Goal**: Generate tests from task description that cover similar ground to real tests without seeing them.

#### File: `src/hillclimber/test-generator.ts`

```typescript
export interface GeneratedTest {
  id: string;
  input: string;                    // Test input
  expectedOutput: string | null;    // What we expect (our reasoning)
  reasoning: string;                // Why this test matters
  category: TestCategory;           // Which layer/type
  confidence: number;               // 0-1: how sure we are this is right
}

export type TestCategory =
  | "existence"      // File/output created
  | "format"         // Structure valid
  | "happy_path"     // Basic correct behavior
  | "boundary"       // Min/max limits
  | "edge_case"      // Tricky scenarios
  | "invalid_input"  // Should fail/reject
  | "integration";   // System-level

export interface TestGenerationResult {
  tests: GeneratedTest[];
  requirements: string[];     // Parsed from description
  assumptions: string[];      // What we assumed
  uncertainties: string[];    // What we're unsure about
}

export async function generateTestsFromDescription(
  taskDescription: string,
  taskId: string,
  options?: {
    minTests?: number;        // Default: 10
    maxTests?: number;        // Default: 30
    reasonerModel?: string;   // Model for reasoning
  }
): Promise<TestGenerationResult>;
```

#### Test Generation Prompt (Core Prompt Engineering)

```markdown
You are a QA engineer designing tests for a programming task. You must create
tests that would verify a correct implementation WITHOUT seeing the actual test suite.

## Task Description
{description}

## Your Mission
Generate test cases that a correct implementation MUST pass. Think like a QA engineer:
1. What are the explicit requirements?
2. What edge cases are hinted at but not stated?
3. What boundaries exist (min/max, valid/invalid)?
4. What would a naive implementation get wrong?

## Test Categories (generate at least 2 per applicable category):

### 1. Existence Tests
- Does the output file exist?
- Is it non-empty?

### 2. Format Tests
- Does the output have correct structure?
- Headers, magic bytes, encoding correct?

### 3. Happy Path Tests
- Basic valid input → expected output
- The "golden path" through the requirements

### 4. Boundary Tests
For each numeric/range constraint in the description:
- Minimum valid value
- Maximum valid value
- Just below minimum (should fail)
- Just above maximum (should fail)

### 5. Edge Case Tests
For each constraint mentioned:
- What about the edge between valid/invalid?
- What about multiple instances?
- What about order/sequence?

### 6. Invalid Input Tests
What inputs should be rejected or produce no output?
- Invalid formats
- Out-of-range values
- Malformed data

## Output Format
For each test, provide:
- id: unique identifier (e.g., "happy_path_1")
- input: the test input (e.g., log line, chess position)
- expectedOutput: what correct behavior produces (or null for rejection)
- reasoning: WHY this test is important
- category: which category above
- confidence: 0-1 how certain you are this test is correct

## Critical Rules
1. You do NOT have access to the real test suite
2. You must REASON about what tests would be needed
3. Err on the side of more tests, not fewer
4. Include tests you're uncertain about (mark confidence low)
5. Think adversarially: what would break a naive implementation?
```

#### Task-Specific Test Generation Hints

Different task types need different test generation strategies:

| Task Type | Focus On | Example Tests |
|-----------|----------|---------------|
| **Regex/Pattern** | Boundary assertions, false positives | Valid matches, invalid that looks valid |
| **Algorithmic** | Numerical precision, edge inputs | Zero, negative, very large values |
| **File Format** | Magic bytes, structure, encoding | Malformed headers, wrong encoding |
| **Bio/Science** | Physical constraints, ranges | Tm limits, length ranges, sequence validity |
| **System/Deploy** | Integration, timing, permissions | Multi-step verification, retries |
| **Security** | Bypass attempts, edge cases | Encoding tricks, boundary exploitation |

### Phase 2: Self-Verifier

**Goal**: Run agent's self-generated tests with full feedback (agent can see its own test results).

#### File: `src/hillclimber/self-verifier.ts`

```typescript
export interface SelfVerificationResult {
  passed: boolean;
  testsRun: number;
  testsPassed: number;
  failures: Array<{
    testId: string;
    input: string;
    expected: string | null;
    actual: string | null;
    category: TestCategory;
  }>;
}

export async function runSelfVerification(
  task: TerminalBenchTask,
  workspace: string,
  generatedTests: GeneratedTest[]
): Promise<SelfVerificationResult>;
```

Implementation varies by task type:
- **regex-log**: Read regex from `/app/regex.txt`, apply to test inputs
- **path-tracing**: Compile image.c, run, compare output to generated expectations
- **dna-assembly**: Parse primers, validate against generated biological constraints

### Phase 3: Blind Verifier

**Goal**: Run REAL benchmark tests but reveal only pass/fail + generic feedback.

#### File: `src/hillclimber/blind-verifier.ts`

```typescript
export interface BlindVerificationResult {
  passed: boolean;
  progress: number;           // 0-1 rough score (tests passing / total)
  feedback?: BlindFeedback;   // Generic hints only
}

export type BlindFeedback =
  | "existence_issues"        // Output file doesn't exist
  | "format_issues"           // Output structure wrong
  | "some_cases_failing"      // Happy path works, edge cases don't
  | "boundary_issues"         // Limits not handled correctly
  | "precision_issues"        // Numerical accuracy problems
  | "integration_issues";     // System-level failures

export async function runBlindVerification(
  task: TerminalBenchTask,
  workspace: string
): Promise<BlindVerificationResult>;
```

**Critical**: The blind verifier must NOT reveal:
- Exact expected values
- Which specific test case failed
- What the correct answer is
- Anything that would let agent reverse-engineer the answer

### Phase 4: Blind Orchestrator

**Goal**: Coordinate the full flow maintaining blindness.

#### File: `src/hillclimber/blind-orchestrator.ts`

```typescript
export async function runBlindMAP(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  options: BlindMAPOptions
): Promise<BlindMAPResult> {

  // Phase 1: Generate self-tests (ONCE, before implementation)
  const testGen = await generateTestsFromDescription(
    task.description,
    task.id,
    { reasonerModel: options.reasonerModel }
  );

  log(`Generated ${testGen.tests.length} self-tests`);
  log(`Categories: ${summarizeCategories(testGen.tests)}`);
  log(`Uncertainties: ${testGen.uncertainties.join(', ')}`);

  // Phase 2: Implementation loop with self-verification
  let selfResult: SelfVerificationResult;
  let attempts = 0;
  const maxSelfAttempts = options.maxSelfAttempts ?? 10;

  while (attempts < maxSelfAttempts) {
    // FM implements/iterates
    await runFMImplementation(task, workspace, {
      selfTests: testGen.tests,
      previousFailures: selfResult?.failures,
    });

    // Run SELF-generated tests (full feedback allowed)
    selfResult = await runSelfVerification(task, workspace, testGen.tests);

    if (selfResult.passed) {
      log(`Self-tests passing after ${attempts} attempts`);
      break;
    }

    attempts++;
  }

  // Phase 3: Blind benchmark verification
  const blindResult = await runBlindVerification(task, workspace);

  if (blindResult.passed) {
    return {
      passed: true,
      selfTestsGenerated: testGen.tests.length,
      selfAttempts: attempts,
      generalizationProven: true,
    };
  }

  // Phase 4: If blind fails, agent can generate MORE self-tests
  // based on generic feedback, but CANNOT see actual expected values
  if (options.allowTestRefinement && blindResult.feedback) {
    const additionalTests = await refineTests(
      testGen,
      blindResult.feedback,
      task.description
    );
    // ... retry loop with expanded test set
  }

  return {
    passed: false,
    selfTestsGenerated: testGen.tests.length,
    selfAttempts: attempts,
    blindFeedback: blindResult.feedback,
  };
}
```

---

## Files to Create/Modify

### Create New
| File | Purpose |
|------|---------|
| `src/hillclimber/test-generator.ts` | Generate tests from description |
| `src/hillclimber/self-verifier.ts` | Run agent's own tests |
| `src/hillclimber/blind-verifier.ts` | Run real tests with minimal feedback |
| `src/hillclimber/blind-orchestrator.ts` | Coordinate blind flow |
| `src/hillclimber/test-runner/regex.ts` | Run tests for regex tasks |
| `src/hillclimber/test-runner/compile.ts` | Run tests for compilation tasks |

### Modify Existing
| File | Changes |
|------|---------|
| `src/hillclimber/runner.ts` | Add `--blind` mode flag |
| `src/hillclimber/evaluator.ts` | Add blind mode (no expected values) |
| `src/skills/library/tb2-skills.ts` | Remove task-specific patterns |
| `src/hillclimber/decomposer.ts` | Make generic (no task-specific hints) |

---

## Implementation Order

### Step 1: Test Generator Core (First Priority)

1. Create `src/hillclimber/test-generator.ts` with:
   - `GeneratedTest` interface
   - `generateTestsFromDescription()` function
   - Prompt template with TB2 principles

2. Test it standalone:
   ```bash
   bun run test-gen --task regex-log --verbose
   ```

3. Validate quality:
   - Do generated tests cover similar ground to real tests?
   - What's the overlap percentage?
   - What categories are missing?

### Step 2: Self-Verifier

1. Create `src/hillclimber/self-verifier.ts`
2. Create task-specific runners in `src/hillclimber/test-runner/`:
   - `regex.ts` - Apply regex to inputs, capture matches
   - `compile.ts` - Compile code, run, capture output
   - `python.ts` - Run Python scripts, capture output

### Step 3: Blind Verifier

1. Create `src/hillclimber/blind-verifier.ts`
2. Modify existing evaluator to strip expected values
3. Map pytest output to generic feedback categories

### Step 4: Orchestrator Integration

1. Create `src/hillclimber/blind-orchestrator.ts`
2. Add `--blind` flag to runner
3. Wire everything together

### Step 5: Skill/Decomposer Cleanup

1. Remove task-specific patterns from `tb2-skills.ts`
2. Keep only general domain knowledge:
   - Regex syntax (not specific patterns)
   - C compilation basics (not specific algorithms)
   - Python stdlib usage (not specific solutions)

3. Remove task-specific decomposition from `decomposer.ts`
4. Replace with generic process:
   - Understand requirements
   - Design self-tests
   - Implement
   - Iterate
   - Verify

---

## Validation Strategy

### Holdout Testing

Reserve 20% of TB2 tasks (19 tasks) as holdout:
- Never look at their tests during development
- Never tune for them specifically
- Use them ONLY for final generalization testing

**Holdout Selection** (random sample):
```
adaptive-rejection-sampler, caffe-cifar-10, cobol-modernization,
constraints-scheduling, distribution-search, extract-elf,
feal-differential-cryptanalysis, git-leak-recovery, gpt2-codegolf,
hf-model-inference, llm-inference-batching-scheduler, make-mips-interpreter,
mcmc-sampling-stan, mteb-retrieve, polyglot-rust-c, pytorch-model-recovery,
qemu-startup, schemelike-metacircular-eval, sqlite-with-gcov
```

### Success Metrics

| Metric | Gaming (Bad) | Generalizing (Good) |
|--------|--------------|---------------------|
| Development tasks | High pass rate | High pass rate |
| Holdout tasks | Low pass rate | Similar pass rate |
| Test generation coverage | Low overlap | High overlap with real tests |
| Self-test → Blind success | Low correlation | High correlation |

### Generalization Proof

If the system achieves:
1. **80%+ self-test pass rate** on development tasks
2. **60%+ blind verification pass rate** on development tasks
3. **50%+ blind verification pass rate** on holdout tasks

Then we've proven the architecture generalizes and isn't gaming.

---

## Design Decisions (Confirmed)

### 1. Reasoner Model for Test Generation

**Decision**: Try both Claude and local FM for test generation, measure which produces better test coverage, then decide.

Implementation:
- Add `--test-gen-model` flag: `claude` | `local` | `both`
- Track coverage metrics per model
- Compare overlap with real tests (without revealing real tests to agent)

### 2. Information Boundary (CRITICAL)

**The boundary is absolute**: ZERO information flows from real benchmark tests into the runtime environment.

| Source | Agent Access | Details Shown |
|--------|--------------|---------------|
| Self-generated tests | FULL | All inputs, expected values, actual results, diffs |
| Real benchmark tests | NONE | Only final pass/fail at the very end |

The agent operates in a **closed system** where:
1. It generates its own tests from the task description
2. It runs those tests with full visibility
3. It iterates until self-tests pass
4. Only THEN does final benchmark run
5. Benchmark returns ONLY `passed: true/false` - nothing else

No "category hints", no "progress percentage", no "edge cases failing" - **just pass/fail**.

### 3. Test Refinement on Blind Failure

If blind verification fails, agent can:
1. Re-read task description
2. Reason about what it might have missed
3. Generate ADDITIONAL self-tests
4. Implement fixes based on new self-tests
5. Try blind verification again

But it NEVER gets any information about what the real tests actually check.

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Test generator misses critical edge cases | Agent passes self-tests, fails blind | Analyze gaps, improve prompt |
| Self-tests too easy | Agent passes self, fails blind | Add adversarial test generation |
| Self-tests wrong | Agent chases wrong answer | Include uncertainty markers, multiple test variants |
| Blind feedback too informative | Still gaming | Reduce feedback granularity |
| Local FM can't implement | Nothing passes | Fall back to cloud for some tasks |

---

## Next Steps

1. **Implement test generator** for regex-log first (simplest task type)
2. **Measure overlap** between generated tests and real tests
3. **Iterate on prompt** until overlap is high
4. **Add self-verifier** for regex tasks
5. **Run end-to-end** on regex-log with blind mode
6. **Expand to other task types** (compile, python, bio)
7. **Run holdout validation** to prove generalization

---

*Plan updated 2025-12-08*
