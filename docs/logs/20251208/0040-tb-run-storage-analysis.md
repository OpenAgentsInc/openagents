# TerminalBench Run Storage Analysis & Harmonization Plan

**Date:** 2025-12-08
**Time:** 0040
**Task:** Analyze TerminalBench run storage discrepancies between CLI and UI

---

## Executive Summary

TerminalBench runs triggered from the UI are **not being saved to `.openagents/tb-runs/`** in the format expected by the UI's run browser. This creates a disconnect where:

- **CLI runs** can save to `.openagents/tb-runs/` (if specified) but in the wrong format
- **UI runs** save to `results/<runId>/` in a different format
- **UI loading code** expects `.openagents/tb-runs/` with `TBRunFile` format but finds nothing

The `saveTBRun()` function exists but is **never called**, leaving runs in incompatible formats.

---

## Detailed Analysis

### 1. CLI Runs (`tbench-local.ts`)

**Location:** Specified via `--output` flag (can be `.openagents/tb-runs/...` or any directory)

**Format:** `TerminalBenchResults` format saved as `results.json`:
```typescript
{
  suite_name: string;
  suite_version: string;
  model: string;
  timestamp: string;
  results: Array<{
    task_id: string;
    status: "pass" | "fail" | "timeout" | "error";
    duration_ms: number;
    turns: number;
    tokens_used: number;
    verification_output?: string;
    error_message?: string;
  }>;
  summary: {
    total: number;
    passed: number;
    failed: number;
    timeout: number;
    error: number;
    skipped?: number;
    pass_rate: number;
    avg_duration_ms: number;
    avg_turns: number;
    total_tokens: number;
  };
}
```

**Files Saved:**
- `results.json` - Main results in `TerminalBenchResults` format
- `report.json` - Comparison report (if baseline provided)
- `report.md` - Human-readable markdown summary
- `<task-id>/workspace/` - Task workspaces with agent output
- `<task-id>/output.txt` - Agent conversation logs
- `<task-id>/verification.txt` - Test verification output

**Code Location:** `src/cli/tbench-local.ts:760-771`

```typescript
// Generate final results
const finalResults = toBenchmarkResults(suite, modelRunner.modelName, results);
writeFileSync(join(args.output, "results.json"), JSON.stringify(finalResults, null, 2));

// Generate comparison report
const report = generateComparisonReport(finalResults, baseline);
writeFileSync(join(args.output, "report.json"), JSON.stringify(report, null, 2));
writeFileSync(join(args.output, "report.md"), formatMarkdownReport(report));
```

**Key Points:**
- CLI can save to `.openagents/tb-runs/` if you manually specify `--output .openagents/tb-runs/<name>`
- But it saves in `TerminalBenchResults` format, not `TBRunFile` format
- No conversion to `TBRunFile` format happens
- `saveTBRun()` is never called

---

### 2. UI Runs (via `startTBRun` in `handlers.ts`)

**Location:** `results/<runId>/` (NOT `.openagents/tb-runs/`)

**Format:** Same `TerminalBenchResults` format as CLI runs

**How It Works:**

1. UI calls `startTBRun()` in `src/desktop/handlers.ts:128-245`
2. Default output directory is hardcoded to `results/<runId>` (line 148):

```typescript
const outputDir = options.outputDir
  ? options.outputDir.startsWith("/")
    ? options.outputDir
    : join(PROJECT_ROOT, options.outputDir)
  : join(PROJECT_ROOT, "results", runId);  // ← Default for UI runs
```

3. Spawns `tbench-local.ts` subprocess with that output directory
4. Same files are saved as CLI runs, but in `results/` directory

**Code Flow:**
- `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts:421-470` - UI triggers `runFullBenchmark`
- `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts:473-544` - UI triggers `runRandomTask`
- Both call `socket.startTBRun(runOptions)` which goes to desktop server
- `src/desktop/handlers.ts:128` - `startTBRun()` spawns subprocess
- `src/cli/tbench-local.ts` - Executes and saves to `results/<runId>/`

