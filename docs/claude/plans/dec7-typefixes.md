# TypeScript Error Fix Plan - December 7, 2025

## Summary

**Initial Errors: 618** → **Current Errors: 490** (20.7% fixed)

Run `bun run typecheck` to see all errors.

## Error Breakdown by Type (Updated)

| Error Code | Count | Description | Difficulty |
|------------|-------|-------------|------------|
| TS2322 | 128 | Type 'X' not assignable to type 'Y' | Medium |
| TS2345 | 102 | Argument type mismatch | Medium |
| TS2379 | 52 | exactOptionalPropertyTypes | Medium |
| TS2722 | 43 | Possibly undefined invocation | Easy |
| TS2304 | 35 | Cannot find name | Easy |
| TS2540 | 29 | Read-only property assignment | Medium |
| TS2353 | 20 | Unknown property in object literal | Easy |
| TS2375 | 9 | exactOptionalPropertyTypes | Medium |
| TS2349 | 7 | Expression not callable | Medium |
| TS4115 | 6 | Missing 'override' modifier | Easy |
| TS2561 | 6 | Object literal can only specify known properties | Easy |
| TS1029 | 6 | 'public' modifier must precede | Easy |
| TS2740 | 5 | Missing properties from type | Medium |
| TS6196 | 4 | Unused type import | Easy |
| TS6133 | 4 | Unused variable/import | Easy |
| TS2783 | 4 | Required property missing | Medium |
| TS2300 | 4 | Duplicate identifier | Medium |
| Others | ~26 | Various | Varies |

## Files with Most Errors (Top 20)

| File | Errors |
|------|--------|
| src/learning/__tests__/trm-halt.test.ts | 60 |
| src/effuse/widgets/tb-controls.test.ts | 57 |
| src/effuse/widgets/tb-output.test.ts | 43 |
| src/effuse/widgets/category-tree.test.ts | 41 |
| src/tasks/service.ts | 27 |
| src/effuse/widgets/tb-widgets.test.ts | 27 |
| src/effuse/widgets/mc-tasks.test.ts | 22 |
| src/learning/__tests__/trm-state.test.ts | 18 |
| src/effuse/widgets/tb-results.test.ts | 18 |
| src/skills/library/compositional.ts | 17 |
| src/effuse/widgets/trajectory-pane.test.ts | 12 |
| src/learning/orchestrator.ts | 11 |
| src/learning/__tests__/trm-integration.test.ts | 10 |
| src/effuse/widgets/hf-trajectory-detail.test.ts | 10 |
| src/training/episode-learner.test.ts | 9 |
| src/training/loop-runner.ts | 8 |
| src/storage/database.ts | 8 |
| src/effuse/widgets/container-panes.test.ts | 8 |
| src/reflexion/service.ts | 7 |
| src/skills/service.ts | 6 |

---

## Phase 1: Quick Wins (Est. 1-2 hours)

Start here - these are mechanical fixes that don't require understanding business logic.

### 1.1 Remove Unused Imports/Variables (92 TS6133 + 6 TS6196 = 98 errors)

**Pattern:** Find and remove unused declarations.

```typescript
// Before
import { Foo, Bar, Baz } from "./module.js";
const unused = "hello";

// After - remove if Baz is unused
import { Foo, Bar } from "./module.js";
```

**Files to fix (sample):**
- `src/bench/baseline.test.ts:7` - remove unused `join`
- `src/bench/harness.ts:15` - remove unused `DatabaseService`
- `src/cli/tbench-iterate.ts:36,46,53` - multiple unused imports
- `src/cli/tbench-local.ts:20,353` - unused imports and variables
- `src/training/episode-learner.ts:21,30,292` - unused imports

**Command to find all:**
```bash
bun run typecheck 2>&1 | grep "TS6133\|TS6196"
```

### 1.2 Add Override Modifier (17 TS4115 errors)

**Pattern:** Add `override` to class properties that override base class members.

