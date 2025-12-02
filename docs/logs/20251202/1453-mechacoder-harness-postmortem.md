# 1453 MechaCoder Harness Postmortem

## Summary

After multiple attempts to harden MechaCoder's autonomous loop, the system is still unusable for real work due to fundamental issues with the Grok LLM response times and the complexity of the retry/verification logic.

## What We Tried

### Attempt 1: Typecheck/Test Retry Logic
- Added detection of typecheck/test failures from tool results
- Implemented retry loop that re-prompts agent to fix errors
- **Result**: Retry logic never triggered because agent ran checks BEFORE writing files, not after

### Attempt 2: Verification State Tracking
- Added `dirtySinceVerify` state to track if code was edited since last successful verify
- Enforced post-edit verification before accepting TASK_COMPLETED
- Guard against garbage final messages (git status output)
- **Result**: Agent started getting retry prompts, but then LLM calls took forever

### Attempt 3: Event Streaming
- Changed from async Effect-based logging to synchronous `fs.appendFileSync`
- Added `turn_start` event before LLM calls
- Added `onEvent` callback to `agentLoop` for real-time event emission
- **Result**: Events now stream correctly, but revealed the real problem

## The Real Problem

**Grok-4.1-fast is taking 1-2+ minutes per LLM call.** This is not a harness issue.

Looking at the event timestamps:
```
{"type":"turn_start","ts":"2025-12-02T20:48:02.165Z","turn":2}
{"type":"llm_response","ts":"2025-12-02T20:49:00.605Z","turn":2,...}
```

That's **58 seconds** for a single LLM call. With 5-10 turns per task, that's 5-10 minutes just waiting on the LLM.

Combined with:
- Complex verification retry logic adding more turns
- Wall-clock timeouts (15 min) being hit
- Agent sometimes stopping with garbage messages requiring retries

The harness is fundamentally working but the LLM latency makes it impractical.

## What Actually Works

1. **Event streaming is now real-time** - `tail -f` shows events as they happen
2. **Verification state tracking works** - unverified edits trigger retry prompts
3. **Garbage message detection works** - git status output triggers retry
4. **Synchronous logging works** - no more buffering delays

## What Doesn't Work

1. **Grok-4.1-fast is too slow** - 1+ minute per LLM call
2. **Too many retry loops** - verification enforcement adds complexity
3. **No per-LLM-call timeout** - if Grok hangs, we just wait

## Files Changed Today

- `src/agent/loop.ts` - Added VerifyState, onEvent callback, event emissions during loop
- `src/agent/do-one-task.ts` - Verification enforcement, garbage message detection, sync logging
- `src/agent/runLog.ts` - Sync appendRunEventSync, new event types

## Recommendation

1. **Stop using MechaCoder for now** - It's not ready
2. **Use Claude Code directly** for HUD tasks - Much faster, interactive
3. **Consider switching LLM** - Grok is the bottleneck
4. **Simplify the harness** - Remove verification enforcement until LLM is faster

## Commits from Today's Session

- `d43a1a4d` - Harden MechaCoder loop: verification state tracking + event streaming
- `02ee7a52` - oa-91a779: HUD-3 Flow connection path builder
- `7532c9f3` - Close HUD-3 and test doc task
- `d9956000` - Fix run logging: synchronous writes + turn_start + wall-clock timeout
- `f235644c` - Fix event streaming: emit events DURING agentLoop, not after

## Time Spent

~3 hours debugging and hardening MechaCoder, only to find the LLM is the bottleneck.
