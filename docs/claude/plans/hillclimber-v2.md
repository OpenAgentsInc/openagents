# HillClimber v2: 10x Architecture Redesign

## Executive Summary

After 285 runs (0% pass rate) and deep analysis of research papers on test-time compute and task decomposition, the problem is **architectural, not parametric**. Changing hints, models, or configs won't help because the fundamental execution loop is broken.

**The core insight**: All research showing "small models beat large models" requires **verification feedback during execution**, not just at the end. Our current architecture is single-shot: FM writes solution → verification runs → fail. No iteration.

---

## Root Cause Analysis

### Why HillClimber Fails (0% pass rate)

| Issue | Current State | Research Says |
|-------|--------------|---------------|
| **Execution Model** | Single-shot: try once, verify at end | Iterate with verification feedback until correct |
| **Domain Knowledge** | Skills DISABLED for TB2 | Inject domain knowledge (regex patterns, etc.) |
| **Verification Loop** | Only runs on `task_complete` | Run after every write, show FM what failed |
| **Error Feedback** | "Verification failed" (no details) | "Test case 3 failed: expected X, got Y" |
| **Search Strategy** | One path, no backtracking | Best-of-N with candidate selection |
| **Context Window** | 3 previous actions visible | Need full trajectory awareness |

### What Would Make regex-log Pass (Reverse Engineering)

```
CURRENT (Fails):
1. FM reads task description
2. FM writes regex.txt (first attempt)
3. FM calls task_complete
4. Verification runs → 9/26 tests fail
5. Result: FAILED, score 89

NEEDED (Would Pass):
1. FM reads task description
2. FM reads test file to understand edge cases  <- MISSING
3. FM writes initial regex attempt
4. RUN TESTS, see which fail                    <- MISSING
5. FM analyzes failure: "Line 7 matched but shouldn't - IP is invalid 256.x"
6. FM refines regex based on specific failure
7. REPEAT 4-6 until tests pass                  <- MISSING
8. FM calls task_complete
9. Verification passes
```

The regex solution is complex (169 chars) because it must handle:
- Valid IPv4 only (0-255 per octet, no alphanumeric around it)
- Valid dates only (month 1-12, correct days per month, no alphanumeric)
- Match LAST date when multiple exist on a line
- Require IPv4 present on the line

No model will get this right on first try without seeing test feedback.

---

## Proposed Architecture: MAP-Inspired Modular Agent

Based on "Improving Planning with LLMs: A Modular Agentic Architecture" (Nature 2025):

```
┌─────────────────────────────────────────────────────────────────┐
│                      ORCHESTRATOR                                │
│  (Decides when goal is met, coordinates modules, handles retry) │
└─────────────────────────────────────────────────────────────────┘
           │           │           │           │
           ▼           ▼           ▼           ▼
    ┌───────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐
    │   TASK    │ │  ACTOR  │ │ MONITOR │ │EVALUATOR │
    │DECOMPOSER │ │         │ │         │ │          │
    └───────────┘ └─────────┘ └─────────┘ └──────────┘
         │             │           │           │
         │             ▼           │           │
         │      ┌───────────┐      │           │
         │      │  EXECUTOR │◄─────┘           │
         │      │ (FM+Tools)│                  │
         │      └───────────┘                  │
         │             │                       │
         └─────────────┴───────────────────────┘
                       │
                       ▼
               ┌─────────────┐
               │  VERIFIER   │
               │(Run tests,  │
               │return diff) │
               └─────────────┘
```

### Module Responsibilities

| Module | Input | Output | Implementation |
|--------|-------|--------|----------------|
| **Task Decomposer** | Task description + constraints | List of subtasks with success criteria | LLM or heuristic |
| **Actor** | Current subtask + state | Concrete tool call | FM (Apple) |
| **Monitor** | Proposed action + constraints | Accept/reject + reason | Rule-based or LLM |
| **Evaluator** | Current state + goal | Progress score + feedback | Run verification, parse results |
| **Orchestrator** | All module outputs | Next action decision | State machine |
| **Verifier** | Workspace state | Detailed test results | Run test.sh, parse output |

