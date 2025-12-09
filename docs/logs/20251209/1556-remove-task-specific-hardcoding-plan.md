# Plan: Remove ALL Task-Specific Hardcoding from HillClimber

## The Problem

The HillClimber has **massive task-specific hardcoding** that completely undermines the thesis:

> "Architecture beats model size"

If we hardcode task-specific knowledge, we're NOT proving architecture works — we're proving we can game benchmarks. This is the **opposite** of what we're trying to demonstrate.

### Scope of Hardcoding Found

| File | Hardcoding Type | Count |
|------|-----------------|-------|
| `decomposer.ts` | 5 complete task decompositions | ~400 lines |
| `decomposer.ts` | 50+ task-specific hints | embedded |
| `test-generator-iterative.ts` | IPv4/date/regex pattern rules | ~80 lines |
| `tb2-skills.ts` | 10 domain skills + task mapping | ~350 lines |
| `meta-reasoner.ts` | TASK_CONSTRAINTS, DEFAULT_TASK_HINTS | ~50 lines |
| `monitor.ts` | solutionFiles mapping | ~10 lines |
| `map-orchestrator.ts` | Subtask-specific action guidance | ~20 lines |
| `sampling-orchestrator.ts` | Hardcoded "regex.txt" | ~5 lines |

**Total: ~900+ lines of task-specific code that must be removed or generalized.**

---

## The Goal

A **truly general-purpose decomposer** that:

1. Takes ANY task description as input
2. Dynamically generates subtasks by analyzing the description
3. Uses ONLY general process knowledge (TDD, iteration, verification)
4. Has ZERO task-specific hints, skills, or constraints hardcoded
5. Lets FM + TestGen discover solutions through iteration

---

## Implementation Plan

### Phase 1: Create General-Purpose Decomposer

**File:** `src/hillclimber/decomposer.ts`

#### 1.1 Remove ALL hardcoded decompositions

Delete entirely:
- `REGEX_LOG_DECOMPOSITION`
- `PATH_TRACING_DECOMPOSITION`
- `MODEL_EXTRACTION_DECOMPOSITION`
- `VIDEO_PROCESSING_DECOMPOSITION`
- `DNA_ASSEMBLY_DECOMPOSITION`
- `DECOMPOSITIONS` lookup table

#### 1.2 Create dynamic decomposition function

```typescript
export function decomposeTask(task: TerminalBenchTask): TaskDecomposition {
  // Parse task description to extract:
  // 1. What output files are required (from description)
  // 2. What the success criteria are (from description)
  // 3. What tools/languages might be needed (from description)

  // Generate GENERIC subtasks:
  return {
    taskId: task.id,
    subtaskCount: 4,
    subtasks: [
      {
        id: 1,
        name: "understand-requirements",
        goal: "Read the task description carefully. Identify: (1) required output files, (2) success criteria, (3) any constraints mentioned.",
        hints: [
          "Use read_file to examine any example files mentioned",
          "Note the exact output format required",
        ],
        // NO task-specific hints
      },
      {
        id: 2,
        name: "write-initial-solution",
        goal: "Write an initial solution based on your understanding of the requirements.",
        hints: [
          "Start simple - get something working first",
          "Use write_file to create the required output",
        ],
      },
      {
        id: 3,
        name: "test-and-iterate",
        goal: "Run verify_progress to see test results. Analyze failures and fix issues.",
        hints: [
          "Read the failure messages carefully",
          "Make ONE targeted change per iteration",
          "False positives: tighten constraints",
          "False negatives: loosen constraints",
        ],
      },
      {
        id: 4,
        name: "final-validation",
        goal: "Ensure all tests pass. Fix any remaining edge cases.",
        hints: [
          "Check boundary conditions",
          "Test edge cases mentioned in failures",
        ],
      },
    ],
    globalHints: [
      // ONLY general process knowledge
      "Use verify_progress after each change to get feedback",
      "Read failure messages to understand what's wrong",
      "Iterate until all tests pass",
    ],
    // Extract from task description dynamically
    filesToRead: extractFilesToRead(task.description),
    requiredOutputs: extractRequiredOutputs(task.description),
  };
}

// Helper: Extract files mentioned in description
function extractFilesToRead(description: string): string[] {
  const filePattern = /\/app\/[\w\-\.\/]+/g;
  const matches = description.match(filePattern) || [];
  return [...new Set(matches)];
}

// Helper: Extract required output files from description
function extractRequiredOutputs(description: string): string[] {
  // Look for patterns like "write to /app/X" or "output file: /app/X"
  const outputPattern = /(?:write|output|create|save).*?(\/app\/[\w\-\.]+)/gi;
  const matches = [...description.matchAll(outputPattern)];
  return matches.map(m => m[1]);
}
```

