# 1144 Terminal-Bench Output Streaming Implementation

## Context

Following the authentication fix verified in `1118-tb-auth-verification-log.md`, user identified that while TB runs now authenticate and execute successfully, **all output is buffered until the subprocess completes**. This makes debugging difficult and provides poor UX.

User request: "just nothing flushed to console until the end??? good to know, i still want sandbox but lets fix this other issue too so both paths work"

## Problem

When TB subprocess uses `stdout: "pipe"` (required for SDK authentication), output buffers in memory until the process exits. This is because:
1. Pipes capture output into a buffer
2. Without active readers, the buffer fills but doesn't flush to console
3. Only when subprocess exits does Bun flush the buffer

## Solution: Phase 1 Output Streaming

Implemented async stream readers for both stdout and stderr that forward output to the desktop server console in real-time.

### Changes Made

**File**: `src/desktop/handlers.ts:161-195`

Added two async IIFE functions that run concurrently after spawning:

1. **stdout reader** (lines 161-177):
   - Gets reader from `activeTBRun.stdout`
   - Reads chunks asynchronously in a loop
   - Decodes with `TextDecoder` (stream mode)
   - Writes to `process.stdout.write()` for immediate visibility

2. **stderr reader** (lines 179-195):
   - Same pattern for stderr stream
   - Writes to `process.stderr.write()`

Both readers:
- Run in background (not awaited)
- Catch errors gracefully (subprocess kill/cancel is normal)
- Continue until subprocess closes streams

### Implementation Details

```typescript
// Stream stdout asynchronously for real-time output
(async () => {
  try {
    const reader = activeTBRun!.stdout.getReader();
    const decoder = new TextDecoder();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const text = decoder.decode(value, { stream: true });
      process.stdout.write(text); // Stream to desktop server console
    }
  } catch (err) {
    // Reader cancelled or subprocess killed - this is normal
  }
})();
```

**Why `process.stdout.write()` instead of `console.log()`**:
- Preserves exact formatting (no added newlines)
- No timestamp prefix that console.log adds
- More faithful reproduction of subprocess output

**Why `{ stream: true }` in decode**:
- Handles partial UTF-8 sequences across chunks
- Prevents mangled unicode characters

## Testing Plan

1. Start desktop server: `bun dev`
2. Click "Random" in TB controls
3. Observe console output **as agent executes**, not after completion
4. Verify:
   - SDK init messages appear immediately
   - Agent turn output streams in real-time
   - No long pauses with no output

## Next Steps (Phase 2)

Container/sandbox integration as outlined in `/Users/christopherdavid/.claude/plans/ancient-herding-frog.md`:
- Add `--sandbox` flag to TB CLI
- Integrate with `src/sandbox/` infrastructure (Docker/macOS containers)
- Mount credentials using `createCredentialMount()`
- SDK runs on HOST, setup/verification in container

## Related Files

- `src/desktop/handlers.ts` - Modified
- `.claude/plans/ancient-herding-frog.md` - Full implementation plan
- `docs/logs/20251205/1118-tb-auth-verification-log.md` - Previous auth fix

## Validation

✅ **Tested and Verified!**

Test script: `test-streaming.ts` (deleted after test)
Task: regex-log (medium difficulty, data-processing)

Results:
- ✅ Real-time output streams to console as agent executes
- ✅ SDK init messages appear immediately
- ✅ Stream events appear as they arrive
- ✅ No buffering - all output visible during execution

Output sample:
```
Loading suite from .../tasks/terminal-bench-2.json...
Loaded suite: Terminal-Bench 2.0 v2.0.0
=== Starting Terminal-Bench Run ===
=== Running Task: regex-log ===
[SDK] About to call query() with cwd: ...
[SDK] Received message type: system subtype: init
[SDK] Received message type: stream_event subtype: undefined
...
```

All messages appeared in real-time, not after subprocess completion.
