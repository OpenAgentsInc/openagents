import { Effect } from "effect";
import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import { getInitScriptPath, type InitScriptResult, type InitScriptFailureType, type OrchestratorEvent } from "./types.js";

/**
 * Detect the type of failure from init script output.
 * This enables safe mode to determine the appropriate recovery strategy.
 */
export const detectFailureType = (output: string): { type: InitScriptFailureType; canSelfHeal: boolean } => {
  const lowerOutput = output.toLowerCase();

  // TypeScript/type errors - can self-heal by spawning Claude Code to fix
  if (
    lowerOutput.includes("ts") && (
      lowerOutput.includes("error") ||
      lowerOutput.includes("type")
    ) ||
    lowerOutput.includes("typecheck") ||
    lowerOutput.includes("tsc") ||
    /\bts\d{4,5}\b/i.test(output) || // TS error codes like TS2322
    lowerOutput.includes("cannot find name") ||
    lowerOutput.includes("property .* does not exist") ||
    lowerOutput.includes("argument of type")
  ) {
    return { type: "typecheck_failed", canSelfHeal: true };
  }

  // Test failures - can attempt to fix
  if (
    lowerOutput.includes("test failed") ||
    lowerOutput.includes("tests failed") ||
    lowerOutput.includes("test failure") ||
    lowerOutput.includes("assertion") ||
    lowerOutput.includes("expect(") ||
    (lowerOutput.includes("fail") && (lowerOutput.includes("test") || lowerOutput.includes("spec")))
  ) {
    return { type: "test_failed", canSelfHeal: true };
  }

  // Network errors - can continue in offline/degraded mode
  if (
    lowerOutput.includes("network") ||
    lowerOutput.includes("enotfound") ||
    lowerOutput.includes("econnrefused") ||
    lowerOutput.includes("etimedout") ||
    lowerOutput.includes("unable to connect") ||
    lowerOutput.includes("could not resolve")
  ) {
    return { type: "network_error", canSelfHeal: false };
  }

  // Disk full - cannot self-heal
  if (
    lowerOutput.includes("no space left") ||
    lowerOutput.includes("disk full") ||
    lowerOutput.includes("enospc") ||
    lowerOutput.includes("quota exceeded")
  ) {
    return { type: "disk_full", canSelfHeal: false };
  }

  // Permission errors - cannot self-heal
  if (
    lowerOutput.includes("permission denied") ||
    lowerOutput.includes("eacces") ||
    lowerOutput.includes("eperm") ||
    lowerOutput.includes("operation not permitted")
  ) {
    return { type: "permission_denied", canSelfHeal: false };
  }

  // Unknown error - fallback
  return { type: "unknown", canSelfHeal: false };
};

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

    // Detect failure type for safe mode recovery
    const { type: failureType, canSelfHeal } = !success && output
      ? detectFailureType(output)
      : { type: "unknown" as InitScriptFailureType, canSelfHeal: false };

    const result: InitScriptResult = {
      ran: true,
      success,
      hasWarnings,
      exitCode,
      output,
      durationMs,
      ...(errorMessage !== undefined && { error: errorMessage }),
      ...(!success && { failureType, canSelfHeal }),
    };

    emit({ type: "init_script_complete", result });
    return result;
  });
