# Plan: Get HillClimber Solving ONE TBv2 Task

## Executive Summary

**Goal**: Demonstrate that HillClimber can solve at least ONE Terminal-Bench v2 task using the on-device FM (Foundation Models).

**Selected Task**: `regex-log` (medium difficulty)
- Already has detailed decomposition with 4 subtasks
- Used extensively in testgen evolution experiments
- Well-defined verification (9 pytest cases)
- Deterministic success criteria

**Core Insight**: The gap between "testgen produces tests" and "agent solves task" is a **feedback quality gap**, not a capability gap.

---

## Current Architecture Analysis

### What We Have
```
TestGen Evolution Loop (WORKING)
├─ Generates 21 tests per session
├─ Categories: anti_cheat, existence, correctness, boundary, integration
├─ Comprehensiveness score: 8.0/10
├─ Stored in testgen_trajectories table
└─ Evolves configs over time

HillClimber Execution Loop (WORKING)
├─ MAP orchestrator with subtask decomposition
├─ FM agent with micro-task supervisor
├─ Runs verification after file modifications
├─ Records pass/fail in hillclimber_runs
└─ Meta-reasons about config changes
```

### The Critical Gap

```
CURRENT FLOW (BLIND):
Agent modifies /app/regex.txt
    → Evaluator runs: pytest tests/ -v
    → Returns: "3/9 tests passing"
    → Generic suggestion: "Check regex for valid IPv4..."
    → Agent has NO idea which 6 tests failed or why

NEEDED FLOW (INFORMED):
Agent modifies /app/regex.txt
    → Evaluator runs: pytest tests/ -v
    → Parses SPECIFIC failures:
        "test_no_valid_ipv4 FAILED: matched date on line without valid IP"
        "test_last_date FAILED: matched first date, not last"
    → Returns structured feedback with failure reasons
    → Agent can target specific issues
```

---

## Why `regex-log` Is Best Positioned

1. **Decomposition exists** (`decomposer.ts:54-122`):
   - 4 subtasks with checkpoints
   - Detailed hints per subtask
   - Global hints about regex behavior

2. **Task is tractable**:
   - Single output file: `/app/regex.txt`
   - Clear success criteria: 9/9 tests pass
   - Pure algorithmic problem (no I/O, no external deps)

3. **We understand the failure modes**:
   - IPv4 validation (0-255 per octet)
   - Date validation (valid month/day)
   - "Last date" requirement (greedy matching)
   - Boundary assertions (no partial matches)

4. **Quick iteration cycle**:
   - Verification runs in <1 second
   - Changes are single-line edits
   - Easy to debug

---

## Gap Analysis: Forward from TestGen, Backward from Success

### Forward Trace (TestGen → Agent)

```
TestGen produces for regex-log:
├─ anti_cheat: "Verify R/Rscript not used" (not relevant)
├─ existence: "regex.txt exists"
├─ correctness: "Matches date on line with valid IP"
├─ boundary: "Rejects invalid dates (month 13, day 32)"
├─ integration: "Returns last date, not first"

MISSING LINK: These tests are stored in DB but NOT:
- Injected into /app/tests/
- Run during verification
- Used to generate specific feedback
```

### Backward Trace (Success → Requirements)

```
A SUCCESSFUL TRAJECTORY for regex-log:

Turn 1: Read task, understand requirements
  → FM understands: need regex matching dates on lines with valid IPv4s

Turn 2: Write initial regex
  → Creates /app/regex.txt with first attempt

Turn 3: verify_progress
  → Gets: "3/9 tests passing"
  → NEEDS: "Failures: test_boundary_ip (0.0.0.256 matched as valid)"

Turn 4-8: Iterate based on SPECIFIC feedback
  → Fixes IPv4 validation: "now 5/9"
  → Fixes date validation: "now 7/9"
  → Fixes last-date logic: "now 9/9"

Turn 9: task_complete → PASS
```

**What's Missing in Current System**:
1. Failure details from pytest output (which test, what assertion)
2. Translation from pytest failure to actionable hint
3. Context compression so FM can see enough history

---

## Implementation Plan

### DISCOVERY: Evaluator Already Has Infrastructure

Reading `src/hillclimber/evaluator.ts` reveals the infrastructure is mostly there:

