/**
 * Sandbox Runner
 *
 * Provides sandboxed execution for orchestrator operations.
 * When sandbox is enabled, commands run inside a container with the workspace mounted.
 * Falls back to host execution when sandbox is disabled or unavailable.
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import type { SandboxConfig } from "../../tasks/schema.js";
import {
  runInContainer,
  isContainerAvailable,
  autoDetectLayer,
  type ContainerConfig,
} from "../../sandbox/index.js";
import {
  createCredentialMount,
  cleanupCredentialMount,
  type CredentialMount,
} from "../../sandbox/credentials.js";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export interface SandboxRunnerConfig {
  /** Sandbox configuration from project.json */
  sandboxConfig: SandboxConfig;
  /** Working directory (repo root) to mount as /workspace */
  cwd: string;
  /** Emit events for logging/debugging */
  emit?: (event: SandboxRunnerEvent) => void;
}

export type SandboxRunnerEvent =
  | { type: "sandbox_check_start" }
  | { type: "sandbox_available"; backend: string }
  | { type: "sandbox_unavailable"; reason: string }
  | { type: "sandbox_command_start"; command: string[]; inContainer: boolean }
  | { type: "sandbox_command_complete"; command: string[]; exitCode: number; durationMs: number }
  | { type: "sandbox_fallback"; reason: string };

export interface CommandResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  /** Whether the command ran in a container or on host */
  sandboxed: boolean;
}

// ─────────────────────────────────────────────────────────────────────────────
// Default Image
// ─────────────────────────────────────────────────────────────────────────────

const DEFAULT_SANDBOX_IMAGE = "oven/bun:latest";

// ─────────────────────────────────────────────────────────────────────────────
// Sandbox Availability Check
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Check if sandbox execution is available.
 * Returns true if:
 * 1. sandbox.enabled is true in config
 * 2. sandbox.backend is not "none"
 * 3. A container backend is actually available on the system
 */
export const checkSandboxAvailable = (
  config: SandboxConfig,
  emit?: SandboxRunnerConfig["emit"]
): Effect.Effect<boolean, never, never> =>
  Effect.gen(function* () {
    emit?.({ type: "sandbox_check_start" });

    // Check if explicitly disabled
    if (config.enabled === false) {
      emit?.({ type: "sandbox_unavailable", reason: "sandbox.enabled is false" });
      return false;
    }

    // Check if backend is explicitly "none"
    if (config.backend === "none") {
      emit?.({ type: "sandbox_unavailable", reason: "sandbox.backend is 'none'" });
      return false;
    }

    // Check if container backend is available
    const available = yield* Effect.provide(
      isContainerAvailable(),
      autoDetectLayer
    ).pipe(
      Effect.catchAll(() => Effect.succeed(false))
    );

    if (available) {
      emit?.({ type: "sandbox_available", backend: config.backend ?? "auto" });
    } else {
      emit?.({ type: "sandbox_unavailable", reason: "no container backend available" });
    }

    return available;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Container Config Builder
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Build ContainerConfig from SandboxConfig and working directory.
 */
export const buildContainerConfig = (
  sandboxConfig: SandboxConfig,
  cwd: string,
  options?: {
    env?: Record<string, string>;
    volumeMounts?: string[];
  },
): ContainerConfig => ({
  image: sandboxConfig.image ?? DEFAULT_SANDBOX_IMAGE,
  workspaceDir: cwd,
  workdir: "/workspace",
  ...(sandboxConfig.memoryLimit ? { memoryLimit: sandboxConfig.memoryLimit } : {}),
  ...(sandboxConfig.cpuLimit ? { cpuLimit: sandboxConfig.cpuLimit } : {}),
  ...(sandboxConfig.timeoutMs ? { timeoutMs: sandboxConfig.timeoutMs } : {}),
  ...(options?.env ? { env: options.env } : {}),
  ...(options?.volumeMounts ? { volumeMounts: options.volumeMounts } : {}),
  autoRemove: true,
});

// ─────────────────────────────────────────────────────────────────────────────
// Host Execution
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a command on the host (no sandbox).
 */
const runOnHost = (
  command: string[],
  cwd: string,
  timeoutMs?: number
): Effect.Effect<CommandResult, Error, never> =>
  Effect.try({
    try: () => {
      const { execSync } = require("node:child_process") as typeof import("node:child_process");

      // Join command array into shell command string
      const cmdString = command.join(" ");

      try {
        const output = execSync(cmdString, {
          cwd,
          encoding: "utf-8",
          stdio: ["pipe", "pipe", "pipe"],
          timeout: timeoutMs ?? 120000,
        });

        return {
          exitCode: 0,
          stdout: String(output),
          stderr: "",
          sandboxed: false,
        };
      } catch (error: any) {
        // execSync throws on non-zero exit
        return {
          exitCode: error?.status ?? 1,
          stdout: String(error?.stdout ?? ""),
          stderr: String(error?.stderr ?? error?.message ?? ""),
          sandboxed: false,
        };
      }
    },
    catch: (error: any) => new Error(`Host command failed: ${error.message}`),
  });

// ─────────────────────────────────────────────────────────────────────────────
// Sandboxed Execution
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a command in a sandbox container.
 */
const runInSandboxContainer = (
  command: string[],
  containerConfig: ContainerConfig
): Effect.Effect<CommandResult, Error, never> =>
  Effect.gen(function* () {
    const result = yield* Effect.provide(
      runInContainer(command, containerConfig),
      autoDetectLayer
    ).pipe(
      Effect.mapError((e) => new Error(`Container execution failed: ${e.message}`))
    );

    return {
      exitCode: result.exitCode,
      stdout: result.stdout,
      stderr: result.stderr,
      sandboxed: true,
    };
  });

// ─────────────────────────────────────────────────────────────────────────────
// Main Runner API
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a shell command, using sandbox if available and enabled.
 *
 * This function:
 * 1. Checks if sandbox is available
 * 2. If available, injects Claude Code credentials and runs command in container
 * 3. If unavailable, falls back to host execution
 *
 * @param command - Command as array of strings (e.g., ["bun", "test"])
 * @param config - Sandbox runner configuration
 * @param env - Optional environment variables
 */
export const runCommand = (
  command: string[],
  config: SandboxRunnerConfig,
  env?: Record<string, string>,
): Effect.Effect<CommandResult, Error, never> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const sandboxAvailable = yield* checkSandboxAvailable(
      config.sandboxConfig,
      config.emit,
    );

    config.emit?.({
      type: "sandbox_command_start",
      command,
      inContainer: sandboxAvailable,
    });

    let result: CommandResult;

    if (sandboxAvailable) {
      // Try to create credential mount for Claude Code auth
      const credentialMount: CredentialMount | null = yield* createCredentialMount()
        .pipe(
          Effect.provide(BunContext.layer),
          Effect.catchAll((err) => {
            // Log warning but continue without credentials
            console.warn(`[sandbox] Credential injection skipped: ${err.message}`);
            return Effect.succeed(null);
          }),
        );

      const volumeMounts = credentialMount ? [credentialMount.volumeMount] : [];
      const containerConfig = buildContainerConfig(config.sandboxConfig, config.cwd, {
        ...(env ? { env } : {}),
        ...(volumeMounts.length > 0 ? { volumeMounts } : {}),
      });

      // Try running in container, fall back to host on error
      const containerResult = yield* runInSandboxContainer(command, containerConfig)
        .pipe(
          Effect.catchAll((error) => {
            config.emit?.({ type: "sandbox_fallback", reason: error.message });
            return runOnHost(command, config.cwd, config.sandboxConfig.timeoutMs);
          }),
          // Always cleanup credential mount after execution
          Effect.ensuring(
            credentialMount
              ? cleanupCredentialMount(credentialMount).pipe(
                  Effect.provide(BunContext.layer),
                  Effect.catchAll(() => Effect.void),
                )
              : Effect.void,
          ),
        );

      result = containerResult;
    } else {
      result = yield* runOnHost(command, config.cwd, config.sandboxConfig.timeoutMs);
    }

    config.emit?.({
      type: "sandbox_command_complete",
      command,
      exitCode: result.exitCode,
      durationMs: Date.now() - startTime,
    });

    return result;
  });

