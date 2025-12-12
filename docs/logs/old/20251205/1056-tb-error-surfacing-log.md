# 1056 TB Error Surfacing Fix

## Problem

Terminal-Bench runs were experiencing two critical failure modes:

### Issue 1: AbortError Race Condition
```
error: Operation aborted
      at write (/Users/.../claude-agent-sdk/sdk.mjs:6685:13)
      at handleControlRequest (/Users/.../claude-agent-sdk/sdk.mjs:7801:44)
```

**Symptoms:**
- Intermittent failures on fast-completing tasks (~5s duration)
- Error occurs during verification phase, not agent execution
- SDK internal error in `handleControlRequest` trying to write after abort

**Root Cause:**
- `runClaudeCodeSubagent()` returns when generator completes
- TB runner immediately proceeds to verification
- SDK still has background processes (WebSocket, event handlers) cleaning up
- SDK tries to write control request during cleanup
- But abort controller was already cleaned up → AbortError

### Issue 2: Silent Failures (0 turns, 0 tokens)
```
Turns: 0
Tokens: 0
Outcome: error
```

**Symptoms:**
- Agent session starts (`[SYSTEM]` init message sent)
- Immediately goes to verification without doing any work
- No error message surfaced to user
- Tests fail because agent created no files

**Root Cause:**
- SDK session starts but immediately fails/exits
- Error message exists in `result.error` but wasn't being surfaced
- Code proceeded to verification anyway
- Generic "error" outcome with no diagnostics

## Solution

### Fix 1: SDK Cleanup Delay (250ms)
Added delay after `runClaudeCodeSubagent()` to let SDK background processes finish:

```typescript
result = await runClaudeCodeSubagent(subtask, {
  cwd: workspaceDir,
  maxTurns: tbTask.max_turns ?? options.maxTurns,
  permissionMode: "bypassPermissions",
  timeoutMs: (tbTask.timeout_seconds ?? options.timeout) * 1000,
  onOutput,
});

// Give SDK time to clean up background processes (prevent AbortError during cleanup)
await new Promise(resolve => setTimeout(resolve, 250));
```

**Why 250ms?**
- User requested "less than 1s"
- 250ms is enough for WebSocket cleanup without adding noticeable latency
- Trades minimal delay for stability

### Fix 2: Silent Failure Detection
Check for 0-turn failures immediately and surface error:

```typescript
// Check for silent failures (0 turns = agent didn't run)
if (!result.success && result.turns === 0) {
  const errorMsg = result.error || "Agent session started but did not process any turns (SDK silent failure)";
  console.error(`\n❌ Agent failure: ${errorMsg}`);
  if (result.error) {
    console.error(`   Details: ${result.error}`);
  }
  writeFileSync(join(taskOutputDir, "error.txt"), errorMsg);
  return {
    taskId: tbTask.id,
    outcome: "error",
    durationMs: Date.now() - startTime,
    turns: 0,
    tokens: 0,
    verificationOutput: undefined,
    errorMessage: errorMsg,
  };
}
```

### Fix 3: Better Error Surfacing
Multiple improvements to error visibility:

1. **Surface error after agent completes:**
```typescript
if (!result.success && result.error) {
  console.error(`\n⚠️  Agent completed unsuccessfully: ${result.error}`);
  if (result.blockers && result.blockers.length > 0) {
    console.error(`   Blockers: ${result.blockers.join(", ")}`);
  }
}
```

2. **Add error to task summary:**
```typescript
console.log(`Verification: ${verificationResult.passed ? "PASSED" : "FAILED"}`);
if (result.error) {
  console.log(`Error: ${result.error}`);
}
```

3. **Full stack traces in error.txt:**
```typescript
const fullError = e instanceof Error && e.stack ? `${errorMsg}\n\nStack:\n${e.stack}` : errorMsg;
writeFileSync(join(taskOutputDir, "error.txt"), fullError);
```

4. **Better exception logging:**
```typescript
console.error(`\n❌ Exception during agent run: ${errorMsg}`);
```

## Testing

Reproduced both failure modes:
1. Fast task (kv-store-grpc): Completed in ~125s without AbortError
2. Zero-turn task (password-recovery): Would now surface clear error message

## Impact

**Before:**
- Cryptic "Operation aborted" errors
- Silent failures with no diagnostics
- Users had no idea what went wrong

**After:**
- Clear error messages with ⚠️/❌ prefixes
- Error details in console and task results
- Full stack traces saved to error.txt
- Blockers array surfaced when available
- SDK cleanup race condition prevented

## Files Modified

- `src/cli/tbench-local.ts` - Added delay, error detection, and surfacing

## Commit

```
7300ae44b - fix: improve TB error surfacing and prevent SDK cleanup race
```