**Key Points:**
- UI runs **never** save to `.openagents/tb-runs/`
- They save to `results/<runId>/` in `TerminalBenchResults` format
- No conversion to `TBRunFile` format
- `saveTBRun()` is never called

---

### 3. UI Loading Code

**Location:** `src/desktop/handlers.ts:261-336`

**What It Does:**
- `loadRecentTBRuns()` - Loads run metadata from `.openagents/tb-runs/`
- `loadTBRunDetails()` - Loads full run details from `.openagents/tb-runs/`

**Expected Format:** `TBRunFile` format:

```typescript
interface TBRunFile {
  meta: {
    runId: string;
    suiteName: string;
    suiteVersion: string;
    timestamp: string;
    passRate: number;
    passed: number;
    failed: number;
    timeout: number;
    error: number;
    totalDurationMs: number;
    totalTokens: number;
    taskCount: number;
  };
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: TBDifficulty;
    outcome: TBTaskOutcome;
    durationMs: number;
    turns: number;
    tokens: number;
    outputLines?: number;
  }>;
  trajectory?: Trajectory;  // Optional ATIF trajectory
}
```

**Code:**
```typescript
export async function loadRecentTBRuns(count: number = 20): Promise<TBRunHistoryItem[]> {
  const runsDir = join(PROJECT_ROOT, DEFAULT_TB_RUNS_DIR);  // .openagents/tb-runs
  const runs = await loadRecentRuns(count, runsDir);  // From persistence.ts
  // ...
}
```

**Key Points:**
- Expects `.openagents/tb-runs/` directory
- Expects `TBRunFile` format (single JSON files with `meta`, `tasks`, `trajectory`)
- Uses `listTBRuns()` which looks for files matching pattern `*-tb-*.json`
- Finds nothing because runs are saved in different format/location

---

### 4. The `saveTBRun()` Function

**Location:** `src/tbench-hud/persistence.ts:119-130`

**What It Does:**
- Saves runs to `.openagents/tb-runs/` in `TBRunFile` format
- Generates filename: `YYYYMMDD-tb-HHMMSS-<shortId>.json`
- Creates directory if it doesn't exist

**Code:**
```typescript
export const saveTBRun = async (
  run: TBRunFile,
  baseDir = DEFAULT_TB_RUNS_DIR  // .openagents/tb-runs
): Promise<string> => {
  ensureRunsDir(baseDir);
  const filename = generateRunFilename(run.meta.runId, run.meta.timestamp);
  const filepath = join(baseDir, filename);
  await Bun.file(filepath).write(JSON.stringify(run, null, 2));
  return filepath;
};
```

**Key Points:**
- Function exists and is exported
- **Never called anywhere in the codebase**
- No conversion from `TerminalBenchResults` to `TBRunFile` happens
- Helper functions exist: `buildTBRunFile()`, `buildTBRunMeta()` in `persistence.ts:231-270`

---

## Current State Evidence

### Directory Structure

`.openagents/tb-runs/` currently contains:
- Directories like `fm-mini-20251207-221950/` (not single JSON files)
- Each directory contains:
  - `results.json` (in `TerminalBenchResults` format)
  - `report.json`, `report.md`
  - Task subdirectories with workspaces

**Expected structure** (per `TBRunFile` design):
- Single JSON files like `20251207-tb-221950-abc123.json`
- Each file contains `{ meta, tasks, trajectory? }`

### Code Evidence

1. **No calls to `saveTBRun()`:**
   ```bash
   $ grep -r "saveTBRun" src/
   # Only found in:
   # - src/tbench-hud/persistence.ts (definition)
   # - src/tbench-hud/index.ts (export)
   # - docs/claude/plans/terminal-bench-hud-redesign.md (planning doc)
   ```

