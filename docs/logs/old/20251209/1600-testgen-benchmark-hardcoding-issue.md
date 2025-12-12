# TestGen Benchmark-Specific Hardcoding Issue

**Time:** 16:15 CT
**Date:** 2025-12-09
**Status:** Needs refactoring
**Related:** Plan to remove all task-specific hardcoding (lovely-kindling-marble.md)

---

## The Problem

`generateTestsIteratively()` is **hardcoded for Terminal-Bench benchmark tasks** but is being used in contexts where benchmark-specific behavior doesn't make sense:

1. **Commander** — Free-form user prompts
2. **MechaCoder** — Autonomous coding tasks

### What's Hardcoded

The test generator ALWAYS produces 5 test categories:

| Category | Purpose | Benchmark-Specific? |
|----------|---------|---------------------|
| `anti_cheat` | Detect forbidden tools/approaches | **YES** — Only makes sense for TB2 |
| `existence` | Check output files exist | Somewhat general |
| `correctness` | Verify correct behavior | General |
| `boundary` | Test edge cases | General |
| `integration` | System-level tests | General |

### Why `anti_cheat` Doesn't Belong

The `anti_cheat` category was designed for Terminal-Bench rules like:
- "Don't read /app/image.ppm" (path-tracing)
- "Don't use primer3" (dna-assembly)
- "Don't use python for regex" (regex-log)

These are **benchmark-specific constraints**. When a user types a free-form prompt in Commander like:

> "Write a function to parse JSON files"

Generating `anti_cheat` tests makes no sense — there are no forbidden tools.

### Where This Happens

**File:** `src/hillclimber/test-generator-iterative.ts`

```typescript
// This is called for ALL test generation, not just benchmarks
export async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  // ...
) {
  // Hardcoded categories — always generates all 5
  const categories = [
    "anti_cheat",
    "existence",
    "correctness",
    "boundary",
    "integration"
  ];

  for (const category of categories) {
    // Generate tests for each category...
  }
}
```

**Callers that shouldn't get benchmark behavior:**

1. `src/commander/testgen-runner.ts` — Commander UI
2. `src/mechacoder/` — Autonomous coding agent
3. Any future non-benchmark use case

---

## Impact

1. **Wasted FM calls** — Generating anti_cheat tests for tasks that don't need them
2. **Confusing test output** — Users see "anti_cheat" category for their simple coding tasks
3. **Wrong mental model** — TestGen appears benchmark-specific, not general-purpose
4. **Violates thesis** — We're proving architecture works, not gaming TB2

---

## Recommended Refactor

### Option 1: Context-Aware Categories (Recommended)

Add a `context` parameter to determine which categories to generate:

```typescript
type TestGenContext =
  | "benchmark"     // TB2 — all 5 categories including anti_cheat
  | "commander"     // User prompts — correctness, boundary, existence
  | "mechacoder"    // Autonomous coding — correctness, boundary
  | "custom";       // Let caller specify

interface TestGenOptions {
  context: TestGenContext;
  categories?: string[];  // Override for "custom" context
}

export async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  options: TestGenOptions,
) {
  const categories = getCategoriesForContext(options.context, options.categories);
  // ...
}

function getCategoriesForContext(context: TestGenContext, custom?: string[]): string[] {
  switch (context) {
    case "benchmark":
      return ["anti_cheat", "existence", "correctness", "boundary", "integration"];
    case "commander":
      return ["existence", "correctness", "boundary"];
    case "mechacoder":
      return ["correctness", "boundary"];
    case "custom":
      return custom ?? ["correctness"];
  }
}
```

### Option 2: Remove anti_cheat Entirely

If `anti_cheat` is purely benchmark-specific, remove it from the general test generator and only add it in the benchmark runner:

```typescript
// In test-generator-iterative.ts — general purpose
const DEFAULT_CATEGORIES = ["existence", "correctness", "boundary", "integration"];

// In benchmark runner only
const BENCHMARK_CATEGORIES = [...DEFAULT_CATEGORIES, "anti_cheat"];
```

### Option 3: FM-Powered Category Selection

Let FM analyze the task description and decide which categories are relevant:

```typescript
const prompt = `Given this task description, which test categories are relevant?
- anti_cheat: Only if there are forbidden tools/approaches mentioned
- existence: If specific output files must exist
- correctness: Always relevant
- boundary: If there are numeric ranges or edge cases
- integration: If multiple components interact

Task: ${taskDescription}

Return relevant categories as JSON array.`;
```

This aligns with the FM-powered approach in the main refactoring plan.

---

## Files to Modify

