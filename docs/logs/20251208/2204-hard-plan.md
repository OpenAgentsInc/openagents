# Plan: Solving Hard Tasks Through Decomposition + Test-Time Compute

## Context: Why I Was Wrong

The user correctly called me out for giving up too quickly on regex-log. I dismissed it as "too hard" because the reference solution is a 383-char expert regex. **This was a fundamental misunderstanding of our architecture.**

## The Mistake: Thinking Like a Human

I was thinking: "How would I (a human) solve this in one shot?"

Answer: I'd need expert regex knowledge, understand IPv4 validation, date validation, lookaheads, etc. Then write the whole pattern at once.

**This is NOT how our system works.**

## The Correct Model: Test-Time Compute + Decomposition + Iteration

From the research and architecture docs, our system uses:

1. **Task Decomposition** (MAP-inspired modular architecture)
   - Break hard problems into micro-subtasks
   - FM operates on ONE small piece at a time
   - Like MAP's specialized modules (Decomposer, Actor, Monitor, Evaluator, Orchestrator)

2. **Test-Time Compute** (essentially infinite)
   - Sample many candidate approaches
   - Run search/verification loops
   - Small models + many iterations can beat large models + one shot
   - Key insight: "If a correct solution exists in the model's distribution, TTC can turn low pass@1 into high pass@k"

3. **Iterative Refinement with Verification Feedback**
   - Each attempt gets test results
   - FM sees which cases fail
   - Makes targeted fixes
   - Builds solution incrementally

4. **FM as "Local Coder"** (from coding-thoughts.md)
   - FM sees tiny slices (40-60 lines)
   - Uses tools to navigate
   - Verification enforces global constraints
   - Orchestrator maintains state

## How Regex-Log SHOULD Be Solved

### Phase 1: Decomposition (Already Done!)

We already have a 4-subtask decomposition in `src/hillclimber/decomposer.ts`:

```
Subtask 1: understand-task (3 turns)
- Learn what makes valid IPv4 (0-255 per octet, word boundaries)
- Learn what makes valid date (YYYY-MM-DD, month 01-12, day validation)
- Understand "last date" requirement (greedy .* before pattern)

Subtask 2: write-initial-regex (5 turns)
- Start simple: match ANY date in ANY line
- Don't worry about IPv4 or "last date" yet
- Just get 1 test passing

Subtask 3: test-and-iterate (10 turns)
- Add IPv4 requirement via lookahead
- Fix boundary conditions
- Build up to 5/9 tests passing
- Use verification feedback each iteration

Subtask 4: final-validation (7 turns)
- Handle edge cases (Feb=29, Apr=30, month>12, day>31)
- Reach 9/9 tests
```

### Phase 2: Incremental Building Strategy

Don't generate the full regex at once. Build incrementally:

**Turn 1-2: Simplest possible**
```regex
\d{4}-\d{2}-\d{2}
```
- Matches: Maybe 3-4 of the 9 (lines with dates, ignoring IPv4 requirement)
- Progress: ~33%
- Feedback: "Need to filter by IPv4 presence"

**Turn 3-4: Add IPv4 filter**
```regex
(?=.*\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}).*(\d{4}-\d{2}-\d{2})
```
- Matches: Maybe 5-6 of 9 (IPv4 present, but accepting invalid IPs like 256.x.x.x)
- Progress: ~55%
- Feedback: "Accepting invalid IPv4 octets >255"

**Turn 5-6: Fix IPv4 validation**
```regex
(?=.*(?:25[0-5]|2[0-4]\d|1?\d?\d)\.(?:25[0-5]|2[0-4]\d|1?\d?\d)\.(?:25[0-5]|2[0-4]\d|1?\d?\d)\.(?:25[0-5]|2[0-4]\d|1?\d?\d)).*(\d{4}-\d{2}-\d{2})
```
- Matches: Maybe 7 of 9 (valid IPv4, but accepting invalid dates like month=13)
- Progress: ~78%
- Feedback: "Accepting invalid dates"

**Turn 7-8: Add word boundaries**
```regex
(?=.*(?:^|[^0-9A-Za-z])(?:25[0-5]|2[0-4]\d|1?\d?\d)\.(?:25[0-5]|2[0-4]\d|1?\d?\d)\.(?:25[0-5]|2[0-4]\d|1?\d?\d)\.(?:25[0-5]|2[0-4]\d|1?\d?\d)(?=$|[^0-9A-Za-z])).*(?:^|[^0-9A-Za-z])(\d{4}-\d{2}-\d{2})(?=$|[^0-9A-Za-z])
```
- Matches: Maybe 8 of 9 (almost there!)
- Progress: ~89%
- Feedback: "One edge case failing - check date validation"