```typescript
// Before
class MyError extends Error {
  constructor(readonly message: string) { super(message); }
}

// After
class MyError extends Error {
  constructor(override readonly message: string) { super(message); }
}
```

**Files:**
- `src/training/episode-learner.ts:135`
- `src/training/loop-runner.ts:209`
- `src/skills/library/compositional.ts` (multiple)
- `src/learning/orchestrator.ts` (multiple)

---

## Phase 2: Null/Undefined Checks (Est. 2-3 hours)

### 2.1 Possibly Undefined Invocation (43 TS2722 errors)

**Pattern:** Add optional chaining or null checks before invoking.

```typescript
// Before
callback(); // Error: callback is possibly undefined

// After - Option A: Optional chaining
callback?.();

// After - Option B: Guard clause
if (callback) { callback(); }
```

### 2.2 Possibly Null/Undefined Value (7 TS18048 + 3 TS18047 = 10 errors)

**Pattern:** Add null checks or non-null assertions (only if you're certain).

```typescript
// Before
fullConfig.maxDurationMs > 0  // Error: possibly undefined

// After - Option A: Default value
(fullConfig.maxDurationMs ?? 0) > 0

// After - Option B: Non-null assertion (use sparingly)
fullConfig.maxDurationMs! > 0

// After - Option C: Early guard
if (!fullConfig.maxDurationMs) return;
```

**Key files:**
- `src/training/loop-runner.ts:270,275,437,447,516,517`
- `src/cli/tbench-iterate.ts:1104,1185,1186`

---

## Phase 3: exactOptionalPropertyTypes Fixes (Est. 3-4 hours)

This project uses `exactOptionalPropertyTypes: true`. This means optional properties (`foo?: string`) cannot be assigned `undefined` explicitly.

### 3.1 TS2379 Errors (46 errors)

**Pattern:** When creating objects with optional properties, don't include them if the value is undefined.

```typescript
// Before - WRONG
const obj = {
  name: "test",
  description: someValue ?? undefined,  // Error!
};

// After - Option A: Conditionally spread
const obj = {
  name: "test",
  ...(someValue ? { description: someValue } : {}),
};

// After - Option B: Filter undefined before assignment
const base = { name: "test" };
if (someValue) base.description = someValue;
```

### 3.2 TS2375 Errors (27 errors)

Same root cause as TS2379. The fix pattern is the same.

**Key files:**
- `src/bench/harness.ts:232`
- `src/bench/model-adapter.ts:150,593`
- `src/cli/tbench-iterate.ts:690,933,1016,1052`
- `src/tasks/service.ts:399`
- `src/training/loop-runner.ts:312`

---

## Phase 4: Type Assignment Fixes (Est. 4-6 hours)

### 4.1 TS2322 - Type Not Assignable (148 errors)

Most common patterns:

**A) Boolean vs String mismatch** (common in CLI args)
```typescript
// Before - CLI arg can be string | true
const repo: string = args.repo;  // Error!

// After
const repo = typeof args.repo === "string" ? args.repo : undefined;
```

**B) ReadOnly array to mutable array**
```typescript
// Before
const items: string[] = readonlyArray;  // Error!

// After
const items: readonly string[] = readonlyArray;
// OR if you need mutable:
const items = [...readonlyArray];
```

**Key files:**
- `src/cli/tbench-sandbox.ts:130-140` - CLI arg type coercion
- Multiple test files - mock type mismatches

### 4.2 TS2345 - Argument Type Mismatch (114 errors)

**Pattern:** Match function parameter types exactly.

```typescript
// Before
someFunction({ limit: 10 });  // Error: expected SortPolicy

// After - Check what the function expects
someFunction({ sortBy: "date", limit: 10 });
```

**Key files:**
- `src/agent/overnight.ts:620` - wrong arg type
- `src/cli/tbench-sandbox.ts:134-136` - string vs boolean

---

## Phase 5: Read-Only Property Fixes (Est. 1-2 hours)

### 5.1 TS2540 - Cannot Assign to Read-Only (29 errors)

