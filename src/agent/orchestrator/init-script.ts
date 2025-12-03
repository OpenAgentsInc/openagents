import { Effect } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { getInitScriptPath, type InitScriptResult, type OrchestratorEvent } from "./types.js";

/**
 * Run `.openagents/init.sh` at session start to verify the workspace.
 * Follows the pi-mono pattern: run if present, skip silently if missing.
 * Returns an InitScriptResult instead of throwing so the orchestrator can
 * decide whether to proceed.
 */
export const runInitScript = (
  openagentsDir: string,
  cwd: string,
  emit: (event: OrchestratorEvent) => void = () => {}
): Effect.Effect<InitScriptResult, never, never> =>
  Effect.sync(() => {
    const initPath = getInitScriptPath(openagentsDir);

    if (!fs.existsSync(initPath)) {
      return { ran: false, success: true };
    }

    emit({ type: "init_script_start", path: initPath });

    const started = Date.now();

    try {
      const output = execSync(`bash "${initPath}"`, {
        cwd,
        encoding: "utf-8",
        stdio: ["ignore", "pipe", "pipe"],
        timeout: 60000,
      });

      const result: InitScriptResult = {
        ran: true,
        success: true,
        output: output.trim(),
        durationMs: Date.now() - started,
      };

      emit({ type: "init_script_complete", result });
      return result;
    } catch (error: any) {
      const parts = [error?.stdout, error?.stderr, error?.message]
        .filter(Boolean)
        .map(String);

      const result: InitScriptResult = {
        ran: true,
        success: false,
        output: parts.join("\n").trim(),
        durationMs: Date.now() - started,
        error: error?.message,
      };

      emit({ type: "init_script_complete", result });
      return result;
    }
  });