**Existing capabilities** (lines 104-176, 233-282, 394-421):
- `FailureDetail` type with `testName`, `expected`, `actual`, `message`
- `parsePytestOutput()` extracts individual test failures
- `generateSuggestion()` has task-specific logic for regex-log
- `formatForPrompt()` formats failures for FM context

**What's missing**: Verification that this works end-to-end:
1. Are the regex patterns matching actual TB2 pytest output?
2. Is `formatForPrompt()` output actually reaching the FM?
3. Does JSON.parse in generateSuggestion handle malformed output?

### Phase 1: Debug & Verify Existing Parsing

**Files to test**: `src/hillclimber/evaluator.ts`

**Test case**: Run regex-log verification manually and verify parsing
```bash
# Get actual pytest output from TB2 tests
# Compare against parsePytestOutput patterns
# Verify FailureDetail extraction
```

**Potential issues to fix**:
1. Regex pattern `FAILED\s+(\S+)::(\w+)\s*[-–]` may not match actual TB2 output format
2. `JSON.parse(firstFailure.expected)` in generateSuggestion (line 246) will throw if not valid JSON
3. AssertionError pattern may not match Python's actual assertion format

**Implementation**:
1. Add unit test with real pytest output from regex-log
2. Fix regex patterns if they don't match
3. Add try/catch around JSON.parse in generateSuggestion
4. Verify failures array is populated correctly

### Phase 2: CRITICAL DISCOVERY - FM Integration is Placeholder

**Files examined**: `src/hillclimber/map-orchestrator.ts`

**Finding**: The feedback pipeline IS correctly built (lines 148-171, 192-197):
- `buildFMContext()` calls `formatForPrompt(state.lastEvaluation)` ✅
- `formatFMPrompt()` includes verification feedback in "## Verification Status" ✅

**THE ACTUAL GAP** (lines 663-722):
```typescript
// getNextAction() is a MOCK that returns hardcoded actions:
async function getNextAction(...): Promise<FMAction | null> {
  // This is where we'll integrate with the FM worker
  // For now, return a simple action based on subtask
  switch (subtask.name) {
    case "write-initial-regex":
      return { toolName: "write_file", toolArgs: { content: "\\d{4}-\\d{2}-\\d{2}" } };
    // ... hardcoded actions, doesn't actually call FM
  }
}
```

**What's broken**: The FM is never called. The system loops through hardcoded mock actions, ignoring all the carefully constructed feedback.

**What needs to happen**:
1. Replace `getNextAction()` mock with real FM call
2. Pass the formatted prompt (`formatFMPrompt(context)`) to FM
3. Parse FM response into `FMAction` structure
4. Handle FM tool calls properly

### Phase 3: FM Context Optimization

**Files to modify**: `src/fm/micro-task.ts`, `src/hillclimber/map-orchestrator.ts`

**Problem**: FM has ~3000 token limit, current prompts are bloated

**Solution**:
1. **Prioritize failure feedback** over general hints
2. **Compress history** to "Turn N: wrote regex, 3/9 → 5/9"
3. **Include ONLY relevant subtask hints**, not all global hints
4. **Remove redundant task description** after first turn

**Context budget allocation**:
```
Total: ~3000 tokens
├─ System prompt: 200 tokens
├─ Current subtask goal + checkpoint: 100 tokens
├─ Last 2 verification results: 200 tokens
├─ Failure analysis (specific): 150 tokens
├─ Compressed history: 100 tokens
└─ Available for response: 2250 tokens
```

### Phase 4: End-to-End Integration Test

**Files to create**: `src/hillclimber/e2e-regex-log.test.ts`

**Test case**:
```typescript
test("regex-log solves in < 15 turns", async () => {
  const result = await runTaskWithMAP(regexLogTask, config, workspace, 120, 15);
  expect(result.passed).toBe(true);
  expect(result.turns).toBeLessThan(15);
});
```

**Validation points**:
1. FM receives specific failure feedback
2. FM iterates toward solution (progress increases)
3. All 9 tests pass within turn budget

---

## Files to Modify (REVISED)

### Critical Path (Must Do)

