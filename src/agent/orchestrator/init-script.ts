import { Effect } from "effect";
import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import { getInitScriptPath, type InitScriptResult, type OrchestratorEvent } from "./types.js";

/**
 * Run `.openagents/init.sh` at session start to verify the workspace.
 * Follows the pi-mono pattern: run if present, skip silently if missing.
 * Returns an InitScriptResult instead of throwing so the orchestrator can
 * decide whether to proceed.
 *
 * Exit code semantics (per GOLDEN-LOOP-v2.md Section 2.2.1):
 * - 0: All checks passed → success=true
 * - 1: Fatal error → success=false (abort session)
 * - 2: Warnings only → success=true, hasWarnings=true (continue with caution)
 */
export const runInitScript = (
  openagentsDir: string,
  cwd: string,
  emit: (event: OrchestratorEvent) => void = () => {},
  timeoutMs = 120000
): Effect.Effect<InitScriptResult, never, never> =>
  Effect.sync(() => {
    const initPath = getInitScriptPath(openagentsDir);

    if (!fs.existsSync(initPath)) {
      return { ran: false, success: true, exitCode: 0 };
    }

    emit({ type: "init_script_start", path: initPath });

    const started = Date.now();

    // Use spawnSync to get the exit code directly
    const proc = spawnSync("bash", [initPath], {
      cwd,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: timeoutMs,
    });

    const durationMs = Date.now() - started;
    const output = [proc.stdout, proc.stderr].filter(Boolean).join("\n").trim();
    const exitCode = proc.status ?? 1; // Default to 1 (fatal) if status is null

    // Exit code semantics:
    // 0 = success
    // 1 = fatal error (abort)
    // 2 = warnings only (continue)
    const success = exitCode === 0 || exitCode === 2;
    const hasWarnings = exitCode === 2;

    const errorMessage = proc.error?.message || (exitCode === 1 ? "Preflight check failed (exit 1)" : undefined);

    const result: InitScriptResult = {
      ran: true,
      success,
      hasWarnings,
      exitCode,
      output,
      durationMs,
      ...(errorMessage !== undefined && { error: errorMessage }),
    };

    emit({ type: "init_script_complete", result });
    return result;
  });
