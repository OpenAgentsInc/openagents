import { Effect } from "effect";
import { execSync } from "node:child_process";
import { runVerificationWithSandbox, type SandboxRunnerConfig } from "./sandbox-runner.js";
import { runVerificationOnHost, type VerificationRunResult } from "./verification-runner.js";
import type { OrchestratorEvent } from "./types.js";

const SKIP_E2E_LABELS = ["skip-e2e", "no-e2e", "unit-only"];
const E2E_LABELS = ["e2e", "golden-loop", "integration"];
const DEFAULT_E2E_TIMEOUT_MS = 300_000;

export interface VerificationCommandRunnerConfig {
  cwd: string;
  emit: (event: OrchestratorEvent) => void;
  sandboxRunnerConfig?: SandboxRunnerConfig;
}

export interface E2eRunResult {
  ran: boolean;
  passed: boolean;
  outputs: string[];
  reason?: string;
}

export interface VerificationPipelineResult {
  verification: VerificationRunResult;
  e2e: E2eRunResult;
}

export interface VerificationPipelineOptions {
  typecheckCommands?: string[];
  testCommands: string[];
  sandboxTestCommands?: string[];
  e2eCommands?: string[];
  cwd: string;
  emit: (event: OrchestratorEvent) => void;
  sandboxRunnerConfig?: SandboxRunnerConfig;
  taskLabels?: readonly string[];
  verificationCommands?: string[];
}

export const buildVerificationCommands = (
  typecheckCommands: string[] | undefined,
  testCommands: string[],
  sandboxTestCommands?: string[],
  useSandbox?: boolean
): string[] => {
  const effectiveTestCommands =
    useSandbox && sandboxTestCommands && sandboxTestCommands.length > 0
      ? sandboxTestCommands
      : testCommands;

  const effectiveTypecheckCommands = useSandbox ? [] : typecheckCommands ?? [];

  return [...effectiveTypecheckCommands, ...effectiveTestCommands];
};

export const shouldRunE2e = (taskLabels: readonly string[] = [], e2eCommandsConfigured = false): boolean => {
  const normalizedLabels = taskLabels.map((label) => label.toLowerCase());
  const hasSkipLabel = normalizedLabels.some((label) => SKIP_E2E_LABELS.includes(label));

  if (e2eCommandsConfigured) {
    return !hasSkipLabel;
  }

  return normalizedLabels.some((label) => E2E_LABELS.includes(label));
};

export const buildE2eCommands = (commands: string[] | undefined): string[] =>
  commands?.filter((cmd) => cmd.trim().length > 0) ?? [];

export const runE2eOnHost = (
  commands: string[],
  config: { cwd: string; emit: (event: OrchestratorEvent) => void; timeoutMs?: number }
): Effect.Effect<{ passed: boolean; outputs: string[] }, Error, never> =>
  Effect.try({
    try: () => {
      const outputs: string[] = [];
      let allPassed = true;
      const timeoutMs = config.timeoutMs ?? DEFAULT_E2E_TIMEOUT_MS;

      for (const command of commands) {
        config.emit({ type: "e2e_start", command });
        try {
          const output = execSync(command, {
            cwd: config.cwd,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            timeout: timeoutMs,
          });
          outputs.push(String(output));
          config.emit({ type: "e2e_complete", command, passed: true, output: String(output) });
        } catch (error: any) {
          const output = String(error?.stdout || error?.stderr || error?.message || error);
          outputs.push(output);
          config.emit({ type: "e2e_complete", command, passed: false, output });
          allPassed = false;
        }
      }

      return { passed: allPassed, outputs };
    },
    catch: (error: any) => error as Error,
  });

export const runVerificationCommands = (
  commands: string[],
  config: VerificationCommandRunnerConfig
): Effect.Effect<VerificationRunResult, Error, never> => {
  if (commands.length === 0) {
    return Effect.succeed({ passed: true, results: [], outputs: [] });
  }

  const sandboxEnabled = config.sandboxRunnerConfig?.sandboxConfig.enabled !== false;

  if (config.sandboxRunnerConfig && sandboxEnabled) {
    const sandboxEmit = config.emit as (event: { type: string; [key: string]: any }) => void;
    return runVerificationWithSandbox(commands, config.sandboxRunnerConfig, sandboxEmit).pipe(
      Effect.map((result) => ({
        passed: result.passed,
        outputs: result.outputs,
        results: result.outputs.map((output, index) => ({
          command: commands[index] ?? `command-${index + 1}`,
          exitCode: result.passed ? 0 : 1,
          stdout: output,
          stderr: "",
          durationMs: 0,
        })),
      })),
      Effect.catchAll(() => runVerificationOnHost(commands, config.cwd, config.emit))
    );
  }

  return runVerificationOnHost(commands, config.cwd, config.emit);
};

export const runVerificationPipeline = (
  options: VerificationPipelineOptions
): Effect.Effect<VerificationPipelineResult, Error, never> =>
  Effect.gen(function* () {
    const useSandbox = options.sandboxRunnerConfig
      ? options.sandboxRunnerConfig.sandboxConfig.enabled !== false
      : false;
    const verificationCommands =
      options.verificationCommands ??
      buildVerificationCommands(
        options.typecheckCommands,
        options.testCommands,
        options.sandboxTestCommands,
        useSandbox
      );

    const verification = yield* runVerificationCommands(verificationCommands, {
      cwd: options.cwd,
      emit: options.emit,
      ...(options.sandboxRunnerConfig ? { sandboxRunnerConfig: options.sandboxRunnerConfig } : {}),
    });

    const e2eCommands = buildE2eCommands(options.e2eCommands);
    const e2eConfigured = e2eCommands.length > 0;
    const shouldRun = shouldRunE2e(options.taskLabels, e2eConfigured);

    if (shouldRun && e2eConfigured) {
      const e2eResult = yield* runE2eOnHost(e2eCommands, {
        cwd: options.cwd,
        emit: options.emit,
      });

      return {
        verification,
        e2e: {
          ran: true,
          passed: e2eResult.passed,
          outputs: e2eResult.outputs,
        },
      };
    }

    const skipReason = !e2eConfigured ? "No e2eCommands configured" : "Task has skip-e2e label";
    options.emit({ type: "e2e_skipped", reason: skipReason });

    return {
      verification,
      e2e: {
        ran: false,
        passed: true,
        outputs: [],
        reason: skipReason,
      },
    };
  });