---

## Implementation Plan

### Phase 1: Iterative Verification Loop (Milestone: Solve regex-log)

**Goal**: Add verification feedback DURING execution, not just at the end.

#### 1.1 Add `verify_progress` Tool
**File**: `src/fm/tools.ts` (new)
```typescript
// New tool for FM to call mid-execution
{
  name: "verify_progress",
  description: "Run verification tests and see detailed results",
  params: {},
  execute: async (workspace) => {
    // Run test.sh or task verification
    // Return: "3/9 tests passing. Failures:\n- Line 7: expected no match, got '2021-01-01'\n- Line 8: ..."
  }
}
```

**Files to modify**:
- `src/fm/tools.ts` - Add verify_progress tool
- `src/fm/worker.ts` - Register new tool
- `src/hillclimber/executor.ts` - Pass verification config to FM

#### 1.2 Enable TB2-Specific Skills
**File**: `src/fm/hints.ts`

Change:
```typescript
// Current: TB2 gets NO skills
if (mode === "tb2") return undefined;

// New: TB2 gets task-specific skills
if (mode === "tb2") {
  return buildTB2Hint(taskDescription, previousActions);
}
```

**Files to modify**:
- `src/fm/hints.ts` - Add TB2 hint builder
- `src/skills/library/tb2-skills.ts` (new) - TB2-specific skills

#### 1.3 Add TB2 Skills Library
**File**: `src/skills/library/tb2-skills.ts` (new)

```typescript
export const TB2_SKILLS = {
  "regex-log": {
    description: "Write regex for log parsing",
    skills: [
      "IPv4 pattern: (?:25[0-5]|2[0-4]\\d|1?\\d?\\d\\.){3}...",
      "Date pattern: \\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\\d|3[01])",
      "Boundary assertion: (?:^|[^0-9A-Za-z])...(?=$|[^0-9A-Za-z])",
      "Lookahead for constraint: (?=.*pattern)...",
    ],
    hints: [
      "Use verify_progress after each regex attempt to see which tests fail",
      "Invalid IPs have octets >255, analyze test output carefully",
      "Match LAST date means: match date preceded by greedy .*",
    ]
  },
  // More tasks...
}
```

#### 1.4 Modify Orchestrator for Iterative Loop
**File**: `src/fm/orchestrator.ts`

Add verification retry logic:
```typescript
// After tool execution, if workspace has changed:
if (toolName === "write_file" && isVerifiableTask) {
  const verifyResult = await runQuickVerify(workspace);
  state.verificationFeedback = verifyResult;
  // Include in next FM prompt: "Last verification: 3/9 passing. Failures: ..."
}
```

### Phase 2: Evaluator Module (Real-Time Progress Scoring)

**Goal**: Score progress during execution, not just at end.

#### 2.1 Create Evaluator Service
**File**: `src/hillclimber/evaluator.ts` (new)

```typescript
export interface EvaluatorResult {
  progress: number;       // 0-1, how close to goal
  testsTotal: number;
  testsPassing: number;
  failures: FailureDetail[];
  suggestion?: string;    // What to try next
}

export function evaluateProgress(
  task: TerminalBenchTask,
  workspace: string
): Effect.Effect<EvaluatorResult> {
  // Run verification, parse output, return structured feedback
}
```

#### 2.2 Integrate Evaluator into Orchestrator
**File**: `src/fm/orchestrator.ts`

After each turn:
1. Run evaluator
2. If progress improved, continue
3. If progress stuck for N turns, suggest different approach
4. If progress regressed, backtrack

### Phase 3: Best-of-N Sampling (Test-Time Compute)

**Goal**: Try multiple solution candidates, pick best.

#### 3.1 Add Parallel Candidate Generation
**File**: `src/hillclimber/best-of-n.ts` (new)

```typescript
export interface CandidateResult {
  id: string;
  solution: string;
  score: number;
  testsPassed: number;
  testsTotal: number;
}

export async function bestOfN(
  task: TerminalBenchTask,
  config: HillClimberConfig,
  n: number = 5
): Promise<CandidateResult[]> {
  // Run N parallel attempts with different random seeds/temperatures
  // Return all candidates sorted by score
}
```

