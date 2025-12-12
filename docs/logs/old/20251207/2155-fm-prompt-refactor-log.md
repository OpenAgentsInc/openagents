# FM Model Runner Prompt Refactoring

**Date**: 2025-12-07
**Time**: 21:55 CST
**Status**: Complete ✅

## Problem

FM model runner was hitting context limits immediately on first turn, even before any tool calls. Error: "Exceeded model context window size"

**Root Cause**: System prompt + task description was too large for FM's ~1100 char context window.

## Solution

Refactored to use micro-task prompt system per `docs/logs/20251206/1421-coding-thoughts.md`:

### Changes Made

1. **Replaced `buildFMSystemPrompt` with `buildMicroTaskPrompt`**
   - Uses much shorter core prompt (~150 chars vs ~200+ chars)
   - Properly condenses skills (max 300 chars), memories (max 150 chars), reflections (max 100 chars)
   - Ensures system prompt fits within budget

2. **Added task description truncation**
   - Uses `getUserMessageBudget()` to calculate remaining budget after system prompt
   - Truncates task description to fit within remaining budget
   - Logs when truncation occurs

3. **Added context validation**
   - Verifies total initial context (system + user) fits within FM_CONTEXT_BUDGET (900 chars)
   - Further truncates user message if needed
   - Logs warnings when context exceeds budget

4. **Removed unused `FM_BASE_PROMPT` constant**
   - Now handled entirely by `buildMicroTaskPrompt()` in `src/fm/micro-task.ts`

## Files Modified

1. `src/bench/model-adapter.ts`
   - Refactored `buildFMSystemPrompt()` to use `buildMicroTaskPrompt()`
   - Added task description truncation using micro-task utilities
   - Added context size validation and logging
   - Removed unused `FM_BASE_PROMPT` constant

## Expected Impact

- ✅ Initial prompts now fit within FM context budget
- ✅ Task descriptions are truncated if too long
- ✅ Better logging of context usage
- ✅ Follows micro-task philosophy from coding thoughts document

## Testing

- ✅ All 41 tests passing
- ✅ No linting errors
- ✅ Context validation logic tested

## Next Steps

1. Test with actual TB run to verify context limits are respected
2. Monitor context usage logs to ensure prompts stay within budget
3. Consider further optimizations if needed

---

**Status**: Ready for testing ✅

