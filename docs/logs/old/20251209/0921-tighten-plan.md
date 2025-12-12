# Plan: Tighten Iteration Loop for Regex Solving

## Problem Summary

The iteration loop is too slow and lacks visibility:
1. **8+ hour validation test still running** with no clear output or status
2. **Docker verification takes 2-3 min per turn** - major bottleneck
3. **No real-time streaming** - can't see what's happening as it happens
4. **No watchdog/timeout** - stuck runs block progress indefinitely
5. **Logs created post-hoc** - not useful for live debugging

## Current State

- Progress fix implemented (df67bf9e0) - should report actual progress
- System achieved 89.5% (17/19 tests) on regex-log
- TestGen working (~1 min for 21 tests)
- Parallel sampling working (3 candidates)
- **BUT:** A validation test has been running 8+ hours without clear status

## Root Causes of Slow Iteration

| Issue | Impact | Severity |
|-------|--------|----------|
| No streaming output | Can't see live progress | HIGH |
| No run timeout/watchdog | Stuck runs block forever | HIGH |
| Docker cold start | ~30s per container startup | MEDIUM |
| No checkpoint/resume | Lost work if interrupted | MEDIUM |
| Verbose logs only created at end | No visibility during run | HIGH |

## Proposed Solution: 5 Changes

### Change 1: Add Real-Time Log Streaming

**Files:**
- `src/hillclimber/map-orchestrator.ts`
- `scripts/test-progress-fix.ts`

**What:**
- Create a streaming log file that's written to continuously
- Use a tee pattern: console + file
- Add timestamps to every log line
- Log to `logs/live-run-<timestamp>.log`

**Implementation:**
```typescript
const logStream = createWriteStream(`logs/live-run-${Date.now()}.log`);
const log = (msg: string) => {
  const line = `[${new Date().toISOString()}] ${msg}`;
  console.log(line);
  logStream.write(line + '\n');
};
```

### Change 2: Add Global Timeout/Watchdog

**Files:**
- `scripts/test-progress-fix.ts`
- New: `scripts/run-with-watchdog.ts`

**What:**
- Add a global timeout (e.g., 10 min for quick validation, 30 min for full runs)
- If timeout hit, kill process and log final state
- Write checkpoint before timeout so progress isn't lost

**Implementation:**
```typescript
const TIMEOUT_MS = 10 * 60 * 1000; // 10 minutes
const timeout = setTimeout(() => {
  console.error(`[WATCHDOG] Timeout after ${TIMEOUT_MS/1000}s - killing run`);
  saveCheckpoint(state);
  process.exit(1);
}, TIMEOUT_MS);
```

### Change 3: Add Progress Heartbeat

**Files:**
- `src/hillclimber/map-orchestrator.ts`

**What:**
- Emit a heartbeat every 30 seconds showing:
  - Current turn
  - Current subtask
  - Last action
  - Time elapsed
  - Progress so far
- Write heartbeat to both console and log file

**Implementation:**
```typescript
const heartbeat = setInterval(() => {
  log(`[HEARTBEAT] Turn ${state.totalTurns}/${options.maxTurns} | ` +
      `Subtask: ${state.currentSubtask} | Progress: ${(state.bestProgress * 100).toFixed(1)}% | ` +
      `Elapsed: ${Math.floor((Date.now() - startTime) / 1000)}s`);
}, 30_000);
```

### Change 4: Create Ultra-Fast Validation Script

**Files:**
- New: `scripts/quick-validate.ts`

**What:**
- Run with just 1 turn, 1 candidate (no parallel sampling)
- Skip testgen (use cached tests or minimal set)
- 60 second timeout max
- Goal: Validate basic pipeline works in <2 minutes

**Use case:** After any code change, run quick-validate to ensure nothing broke.

### Change 5: Add Docker Container Status Visibility

**Files:**
- `src/bench/tb2-docker-runner.ts`
- `src/hillclimber/sampling-orchestrator.ts`

**What:**
- Log when each Docker container starts/stops
- Show pytest progress as it runs (tail the output)
- Add per-container timeout (2 min max)
- If container hangs, kill it and mark that candidate as failed

**Implementation:**
```typescript
log(`[DOCKER] Starting container ${i+1}/${total}...`);
// ... run container with streaming output
log(`[DOCKER] Container ${i+1} complete: ${passed}/${total} tests in ${duration}ms`);
```

## Execution Order (Parallel Approach)

### Phase 0: Kill Stuck Processes (Immediate)
1. Find and kill any stuck test processes
2. Kill any orphaned Docker containers
3. Clear any temp workspaces

### Phase 1A: Quick Visibility Fix (5 min)
1. Add heartbeat to map-orchestrator (30-second pulse)
2. Add basic streaming log output

### Phase 1B: Run Short Validation (In Parallel)
1. Run 3-turn test with 5-min timeout to verify pipeline works
2. Confirm progress reporting fix is working

### Phase 2: Full Implementation
1. **Implement Change 2:** Tiered timeout system:
   - `--quick` mode: 5 min timeout, 3 turns (for validating changes)
   - `--standard` mode: 15 min timeout, 10 turns (for development)
   - `--full` mode: 45 min timeout, 25 turns (for overnight runs)
2. **Implement Change 4:** Create quick-validate script
3. **Implement Change 5:** Docker container visibility

### Phase 3: Push to 100% on Regex
Once visibility is working:
1. Run standard mode (10-15 turns) to push past 89.5%
2. If still not 100%, run full mode with 25 turns
3. Log everything for analysis

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/hillclimber/map-orchestrator.ts` | Add heartbeat, streaming logs |
| `scripts/test-progress-fix.ts` | Add timeout, streaming |
| `src/bench/tb2-docker-runner.ts` | Add container status logs |
| `src/hillclimber/sampling-orchestrator.ts` | Add parallel status logs |
| New: `scripts/quick-validate.ts` | Ultra-fast validation |
| New: `scripts/run-with-watchdog.ts` | Wrapper with timeout |

## Success Criteria

After implementation:
- [ ] Can see live progress during any test run
- [ ] No test runs longer than 30 min without human intervention
- [ ] Quick validation completes in <2 minutes
- [ ] Can identify stuck runs within 1 minute
- [ ] All Docker containers log start/stop times

## Expected Outcome

With these changes:
- **Before:** Wait 8+ hours wondering if test is stuck
- **After:** See progress every 30 seconds, auto-kill at 30 min timeout, quick validation in 2 min

This will enable much faster iteration toward solving the regex issue (currently at 89.5%, need to get to 100%).

## User Decisions (Confirmed)

- **Kill stuck processes:** Yes, do this first
- **Timeout strategy:** Tiered - short for validation, long for results
- **Approach:** Both in parallel - fix visibility while pushing to 100%

## Implementation Summary

```
Phase 0: Kill stuck processes (~2 min)
         ↓
    ┌────┴────┐
Phase 1A:     Phase 1B:
Add heartbeat  Run 3-turn test
(5 min)        (5 min timeout)
    └────┬────┘
         ↓
Phase 2: Full logging/timeout system (~15 min)
         ↓
Phase 3: Push to 100% (run while monitoring)
```

Total implementation time: ~25 min before running production tests
