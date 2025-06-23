# Effect Detection Fix

**Date**: 2025-06-23 12:05  
**Issue**: API routes still returning 500 errors with empty responses  
**Status**: INVESTIGATING  

## Problem

Even after removing Effect.orDie, API routes still return 500 errors. The frontend gets empty responses that can't be parsed as JSON.

## Root Cause

The Effect detection in Psionic was incorrect:
```typescript
// WRONG - Effects don't have _tag === "Effect"
if (handlerResult && typeof handlerResult === "object" && "_tag" in handlerResult && handlerResult._tag === "Effect") {
  return yield* handlerResult
}
```

## Fix Applied

Use the proper Effect.isEffect() function:
```typescript
// CORRECT
if (Effect.isEffect(handlerResult)) {
  return yield* handlerResult
}
```

This should now properly detect when route handlers return Effects and yield them correctly within the Effect context.