2. **UI default output:**
   ```typescript
   // src/desktop/handlers.ts:148
   : join(PROJECT_ROOT, "results", runId);  // ← Always results/, never .openagents/tb-runs/
   ```

3. **CLI saves only `results.json`:**
   ```typescript
   // src/cli/tbench-local.ts:771
   writeFileSync(join(args.output, "results.json"), JSON.stringify(finalResults, null, 2));
   // No call to saveTBRun() after this
   ```

---

## Harmonization Plan

### Goal

Unify run storage so that:
1. **All runs** (CLI and UI) save to `.openagents/tb-runs/` in `TBRunFile` format
2. **Backward compatibility** maintained for existing `results.json` format
3. **UI run browser** can find and display all runs
4. **ATIF trajectory** optionally included for detailed analysis

### Implementation Steps

#### Step 1: Create Conversion Function

**File:** `src/tbench-hud/persistence.ts` (add new function)

**Function:** Convert `TerminalBenchResults` → `TBRunFile`

```typescript
/**
 * Convert TerminalBenchResults to TBRunFile format.
 *
 * @param results - TerminalBenchResults from tbench-local.ts
 * @param runId - Run ID (from --run-id flag or generated)
 * @param suite - Suite metadata
 * @param trajectory - Optional ATIF trajectory
 * @returns TBRunFile ready for saveTBRun()
 */
export const convertResultsToTBRunFile = (
  results: TerminalBenchResults,
  runId: string,
  suite: { name: string; version: string },
  trajectory?: Trajectory
): TBRunFile => {
  const meta = buildTBRunMeta({
    runId,
    suiteName: results.suite_name,
    suiteVersion: results.suite_version,
    timestamp: results.timestamp,
    passRate: results.summary.pass_rate,
    passed: results.summary.passed,
    failed: results.summary.failed,
    timeout: results.summary.timeout,
    error: results.summary.error,
    totalDurationMs: results.results.reduce((sum, r) => sum + r.duration_ms, 0),
    totalTokens: results.summary.total_tokens,
    taskCount: results.summary.total,
  });

  const tasks: TBTaskResult[] = results.results.map((r) => ({
    id: r.task_id,
    name: r.task_id, // Will need to look up from suite if available
    category: "", // Will need to look up from suite
    difficulty: "medium" as TBDifficulty, // Will need to look up from suite
    outcome: mapStatusToOutcome(r.status),
    durationMs: r.duration_ms,
    turns: r.turns,
    tokens: r.tokens_used,
  }));

  return buildTBRunFile(meta, tasks, trajectory);
};

const mapStatusToOutcome = (
  status: "pass" | "fail" | "timeout" | "error" | "skip"
): TBTaskOutcome => {
  switch (status) {
    case "pass": return "success";
    case "fail": return "failure";
    case "timeout": return "timeout";
    case "error": return "error";
    case "skip": return "error"; // Skip treated as error
    default: return "error";
  }
};
```

**Enhancement:** Look up task metadata from suite to populate `name`, `category`, `difficulty` properly.

---

#### Step 2: Update `tbench-local.ts` to Save TBRunFile

**File:** `src/cli/tbench-local.ts`

**Changes:**

1. Import conversion function:
```typescript
import {
  saveTBRun,
  convertResultsToTBRunFile,
  type TBRunFile,
} from "../tbench-hud/persistence.js";
```

2. After generating final results (around line 771), add:

```typescript
// Generate final results
const finalResults = toBenchmarkResults(suite, modelRunner.modelName, results);
writeFileSync(join(args.output, "results.json"), JSON.stringify(finalResults, null, 2));

// ALSO save in TBRunFile format to .openagents/tb-runs/
if (args.runId) {
  try {
    // Load ATIF trajectory if available (from HUD emitter or file)
    const trajectory = await loadTrajectoryIfAvailable(args.runId, args.output);

    const runFile = convertResultsToTBRunFile(
      finalResults,
      args.runId,
      { name: suite.name, version: suite.version },
      trajectory
    );

    const savedPath = await saveTBRun(runFile);
    console.log(`[TB] Run saved to: ${savedPath}`);
  } catch (err) {
    console.warn(`[TB] Failed to save TBRunFile: ${err}`);
    // Don't fail the run if persistence fails
  }
}
```