| File | Change | Effort |
|------|--------|--------|
| `src/hillclimber/map-orchestrator.ts` | Replace `getNextAction()` placeholder with real FM call | HIGH |
| `src/hillclimber/evaluator.ts` | Add try/catch around JSON.parse in `generateSuggestion`, verify regex patterns | LOW |

### Integration (Must Do)

| File | Change | Effort |
|------|--------|--------|
| `src/fm/worker.ts` or `src/fm/service.ts` | Expose simple chat endpoint for MAP orchestrator | MEDIUM |
| `src/hillclimber/map-orchestrator.ts` | Build FM prompt from `formatFMPrompt(context)`, parse response | MEDIUM |

### Validation (Must Do)

| File | Change | Effort |
|------|--------|--------|
| `src/hillclimber/e2e-regex-log.test.ts` | NEW: Integration test proving regex-log solves | MEDIUM |

### Optional Improvements

| File | Change | Effort |
|------|--------|--------|
| `src/hillclimber/evaluator.ts` | Richer failure analysis for regex-log | LOW |
| `src/hillclimber/decomposer.ts` | Add more specific hints based on common failures | LOW |

---

## Success Criteria

1. **Primary**: `regex-log` task passes (9/9 tests) in < 15 turns
2. **Secondary**: FM receives specific failure feedback (not generic)
3. **Tertiary**: Failure analysis improves turn efficiency vs baseline

---

## Execution Order (REVISED)

### Step 1: Fix Low-Hanging Bugs in Evaluator (~15 min)
1. Add try/catch around JSON.parse in `generateSuggestion()` (line 246)
2. Test `parsePytestOutput()` against real TB2 output
3. Verify regex patterns match actual pytest format

### Step 2: Connect FM to MAP Orchestrator (~60 min)
1. Study existing FM integration in `src/bench/model-adapter.ts` (createFMRunner)
2. Extract or expose a simple chat function from `src/fm/service.ts` or `src/fm/worker.ts`
3. Replace `getNextAction()` placeholder (map-orchestrator.ts:663-722) with:
   - Build prompt from `formatFMPrompt(context)`
   - Call FM with formatted prompt
   - Parse FM response into `FMAction` (toolName, toolArgs)
4. Handle tool response format (FM returns tool calls, not raw text)

### Step 3: Wire Up Integration Test (~30 min)
1. Create `src/hillclimber/e2e-regex-log.test.ts`
2. Test full flow: task → decompose → FM loop → verify → pass
3. Assert: passes in < 15 turns

### Step 4: Run & Debug (~60 min)
1. Run test, observe FM behavior
2. Debug feedback loop issues
3. Tune prompts/hints as needed
4. Iterate until regex-log passes consistently

### Step 5: Documentation (~15 min)
1. Log results to `docs/logs/YYYYMMDD/`
2. Record: turns to solve, key iterations, failure patterns

---

## Why This Will Work

**The agent has the capability** - FM can write regexes, the decomposition provides good hints, the task is tractable.

**The gap is feedback quality** - Currently: "6/9 failed". Needed: "test_boundary_ip failed because IP 256.0.0.1 matched".

**Specific feedback enables iteration** - When the FM knows WHICH test failed and WHY, it can make targeted fixes instead of blind rewrites.

**This is the minimum viable improvement** - We're not changing the model, the task, or the architecture. We're just making the feedback loop informative.

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Pytest output format varies | Use regex patterns that handle multiple formats |
| FM still fails with good feedback | Fallback: add more hints to decomposer |
| Context budget too tight | Aggressive compression, drop old history |
| FM gets stuck in loops | Monitor uses repetition detection in `monitor.ts` |

---

## Appendix: Example Pytest Output Parsing

**Input** (pytest verbose):
```
tests/test_regex.py::test_boundary_ip FAILED
E   AssertionError: assert ['2024-01-15'] == []
E     Left contains one more item: '2024-01-15'
E     Full diff:
E     - []
E     + ['2024-01-15']
```

**Parsed Output**:
```typescript
{
  testName: "test_boundary_ip",
  expected: "no matches (empty list)",
  actual: "matched '2024-01-15'",
  reason: "Regex matched a date on a line with invalid IP"
}
```

**Generated Hint**:
```
"The regex matched a date on a line where the IP was invalid. Check your IP validation - are you allowing octets > 255?"
```