#### 1.3 Add guardrails to prevent future hardcoding

```typescript
// GUARDRAIL: This function must NEVER contain task-specific logic.
// All knowledge must come from the task description parameter.
// If you find yourself adding "if (taskId === 'regex-log')" - STOP.
// That defeats the entire purpose of proving architecture beats model size.
```

---

### Phase 2: Remove Task-Specific Skills

**File:** `src/skills/library/tb2-skills.ts`

#### 2.1 Delete ALL task-specific skills

Delete entirely:
- `REGEX_BOUNDARY_SKILL`
- `REGEX_LOOKAHEAD_SKILL`
- `REGEX_DATE_VALIDATION_SKILL`
- `PPM_FORMAT_SKILL`
- `RAY_SPHERE_INTERSECTION_SKILL`
- `NUMPY_ARRAY_MANIPULATION_SKILL`
- `MODEL_EXTRACTION_PATTERNS_SKILL`
- `VIDEO_FRAME_ANALYSIS_SKILL`
- `TOML_OUTPUT_SKILL`
- `BIOPYTHON_PATTERNS_SKILL`
- `PRIMER_DESIGN_SKILL`
- `FASTA_FORMAT_SKILL`
- `taskSkillMap`
- `getSkillsForTask()`

#### 2.2 Keep ONLY general-purpose skills (if any)

If there are truly general skills (like "how to read files" or "how to iterate"), keep those. But anything that mentions specific patterns, formats, or algorithms must go.

---

### Phase 3: Remove Task-Specific Edge Case Rules

**File:** `src/hillclimber/test-generator-iterative.ts`

#### 3.1 Refactor extractTaskEdgeCases()

Current code has hardcoded patterns for IPv4, dates, regex, logs. This must be generalized.

**Option A: Remove entirely** — Let FM generate edge cases from description alone

**Option B: Make pattern detection generic** — Detect ANY numeric ranges, ANY format patterns, without hardcoding specific ones

```typescript
export function extractTaskEdgeCases(description: string): TaskEdgeCases {
  // Generic pattern detection - NO hardcoded patterns
  return {
    // Detect numeric mentions and infer ranges
    numericPatterns: detectNumericPatterns(description),
    // Detect format mentions (dates, IPs, etc.) from description text
    formatPatterns: detectFormatPatterns(description),
    // Detect file types mentioned
    fileTypes: detectFileTypes(description),
  };
}

// These functions should use NLP/regex on the description text
// NOT hardcoded knowledge about what IPv4 or dates look like
```

#### 3.2 Remove formatEdgeCasesForCategory() hardcoded test cases

Delete the entire section that generates specific test inputs like:
- `'256.1.1.1 2024-01-15' → expectedOutput: null`
- `'192.168.1.1 2024-00-15' → expectedOutput: null`

These are TB2 test data leakage.

---

### Phase 4: Remove Task-Specific Constraints

**File:** `src/hillclimber/meta-reasoner.ts`

#### 4.1 Delete TASK_CONSTRAINTS

```typescript
// DELETE THIS ENTIRE OBJECT
const TASK_CONSTRAINTS = {
  "path-tracing": { forbidden: [...], example: "..." },
  "regex-log": { forbidden: [...], required: [...], example: "..." },
  // ...
};
```

#### 4.2 Delete DEFAULT_TASK_HINTS

```typescript
// DELETE THIS
const DEFAULT_TASK_HINTS = {
  "regex-log": "Write the regex directly to /app/regex.txt..."
};
```

#### 4.3 Refactor validateHint() to be generic

