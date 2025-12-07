# TypeScript Error Fix Plan - December 7, 2025 (Updated)

## Summary

**Initial Errors: 834** → **Previous: 376** → **Current Errors: 105** (87% fixed, 729 errors resolved)

Run `bun run typecheck` to see all errors.

**Session Progress:**
- **Commit d660265fe**: Fixed 93 errors (834→741) - Database layer infrastructure
- **Commit 1b4ae2302**: Fixed 365 errors (741→376) - Effuse, TaskService API updates
- **Commit [Latest]**: Fixed 271 errors (376→105) - Major cleanup in Learning, Tasks, and TB Widgets

**Key Fixes Completed:**
- ✅ `src/tasks/service.ts` (27 errors) - **CLEARED**
- ✅ `src/learning/__tests__/trm-halt.test.ts` (60 errors) - **CLEARED**
- ✅ `src/effuse/widgets/tb-controls.test.ts` (57 errors) - **CLEARED**
- ✅ `src/effuse/widgets/tb-output.test.ts` (43 errors) - **CLEARED**
- ✅ `src/storage/database.ts` (8 errors) - **CLEARED**

## Files with Most Errors (Top 10)

| File | Errors |
|------|--------|
| `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts` | 15 |
| `src/effuse/widgets/trajectory-pane.test.ts` | 12 |
| `src/effuse/widgets/hf-trajectory-detail.test.ts` | 10 |
| `src/effuse/widgets/category-tree.test.ts` | 8 |
| `src/effuse/widgets/container-panes.test.ts` | 8 |
| `src/learning/orchestrator.ts` | 7 |
| `src/effuse/widgets/hf-trajectory-browser.e2e.test.ts` | 3 |
| `src/llm/ollama.ts` | 3 |
| `src/memory/schema.ts` | 3 |
| `src/trainer/service.ts` | 3 |

---

## Agent Batch Assignments (Revised)

Work is divided into 3 focused batches to clear the remaining 105 errors.

---

## Batch 1: Effuse Widgets Final Polish (~58 errors)

**Agent Assignment:** Agent 1
**Focus:** Complex widget state and test interactions.

**Files:**
- `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts` (15 errors)
  - *Note:* Likely missing properties in state objects or event handlers.
- `src/effuse/widgets/trajectory-pane.test.ts` (12 errors)
- `src/effuse/widgets/hf-trajectory-detail.test.ts` (10 errors)
- `src/effuse/widgets/category-tree.test.ts` (8 errors)
- `src/effuse/widgets/container-panes.test.ts` (8 errors)
- `src/effuse/widgets/hf-trajectory-browser.e2e.test.ts` (3 errors)
- `src/effuse/widgets/hf-trajectory-list.test.ts` (2 errors)

**Common Issues:**
- `StateCell` type mismatches (use generic `StateCell.make<Type>({...})`)
- Missing props in mock objects
- `exactOptionalPropertyTypes` violations

---

## Batch 2: Learning & Skills Cleanup (~20 errors)

**Agent Assignment:** Agent 2
**Focus:** Logic and schema strictness in core modules.

**Files:**
- `src/learning/orchestrator.ts` (7 errors)
- `src/learning/__tests__/ttt-integration.test.ts` (2 errors)
- `src/skills/evolution.test.ts` (2 errors) - Read-only property assignment
- `src/skills/evolution.ts` (1 error) - Missing `override` modifier
- `src/skills/library/compositional.ts` (1 error) - Missing `description` property
- `src/skills/library/index.ts` (2 errors) - `undefined` index type
- `src/skills/schema.test.ts` (2 errors) - Enum/Union type mismatch

---

## Batch 3: Infrastructure & Misc (~27 errors)

**Agent Assignment:** Agent 3
**Focus:** System services, types, and scattered fixes.

**Files:**
- `src/tasks/test-helpers.ts` (2 errors)
  - *Critical:* `Layer.provide` argument mismatch (Layer vs Tag)
- `src/researcher/tasks.ts` (2 errors)
  - *Issue:* `exactOptionalPropertyTypes` with `Effect.provide`
- `src/memory/schema.ts` (3 errors)
- `src/llm/ollama.ts` (3 errors)
- `src/trainer/service.ts` (3 errors) - Unused vars & `exactOptionalPropertyTypes`
- `src/fm/service.ts` (2 errors)
- `src/huggingface/openthoughts.ts` (2 errors)
- `src/tasks/repository.test.ts` (1 error) - Extra property `author` in `addComment`
- `test-tb-real.ts` (1 error) - `error` is `unknown`

---

## Specific Fix Instructions

### 1. `src/tasks/test-helpers.ts` Layer Issue
The error `Argument of type 'Tag<FileSystem, FileSystem>' is not assignable to parameter of type 'Layer<never, any, any>'` suggests a raw Tag is being passed where a Layer is expected.
**Fix:** Use `FileSystem.FileSystem.Default` or `Layer.succeed(FileSystem.FileSystem, ...)` depending on context.

### 2. `exactOptionalPropertyTypes` in `Effect.provide`
Errors like `Type 'DatabaseService' is not assignable to type 'never'` often happen when `Effect.provide` is used with a context that has optional services but strict checking is on.
**Fix:** Ensure the provided layer matches the expected requirements exactly, or use `Effect.provideService` for individual services.

### 3. Read-only Properties in Tests
Errors like `Cannot assign to 'successRate' because it is a read-only property`.
**Fix:**
```typescript
// WRONG
skill.successRate = 0.5;

// CORRECT (Create new object)
const updatedSkill = { ...skill, successRate: 0.5 };
// OR (Cast if strictly necessary for test setup)
(skill as Mutable<Skill>).successRate = 0.5;
```

### 4. `src/skills/library/index.ts` Undefined Index
`bySource[skill.source] = ...` where `skill.source` might be undefined.
**Fix:** Ensure `skill.source` is defined or provide a default key.

```typescript
const key = skill.source ?? 'unknown';
bySource[key] = (bySource[key] ?? 0) + 1;
```