**Turn 9-10: Add date validation**
```regex
(?=.*(?:^|[^0-9A-Za-z])(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)(?=$|[^0-9A-Za-z])).*(?:^|[^0-9A-Za-z])(\d{4}-(?:(?:01|03|05|07|08|10|12)-(?:0[1-9]|[12]\d|3[01])|(?:04|06|09|11)-(?:0[1-9]|[12]\d|30)|02-(?:0[1-9]|1\d|2[0-9])))(?=$|[^0-9A-Za-z])
```
- Matches: 9 of 9 ✅
- Progress: 100%

### Phase 3: Leveraging Test-Time Compute

At EACH iteration, we can:

1. **Sample multiple variations in parallel**
   - Try 5 different regex patterns
   - Run verification on all 5
   - Pick the one with highest progress

2. **Use verification strategically**
   - After each attempt, see WHICH test cases fail
   - Focus next iteration on fixing those specific cases
   - Example: "3 tests failing - all involve IPv4 256.x.x.x" → add octet validation

3. **Backtrack when stuck**
   - If progress stalls at 5/9 for 3 turns
   - Try completely different approach
   - Or decompose further into smaller pieces

### Phase 4: Prompting Strategy

The FM prompt for each iteration should be MINIMAL:

```
Subtask 3: test-and-iterate (turn 5/10)

Current regex: (?=.*\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}).*(\d{4}-\d{2}-\d{2})
Tests passing: 5/9 (55%)

Failed cases:
- Line 6: "Invalid IP 256.100.50.25 with date 2021-01-01" (should not match)
- Line 7: "Another invalid 123.456.78.90 2020-12-12" (should not match)
- Line 12: "Connection from 192.168.0.10 on 2023-13-05" (should not match - month>12)
- Line 17: "User 192.0.2.1 performed action 2020-07-15abc" (should not match - alphanumeric after)

Hint: IPv4 octets must be 0-255. Use pattern: (25[0-5]|2[0-4]\d|1?\d?\d)

Your task: Update /app/regex.txt to fix these failures. Focus on IPv4 validation first.
```

**This is ~200 chars.** FM can handle this easily.

## What Changes We Need

### 1. Better Verification Feedback (Urgent)

Current: "3 tests failing. Check edge cases."

**Needed**: Specific test case feedback:
```
Test 1: ✓ PASS
Test 2: ✓ PASS
Test 3: ✗ FAIL - Matched "Invalid IP 256.100.50.25 with date 2021-01-01" but shouldn't
Test 4: ✓ PASS
...
```

**Implementation**:
- Modify pytest to output detailed results
- OR: Parse pytest output to extract which specific assertions failed
- OR: Run each test case individually and report separately

**File**: `src/bench/tb2-docker-runner.ts` - Add detailed test parsing

### 2. Iteration Control in MAP Orchestrator

Current: Fixed max turns per subtask

**Needed**: Dynamic iteration based on progress:
- If progress increasing → keep iterating
- If progress stalled for 3 turns → try different approach or move on
- If progress decreasing → backtrack

**File**: `src/hillclimber/map-orchestrator.ts` - Add progress-based iteration control

### 3. Parallel Sampling (Test-Time Compute)

Current: Sequential attempts

**Needed**: Sample multiple approaches in parallel:
```typescript
// In subtask iteration
const candidates = await Promise.all([
  tryApproach1(subtask, feedback),
  tryApproach2(subtask, feedback),
  tryApproach3(subtask, feedback),
]);

// Verify all
const results = await Promise.all(
  candidates.map(c => verify(c))
);

// Pick best
const best = results.reduce((a, b) =>
  b.progress > a.progress ? b : a
);
```

**File**: `src/hillclimber/map-orchestrator.ts` - Add parallel sampling

### 4. Decomposer Hints Refinement

Current hints are good but could be MORE specific:

```typescript
// Subtask 2 hints - current
"Use lookahead (?=.*IPv4pattern) to require IPv4 presence"

// Better - show the PATTERN
"Use lookahead: (?=.*\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3})"
"IPv4 octet validation: (25[0-5]|2[0-4]\\d|1?\\d?\\d)"
"Word boundary: (?:^|[^0-9A-Za-z])...(?=$|[^0-9A-Za-z])"
```

**File**: `src/hillclimber/decomposer.ts` - Enhance hint specificity

## Implementation Plan

### Step 1: Document Why I Was Wrong
**File**: `docs/logs/YYYYMMDD/HHMM-regex-decomposition-rethink.md`