Remove any task-specific validation. Hints should be validated against general principles only.

---

### Phase 5: Remove Task-Specific Mappings

**File:** `src/hillclimber/monitor.ts`

#### 5.1 Remove solutionFiles mapping

```typescript
// DELETE THIS
const solutionFiles: Record<string, string[]> = {
  "regex-log": ["regex.txt"],
  "path-tracing": ["image.c"],
  // ...
};
```

#### 5.2 Extract solution files from task description dynamically

```typescript
function getSolutionFiles(task: TerminalBenchTask): string[] {
  // Parse task.description to find output files
  return extractRequiredOutputs(task.description);
}
```

---

### Phase 6: Remove Hardcoded Action Guidance

**File:** `src/hillclimber/map-orchestrator.ts`

#### 6.1 Remove subtask-specific action guidance

Delete:
```typescript
if (context.currentSubtask.name === "write-initial-regex" ||
    context.currentSubtask.name === "write-ipv4-aware-regex") {
  // DELETE ALL OF THIS
}
```

#### 6.2 Make action guidance generic

```typescript
// Generic action guidance based on subtask phase, not task type
if (context.currentSubtask.name === "write-initial-solution") {
  lines.push("Use write_file to create your solution");
}
if (context.currentSubtask.name === "test-and-iterate") {
  lines.push("Use verify_progress to run tests");
}
```

---

### Phase 7: Remove Hardcoded Filenames

**File:** `src/hillclimber/sampling-orchestrator.ts`

#### 7.1 Remove default "regex.txt"

```typescript
// Change from:
solutionFilename?: string; // default: "regex.txt"

// To:
solutionFilename: string; // REQUIRED - no default
```

#### 7.2 Pass filename from task description

The caller must extract the output filename from the task description and pass it explicitly.

---

### Phase 8: Add Guardrails

#### 8.1 Add lint rules or comments

Every file that was cleaned up should have a guardrail comment:

```typescript
// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from:
// 1. The task description (passed as parameter)
// 2. General process knowledge (TDD, iteration)
//
// If you're tempted to add task-specific code, you're defeating the thesis:
// "Architecture beats model size"
// ============================================================================
```

#### 8.2 Add test to detect hardcoding

```typescript
// test/no-hardcoding.test.ts
test("no task-specific hardcoding in hillclimber", () => {
  const files = [
    "src/hillclimber/decomposer.ts",
    "src/hillclimber/map-orchestrator.ts",
    "src/hillclimber/test-generator-iterative.ts",
    // ...
  ];

  const forbidden = [
    "regex-log",
    "path-tracing",
    "model-extraction",
    "video-processing",
    "dna-assembly",
    "IPv4",
    "YYYY-MM-DD",
    "/app/regex.txt",
  ];

  for (const file of files) {
    const content = readFileSync(file, "utf-8");
    for (const term of forbidden) {
      expect(content).not.toContain(term);
    }
  }
});
```

---

## Files to Modify

| File | Action |
|------|--------|
| `src/hillclimber/decomposer.ts` | Delete 5 decompositions, create generic function |
| `src/skills/library/tb2-skills.ts` | Delete all 10 skills and task mapping |
| `src/hillclimber/test-generator-iterative.ts` | Remove hardcoded patterns, add FM-powered extraction |
| `src/hillclimber/meta-reasoner.ts` | Delete TASK_CONSTRAINTS, DEFAULT_TASK_HINTS |
| `src/hillclimber/monitor.ts` | Delete solutionFiles mapping |
| `src/hillclimber/map-orchestrator.ts` | Remove subtask guidance, add guided gen for tool calls |
| `src/hillclimber/sampling-orchestrator.ts` | Remove default filename |
| `swift/.../GuidedTypes.swift` | Add EdgeCaseExtraction + ToolCallRequest schemas |
| `swift/.../ChatHandler.swift` | Add handler cases for new schemas |
| `test/no-hardcoding.test.ts` | NEW: Add guardrail test |

---

## What Stays

1. **General process knowledge:**
   - "Use verify_progress to get feedback"
   - "Iterate based on test failures"
   - "Make one change at a time"