#### 3.2 Add Candidate Selection
**File**: `src/hillclimber/candidate-selector.ts` (new)

```typescript
// Implement "Shortest Majority Vote" from Zeng et al. 2025
// If multiple candidates pass, prefer shorter solutions
// If none pass, pick highest partial score
```

### Phase 4: Task Decomposer (For Complex Tasks)

**Goal**: Break complex tasks into subtasks with verification checkpoints.

#### 4.1 Create Task Decomposer
**File**: `src/hillclimber/decomposer.ts` (new)

For regex-log:
```typescript
{
  subtasks: [
    { id: 1, goal: "Write regex that matches valid IPv4", checkpoint: "verify IPv4 matching" },
    { id: 2, goal: "Extend to match dates after IPv4", checkpoint: "verify date matching" },
    { id: 3, goal: "Handle edge cases", checkpoint: "verify all 26 test cases" }
  ]
}
```

---

## Files to Modify/Create

### Modify Existing
| File | Changes |
|------|---------|
| `src/fm/hints.ts` | Add TB2-specific hints |
| `src/fm/orchestrator.ts` | Add verification feedback loop |
| `src/fm/worker.ts` | Add verify_progress tool |
| `src/hillclimber/executor.ts` | Pass verification context |
| `src/hillclimber/runner.ts` | Support iterative mode |
| `src/bench/model-adapter.ts` | Enable skills for TB2 |

### Create New
| File | Purpose |
|------|---------|
| `src/fm/tools/verify.ts` | verify_progress tool |
| `src/skills/library/tb2-skills.ts` | TB2-specific skills |
| `src/hillclimber/evaluator.ts` | Real-time progress scoring |
| `src/hillclimber/best-of-n.ts` | Parallel candidate generation |
| `src/hillclimber/decomposer.ts` | Task decomposition |

---

## Milestone: Solve regex-log

### Success Criteria
- [ ] regex-log passes verification at least 1 time
- [ ] FM receives test feedback during execution
- [ ] FM iterates based on specific failure reasons
- [ ] Solution converges within 15 turns

### Validation Steps
1. Run `bun run hillclimber --task regex-log --max-runs 10`
2. Verify logs show verification feedback being injected
3. Verify FM is iterating based on test results
4. At least 1 pass in 10 runs

---

## Long-Term Direction: #1 on Terminal-Bench with Local Models

### Strategy
1. **Phase 1-2**: Iterative verification + evaluator (solve easy/medium tasks)
2. **Phase 3**: Best-of-N (improve reliability on medium tasks)
3. **Phase 4**: Task decomposition (tackle hard tasks)
4. **Phase 5**: Full MAP architecture (compete with frontier models)

### Key Research Insights Applied
- **TTC (Snell 2024)**: Allocate compute adaptively by difficulty
- **Large Language Monkeys (Brown 2024)**: Repeated sampling + verification = 15.9% → 56%
- **MAP (Nature 2025)**: Modular architecture with specialized modules
- **Shortest Majority Vote (Zeng 2025)**: Prefer shorter correct solutions

### Why This Can Beat Cloud Models
1. **Local inference = zero latency** for verification loops
2. **Unlimited retries** (no API costs)
3. **Structured verification** beats brute-force intelligence
4. **Domain-specific skills** > general knowledge

---

## User Choices

- **Approach**: Full MAP Architecture (comprehensive)
- **Skills**: No solution injection - let FM discover patterns
- **Scope**: All 5 tasks simultaneously

---

## Implementation Order

### Step 1: Core MAP Modules (~2 hours)

**1.1 Evaluator Module** (`src/hillclimber/evaluator.ts`)
- Run verification, parse test output
- Return structured feedback: tests passing, failures with details
- Works for all 5 tasks

**1.2 Monitor Module** (`src/hillclimber/monitor.ts`)
- Validate actions before execution
- Catch obvious mistakes (invalid paths, missing dependencies)
- Rule-based, no LLM needed

