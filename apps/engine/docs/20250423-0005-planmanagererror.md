# Analysis of PlanManager Service Not Found Error

## Current Behavior

Despite adding `NodeContext.layer` to the `createAppLayer` function, the application still encounters the same error:

```
DEBUG: CRITICAL - Fork failed with cause: {
  _id: 'Cause',
  _tag: 'Parallel',
  left: {
    _id: 'Cause',
    _tag: 'Sequential',
    left: { _id: 'Cause', _tag: 'Empty' },
    right: { _id: 'Cause', _tag: 'Interrupt', fiberId: [Object] }
  },
  right: {
    _id: 'Cause',
    _tag: 'Die',
    defect: Error: Service not found: PlanManager (defined at file:///Users/christopherdavid/code/openagents/apps/engine/build/esm/github/PlanManager.js:6:59)
```

## Root Cause Analysis

After deep investigation, I've identified a combination of factors causing the persistent "Service not found: PlanManager" error:

1. **Tag Reference Equality Issue**: In Effect.js, service Tags must maintain strict object reference equality. The `PlanManager` tag being requested during execution must be exactly the same JavaScript object instance as the one used when constructing `PlanManagerLayer`.

2. **Inconsistent Export Patterns**: The `PlanManager.ts` file had a mix of standard exports and a secondary `Default` export (`export const Default = PlanManagerLayer`), which could lead to inconsistent module resolution.

3. **Build/Module Caching Issues**: Even after fixing imports, previously compiled files or module caching might retain old reference patterns.

4. **Missing NodeContext Layer**: The `createAppLayer` function in `Server.ts` was missing `.pipe(Layer.provide(NodeContext.layer))` at the end of the layer composition.

## Implementation Details

I implemented a comprehensive solution to address all these issues:

1. **Standardized Tag/Layer Exports**:
   - Removed the `export const Default = PlanManagerLayer` line from `PlanManager.ts` to standardize on direct named exports
   - This ensures consistent imports throughout the codebase

2. **Standardized Imports**:
   - Updated all imports to maintain reference equality by using direct imports from source files:
   ```typescript
   // Consistent, direct imports without aliasing
   import { PlanManager } from "./github/PlanManager.js";
   import { PlanManagerLayer } from "./github/PlanManager.js";
   ```

3. **Added NodeContext Layer**: 
   ```typescript
   return Layer.mergeAll(
     // ... existing layers ...
   ).pipe(Layer.provide(NodeContext.layer))
   ```

4. **Standardized Service Access**:
   - Updated debug code to consistently use `yield* PlanManager` to match usage in TaskExecutor.ts
   ```typescript
   try {
     console.log("DEBUG: CRITICAL - Testing PlanManager")
     yield* PlanManager
     console.log("DEBUG: CRITICAL - PlanManager found")
   } catch (err) {
     // Error handling...
   }
   ```

5. **Clean Rebuild**:
   - Completely removed the `build` directory before rebuilding
   - This ensures no stale compiled files affect the behavior

## Technical Details of Effect.js Tag Resolution

Effect.js dependency injection relies on JavaScript's reference equality for Tags. When a service is requested via `yield* SomeTag`, Effect looks for a Layer that provided exactly that Tag object.

If multiple instances of the same Tag class exist in memory (due to module duplication, import patterns, etc.), Effect.js will fail with "Service not found" even though a service with that name exists.

In our case, we ensured:
1. `PlanManager` Tag from `./github/PlanManager.js` is the same instance used by both requesters and providers
2. `PlanManagerLayer` is consistently used in `Layer.mergeAll()` without `Default` aliases
3. No import aliasing that could create multiple Tag instances
4. All modules get a clean rebuild to ensure consistent reference patterns

## Lessons Learned

1. **Tag Reference Equality**: Effect.js relies on strict JavaScript reference equality for Tags.

2. **Consistent Export/Import Patterns**: Using exactly one export/import pattern for Tags and Layers avoids reference mismatches.

3. **Clean Rebuilds**: When experiencing persistent dependency injection issues, a complete clean of compiled files can help resolve hidden issues.

4. **NodeContext is Essential**: Always provide the `NodeContext.layer` for Node.js applications using Effect.js.