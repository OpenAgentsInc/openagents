/**
 * HillClimber Exporter
 *
 * Exports learned hints from HillClimber to the hints system.
 * Creates a JSON file with task-specific hints that can be integrated
 * into the FM hints system.
 */

import { Effect } from "effect";
import { HillClimberStore, HillClimberStoreLive } from "./store.js";
import { isStableForExport } from "./scoring.js";

// ============================================================================
// Types
// ============================================================================

export interface LearnedHint {
  taskId: string;
  hint: string;
  score: number;
  passCount: number;
  totalRuns: number;
  passRate: number;
  learnedAt: string;
}

export interface LearnedHintsExport {
  version: 1;
  exportedAt: string;
  hints: LearnedHint[];
}

// ============================================================================
// Export Path
// ============================================================================

/** Default path for exported hints */
export const LEARNED_HINTS_PATH = ".openagents/learned-hints.json";

// ============================================================================
// Export Functions
// ============================================================================

/**
 * Get all hints that are stable enough for export.
 */
export const getExportableHints = (): Effect.Effect<
  LearnedHint[],
  Error,
  HillClimberStore
> =>
  Effect.gen(function* () {
    const store = yield* HillClimberStore;

    const bestConfigs = yield* store.getBestConfigs();
    const hints: LearnedHint[] = [];

    for (const best of bestConfigs) {
      // Check if stable enough for export
      if (!isStableForExport(best.passCount, best.totalRuns, best.score)) {
        console.log(
          `[Exporter] Skipping ${best.taskId}: not stable (passes=${best.passCount}, runs=${best.totalRuns}, score=${best.score})`,
        );
        continue;
      }

      // Get the actual config to retrieve the hint
      const config = yield* store.getConfigById(best.configId);
      if (!config?.hint) {
        console.log(`[Exporter] Skipping ${best.taskId}: no hint configured`);
        continue;
      }

      hints.push({
        taskId: best.taskId,
        hint: config.hint,
        score: best.score,
        passCount: best.passCount,
        totalRuns: best.totalRuns,
        passRate: best.totalRuns > 0 ? best.passCount / best.totalRuns : 0,
        learnedAt: best.updatedAt,
      });
    }

    return hints;
  });

/**
 * Export learned hints to a JSON file.
 */
export const exportHints = (
  outputPath: string = LEARNED_HINTS_PATH,
): Effect.Effect<LearnedHintsExport, Error, HillClimberStore> =>
  Effect.gen(function* () {
    const hints = yield* getExportableHints();

    const exportData: LearnedHintsExport = {
      version: 1,
      exportedAt: new Date().toISOString(),
      hints,
    };

    // Write to file
    const fs = require("node:fs");
    const path = require("node:path");

    // Ensure directory exists
    const dir = path.dirname(outputPath);
    if (dir !== ".") {
      fs.mkdirSync(dir, { recursive: true });
    }

    fs.writeFileSync(outputPath, JSON.stringify(exportData, null, 2));

    console.log(`[Exporter] Exported ${hints.length} hints to ${outputPath}`);

    return exportData;
  });

/**
 * Export a single task's hint.
 */
