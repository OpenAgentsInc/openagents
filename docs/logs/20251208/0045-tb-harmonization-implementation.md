# TerminalBench Run Storage Harmonization - Implementation Log

**Date:** 2025-12-08
**Time:** 0045
**Task:** Implement harmonization plan to unify TB run storage

---

## Summary

Implemented the core functionality to save TerminalBench runs in `TBRunFile` format to `.openagents/tb-runs/` so the UI run browser can find and display them.

---

## Changes Made

### 1. Created Conversion Function (`src/tbench-hud/persistence.ts`)

**Added:**
- `convertResultsToTBRunFile()` - Converts `TerminalBenchResults` format to `TBRunFile` format
- `mapStatusToOutcome()` - Helper to map status strings ("pass"/"fail" etc.) to `TBTaskOutcome` enum

**Key Features:**
- Accepts full `TerminalBenchSuite` object for task metadata lookup (name, category, difficulty)
- Falls back gracefully if only suite name/version provided
- Calculates total duration from individual task durations
- Optionally includes ATIF trajectory for detailed analysis

**Code:**
```typescript
export const convertResultsToTBRunFile = (
  results: TerminalBenchResults,
  runId: string,
  suite: TerminalBenchSuite | { name: string; version: string },
  trajectory?: Trajectory
): TBRunFile
```

### 2. Updated Exports (`src/tbench-hud/index.ts`)

**Added:**
- Exported `convertResultsToTBRunFile` function

### 3. Updated CLI to Save TBRunFile (`src/cli/tbench-local.ts`)

**Added:**
- Import of `saveTBRun` and `convertResultsToTBRunFile` from `tbench-hud/persistence`
- `loadTrajectoryIfAvailable()` helper function to optionally load ATIF trajectory
- Code after run completion to save `TBRunFile` format to `.openagents/tb-runs/`

**Implementation Details:**
- Only saves if `--run-id` flag is provided (UI runs always provide this)
- Loads trajectory from `trajectory.json` in output directory if available
- Uses full suite object for accurate task metadata (name, category, difficulty)
- Gracefully handles errors (warns but doesn't fail the run)

**Code Location:** After line 792 (after `tbEmitter.close()`)

```typescript
// Save TBRunFile format to .openagents/tb-runs/ for UI run browser
if (args.runId) {
  try {
    const trajectory = await loadTrajectoryIfAvailable(args.output);
    const runFile = convertResultsToTBRunFile(
      finalResults,
      args.runId,
      suite,
      trajectory
    );
    const savedPath = await saveTBRun(runFile);
    console.log(`[TB] Run saved to: ${savedPath}`);
  } catch (err) {
    console.warn(`[TB] Failed to save TBRunFile: ${err}`);
  }
}
```

---

## How It Works

### CLI Runs

When running via CLI with `--run-id` flag:
```bash
bun src/cli/tbench-local.ts --suite tasks/suite.json --output results/run1 --run-id tb-123
```

1. Run executes and saves `results.json` to `results/run1/` (backward compatible)
2. After completion, converts `TerminalBenchResults` to `TBRunFile` format
3. Saves to `.openagents/tb-runs/YYYYMMDD-tb-HHMMSS-<shortId>.json`
4. UI run browser can now find and display the run

### UI Runs

When triggered from UI (via `startTBRun()` in `handlers.ts`):
1. UI always provides `--run-id` flag (generated in `startTBRun()`)
2. Run executes and saves to `results/<runId>/` (as before)
3. After completion, also saves `TBRunFile` to `.openagents/tb-runs/`
4. UI run browser can now find and display the run

---

## Testing Status

✅ **Build:** TypeScript compilation successful
⏳ **CLI Test:** Pending - need to test with `--run-id` flag
⏳ **UI Test:** Pending - need to trigger run from UI and verify save
⏳ **UI Browser Test:** Pending - need to verify run browser can load runs

---

## Next Steps

1. **Test CLI run:**
   ```bash
   bun src/cli/tbench-local.ts --suite tasks/fm-mini-suite.json \
     --output results/test-run --run-id tb-test-001 --model fm
   ```
   Verify `.openagents/tb-runs/` contains new JSON file

2. **Test UI run:**
   - Trigger full benchmark or random task from UI
   - Verify `.openagents/tb-runs/` contains new JSON file
   - Check UI run browser can see the run

3. **Verify format:**
   - Check saved JSON has `meta`, `tasks`, and optionally `trajectory` fields
   - Verify task metadata (name, category, difficulty) is populated correctly

4. **Optional enhancements:**
   - Add trajectory buffering to `TBEmitter` for real-time collection
   - Create migration script for existing runs
   - Update UI default output directory (or keep `results/` for compatibility)

---

## Files Modified

1. `src/tbench-hud/persistence.ts` - Added conversion function
2. `src/tbench-hud/index.ts` - Exported conversion function
3. `src/cli/tbench-local.ts` - Added TBRunFile save logic

---

## Notes

- Backward compatibility maintained: `results.json` still saved to output directory
- UI runs will now be discoverable by run browser (they provide `--run-id`)
- CLI runs without `--run-id` won't save TBRunFile (but that's fine, they're not from UI)
- Trajectory loading is optional - runs work fine without it
- Error handling is graceful - run doesn't fail if TBRunFile save fails
