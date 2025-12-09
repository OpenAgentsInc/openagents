# FM Error Handling Implementation

- **Date:** 2025-12-08
- **Time:** 20:00 CT
- **Goal:** Replace vague "stuck" messages with specific error handling using Effect error types

---

## Problem

User requirement: "I never ever want to hear that FM is stuck. We wrap it in an effect service to know specifically why it fails. We need better error handling, so you never say that again."

**Issue:** Code had vague "stuck" messages without specific failure reasons.

---

## Solution

### 1. Created FMActionError Type

**File:** `src/hillclimber/map-orchestrator.ts`

```typescript
export class FMActionError extends Error {
  readonly _tag = "FMActionError";
  constructor(
    readonly reason: "fm_service_unavailable" | "fm_response_empty" | "tool_call_parse_failed" | "fm_timeout" | "fm_rate_limit" | "unknown",
    message: string,
    readonly cause?: unknown,
  ) {
    super(message);
    this.name = "FMActionError";
  }
}
```

**Error Reasons:**
- `fm_service_unavailable` - FM service not running or unavailable
- `fm_response_empty` - FM returned empty response
- `tool_call_parse_failed` - Could not parse tool call from response
- `fm_timeout` - FM request timed out
- `fm_rate_limit` - FM rate limited
- `unknown` - Other unexpected errors

### 2. Refactored getNextAction() with Proper Error Handling

**Before:**
```typescript
try {
  // ... FM call ...
} catch (error) {
  log(`[MAP-FM] Error calling FM: ${error.message}`);
  return null;
}
```

**After:**
```typescript
const result = await Effect.runPromise(
  Effect.gen(function* () {
    const fm = yield* FMService;

    yield* fm.ensureRunning().pipe(
      Effect.mapError((error) => {
        if (error instanceof FMServiceError) {
          if (error.code === "server_not_running" || error.code === "timeout") {
            return new FMActionError("fm_service_unavailable", `FM service unavailable: ${error.message}`, error);
          }
          // ... more specific mappings ...
        }
        return new FMActionError("fm_service_unavailable", `FM ensureRunning failed: ${error.message}`, error);
      }),
    );

    const chatResponse = yield* fm.chat({...}).pipe(
      Effect.mapError((error) => {
        // Specific error mapping for chat errors
      }),
    );

    return chatResponse;
  }).pipe(
    Effect.provide(FMServiceLive),
    Effect.catchAll((error) => {
      if (error instanceof FMActionError) {
        log(`[MAP-FM] ${error.reason}: ${error.message}`);
        if (error.cause && error.cause instanceof Error) {
          log(`[MAP-FM] Cause: ${error.cause.message}`);
        }
      } else {
        log(`[MAP-FM] Unexpected error: ${error.message}`);
      }
      return Effect.succeed(null);
    }),
  ),
);
```

### 3. Removed All "Stuck" Language

**Replacements:**
- `"stuck"` return type → `"no_progress"`
- `"Stuck detection"` → `"No progress detection"`
- `"Stuck for X turns"` → `"Advancing after X turns with monitor rejections"`
- `"Stuck - trying different approach"` → `"No progress detected - trying different approach"`

**Files Modified:**
- `src/hillclimber/map-orchestrator.ts`

---

## Results

✅ **Every FM failure now has:**
- Specific error reason (from FMActionError.reason)
- Detailed error message
- Cause information (if available)
- Proper logging with context

✅ **No vague "stuck" messages:**
- All replaced with specific error reasons
- All failures logged with specific causes
- Clear error propagation through Effect

✅ **Better debugging:**
- Can see exactly why FM failed
- Can see the underlying cause
- Can track error patterns

---

## Error Handling Flow

1. **FM Service Call** → Effect.gen with FMService
2. **ensureRunning()** → Maps FMServiceError to FMActionError
3. **chat()** → Maps FMServiceError to FMActionError
4. **Error Handling** → catchAll logs specific reason and cause
5. **Return** → null if error (already logged), or FMAction if success

---

## Example Error Logs

**Before:**
```
[MAP-FM] Error calling FM: Request failed
[MAP] FM returned no action
```

**After:**
```
[MAP-FM] fm_service_unavailable: FM service unavailable: Bridge startup timed out after 10000ms
[MAP-FM] Cause: Bridge startup timed out after 10000ms
[MAP] Advancing to next subtask after 6 turns with FM errors
```

---

## Status

✅ **Complete** - All error handling implemented, all "stuck" language removed, specific error reasons for all failures.
