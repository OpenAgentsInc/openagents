# HillClimber v3: Environment-Aware Blind Verification

## Problem Statement

The current test generation only uses task descriptions, missing critical context from the execution environment. Our analysis shows ~40% alignment with actual TB2 tests because we lack:

1. **Anti-cheat tests** - what tools should NOT be present (e.g., R/RStan for conversion tasks)
2. **Parameter discovery** - what the environment files contain (R scripts, data files)
3. **System boundaries** - what packages/languages are available
4. **File structure** - what files exist and their patterns

**The goal**: Use ALL available environment information to generate comprehensive tests that catch both correctness AND gaming behavior.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    ENVIRONMENT-AWARE BLIND ORCHESTRATOR                   │
│  1. Introspect container environment (packages, files, languages)        │
│  2. Read task description + environment context                          │
│  3. Generate self-tests from BOTH (BLIND to real tests)                 │
│  4. Implement + iterate against self-tests                               │
│  5. Final blind verification (pass/fail only)                           │
└─────────────────────────────────────────────────────────────────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   ENVIRONMENT    │  │    TEST      │  │    SELF      │  │    BLIND     │
│   INTROSPECTOR   │  │  GENERATOR   │  │  VERIFIER    │  │  VERIFIER    │
│                  │  │              │  │              │  │              │
│ Docker/Container │  │ Env + Desc → │  │ Run agent's  │  │ Run REAL     │
│ → EnvironmentInfo│  │ Generate     │  │ own tests    │  │ benchmark    │
│ (structured)     │  │ test cases   │  │ Full detail  │  │ Pass/fail    │
└──────────────────┘  └──────────────┘  └──────────────┘  └──────────────┘
```

---

## Phase 0: Environment-Aware Test Generation (NEW - PRIORITY)

### Core Insight

The container environment tells us things the task description doesn't:

| Information Source | What We Learn | Test Implications |
|--------------------|---------------|-------------------|
| **Installed packages** | `pip list`, `apt list`, `which <tool>` | Anti-cheat tests: verify prohibited tools NOT installed |
| **File listings** | `ls /app/`, `find . -name "*.py"` | File existence tests, output path validation |
| **File contents preview** | First 50 lines of scripts/configs | Parameter names, structure hints, format expectations |
| **Language versions** | `python --version`, `node --version` | API compatibility tests |
| **System resources** | Memory limits, CPU count | Performance boundary tests |
| **Environment variables** | `env`, preset vars | Configuration validation tests |

### EnvironmentInfo Type (TypeScript)

```typescript
// src/hillclimber/environment-info.ts

export interface EnvironmentInfo {
  // System identification
  platform: "docker" | "container" | "local";
  containerImage?: string;

  // Language environments
  languages: {
    python?: { version: string; packages: string[] };  // pip list --format=freeze
    node?: { version: string; packages: string[] };    // npm list --depth=0
    ruby?: { version: string; gems: string[] };
    rust?: { version: string };
    go?: { version: string };
    r?: { version: string; packages: string[] };       // R -e "installed.packages()"
  };

  // System tools
  tools: {
    available: string[];    // which git, docker, curl, etc.
    prohibited?: string[];  // Tools that SHOULD NOT exist (inferred from task)
  };

  // File system
  files: {
    workdir: string;
    listing: FileEntry[];          // ls -la output
    taskFiles: FilePreview[];      // First 50 lines of relevant files
  };

  // Resources
  resources: {
    memoryLimitMB?: number;
    cpuCount?: number;
    diskSpaceMB?: number;
  };

  // Environment variables (filtered for safety)
  env: Record<string, string>;
}

export interface FileEntry {
  name: string;
  type: "file" | "directory" | "symlink";
  size: number;
  permissions: string;
}

export interface FilePreview {
  path: string;
  extension: string;
  lineCount: number;
  preview: string;  // First 50 lines or 2KB, whichever is smaller
  detectedType?: "python" | "r_script" | "stan_model" | "json" | "csv" | "config";
}
```

### Environment Introspector

**Runs INSIDE the container** during task setup phase to get accurate real-time information.

```typescript
// src/hillclimber/environment-introspector.ts

