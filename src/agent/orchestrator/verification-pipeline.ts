import { Effect } from "effect";
import { execSync } from "node:child_process";
import type { OrchestratorEvent } from "./types.js";
import {
  runVerificationOnHost,
  type VerificationRunResult,
} from "./verification-runner.js";
import {
  runVerificationWithSandbox,
  type SandboxRunnerConfig,
} from "./sandbox-runner.js";

interface BuildPlanOptions {
  typecheckCommands?: string[];
  testCommands: string[];
  sandboxTestCommands?: string[];
  e2eCommands?: string[];
  taskLabels?: readonly string[];
  useSandbox?: boolean;
}

export interface VerificationPlan {
  verificationCommands: string[];
  e2eCommands: string[];
  runE2e: boolean;
  useSandbox: boolean;
}

export interface VerificationPipelineOptions {
  plan: VerificationPlan;
  cwd: string;
  emit: (event: OrchestratorEvent) => void;
  sandboxConfig?: SandboxRunnerConfig | undefined;
}

export interface VerificationPipelineResult {
  verification: VerificationRunResult;
  e2e?: {
    ran: boolean;
    passed: boolean;
    outputs: string[];
  };
}

export interface VerificationPipelineDeps {
  runHost?: typeof runVerificationOnHost;
  runSandbox?: typeof runVerificationWithSandbox;
  runE2eHost?: typeof runE2eOnHost;
}

const buildVerificationCommands = (
  typecheckCommands: string[] | undefined,
  testCommands: string[],
  sandboxTestCommands: string[] | undefined,
  useSandbox: boolean,
): string[] => {
  const effectiveTestCommands =
    useSandbox && sandboxTestCommands && sandboxTestCommands.length > 0
      ? sandboxTestCommands
      : testCommands;

  // Skip typecheck inside sandbox to avoid memory pressure
  const effectiveTypecheckCommands = useSandbox ? [] : typecheckCommands ?? [];

  return [...effectiveTypecheckCommands, ...effectiveTestCommands];
};

const buildE2eCommands = (commands: string[] | undefined): string[] =>
  commands?.filter((cmd) => cmd.trim().length > 0) ?? [];

const shouldRunE2e = (
  taskLabels: readonly string[] = [],
  e2eCommandsConfigured = false,
): boolean => {
  const skipE2eLabels = ["skip-e2e", "no-e2e", "unit-only"];
  const hasSkipLabel = taskLabels.some((label) =>
    skipE2eLabels.includes(label.toLowerCase()),
  );

  if (e2eCommandsConfigured) {
    return !hasSkipLabel;
  }

  const e2eLabels = ["e2e", "golden-loop", "integration"];
  return taskLabels.some((label) => e2eLabels.includes(label.toLowerCase()));
};

export const buildVerificationPlan = (options: BuildPlanOptions): VerificationPlan => {
  const useSandbox = options.useSandbox === true;
  const verificationCommands = buildVerificationCommands(
    options.typecheckCommands,
    options.testCommands,
    options.sandboxTestCommands,
    useSandbox,
  );

  const e2eCommands = buildE2eCommands(options.e2eCommands);
  const runE2e =
    e2eCommands.length > 0 &&
    shouldRunE2e(options.taskLabels ?? [], e2eCommands.length > 0);

  return { verificationCommands, e2eCommands, runE2e, useSandbox };
};

const mapSandboxResultToVerification = (
  commands: string[],
  result: { passed: boolean; outputs: string[] },
): VerificationRunResult => {
  const results = result.outputs.map((output, idx) => ({
    command: commands[idx] ?? `command-${idx + 1}`,
    exitCode: result.passed ? 0 : 1,
    stdout: output,
    stderr: "",
    durationMs: 0,
  }));

  return {
    passed: result.passed,
    outputs: result.outputs,
    results,
  };
};

const runE2eOnHost = (
  commands: string[],
  cwd: string,
  emit: (event: OrchestratorEvent) => void,
): Effect.Effect<{ passed: boolean; outputs: string[] }, Error, never> =>
  Effect.try({
    try: () => {
      const outputs: string[] = [];
      let allPassed = true;

      for (const cmd of commands) {
        emit({ type: "e2e_start", command: cmd });
        try {
          const output = execSync(cmd, {
            cwd,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            timeout: 300000,
          });
          outputs.push(String(output));
          emit({ type: "e2e_complete", command: cmd, passed: true, output: String(output) });
        } catch (error: any) {
          const output = String(error?.stdout || error?.stderr || error?.message || error);
          outputs.push(output);
          emit({ type: "e2e_complete", command: cmd, passed: false, output });
          allPassed = false;
        }
      }

      return { passed: allPassed, outputs };
    },
    catch: (error: any) => error as Error,
  });

export const runVerificationPipeline = (
  options: VerificationPipelineOptions,
  deps: VerificationPipelineDeps = {},
): Effect.Effect<VerificationPipelineResult, Error, never> =>
  Effect.gen(function* () {
    const runHost = deps.runHost ?? runVerificationOnHost;
    const runSandbox = deps.runSandbox ?? runVerificationWithSandbox;
    const runE2eHost = deps.runE2eHost ?? runE2eOnHost;

    const verificationCommands = options.plan.verificationCommands;
    const verificationResult = yield* (options.plan.useSandbox && options.sandboxConfig
      ? runSandbox(verificationCommands, options.sandboxConfig, options.emit as any).pipe(
          Effect.map((result) => mapSandboxResultToVerification(verificationCommands, result)),
          Effect.catchAll(() => runHost(verificationCommands, options.cwd, options.emit)),
        )
      : runHost(verificationCommands, options.cwd, options.emit));

    let e2eResult: VerificationPipelineResult["e2e"];

  if (options.plan.runE2e) {
    const result = yield* runE2eHost(options.plan.e2eCommands, options.cwd, options.emit);
    e2eResult = { ran: true, passed: result.passed, outputs: result.outputs };
  } else if (options.plan.e2eCommands.length > 0) {
    e2eResult = { ran: false, passed: true, outputs: [] };
    options.emit({
      type: "e2e_skipped",
      reason: "Task has skip-e2e label",
    } as any);
  }

    return {
      verification: verificationResult,
      ...(e2eResult ? { e2e: e2eResult } : {}),
    };
  });
