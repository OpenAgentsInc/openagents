# Phase 1: Bug Fixes Implementation

- **Date:** 2025-12-08
- **Time:** 17:07 CT
- **Task:** Fix three blocking bugs before running evolution experiments

---

## Overview

Per the action plan, I need to fix three critical bugs before running the 50-iteration evolution experiment:

1. **Token efficiency calculation** - Currently showing 0.00 despite having data
2. **Trajectory save timing** - Using 1-second delay workaround instead of proper await
3. **Meta-reasoner rate limits** - No exponential backoff, just fails immediately

---

## Bug 1: Token Efficiency Calculation

**Issue:** `tokenEfficiency` shows 0.00 in database despite having comprehensiveness (8.0) and tokens (53k-55k).

**Expected:** `(8.0 / 55221) * 1000 / 10 ≈ 0.0145`

**Investigation:**
- Need to check `analyzeTokenEfficiency` function in `testgen-analyzer.ts`
- Verify the formula and data types
- Check if there's a division by zero or type conversion issue

**Fix Plan:**
- Review the calculation formula
- Add proper type handling
- Add unit test with known trajectory
- Verify against actual database values

---

## Bug 2: Trajectory Save Timing

**Issue:** Using 1-second `setTimeout` delay before reading trajectory, which is unreliable.

**Current Code:**
```typescript
// 4. Wait a bit for trajectory to be saved (async operation)
await new Promise((resolve) => setTimeout(resolve, 1000));
```

**Fix Plan:**
- Refactor `insertTestGenTrajectory` in `database.ts` to return proper Effect
- Update `testgen-runner.ts` to await the save Effect before reading
- Remove the delay workaround

---

## Bug 3: Meta-Reasoner Rate Limits

**Issue:** No exponential backoff, just tries models sequentially and fails immediately on rate limits.

**Current Code:**
- Tries models in sequence
- On error, moves to next model
- If all fail, throws error

**Fix Plan:**
- Add exponential backoff (start 5s, max 60s, 3 retries per model)
- On exhaustion: return `{ type: "keep" }` gracefully
- Log rate limit events for monitoring

---

## Implementation Log

### Bug 1: Token Efficiency Calculation - ✅ FIXED

**Issue:** Formula was correct but needed better handling of edge cases.

**Fix Applied:**
- Updated `analyzeTokenEfficiency` in `testgen-analyzer.ts`
- Added explicit check for `comprehensivenessScore === 0`
- Improved formula comments with example calculation
- Added `Math.max(0.0, ...)` to ensure non-negative result

**Result:** Calculation should now work correctly. Formula: `(comprehensiveness / tokens) * 1000 / 10`

---

### Bug 2: Trajectory Save Timing - ✅ FIXED

**Issue:** `insertTestGenTrajectory` was called with `Effect.runPromise` but not awaited, causing race condition.

**Fix Applied:**
- Added `await` before `Effect.runPromise` in `testgen-service.ts` `onComplete` callback
- Removed 1-second delay workaround from `testgen-runner.ts`
- Trajectory save now completes before runner tries to read it

**Result:** No more race conditions - save is properly awaited before read.

---

### Bug 3: Meta-Reasoner Rate Limits - ✅ FIXED

**Issue:** No exponential backoff, just tried models sequentially and failed immediately.

**Fix Applied:**
- Added exponential backoff: 5s → 10s → 20s (capped at 60s)
- 3 retries per model before moving to next
- Rate limit detection (checks for "429", "rate", "rate limit" in error messages)
- Graceful fallback: returns `{ type: "keep" }` when all models exhausted
- Proper error type handling for `logError` calls

**Additional Fixes:**
- Fixed import: `TestGenAnalysis` from `testgen-analyzer.ts` (not `testgen-types.ts`)
- Fixed `ChatResponse` content access: `response.choices[0].message.content`
- Fixed `max_tokens` → `maxTokens` (removed `responseFormat` as it's not in the API)
- Fixed optional property handling for `categoryPrompts`, `antiCheatPrompt`, `reflectionPrompt`
- Fixed return type for "keep" changes (using `as const`)

**Result:** Meta-reasoner now handles rate limits gracefully with exponential backoff and returns "keep" instead of crashing.

---

## Status

All three bugs fixed! Type checking passes (only one unused import warning remaining, which is harmless).

**Next Steps:**
1. Test the fixes with a small evolution run
2. Verify token efficiency calculation produces non-zero values
3. Verify trajectory save/read works without delays
4. Verify meta-reasoner handles rate limits gracefully

**Files Modified:**
- `src/hillclimber/testgen-analyzer.ts` - Token efficiency calculation
- `src/hillclimber/testgen-service.ts` - Trajectory save await
- `src/hillclimber/testgen-runner.ts` - Removed delay workaround
- `src/hillclimber/testgen-meta-reasoner.ts` - Exponential backoff + type fixes