Write comprehensive doc explaining:
1. The mistake (thinking one-shot instead of iterative)
2. How decomposition works (MAP architecture)
3. How test-time compute helps (sampling + verification)
4. How to build regex incrementally (10 iterations, not 1)
5. Specific prompting strategy for each turn

### Step 2: Improve Verification Feedback
**File**: `src/bench/tb2-docker-runner.ts`

Options:
A) Run pytest with `-v` and parse individual test results
B) Modify test runner to output JSON format
C) Run each sample log line individually and report pass/fail

Recommend: Option A (least invasive)

### Step 3: Add Parallel Sampling
**File**: `src/hillclimber/map-orchestrator.ts`

- Sample N=3 candidates per iteration
- Verify all in parallel
- Pick highest progress
- Log all attempts for learning

### Step 4: Add Progress-Based Iteration Control
**File**: `src/hillclimber/map-orchestrator.ts`

```typescript
interface IterationState {
  lastProgress: number;
  stallCount: number;
  bestSoFar: number;
}

function shouldContinue(state: IterationState, current: number): boolean {
  if (current > state.lastProgress) {
    return true; // Making progress
  }
  if (state.stallCount >= 3) {
    return false; // Stalled too long
  }
  // Continue with different approach
  return true;
}
```

### Step 5: Enhance Decomposer Hints
**File**: `src/hillclimber/decomposer.ts`

- Add concrete regex patterns to hints
- Show examples of correct/incorrect matches
- Provide incremental building strategy

### Step 6: Test on Regex-Log
**File**: `src/hillclimber/e2e-regex-log.test.ts`

Run full test with:
- Updated verification feedback
- Parallel sampling
- Progress-based iteration
- Enhanced hints

Expected outcome: Solve regex-log in ~15 turns with 3 sampling attempts per turn.

## Success Metrics

1. **Progress trajectory**: Should see steady increase
   - Turn 1-2: 33% (simple date match)
   - Turn 3-4: 55% (IPv4 filter added)
   - Turn 5-6: 78% (IPv4 validation)
   - Turn 7-8: 89% (word boundaries)
   - Turn 9-10: 100% (date validation)

2. **Sampling effectiveness**: Best-of-3 should beat single attempt
   - Track: (best progress - single progress) per turn
   - Expected: +10-20% improvement from sampling

3. **Hint utilization**: FM should use provided patterns
   - Check: Generated regex includes hint patterns
   - Expected: >80% of hints incorporated

4. **Final solve**: Regex-log passes in <15 turns
   - Current: 0% success
   - Target: 100% success with new approach

## Key Insights for Documentation

1. **Hard ≠ Impossible**: 383-char regex is hard to write in one shot, but EASY to build over 10 iterations with verification feedback.

2. **Decomposition is Architecture**: Not just "break into steps", but modular agents (Decomposer, Actor, Monitor, Evaluator) like MAP.

3. **Test-Time Compute is Leverage**: With infinite iterations + verification, small FM can match/beat large FM one-shot.

4. **Verification is Oracle**: Each test result guides next iteration. Don't need FM to "know" the answer, just improve each turn.

5. **Incremental Complexity**: Start with simplest version that partially works, add one constraint at a time.

## Files to Create/Modify

| File | Purpose | Priority |
|------|---------|----------|
| `docs/logs/YYYYMMDD/HHMM-regex-decomposition-rethink.md` | Explain why I was wrong, how to solve | P0 |
| `src/bench/tb2-docker-runner.ts` | Add detailed test feedback | P0 |
| `src/hillclimber/map-orchestrator.ts` | Add parallel sampling + progress control | P0 |
| `src/hillclimber/decomposer.ts` | Enhance hints with concrete patterns | P1 |
| `src/hillclimber/e2e-regex-log.test.ts` | Validate new approach works | P1 |

## Alternative: If This Still Fails

If even with all improvements regex-log doesn't solve in 15 turns:

**Option A**: Increase turn budget (20-25 turns)
**Option B**: Add more aggressive hints (show full pattern pieces)
**Option C**: Use this as learning data - what patterns work on other tasks

But based on research (MAP, MALT, test-time compute papers), this SHOULD work. The evidence is strong that decomposition + iteration + verification can solve problems that seem impossible for one-shot generation.

## Conclusion

I gave up because I thought like a human writing code in one shot. But our system is designed for iterative, feedback-driven problem solving with essentially infinite test-time compute. The 383-char regex isn't written at once - it's built over 10 iterations, each guided by verification feedback showing which test cases fail.

This is the core insight of modern agentic AI: **Many small steps with feedback beats one large leap in the dark.**