**Pattern:** Don't mutate readonly objects. Create new objects instead.

```typescript
// Before
task.deps = newDeps;  // Error: deps is readonly

// After
const updatedTask = { ...task, deps: newDeps };
```

**Key files:**
- `src/storage/database.ts:408,432,557,587,667,697`
- `src/tasks/service.ts:53` (21 errors at same line!)

**Special case - src/tasks/service.ts:53:**
This line has 21 errors. It's likely doing Object.assign() or spread on a readonly type. Consider using a type assertion or creating a proper update function.

---

## Phase 6: Missing/Unknown Properties (Est. 2-3 hours)

### 6.1 TS2339 - Property Doesn't Exist (20 errors)

**Pattern:** The property was removed or renamed. Check the type definition.

```typescript
// Before
result.blockers  // Error: 'blockers' doesn't exist

// After - Check the actual type
// If it was renamed:
result.blockingIssues
// If it was moved:
result.details?.blockers
```

**Files:**
- `src/cli/tbench-local.ts:459-460` - `blockers` property
- `src/cli/tbench-sandbox.ts:480-481` - `blockers` property

### 6.2 TS2353 - Unknown Property in Object Literal (22 errors)

**Pattern:** You're adding properties that don't exist in the target type.

```typescript
// Before
const update = { status: "closed", closedAt: new Date() };  // Error: closedAt unknown

// After - Check the type definition
// If closedAt should be there, add it to the type
// If it shouldn't, remove it from the object
```

**Files:**
- `src/tasks/cli.ts:1087` - `status` property unknown
- `src/tasks/repository.test.ts:76` - `author` property
- `src/tasks/service.ts:313,338` - `closedAt` property

---

## Phase 7: Miscellaneous (Est. 1-2 hours)

### 7.1 TS2367 - Unintentional Comparison (5 errors)

**Pattern:** Comparing incompatible types (e.g., enum to string that's not in the enum).

```typescript
// Before
if (status === "N/A")  // Error: "N/A" not in status union

// After - Use a value that's actually in the type
if (status === "skip")
// OR update the type if "N/A" should be valid
```

**Files:**
- `src/bench/baseline.ts:215`

### 7.2 TS2304 - Cannot Find Name (1 error)

```typescript
// src/agent/orchestrator/subagent-router.test.ts:568
// FMSettings is not imported
import { FMSettings } from "...";
```

### 7.3 TS2554 - Wrong Number of Arguments (4 errors)

Check function signatures and provide required arguments.

---

## Recommended Fix Order

1. **Phase 1** - Quick wins first (removes ~115 errors)
2. **Phase 6.2** - Unknown properties (often reveals schema issues)
3. **Phase 5** - Read-only fixes (systemic issue in a few files)
4. **Phase 2** - Null checks
5. **Phase 3** - exactOptionalPropertyTypes
6. **Phase 4** - Type assignments (largest category)
7. **Phase 6.1 + 7** - Remaining issues

## Validation

After each batch of fixes, run:
```bash
bun run typecheck 2>&1 | grep "error TS" | wc -l
```

Target: 0 errors

## Notes for Junior Dev

1. **Don't use `any` to silence errors** - fix the root cause
2. **Don't use `@ts-ignore` or `@ts-expect-error`** - these hide problems
3. **Test after each file** - run typecheck frequently to catch regressions
4. **When stuck**, check the type definition (`Cmd+Click` in VS Code)
5. **For exactOptionalPropertyTypes**, the pattern is almost always "conditionally spread"
6. **Commit frequently** - small commits per file or error type are easier to review

## Quick Reference Commands

```bash
# Full typecheck
bun run typecheck

# Count errors
bun run typecheck 2>&1 | grep "error TS" | wc -l

# Errors in specific file
bun run typecheck 2>&1 | grep "src/tasks/service.ts"

# Errors of specific type
bun run typecheck 2>&1 | grep "TS6133"

# All errors sorted by file
bun run typecheck 2>&1 | grep "error TS" | sort
```
