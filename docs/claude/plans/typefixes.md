# Plan: Fix 872 TypeScript Errors After SQLite Migration

## Overview
The SQLite migration has introduced 872 TypeScript errors across 123 files. The errors fall into 8 distinct categories that can be fixed systematically in order of dependency and impact.

## Error Breakdown
- **Total errors**: 872
- **Files affected**: 123
- **Main cause**: SQLite migration changed TaskService from JSONL to database, requiring:
  - New `DatabaseService` in Effect contexts
  - Changed API signatures (removed `appendCommits`, `tasksPath` → `dbPath`, etc.)
  - Updated return types for archive/init operations
  - Schema changes in widget message types

## Implementation Steps

### Step 1: Auto-fix Unused Imports (106 errors)
**Impact**: Reduces errors from 872 → 766

Run automated linter to remove unused variables and imports:
```bash
bun biome lint --write .
```

**Files affected**:
- Test files with unused `runFMSubagent`, `Schedule`, `FMService`
- Widget tests with unused socket buffers
- Orchestrator tests with unused imports

**Validation**: Run `bun run build:check` to confirm error count drops

---

### Step 2: Create Database Test Layer Helper (~150 errors)
**Impact**: Reduces errors from 766 → 616

**Critical files**:
- `src/tasks/test-helpers.ts` (new or update existing)

Create a unified test helper that provides `DatabaseService` layer:

```typescript
// src/tasks/test-helpers.ts
import { Effect, Layer } from "effect";
import { BunContext } from "@effect/platform-bun";
import { DatabaseLive } from "../storage/database.js";

export const TestContext = Layer.mergeAll(
  DatabaseLive,
  BunContext.layer
);

export const runWithTestDb = <A, E>(
  program: Effect.Effect<A, E, any>
) => Effect.runPromise(program.pipe(Effect.provide(TestContext)));
```

**Files to update** (add DatabaseService to Effect.provide chains):
- `src/tasks/service.test.ts`
- `src/tasks/repository.test.ts`
- `src/tasks/cli.integration.test.ts`
- All `src/agent/orchestrator/*.test.ts` files
- `src/learning/__tests__/trm-halt.test.ts`

**Pattern**:
```typescript
// Before
Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)))

// After
Effect.runPromise(program.pipe(Effect.provide(TestContext)))
```

---

### Step 3: Update Entry Point Layers (Application roots)
**Impact**: Fixes runtime context provision

**Critical files**:
- `scripts/bootstrap-tasks.ts`
- `src/agent/do-one-task.ts`
- `src/agent/orchestrator/orchestrator.ts`
- `src/agent/overnight.ts`
- `src/agent/overnight-parallel.ts`

Add `DatabaseLive` layer to main application contexts:

```typescript
import { DatabaseLive } from "../storage/database.js";

const MainLayer = Layer.mergeAll(
  DatabaseLive,
  BunContext.layer,
  // ... other layers
);
```

---

### Step 4: Fix TaskService API Calls (~50 errors)
**Impact**: Reduces errors from 616 → 566

**Removed/changed parameters**:
1. **`appendCommits`** → Use update object pattern
2. **`daysOld`** → Changed to optional in different structure
3. **`dryRun`** → Changed to `preview?`
4. **`fromPrefix`** → Removed from rename function
5. **`ids`** → Changed to `sourceIds`

**Files to update**:
- `src/tasks/cli.ts` (43 errors)
- `src/agent/orchestrator/recovery.ts`
- `src/agent/orchestrator/orchestrator.ts`

**Pattern changes**:

```typescript
// BEFORE: appendCommits parameter
updateTask({
  id,
  appendCommits: [sha]
})

// AFTER: commits in update object
updateTask({
  id,
  commits: [...existingCommits, sha]
})

// BEFORE: archiveTasks with daysOld
archiveTasks({
  tasksPath,
  taskIds: ids,
  daysOld: 90
})

// AFTER: new signature
archiveTasks({
  dbPath,
  options: {
    olderThan: { days: 90 }
  }
})
```

---

### Step 5: Fix Return Type Property Access (~76 errors)
**Impact**: Reduces errors from 566 → 490

**Changed properties**:
1. **`tasksPath` → `dbPath`** (init function)
2. **Archive results** → Returns `number` instead of object
3. **`blockers`** → Removed from subagent results
4. **`generate`** → Missing from IFMService (API change)

**Files to update**:
- `src/cli/openagents-init.ts`
- `src/tasks/cli.ts`
- `src/agent/orchestrator/*.ts`

**Pattern changes**:

```typescript
// BEFORE
const { tasksPath } = await init()

// AFTER
const { dbPath } = await init()

// BEFORE
const { archived, remaining } = await archiveTasks(...)
console.log(`Archived ${archived} tasks`)

// AFTER
const archivedCount = await archiveTasks(...)
console.log(`Archived ${archivedCount} tasks`)
```

---

### Step 6: Update Effuse Widget Schemas (~264 errors)
**Impact**: Reduces errors from 490 → 226

**Schema changes affecting widgets**:
1. **`TBSuiteInfoMessage`** → `suiteName` doesn't exist
2. **`TBSuiteInfo`** → `taskCount` doesn't exist
3. **Tool call types** → `id` changed to `tool_call_id`
4. **`SocketService`** → `messages` property missing in mocks

**Files to update**:
- `src/effuse/widgets/tb-controls.test.ts` (57 errors)
- `src/effuse/widgets/tb-output.test.ts` (44 errors)
- `src/effuse/widgets/category-tree.test.ts` (36 errors)
- `src/effuse/widgets/tb-widgets.test.ts` (24 errors)
- `src/effuse/testing/layers/webview.ts` (22 errors)

**Strategy**:
1. Check schema definitions in `src/schemas/` to confirm current property names
2. Update widget code to match actual schema
3. Update test mocks to include required properties
4. Fix tool call ID references (`id` → `tool_call_id`)

---

### Step 7: Fix Orchestrator Tests (~64 errors)
**Impact**: Reduces errors from 226 → 162

**Files to update**:
- `src/agent/orchestrator/orchestrator.test.ts`
- `src/agent/orchestrator/recovery.test.ts` (30 errors)
- `src/agent/orchestrator/golden-loop-smoke.e2e.test.ts`

**Changes needed**:
1. Add `DatabaseService` to test layers (covered in Step 2)
2. Update task update calls (no `appendCommits` - covered in Step 4)
3. Fix property access on results (covered in Step 5)
4. Update mock data structures to match new schemas

---

### Step 8: Add Null Checks for DOM Queries (~43 errors)
**Impact**: Reduces errors from 162 → 119

**Files to update**:
- `src/effuse/widgets/*.test.ts` (widget tests)

**Pattern**:

```typescript
// BEFORE (TS2722: possibly undefined)
const button = dom.querySelector('.expand')
button()

// AFTER (option 1: null check)
const button = dom.querySelector('.expand')
if (button) button()

// AFTER (option 2: non-null assertion in tests)
const button = dom.querySelector('.expand')!
button()
```

**Recommendation**: Use non-null assertions in test code where we control the HTML structure.

---

### Step 9: Fix Remaining Edge Cases (~119 errors)
**Impact**: Reduces errors from 119 → 0

Address remaining errors that don't fit the above patterns:
- Duplicate index signatures (TS2375) - ~36 errors
- Module export issues (TS2305) - ~19 errors
- Computed property names (TS4115) - ~23 errors
- Other type mismatches - ~41 errors

**Strategy**: Review each file individually and apply targeted fixes based on context.

---

## Validation Strategy

After each step, run:
```bash
# Type checking
bun run build:check

# Tests (when errors reduced enough)
bun test

# E2E tests (final validation)
bun run test:e2e
```

## Success Criteria

- ✅ Zero TypeScript errors (`bun run build:check` passes)
- ✅ All existing tests pass (`bun test` passes)
- ✅ E2E tests pass (`bun run test:e2e` passes)
- ✅ No functionality regression
- ✅ Database migration works correctly
- ✅ Task service CRUD operations work

## Risk Mitigation

1. **Work incrementally**: Commit after each successful step
2. **Run tests frequently**: Catch regressions early
3. **Database backups**: `.openagents/tasks.jsonl.backup` exists for rollback
4. **Type safety**: Let TypeScript guide the refactoring
5. **Pattern consistency**: Use same patterns across similar fixes

## Estimated Effort by Step

1. Auto-fix unused imports: 5 minutes (automated)
2. Database test layer: 30 minutes
3. Entry point layers: 20 minutes
4. TaskService API calls: 45 minutes
5. Return type properties: 30 minutes
6. Effuse widget schemas: 90 minutes (largest module)
7. Orchestrator tests: 45 minutes
8. Null checks: 20 minutes
9. Edge cases: 60-120 minutes (depends on complexity)

**Total**: 5-6 hours of focused work

## Notes

- The migration to SQLite is architecturally sound - these are surface-level API compatibility issues
- Most errors are in test files, suggesting production code paths may work
- Creating the `TestContext` helper in Step 2 is the key unlock for ~150 errors
- Effuse widgets (Step 6) are the largest chunk and may reveal schema documentation needs