| File | Change |
|------|--------|
| `src/hillclimber/test-generator-iterative.ts` | Add context parameter, conditional categories |
| `src/commander/testgen-runner.ts` | Pass `context: "commander"` |
| `src/mechacoder/*.ts` | Pass `context: "mechacoder"` |
| `src/bench/*.ts` | Pass `context: "benchmark"` |

---

## Relationship to Main Refactor

This is part of the larger effort to remove benchmark-specific hardcoding from the HillClimber system. See:

- `/Users/christopherdavid/.claude/plans/lovely-kindling-marble.md` — Full refactoring plan
- `docs/logs/20251209/1454-decomposer-cleanup-no-cheating.md` — Philosophy

The test generator should be **general-purpose**:
1. Works for ANY task, not just TB2
2. Categories determined by context/description, not hardcoded
3. No benchmark-specific knowledge embedded

---

## Quick Fix (If Needed Before Full Refactor)

If you need a quick fix before the full refactor:

```typescript
// In test-generator-iterative.ts
export async function generateTestsIteratively(
  taskDescription: string,
  taskId: string,
  env: any,
  emitter: any,
  options: {
    model?: string;
    verbose?: boolean;
    skipAntiCheat?: boolean;  // NEW: Quick fix flag
  }
) {
  let categories = ["anti_cheat", "existence", "correctness", "boundary", "integration"];

  // Quick fix: skip anti_cheat if flag is set
  if (options.skipAntiCheat) {
    categories = categories.filter(c => c !== "anti_cheat");
  }

  // ... rest of function
}
```

Then in Commander:
```typescript
await generateTestsIteratively(taskDescription, taskId, env, emitter, {
  model: options.model,
  skipAntiCheat: true,  // Don't generate benchmark-specific tests
});
```

---

## Summary

| Problem | TestGen hardcoded for TB2 benchmark |
|---------|-------------------------------------|
| Symptom | `anti_cheat` tests generated for Commander/MechaCoder |
| Root cause | No context awareness in test category selection |
| Fix | Add context parameter or FM-powered category selection |
| Priority | Should be part of main hardcoding removal refactor |

---

**Next Steps:**
1. Decide on approach (context-aware vs FM-powered)
2. Implement as part of Phase 3 of the main refactor plan
3. Update all callers to pass appropriate context

---

## Implementation Log

**Date:** 2025-12-09
**Time:** ~16:30 CT
**Status:** ✅ Completed

### What Was Done

Implemented **Option 1: Context-Aware Categories** as recommended in this document.

#### Changes Made

1. **`src/hillclimber/test-generator-iterative.ts`**
   - Added `TestGenContext` type: `"benchmark" | "commander" | "mechacoder" | "custom"`
   - Added `getCategoriesForContext()` function that returns appropriate categories:
     - `benchmark`: All 5 categories including `anti_cheat`
     - `commander`: `existence`, `correctness`, `boundary` (no `anti_cheat`)
     - `mechacoder`: `correctness`, `boundary` (minimal set)
     - `custom`: User-specified categories (defaults to `correctness` if none provided)
   - Updated `generateTestsIteratively()` to accept `context` and `categories` options
   - Changed hardcoded `CATEGORIES` array to `ALL_CATEGORIES` and use context-aware selection
   - Added logging for context and selected categories

2. **`src/hillclimber/testgen-service.ts`**
   - `runTestGenWithStreaming()`: Added `context: "benchmark"` (for TB2 tasks)
   - `runCustomTestGen()`: Added `context: "commander"` (for free-form prompts)

3. **`src/hillclimber/testgen-integration.ts`**
   - `runTestGenForTask()`: Added `context: "benchmark"` (for TB2 tasks)

4. **`scripts/run-testgen-regex-log.ts`**
   - Added `context: "benchmark"` (for TB2 benchmark script)

### Impact

- ✅ **Commander** no longer generates `anti_cheat` tests for free-form prompts
- ✅ **Benchmark tasks** still get full test suite including `anti_cheat`
- ✅ **Backward compatible**: Defaults to `"benchmark"` if context not specified
- ✅ **Extensible**: Easy to add new contexts or customize categories

### Testing

- No lint errors introduced
- All callers updated to pass appropriate context
- Changes committed and pushed to main

### Commit

```
Refactor test generator to use context-aware categories

Remove benchmark-specific hardcoding from generateTestsIteratively().
TestGen now supports different contexts:
- benchmark: All 5 categories including anti_cheat (for TB2)
- commander: existence, correctness, boundary (for free-form prompts)
- mechacoder: correctness, boundary (for autonomous coding)
- custom: User-specified categories

This prevents anti_cheat tests from being generated for non-benchmark
use cases like Commander free-form prompts, which don't have forbidden
tools.
```

**Commit hash:** `c2b5a974e`
