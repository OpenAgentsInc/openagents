# TypeScript Error Fix Plan - December 7, 2025

## Summary

**Initial Errors: 618** → **Current Errors: 479** (22.5% fixed)

Run `bun run typecheck` to see all errors.

**Batch Status:**
- Batch 1 (TB Widget Tests): **COMPLETED** (57→27→7 errors left)

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

## Agent Batch Assignments

Work is divided into 7 batches. Each agent should work on ONE batch only to avoid conflicts.

---

## Batch 1: Effuse Widget Tests - TB Group (~170 errors)

**Agent Assignment:** Agent 1
**Est. Time:** 2-3 hours
**Files:**
- `src/effuse/widgets/tb-controls.test.ts` (57 errors)
- `src/effuse/widgets/tb-output.test.ts` (43 errors)
- `src/effuse/widgets/category-tree.test.ts` (41 errors)
- `src/effuse/widgets/tb-widgets.test.ts` (27 errors)

**Common Error Patterns:**
- TS2345: StateCell type mismatch - mock state doesn't match actual state type
- TS2722: Possibly undefined invocation (add `?.` operator)
- TS2353: Unknown properties in object literals (check schema updates)
- TS2379: exactOptionalPropertyTypes (use conditional spread)
- TS2349: Expression not callable (likely calling `.get()` result instead of awaiting)
- TS2740: Missing properties from type (add required fields to mock state)

**Fix Pattern for StateCell mocks:**
```typescript
// WRONG - selectedTaskId: null is too narrow
const state = StateCell.make({
  selectedTaskId: null,  // Error: should be string | null
});

// CORRECT - use type annotation
const state = StateCell.make<CategoryTreeState>({
  selectedTaskId: null,
});
```

---

## Batch 2: Effuse Widget Tests - MC/Trajectory/Other (~70 errors)

**Agent Assignment:** Agent 2
**Est. Time:** 1.5-2 hours
**Files:**
- `src/effuse/widgets/mc-tasks.test.ts` (22 errors)
- `src/effuse/widgets/tb-results.test.ts` (18 errors)
- `src/effuse/widgets/trajectory-pane.test.ts` (12 errors)
- `src/effuse/widgets/hf-trajectory-detail.test.ts` (10 errors)
- `src/effuse/widgets/container-panes.test.ts` (8 errors)

**Common Error Patterns:**
- Same StateCell type mismatches as Batch 1
- Labels array typed as `never[]` instead of `string[]`
- TS2722: Add optional chaining for possibly undefined callbacks

---

## Batch 3: Learning Module (~99 errors)

**Agent Assignment:** Agent 3
**Est. Time:** 2-3 hours
**Files:**
- `src/learning/__tests__/trm-halt.test.ts` (60 errors)
- `src/learning/__tests__/trm-state.test.ts` (18 errors)
- `src/learning/__tests__/trm-integration.test.ts` (10 errors)
- `src/learning/orchestrator.ts` (11 errors)
- `src/learning/loop.ts` (3 errors)

**Common Error Patterns:**
- TS2304: Cannot find name (35 total) - missing imports or schema changes
- TS4115: Override modifier needed
- TS2322/TS2345: Type assignment mismatches

**Likely Root Cause:** Schema changes in learning types. Check `src/learning/schema.ts` for current definitions.

---

## Batch 4: Training Module (~20 errors)

**Agent Assignment:** Agent 4
**Est. Time:** 1 hour
**Files:**
- `src/training/episode-learner.test.ts` (9 errors)
- `src/training/loop-runner.ts` (8 errors)
- `src/training/episode-learner.ts` (3 errors)

**Common Error Patterns:**
- TS4115: Missing `override` modifier on Error subclass
- TS2722: Possibly undefined
- TS2379: exactOptionalPropertyTypes

**Note:** Training files interact with learning module - coordinate with Agent 3 if needed.

---

## Batch 5: Tasks & Storage (~38 errors)

**Agent Assignment:** Agent 5
**Est. Time:** 1.5 hours
**Files:**
- `src/tasks/service.ts` (27 errors)
- `src/tasks/cli.ts` (3 errors)
- `src/storage/database.ts` (8 errors)