**Note:** Need to implement `loadTrajectoryIfAvailable()` to optionally load ATIF trajectory from HUD or saved files.

---

#### Step 3: Update UI Default Output Directory

**File:** `src/desktop/handlers.ts`

**Change:** Default output to `.openagents/tb-runs/<runId>` instead of `results/<runId>`

```typescript
const outputDir = options.outputDir
  ? options.outputDir.startsWith("/")
    ? options.outputDir
    : join(PROJECT_ROOT, options.outputDir)
  : join(PROJECT_ROOT, ".openagents", "tb-runs", runId);  // ← Changed from results/
```

**Alternative (safer):** Keep `results/` for workspace files, but ensure `TBRunFile` is saved to `.openagents/tb-runs/`:

```typescript
// Keep results/ for backward compatibility and workspace files
const outputDir = options.outputDir
  ? options.outputDir.startsWith("/")
    ? options.outputDir
    : join(PROJECT_ROOT, options.outputDir)
  : join(PROJECT_ROOT, "results", runId);

// But also ensure TBRunFile is saved (handled in tbench-local.ts)
```

**Recommendation:** Use alternative approach to maintain backward compatibility.

---

#### Step 4: Handle ATIF Trajectory Collection

**File:** `src/cli/tbench-local.ts` or new `src/tbench-hud/trajectory-loader.ts`

**Function:** Collect ATIF trajectory from HUD emitter or saved files

```typescript
/**
 * Load ATIF trajectory if available.
 * Trajectory may be:
 * 1. Collected from TBEmitter during run (if HUD connected)
 * 2. Saved to output directory during run
 * 3. Not available (return undefined)
 */
async function loadTrajectoryIfAvailable(
  runId: string,
  outputDir: string
): Promise<Trajectory | undefined> {
  // Try to load from output directory first
  const trajectoryPath = join(outputDir, "trajectory.json");
  if (existsSync(trajectoryPath)) {
    try {
      const content = await Bun.file(trajectoryPath).json();
      return content as Trajectory;
    } catch (err) {
      console.warn(`[TB] Failed to load trajectory: ${err}`);
    }
  }

  // TODO: Try to collect from TBEmitter if it has trajectory buffer
  // This would require enhancing TBEmitter to buffer trajectory steps

  return undefined;
}
```

**Enhancement:** Modify `TBEmitter` to buffer ATIF steps during run, then provide `getTrajectory()` method.

---

#### Step 5: Update Task Metadata Lookup

**Enhancement:** When converting `TerminalBenchResults` to `TBRunFile`, look up task metadata from suite:

```typescript
export const convertResultsToTBRunFile = (
  results: TerminalBenchResults,
  runId: string,
  suite: TerminalBenchSuite,  // Full suite, not just name/version
  trajectory?: Trajectory
): TBRunFile => {
  // Create lookup map
  const taskMap = new Map(suite.tasks.map(t => [t.id, t]));

  const tasks: TBTaskResult[] = results.results.map((r) => {
    const taskDef = taskMap.get(r.task_id);
    return {
      id: r.task_id,
      name: taskDef?.name ?? r.task_id,
      category: taskDef?.category ?? "",
      difficulty: (taskDef?.difficulty ?? "medium") as TBDifficulty,
      outcome: mapStatusToOutcome(r.status),
      durationMs: r.duration_ms,
      turns: r.turns,
      tokens: r.tokens_used,
    };
  });

  // ... rest of conversion
};
```

**Update:** Pass full `suite` object to conversion function in `tbench-local.ts`.

---

