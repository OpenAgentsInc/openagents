import { Effect } from "effect";
import { spawn } from "node:child_process";
import type { OrchestratorEvent } from "./types.js";

export interface VerificationCommandResult {
  command: string;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut?: boolean;
  errorMessage?: string;
}

export interface VerificationRunResult {
  passed: boolean;
  results: VerificationCommandResult[];
  outputs: string[];
}

const DEFAULT_TIMEOUT_MS = 120_000;

const summarizeOutput = (stdout: string, stderr: string): string =>
  `${stdout}${stderr}`.trim();

const runCommand = (
  command: string,
  cwd: string,
  emit?: (event: OrchestratorEvent) => void,
): Effect.Effect<VerificationCommandResult, Error, never> =>
  Effect.async((resume) => {
    const start = Date.now();
    emit?.({ type: "verification_start", command });

    const proc = spawn(command, {
      cwd,
      shell: true,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    let finished = false;

    const finish = (result: VerificationCommandResult) => {
      if (finished) return;
      finished = true;
      clearTimeout(timeoutHandle);
      emit?.({
        type: "verification_complete",
        command,
        passed: result.exitCode === 0 && !result.timedOut,
        output: summarizeOutput(stdout, stderr),
      });
      resume(Effect.succeed(result));
    };

    proc.stdout?.on("data", (data: Buffer) => {
      const chunk = data.toString("utf-8");
      stdout += chunk;
      emit?.({ type: "verification_output", command, chunk, stream: "stdout" });
    });

    proc.stderr?.on("data", (data: Buffer) => {
      const chunk = data.toString("utf-8");
      stderr += chunk;
      emit?.({ type: "verification_output", command, chunk, stream: "stderr" });
    });

    proc.on("error", (error: Error) => {
      finish({
        command,
        exitCode: 1,
        stdout,
        stderr,
        durationMs: Date.now() - start,
        errorMessage: error.message,
      });
    });

    proc.on("close", (code, signal) => {
      const durationMs = Date.now() - start;
      const timedOut = signal === "SIGKILL";
      const exitCode = code ?? (timedOut ? 137 : 1);
      finish({
        command,
        exitCode,
        stdout,
        stderr,
        durationMs,
        timedOut,
      });
    });

    const timeoutMs = DEFAULT_TIMEOUT_MS;
    const timeoutHandle = setTimeout(() => {
      if (finished) return;
      proc.kill("SIGKILL");
      // close handler will resolve the promise
    }, timeoutMs);
  });

export const runVerificationOnHost = (
  commands: string[],
  cwd: string,
  emit: (event: OrchestratorEvent) => void,
): Effect.Effect<VerificationRunResult, Error, never> =>
  Effect.gen(function* () {
    const results = yield* Effect.forEach(
      commands,
      (command) => runCommand(command, cwd, emit),
      { concurrency: 1 },
    );

    const passed = results.every((result) => result.exitCode === 0 && !result.timedOut);
    const outputs = results.map((result) => summarizeOutput(result.stdout, result.stderr));

    return { passed, results, outputs };
  });