**Common Error Patterns:**
- TS2540: Cannot assign to read-only property (29 total in database.ts and service.ts)
- TS2353: Unknown properties (closedAt, status)
- TS2300: Duplicate identifier

**Fix Pattern for Read-Only:**
```typescript
// WRONG
task.deps = newDeps;

// CORRECT
const updatedTask = { ...task, deps: newDeps };
// OR use type assertion if you're sure
(task as Mutable<Task>).deps = newDeps;
```

**Special Case - src/tasks/service.ts:53:**
This line has ~21 read-only errors. Look for Object.assign or spread on readonly type. Create a proper update helper.

---

## Batch 6: Skills Module (~31 errors)

**Agent Assignment:** Agent 6
**Est. Time:** 1.5 hours
**Files:**
- `src/skills/library/compositional.ts` (17 errors)
- `src/skills/service.ts` (6 errors)
- `src/skills/schema.ts` (4 errors)
- `src/skills/retrieval.ts` (4 errors)

**Common Error Patterns:**
- TS4115: Override modifier (in compositional.ts)
- TS1029: 'public' modifier must precede
- TS2322/TS2345: Type mismatches

---

## Batch 7: Memory, Reflexion & Misc (~62 errors)

**Agent Assignment:** Agent 7
**Est. Time:** 2 hours
**Files:**
- `src/reflexion/service.ts` (7 errors)
- `src/reflexion/schema.ts` (3 errors)
- `src/memory/service.ts` (4 errors)
- `src/memory/schema.ts` (3 errors)
- `src/effuse/testing/layers/webview.ts` (2 errors)
- `src/effuse/widget/mount.ts` (1 error)
- `src/effuse/widgets/apm-widget.test.ts` (1 error)
- `src/effuse/widgets/atif-details.test.ts` (1 error)
- `src/llm/ollama.ts` (3 errors)
- `src/trainer/service.ts` (3 errors)
- All remaining scattered files (~34 errors)

**Check for remaining errors:**
```bash
bun run typecheck 2>&1 | grep "error TS" | sed 's/(.*//g' | sort | uniq -c | sort -rn
```

---

## Common Fix Patterns Reference

### exactOptionalPropertyTypes (TS2379, TS2375)

```typescript
// WRONG
const obj = { name: "test", description: undefined };

// CORRECT - use conditional spread
const obj = {
  name: "test",
  ...(description ? { description } : {}),
};
```

### Possibly Undefined Invocation (TS2722)

```typescript
// WRONG
callback();

// CORRECT
callback?.();
// OR
if (callback) callback();
```

### Override Modifier (TS4115)

```typescript
// WRONG
class MyError extends Error {
  constructor(readonly message: string) { super(message); }
}

// CORRECT
class MyError extends Error {
  constructor(override readonly message: string) { super(message); }
}
```

### StateCell Type Mismatch (TS2345)

```typescript
// WRONG - TypeScript infers narrow type
const state = StateCell.make({ selectedId: null });

// CORRECT - explicit type annotation
const state = StateCell.make<MyState>({ selectedId: null });
```

---

## Validation Commands

```bash
# Count total errors
bun run typecheck 2>&1 | grep "error TS" | wc -l

# Errors in your batch files
bun run typecheck 2>&1 | grep "src/effuse/widgets/tb-controls"

# All errors sorted by file
bun run typecheck 2>&1 | grep "error TS" | sort

# Error breakdown by type
bun run typecheck 2>&1 | grep "error TS" | sed 's/.*error /error /' | sed 's/:.*/:/' | sort | uniq -c | sort -rn
```

---

## Rules for All Agents

1. **Don't use `any`** - fix the root cause
2. **Don't use `@ts-ignore`** - these hide problems
3. **Run typecheck after each file** to catch regressions
4. **Check type definitions** when stuck (`Cmd+Click` in VS Code)
5. **Commit frequently** - small commits per file
6. **Don't touch files outside your batch** to avoid merge conflicts
7. **Target: 0 errors** in your batch before declaring done