2. **Generic subtask structure:**
   - understand-requirements
   - write-initial-solution
   - test-and-iterate
   - final-validation

3. **Dynamic extraction from task description:**
   - Files to read (parsed from description)
   - Output files required (parsed from description)

---

### Phase 9: FM-Powered Edge Case Extraction

**File:** `src/hillclimber/test-generator-iterative.ts`

Replace hardcoded pattern detection with FM-powered analysis:

```typescript
export async function extractTaskEdgeCases(
  description: string,
  fm: FMService
): Promise<TaskEdgeCases> {
  // Use FM to analyze the task description dynamically
  const prompt = `Analyze this task description and identify:
1. What data formats are mentioned (dates, IPs, numbers, etc.)
2. What are the valid ranges for each format
3. What are boundary conditions to test
4. What are likely edge cases

Task description:
${description}

Return as JSON with structure:
{
  "formats": [{ "name": "...", "validRange": "...", "examples": [...] }],
  "boundaries": ["..."],
  "edgeCases": ["..."]
}`;

  const response = await fm.chat({
    messages: [{ role: "user", content: prompt }],
    responseFormat: {
      type: "json_schema",
      schema_type: "edge_case_extraction", // New schema
    },
  });

  return JSON.parse(response.choices[0].message.content);
}
```

#### 9.1 Add Swift schema for edge case extraction

**File:** `swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift`

```swift
@Generable(description: "Format pattern found in task description")
struct FormatPattern: Codable {
    var name: String
    var validRange: String
    var examples: [String]
}

@Generable(description: "Edge cases extracted from task description")
struct EdgeCaseExtraction: Codable {
    var formats: [FormatPattern]
    var boundaries: [String]
    var edgeCases: [String]
}
```

---

### Phase 10: Add Guided Generation for Tool Calls

**Purpose:** Prevent FM from hallucinating non-existent tools like `edit_file`.

#### 10.1 Add Swift schema for tool calls

**File:** `swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift`

```swift
@Generable(description: "Arguments for tool calls")
struct ToolArguments: Codable {
    var path: String?
    var content: String?
}

@Generable(description: "A tool call from the agent")
struct ToolCallRequest: Codable {
    @Guide(description: "Tool to call", .anyOf([
        "read_file",
        "write_file",
        "verify_progress"
    ]))
    var name: String

    var arguments: ToolArguments

    var reasoning: String?
}
```

#### 10.2 Add handler case

**File:** `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift`

```swift
case "tool_call":
    let response = try await session.respond(
        to: prompt,
        generating: ToolCallRequest.self
    )
    return encodeToJSON(response.content)
```

#### 10.3 Update MAP orchestrator to use guided generation

**File:** `src/hillclimber/map-orchestrator.ts`

```typescript
const chatResponse = yield* fm.chat({
  messages: [{ role: "user", content: prompt }],
  temperature,
  maxTokens: 512,
  responseFormat: {
    type: "json_schema",
    schema_type: "tool_call",  // Constrained to valid tools only
  },
});

// FM can ONLY output read_file, write_file, or verify_progress
const toolCall = JSON.parse(chatResponse.choices[0].message.content);
```

#### 10.4 Rebuild Swift bridge

```bash
cd swift/foundation-bridge
swift build
cp .build/debug/foundation-bridge ../../bin/
```

---

## Expected Outcome

After this refactor:

1. **ANY task** can be processed by the decomposer
2. **ZERO task-specific knowledge** is hardcoded
3. **FM must discover** solutions through iteration
4. **TestGen must generate** tests from description alone
5. **The thesis is testable:** Does architecture beat model size?

If we hit 100% on regex-log after this refactor, we've proven something real. If we don't, we know the architecture needs improvement — not more hardcoding.

---

## Risk Assessment

**Risk:** Performance may drop initially without task-specific hints.

**Mitigation:** This is EXPECTED and CORRECT. The previous results were invalid because they relied on hardcoded knowledge. We need to see true performance.

**Risk:** TestGen may generate worse tests without pattern knowledge.

**Mitigation:** Improve TestGen's ability to parse task descriptions dynamically. This is legitimate improvement to the architecture, not hardcoding.