/**
 * Run a shell command string, using sandbox if available.
 * Parses the command string into arguments for container execution.
 *
 * @param commandString - Shell command string (e.g., "bun test")
 * @param config - Sandbox runner configuration
 * @param env - Optional environment variables
 */
export const runCommandString = (
  commandString: string,
  config: SandboxRunnerConfig,
  env?: Record<string, string>
): Effect.Effect<CommandResult, Error, never> => {
  // Parse command string into array
  // Simple parsing - splits on whitespace (doesn't handle quoted strings perfectly)
  const command = commandString.trim().split(/\s+/);
  return runCommand(command, config, env);
};

// ─────────────────────────────────────────────────────────────────────────────
// Verification Command Runner
// ─────────────────────────────────────────────────────────────────────────────

export interface VerificationResult {
  passed: boolean;
  outputs: string[];
  sandboxed: boolean;
}

/**
 * Run verification commands (typecheck, tests) with sandbox support.
 * This is the main integration point for the orchestrator.
 *
 * @param commands - Array of verification command strings
 * @param config - Sandbox runner configuration
 * @param emit - Optional event emitter for orchestrator events
 */
export const runVerificationWithSandbox = (
  commands: string[],
  config: SandboxRunnerConfig,
  emit?: (event: { type: string; [key: string]: any }) => void
): Effect.Effect<VerificationResult, Error, never> =>
  Effect.gen(function* () {
    const outputs: string[] = [];
    let allPassed = true;
    let anySandboxed = false;

    for (const cmd of commands) {
      emit?.({ type: "verification_start", command: cmd });

      const result = yield* runCommandString(cmd, config);
      const output = result.stdout + (result.stderr ? `\n${result.stderr}` : "");
      outputs.push(output);

      if (result.sandboxed) {
        anySandboxed = true;
      }

      const passed = result.exitCode === 0;
      emit?.({
        type: "verification_complete",
        command: cmd,
        passed,
        output,
        exitCode: result.exitCode,
        sandboxed: result.sandboxed,
      });

      if (!passed) {
        allPassed = false;
      }
    }

    return {
      passed: allPassed,
      outputs,
      sandboxed: anySandboxed,
    };
  });

// ─────────────────────────────────────────────────────────────────────────────
// Exports for Testing
// ─────────────────────────────────────────────────────────────────────────────

export { runOnHost as _runOnHost, runInSandboxContainer as _runInSandboxContainer };