export const introspectEnvironment = (
  container: ContainerHandle,
  workspace: string,
  taskDescription: string
): Effect.Effect<EnvironmentInfo, IntrospectionError> =>
  Effect.gen(function* () {
    // 1. Detect platform
    const platform = yield* detectPlatform(container);

    // 2. Discover languages and packages
    const languages = yield* discoverLanguages(container);

    // 3. List available/prohibited tools
    const tools = yield* discoverTools(container, taskDescription);

    // 4. Scan file system
    const files = yield* scanFiles(container, workspace, taskDescription);

    // 5. Detect resources
    const resources = yield* detectResources(container);

    // 6. Capture relevant env vars
    const env = yield* captureEnvVars(container);

    return { platform, languages, tools, files, resources, env };
  });
```

### Introspection Commands (Run Inside Container)

| Category | Command | Output Parsing |
|----------|---------|----------------|
| **Platform** | `cat /etc/os-release` | Extract ID, VERSION_ID |
| **Python** | `python3 --version && pip list --format=freeze` | Version + package list |
| **Node** | `node --version && npm list --depth=0 --json 2>/dev/null` | Version + deps |
| **R** | `R --version && Rscript -e "installed.packages()[,1]"` | Version + packages |
| **Rust** | `rustc --version` | Version string |
| **Go** | `go version` | Version string |
| **Tools** | `which git docker curl wget jq make gcc` | Available paths |
| **Prohibited** | `which R Rscript rstan` (for R→Python) | Should return empty |
| **Files** | `find /app -maxdepth 2 -type f \| head -50` | File listing |
| **File preview** | `head -50 /app/*.{py,R,stan,json,csv}` | Content previews |
| **Memory** | `cat /sys/fs/cgroup/memory/memory.limit_in_bytes` | Memory limit |
| **CPU** | `nproc` | CPU count |
| **Env** | `env \| grep -v PASSWORD \| grep -v SECRET` | Safe env vars |

### Introspection Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                       TASK EXECUTION FLOW                            │
│                                                                      │
│  1. Start container with task image                                 │
│  2. Run INTROSPECTION SCRIPT (collects EnvironmentInfo)             │
│  3. Pass EnvironmentInfo + TaskDescription to TEST GENERATOR        │
│  4. Generate tests with guided generation                           │
│  5. Run task implementation with self-tests                         │
│  6. Final blind verification                                        │
└─────────────────────────────────────────────────────────────────────┘
```

### Introspection Script (Shell)

```bash
#!/bin/bash
# introspect.sh - Run inside container, output JSON to stdout

cat <<EOF
{
  "platform": "$(cat /etc/os-release 2>/dev/null | grep ^ID= | cut -d= -f2 | tr -d '"')",
  "languages": {
    "python": $(python3 --version 2>/dev/null && pip list --format=json 2>/dev/null || echo 'null'),
    "r": $(R --version 2>/dev/null | head -1 || echo 'null'),
    "node": $(node --version 2>/dev/null || echo 'null')
  },
  "tools": {
    "available": [$(for t in git curl wget make gcc python3 pip R node npm go rustc; do
      which $t >/dev/null 2>&1 && echo "\"$t\","
    done | sed 's/,$//')],
    "prohibited_check": {
      "R": $(which R >/dev/null 2>&1 && echo 'true' || echo 'false'),
      "Rscript": $(which Rscript >/dev/null 2>&1 && echo 'true' || echo 'false')
    }
  },
  "files": {
    "listing": $(ls -la /app 2>/dev/null | tail -n +4 | head -20 | jq -Rs 'split("\n") | map(select(length > 0))'),
    "previews": {}
  },
  "resources": {
    "memory_mb": $(cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null | awk '{print int($1/1024/1024)}' || echo 'null'),
    "cpu_count": $(nproc 2>/dev/null || echo 'null')
  }
}
EOF
```

### Prohibited Tools Inference

```typescript
// Helper: infer prohibited tools from task description
const inferProhibitedTools = (description: string): string[] => {
  const prohibited: string[] = [];

  // "Convert R to Python" → R should NOT be installed
  if (description.match(/convert.*r.*to.*python/i)) {
    prohibited.push("R", "Rscript", "rstan");
  }

  // "Convert Python to Rust" → Python solver shouldn't exist
  if (description.match(/convert.*python.*to.*rust/i)) {
    // Check if task mentions specific libraries to avoid
  }

  // "Implement from scratch" → no external solver
  if (description.match(/from scratch|without using|do not use/i)) {
    // Parse what should be avoided from context
  }

  // "Manual implementation" patterns
  if (description.match(/implement.*yourself|write your own/i)) {
    // Extract the library/tool to avoid
  }

  return prohibited;
};
```

### Swift Generable Type for Environment-Aware Generation

```swift
// swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift

// NEW: Environment context passed to test generation
@Generable(description: "Environment information for test generation")
struct EnvironmentContext: Codable {
    @Guide(description: "Execution platform")
    var platform: String

    @Guide(description: "Available programming languages with versions")
    var languages: [LanguageInfo]

    @Guide(description: "Available system tools")
    var availableTools: [String]

    @Guide(description: "Tools that should NOT be present (anti-cheat)")
    var prohibitedTools: [String]

    @Guide(description: "Files in workspace")
    var files: [FileInfo]

    @Guide(description: "Preview of key files (first 50 lines)")
    var filePreviews: [FilePreview]
}

@Generable(description: "Language runtime information")
struct LanguageInfo: Codable {
    var name: String      // "python", "r", "node"
    var version: String
    var packages: [String]  // installed packages
}

@Generable(description: "File information")
struct FileInfo: Codable {
    var path: String
    var type: String      // "file", "directory"
    var size: Int
}

@Generable(description: "File content preview")
struct FilePreview: Codable {
    var path: String
    var extension: String
    var preview: String    // First 50 lines
    var detectedType: String?
}

// NEW: Environment-aware test generation result
@Generable(description: "Environment-aware test suite")
struct EnvironmentAwareTestResult: Codable {
    @Guide(description: "Requirements from task description")
    var descriptionRequirements: [String]

    @Guide(description: "Requirements inferred from environment")
    var environmentRequirements: [String]

    @Guide(description: "Anti-cheat tests (prohibited tools/patterns)")
    var antiCheatTests: [GeneratedTest]

    @Guide(description: "File existence and structure tests")
    var existenceTests: [GeneratedTest]

    @Guide(description: "Correctness tests from description")
    var correctnessTests: [GeneratedTest]

    @Guide(description: "Boundary tests from environment")
    var boundaryTests: [GeneratedTest]

    @Guide(description: "Integration tests")
    var integrationTests: [GeneratedTest]

    @Guide(description: "Uncertainties and assumptions")
    var uncertainties: [String]
}
```

### Updated Test Generation Prompt

```markdown
You are a QA engineer designing tests for a programming task. You have access to:
1. The task description (what the agent is supposed to do)
2. The execution environment (what's actually available in the container)

## Task Description
{description}

## Environment Context
Platform: {environment.platform}
Languages: {environment.languages}
Available tools: {environment.tools.available}
Prohibited tools (should NOT exist): {environment.tools.prohibited}

### Files in Workspace
{for file in environment.files.listing}
- {file.name} ({file.type}, {file.size} bytes)
{/for}

### File Previews (key files)
{for preview in environment.files.taskFiles}
=== {preview.path} ({preview.detectedType}) ===
{preview.preview}
{/for}

## Your Mission

Generate comprehensive tests in these categories:

### 1. Anti-Cheat Tests (CRITICAL)
Based on the task description, what tools/approaches should be PROHIBITED?
- If this is a conversion task (e.g., R to Python), test that the original tool is NOT present
- If this is an "implement from scratch" task, test that no pre-built solutions exist
- Think: "What would a lazy implementation do that we should catch?"

### 2. Environment Boundary Tests
Based on what's actually available:
- Test that required output files are created in the correct paths
- Test that outputs match the language/format expected (Python vs R, JSON vs CSV)
- Test version compatibility if relevant

### 3. Parameter/Structure Tests
Based on file previews:
- If you see a script with variables (alpha, sigma, rho, beta), test ALL of them
- If you see a CSV with columns, test column existence and types
- If you see a config with ranges, test boundary values

### 4. Correctness Tests
Based on task requirements:
- Happy path tests
- Edge cases mentioned or implied in description
- Error handling if specified

Generate tests with HIGH SPECIFICITY based on what you actually see in the environment.
```

### Integration with FM Bridge

```swift
// ChatHandler.swift - new schema type

case "environment_aware_test_generation":
    let response = try await session.respond(
        to: prompt,
        generating: EnvironmentAwareTestResult.self
    )
    return encodeToJSON(response.content)
```

### Updated Test Generator API

```typescript
// src/hillclimber/test-generator.ts

export async function generateTestsFromEnvironment(
  taskDescription: string,
  taskId: string,
  environment: EnvironmentInfo,
  options: TestGeneratorOptions = {},
): Promise<EnvironmentAwareTestResult> {
  const prompt = buildEnvironmentAwarePrompt(taskDescription, environment, options);

  const response = yield* fm.chat({
    messages: [{ role: "user", content: prompt }],
    temperature: options.temperature ?? 0.3,
    maxTokens: 8192,  // More tokens for richer output
    responseFormat: {
      type: "json_schema",
      schema_type: "environment_aware_test_generation",
    },
  });

  return JSON.parse(response.choices[0].message.content);
}
```

### Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `src/hillclimber/environment-info.ts` | CREATE | EnvironmentInfo type definitions |
| `src/hillclimber/environment-introspector.ts` | CREATE | Collect environment data from container |
| `swift/.../GuidedTypes.swift` | MODIFY | Add environment-related Generable types |
| `swift/.../ChatHandler.swift` | MODIFY | Add `environment_aware_test_generation` case |
| `src/hillclimber/test-generator.ts` | MODIFY | Add `generateTestsFromEnvironment()` |
| `src/hillclimber/test-gen-cli.ts` | MODIFY | Add `--env` flag to include environment |
| `src/hillclimber/test-gen-compare.ts` | MODIFY | Compare env-aware vs basic generation |

### Implementation Order

1. **Define TypeScript types** (`environment-info.ts`)
2. **Build introspector** (`environment-introspector.ts`)
3. **Add Swift Generable types** (`GuidedTypes.swift`)
4. **Add handler case** (`ChatHandler.swift`)
5. **Rebuild bridge** (`swift build`)
6. **Update test generator** (`test-generator.ts`)
7. **Test on regex-log** with environment context
8. **Compare alignment** with actual TB2 tests

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

## Next Steps (Updated Priority)

### Phase 0: Environment-Aware Test Generation (CURRENT PRIORITY)

1. **Create `environment-info.ts`** - TypeScript types for EnvironmentInfo
2. **Create `environment-introspector.ts`** - Collect data from inside container
3. **Add Swift Generable types** - EnvironmentContext, EnvironmentAwareTestResult
4. **Add handler case** - `environment_aware_test_generation` in ChatHandler.swift
5. **Rebuild bridge** - `swift build` and copy binary
6. **Update test-generator.ts** - Add `generateTestsFromEnvironment()`
7. **Test on rstan-to-pystan** - Should now generate anti-cheat test
8. **Compare alignment** - Measure improvement from ~40% baseline

### Then Continue Original Plan

9. **Add self-verifier** for regex/python tasks
10. **Run end-to-end** on regex-log with blind mode
11. **Expand to other task types** (compile, bio)
12. **Run holdout validation** to prove generalization

---

## Files to Create/Modify (Summary)

| File | Action | Purpose |
|------|--------|---------|
| `src/hillclimber/environment-info.ts` | **CREATE** | TypeScript types |
| `src/hillclimber/environment-introspector.ts` | **CREATE** | Container introspection |
| `swift/.../GuidedTypes.swift` | **MODIFY** | Add Generable types |
| `swift/.../ChatHandler.swift` | **MODIFY** | Add handler case |
| `src/hillclimber/test-generator.ts` | **MODIFY** | Add env-aware generation |
| `src/hillclimber/test-gen-cli.ts` | **MODIFY** | Add `--env` flag |
| `src/hillclimber/test-gen-compare.ts` | **MODIFY** | Compare env-aware vs basic |

---

*Plan updated 2025-12-08 - Added environment-aware test generation*