export const exportTaskHint = (
  taskId: string,
  outputPath: string = LEARNED_HINTS_PATH,
): Effect.Effect<LearnedHint | null, Error, HillClimberStore> =>
  Effect.gen(function* () {
    const store = yield* HillClimberStore;

    const best = yield* store.getBestConfigForTask(taskId);
    if (!best) {
      console.log(`[Exporter] No best config found for task: ${taskId}`);
      return null;
    }

    // Check if stable enough for export
    if (!isStableForExport(best.passCount, best.totalRuns, best.score)) {
      console.log(
        `[Exporter] Task ${taskId} not stable enough for export (passes=${best.passCount}, runs=${best.totalRuns}, score=${best.score})`,
      );
      return null;
    }

    const config = yield* store.getConfigById(best.configId);
    if (!config?.hint) {
      console.log(`[Exporter] No hint configured for task: ${taskId}`);
      return null;
    }

    const hint: LearnedHint = {
      taskId: best.taskId,
      hint: config.hint,
      score: best.score,
      passCount: best.passCount,
      totalRuns: best.totalRuns,
      passRate: best.totalRuns > 0 ? best.passCount / best.totalRuns : 0,
      learnedAt: best.updatedAt,
    };

    // Load existing exports and merge
    const fs = require("node:fs");
    let existing: LearnedHintsExport = {
      version: 1,
      exportedAt: new Date().toISOString(),
      hints: [],
    };

    if (fs.existsSync(outputPath)) {
      try {
        existing = JSON.parse(fs.readFileSync(outputPath, "utf-8"));
      } catch {
        // Ignore parse errors, start fresh
      }
    }

    // Update or add hint
    const existingIdx = existing.hints.findIndex((h) => h.taskId === taskId);
    if (existingIdx >= 0) {
      existing.hints[existingIdx] = hint;
    } else {
      existing.hints.push(hint);
    }

    existing.exportedAt = new Date().toISOString();

    // Ensure directory exists
    const path = require("node:path");
    const dir = path.dirname(outputPath);
    if (dir !== ".") {
      fs.mkdirSync(dir, { recursive: true });
    }

    fs.writeFileSync(outputPath, JSON.stringify(existing, null, 2));

    console.log(`[Exporter] Exported hint for task: ${taskId}`);

    return hint;
  });

// ============================================================================
// Load Functions
// ============================================================================

/**
 * Load learned hints from the export file.
 */
export const loadLearnedHints = (
  inputPath: string = LEARNED_HINTS_PATH,
): LearnedHintsExport | null => {
  const fs = require("node:fs");

  if (!fs.existsSync(inputPath)) {
    return null;
  }

  try {
    const data = fs.readFileSync(inputPath, "utf-8");
    return JSON.parse(data) as LearnedHintsExport;
  } catch {
    return null;
  }
};

/**
 * Get a learned hint for a specific task.
 */
export const getLearnedHint = (
  taskId: string,
  inputPath: string = LEARNED_HINTS_PATH,
): string | null => {
  const exported = loadLearnedHints(inputPath);
  if (!exported) return null;

  const hint = exported.hints.find((h) => h.taskId === taskId);
  return hint?.hint ?? null;
};

// ============================================================================
// Integration with hints.ts
// ============================================================================

/**
 * Generate TypeScript code for adding learned hints to hints.ts.
 * This can be used to manually update the hints system.
 */
export const generateHintsCode = (): Effect.Effect<
  string,
  Error,
  HillClimberStore
> =>
  Effect.gen(function* () {
    const hints = yield* getExportableHints();

    if (hints.length === 0) {
      return "// No learned hints to export yet";
    }

    let code = `/**
 * Learned hints from HillClimber overnight runs.
 * Generated at: ${new Date().toISOString()}
 */
export const LEARNED_HINTS: Record<string, string> = {\n`;

    for (const hint of hints) {
      const escapedHint = hint.hint.replace(/"/g, '\\"').replace(/\n/g, "\\n");
      code += `  "${hint.taskId}": "${escapedHint}",\n`;
    }

    code += `};\n\n/**
 * Get a learned hint for a task.
 */
export function getLearnedHint(taskId: string): string | undefined {
  return LEARNED_HINTS[taskId];
}\n`;

    return code;
  });

// ============================================================================
// CLI Helper
// ============================================================================

/**
 * Export hints to file (for CLI usage).
 */
export const runExport = async (
  outputPath: string = LEARNED_HINTS_PATH,
): Promise<void> => {
  const program = exportHints(outputPath).pipe(
    Effect.provide(HillClimberStoreLive),
  );

  await Effect.runPromise(program);
};
