# HttpClient Service Issue - RESOLVED

**Date**: 2025-06-23 12:36  
**Issue**: "Service not found: @effect/platform/HttpClient"  
**Status**: RESOLVED ✅

## Root Cause
The issue was caused by mixing Effect and Promise paradigms in Psionic. When routes returned Promises, `Effect.runPromise` created isolated execution contexts without server-provided layers.

## Solution Applied

### 1. Full Effect Integration
Refactored Psionic to support Effect-based route handlers natively:
- Updated RouteHandler type to accept Effect returns
- Modified route handler execution to detect and yield Effects
- All API routes now return Effects instead of Promises

### 2. Fixed Effect Detection
Changed from incorrect tag checking to proper API:
```typescript
// WRONG
if (handlerResult._tag === "Effect")

// CORRECT  
if (Effect.isEffect(handlerResult))
```

### 3. Removed Unnecessary Transformations
Removed `.pipe(Effect.orDie)` from HttpServerResponse methods which was corrupting the Effect structure.

## Test Results
All API routes now work correctly:
- ✅ GET /api/config
- ✅ GET /api/test
- ✅ POST /api/conversations
- ✅ POST /api/conversations/:id/messages

## Key Files Modified
- packages/psionic/src/core/app.ts - Core framework changes
- packages/psionic/src/types/index.ts - Type updates
- apps/openagents.com/src/routes/api/*.ts - All API routes converted to Effect

The system is now fully Effect-based as requested.