#### Step 6: Migration Script (Optional)

**File:** `src/cli/migrate-tb-runs.ts` (new)

**Purpose:** Convert existing `results.json` files in `.openagents/tb-runs/` directories to `TBRunFile` format

```typescript
#!/usr/bin/env bun
/**
 * Migrate existing TerminalBench runs to TBRunFile format.
 *
 * Scans .openagents/tb-runs/ for directories containing results.json,
 * converts them to TBRunFile format, and saves as single JSON files.
 */

import { readdirSync, existsSync, readFileSync } from "fs";
import { join } from "path";
import { saveTBRun, convertResultsToTBRunFile } from "../tbench-hud/persistence.js";
import type { TerminalBenchResults } from "../bench/terminal-bench.js";

async function migrateRuns() {
  const runsDir = ".openagents/tb-runs";
  const dirs = readdirSync(runsDir, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name);

  for (const dir of dirs) {
    const resultsPath = join(runsDir, dir, "results.json");
    if (!existsSync(resultsPath)) continue;

    try {
      const results: TerminalBenchResults = JSON.parse(
        readFileSync(resultsPath, "utf-8")
      );

      // Extract runId from directory name or generate
      const runId = extractRunIdFromDir(dir) || `tb-${Date.now()}`;

      // Load suite if available (may need to infer)
      const suite = { name: results.suite_name, version: results.suite_version };

      const runFile = convertResultsToTBRunFile(results, runId, suite);
      await saveTBRun(runFile);

      console.log(`✓ Migrated ${dir} → ${runFile.meta.runId}`);
    } catch (err) {
      console.error(`✗ Failed to migrate ${dir}: ${err}`);
    }
  }
}

migrateRuns();
```

---

### Testing Plan

1. **Unit Tests:**
   - Test `convertResultsToTBRunFile()` with various `TerminalBenchResults`
   - Test edge cases (empty results, missing fields, etc.)

2. **Integration Tests:**
   - Run CLI with `--run-id` flag, verify `TBRunFile` saved to `.openagents/tb-runs/`
   - Run UI-triggered run, verify `TBRunFile` saved
   - Verify UI run browser can load and display runs

3. **Backward Compatibility:**
   - Verify existing `results.json` files still work
   - Verify migration script converts old format correctly

---

### Rollout Strategy

1. **Phase 1:** Implement conversion function and update `tbench-local.ts`
   - CLI runs start saving `TBRunFile` format
   - Keep `results.json` for backward compatibility

2. **Phase 2:** Update UI default output (or keep `results/` but ensure `TBRunFile` saved)
   - UI runs also save `TBRunFile` format
   - UI run browser can now find runs

3. **Phase 3:** (Optional) Run migration script
   - Convert existing runs to new format
   - Clean up old directory structure if desired

4. **Phase 4:** Enhance trajectory collection
   - Add trajectory buffering to `TBEmitter`
   - Include full ATIF trajectory in saved runs

---

## Summary

**Current State:**
- CLI runs: Save to `--output` in `TerminalBenchResults` format
- UI runs: Save to `results/<runId>/` in `TerminalBenchResults` format
- UI loading: Expects `.openagents/tb-runs/` in `TBRunFile` format
- **Disconnect:** No conversion, no saving to expected location/format

**Solution:**
1. Create `convertResultsToTBRunFile()` function
2. Call `saveTBRun()` from `tbench-local.ts` after run completes
3. Ensure UI runs also trigger this (via `--run-id` flag)
4. Optionally migrate existing runs

**Result:**
- All runs saved to `.openagents/tb-runs/` in `TBRunFile` format
- UI run browser can find and display all runs
- Backward compatibility maintained with `results.json` format

---

**Next Steps:**
1. Review and approve plan
2. Implement conversion function
3. Update `tbench-local.ts` to save `TBRunFile`
4. Test with CLI and UI runs
5. Deploy and verify UI run browser works