**1.3 Task Decomposer** (`src/hillclimber/decomposer.ts`)
- Break each task into subtasks with checkpoints
- Task-specific decomposition rules for all 5 tasks

### Step 2: Orchestrator Rewrite (~1.5 hours)

**2.1 New Orchestrator** (`src/hillclimber/map-orchestrator.ts`)
- Coordinate all modules
- Implement feedback loop: Action → Execute → Evaluate → Adjust
- Handle verification feedback injection
- Decide when to switch strategies

**2.2 Integration with HillClimber** (`src/hillclimber/runner.ts`)
- Option to use MAP orchestrator vs legacy
- Pass task decomposition to executor

### Step 3: Verification Feedback Loop (~1 hour)

**3.1 Add verify_progress Tool** (`src/fm/tools/verify.ts`)
- FM can call mid-execution to see test results
- Returns structured failure information

**3.2 Modify FM Worker** (`src/fm/worker.ts`)
- Include verification feedback in context
- "Previous verification: 3/9 passing. Failures: ..."

### Step 4: Structural Skills (Not Solutions) (~30 min)

**4.1 TB2 Structural Skills** (`src/skills/library/tb2-skills.ts`)
- Regex: boundary assertions, lookaheads, character classes
- C: PPM format, basic graphics algorithms
- Python: NumPy array manipulation, OpenCV patterns
- Bio: BioPython patterns, DNA string manipulation
- NOT the actual solutions, just structural patterns

### Step 5: Best-of-N + Candidate Selection (~1 hour)

**5.1 Parallel Execution** (`src/hillclimber/best-of-n.ts`)
- Run N attempts with different random seeds
- Collect all candidates

**5.2 Candidate Selector** (`src/hillclimber/candidate-selector.ts`)
- Shortest Majority Vote (prefer shorter correct solutions)
- Partial score ranking for non-passing candidates

---

## Validation Plan

### Per-Task Success Criteria

| Task | Current Score | Target | Validation |
|------|--------------|--------|------------|
| regex-log | 89 (0% pass) | 1000+ (pass) | 9/9 test cases |
| path-tracing | 89 (0% pass) | 1000+ (pass) | >= 0.99 similarity |
| model-extraction | 89 (0% pass) | 1000+ (pass) | 30/30 rows matched |
| video-processing | 89 (0% pass) | 1000+ (pass) | Frames within range |
| dna-assembly | 89 (0% pass) | 1000+ (pass) | 8 valid primers |

### Test Run

After implementation:
```bash
bun run hillclimber --max-runs 50 --sleep 10000
```

Check:
- [ ] Evaluator feedback appearing in logs
- [ ] FM iterating based on test failures
- [ ] At least 1 task passes in 50 runs

---

## Key Architecture Diagram

```
                    ┌─────────────────┐
                    │  HILLCLIMBER    │
                    │  RUNNER         │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ MAP ORCHESTRATOR │
                    │                  │
                    │ ┌──────────────┐ │
                    │ │ Task         │ │
                    │ │ Decomposer   │ │
                    │ └──────┬───────┘ │
                    │        │         │
                    │ ┌──────▼───────┐ │
                    │ │ For each     │ │
                    │ │ subtask:     │ │
                    │ │              │ │
                    │ │ ┌──────────┐ │ │
                    │ │ │ Monitor  │ │ │ ◄── Validate before execute
                    │ │ └────┬─────┘ │ │
                    │ │      │       │ │
                    │ │ ┌────▼─────┐ │ │
                    │ │ │ FM Actor │ │ │ ◄── Apple Foundation Models
                    │ │ └────┬─────┘ │ │
                    │ │      │       │ │
                    │ │ ┌────▼─────┐ │ │
                    │ │ │Evaluator │ │ │ ◄── Run tests, parse results
                    │ │ └────┬─────┘ │ │
                    │ │      │       │ │
                    │ │   Feedback   │ │
                    │ │   Loop       │ │
                    │ └──────────────┘ │
                    └─────────────────-┘
```

The goal is not to tune hyperparameters, but to **change the execution architecture** so the FM gets feedback it can act on, iterate intelligently, and converge on correct solutions.
