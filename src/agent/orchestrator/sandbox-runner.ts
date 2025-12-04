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
  type ContainerRunOptions,
} from "../../sandbox/index.js";
import {
  createCredentialMount,
  cleanupCredentialMount,
  type CredentialMount,
} from "../../sandbox/credentials.js";
import type {
  HudMessage,
  ExecutionContext,
} from "../../hud/protocol.js";

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
  /** Emit HUD messages for streaming output to UI */
  emitHud?: (message: HudMessage) => void;
  /** Execution context for UI grouping (default: "verification") */
  context?: ExecutionContext;
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
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/** Generate a unique execution ID for correlating container events */
const generateExecutionId = (): string =>
  `exec-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;

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
 * Run a command on the host (no sandbox) with optional streaming callbacks.
 */
const runOnHostWithCallbacks = (
  command: string[],
  cwd: string,
  options?: {
    timeoutMs?: number | undefined;
    onStdout?: (chunk: string) => void;
    onStderr?: (chunk: string) => void;
  }
): Effect.Effect<CommandResult, Error, never> =>
  Effect.async((resume) => {
    const { spawn } = require("node:child_process") as typeof import("node:child_process");

    // Join command array into shell command string
    const cmdString = command.join(" ");
    const proc = spawn(cmdString, {
      cwd,
      shell: true,
      stdio: ["pipe", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    let finished = false;

    // Set timeout
    const timeoutMs = options?.timeoutMs ?? 120000;
    const timeoutHandle = setTimeout(() => {
      if (!finished) {
        proc.kill("SIGKILL");
        finished = true;
        resume(Effect.fail(new Error(`Host command timed out after ${timeoutMs}ms`)));
      }
    }, timeoutMs);

    proc.stdout?.on("data", (data: Buffer) => {
      const chunk = data.toString("utf-8");
      stdout += chunk;
      options?.onStdout?.(chunk);
    });

    proc.stderr?.on("data", (data: Buffer) => {
      const chunk = data.toString("utf-8");
      stderr += chunk;
      options?.onStderr?.(chunk);
    });

    proc.on("error", (err: Error) => {
      if (!finished) {
        clearTimeout(timeoutHandle);
        finished = true;
        resume(Effect.fail(new Error(`Host command failed: ${err.message}`)));
      }
    });

    proc.on("close", (code: number | null) => {
      if (!finished) {
        clearTimeout(timeoutHandle);
        finished = true;
        resume(Effect.succeed({
          exitCode: code ?? 1,
          stdout,
          stderr,
          sandboxed: false,
        }));
      }
    });

    // Cleanup on abort
    return Effect.sync(() => {
      if (!finished) {
        clearTimeout(timeoutHandle);
        proc.kill("SIGKILL");
      }
    });
  });

/**
 * Run a command on the host (no sandbox).
 * @deprecated Use runOnHostWithCallbacks for streaming support
 */
const runOnHost = (
  command: string[],
  cwd: string,
  timeoutMs?: number
): Effect.Effect<CommandResult, Error, never> =>
  runOnHostWithCallbacks(command, cwd, { timeoutMs });

// ─────────────────────────────────────────────────────────────────────────────
// Sandboxed Execution
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a command in a sandbox container.
 */
const runInSandboxContainer = (
  command: string[],
  containerConfig: ContainerConfig,
  streamingOptions?: ContainerRunOptions
): Effect.Effect<CommandResult, Error, never> =>
  Effect.gen(function* () {
    const result = yield* Effect.provide(
      runInContainer(command, containerConfig, streamingOptions),
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
 * 4. Emits HUD events for real-time streaming to UI
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
    const executionId = generateExecutionId();
    let stdoutSeq = 0;
    let stderrSeq = 0;

    const sandboxAvailable = yield* checkSandboxAvailable(
      config.sandboxConfig,
      config.emit,
    );

    config.emit?.({
      type: "sandbox_command_start",
      command,
      inContainer: sandboxAvailable,
    });

    // Emit container_start HUD event
    config.emitHud?.({
      type: "container_start",
      executionId,
      image: sandboxAvailable ? (config.sandboxConfig.image ?? DEFAULT_SANDBOX_IMAGE) : "host",
      command,
      context: config.context ?? "verification",
      sandboxed: sandboxAvailable,
      workdir: config.cwd,
      timestamp: new Date().toISOString(),
    });

    // Create streaming callbacks for HUD
    const streamingCallbacks = {
      onStdout: (chunk: string) => {
        config.emitHud?.({
          type: "container_output",
          executionId,
          text: chunk,
          stream: "stdout",
          sequence: ++stdoutSeq,
          sandboxed: sandboxAvailable,
        });
      },
      onStderr: (chunk: string) => {
        config.emitHud?.({
          type: "container_output",
          executionId,
          text: chunk,
          stream: "stderr",
          sequence: ++stderrSeq,
          sandboxed: sandboxAvailable,
        });
      },
    };

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

      // Try running in container with streaming, fall back to host on error
      const containerResult = yield* runInSandboxContainer(command, containerConfig, streamingCallbacks)
        .pipe(
          Effect.catchAll((error) => {
            config.emit?.({ type: "sandbox_fallback", reason: error.message });
            return runOnHostWithCallbacks(command, config.cwd, {
              timeoutMs: config.sandboxConfig.timeoutMs,
              ...streamingCallbacks,
            });
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
      result = yield* runOnHostWithCallbacks(command, config.cwd, {
        timeoutMs: config.sandboxConfig.timeoutMs,
        ...streamingCallbacks,
      });
    }

    const durationMs = Date.now() - startTime;

    config.emit?.({
      type: "sandbox_command_complete",
      command,
      exitCode: result.exitCode,
      durationMs,
    });

    // Emit container_complete HUD event
    config.emitHud?.({
      type: "container_complete",
      executionId,
      exitCode: result.exitCode,
      durationMs,
      sandboxed: result.sandboxed,
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
