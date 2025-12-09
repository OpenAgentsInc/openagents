# Guided Tool Calls + IPv4 Hints Implementation

**Time:** 15:48 CT
**Date:** 2025-12-09
**Status:** ✅ Complete

---

## Summary

Implemented both Phase 1 (IPv4 hints) and Phase 2 (guided tool calls) from the plan.

---

## Phase 1: IPv4 Domain Knowledge Hints ✅

**File:** `src/hillclimber/decomposer.ts`

**Changes:**
- Added IPv4 domain knowledge hints to Subtask 1
- Explicitly states IPv4 uses DOT notation (not dashes)
- Provides example: `192.168.1.1`
- Shows pattern: `\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}`

**Rationale:** This is legitimate domain knowledge (what IPv4 IS), not the solution (how to combine it).

---

## Phase 2: Guided Tool Calls ✅

### Step 1: Swift Schema Added

**File:** `swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift`

**Added:**
- `ToolCallRequest` struct with `@Generable` macro
- `ToolArguments` struct for arguments
- `@Guide` constraint on `name` field: `.anyOf(["read_file", "write_file", "verify_progress"])`

**Result:** FM can ONLY output valid tool names.

### Step 2: Handler Case Added

**File:** `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift`

**Added:**
- Case `"tool_call"` in `handleGuidedGeneration`
- Uses `ToolCallRequest.self` for constrained generation

### Step 3: Bridge Rebuilt

```bash
cd swift/foundation-bridge && swift build
cp .build/debug/foundation-bridge ../../bin/
```

**Result:** ✅ Build successful, binary copied.

### Step 4: MAP Orchestrator Updated

**File:** `src/hillclimber/map-orchestrator.ts`

**Changes:**
1. Added `responseFormat` to FM chat call:
   ```typescript
   responseFormat: {
     type: "json_schema",
     schema_type: "tool_call",
   }
   ```

2. Updated parsing logic to handle guided generation JSON:
   - Tries parsing as direct JSON first (guided generation format)
   - Falls back to `parseToolCalls` for backward compatibility

**Result:** FM responses are guaranteed to have valid tool names.

---

## Expected Results

### Before:
- FM could call `edit_file` (doesn't exist) → wasted turns
- FM wrote wrong IPv4 pattern: `\d{1,3}-\d{2}-\d{4}` (dashes)

### After:
- FM can ONLY call `read_file`, `write_file`, or `verify_progress` → no wasted turns
- FM knows IPv4 uses dots: `\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}`

---

## Testing

**Next step:** Run test to verify both fixes work:

```bash
bun scripts/test-progress-fix.ts --standard
```

**Expected:**
- No `edit_file` errors
- FM writes correct IPv4 pattern in lookahead
- Progress climbs toward 100%

---

## Files Modified

1. `src/hillclimber/decomposer.ts` - IPv4 hints
2. `swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift` - ToolCallRequest schema
3. `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift` - tool_call handler
4. `src/hillclimber/map-orchestrator.ts` - guided generation + parsing
5. `bin/foundation-bridge` - rebuilt binary

---

**Status:** ✅ Implementation complete, ready for testing
