/**
 * Sandbox Runner
 *
 * Provides sandboxed execution for orchestrator operations.
 * When sandbox is enabled, commands run inside a container with the workspace mounted.
 * Falls back to host execution when sandbox is disabled or unavailable.
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import { spawn } from "node:child_process";
import type { SandboxConfig } from "../../tasks/schema.js";
import {
  runInContainer,
  isContainerAvailable,
  autoDetectLayer,
  type ContainerConfig,
  type ContainerRunOptions,
} from "../../sandbox/index.js";
import {
  createSandboxHudAdapter,
  type SandboxHudAdapter,
} from "../../sandbox/hud-adapter.js";
import {
  createCredentialMount,
  cleanupCredentialMount,
  type CredentialMount,
} from "../../sandbox/credentials.js";
import type {
  HudMessage,
  ExecutionContext,
} from "../../hud/protocol.js";
import * as nodeFs from "node:fs";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export interface SandboxRunnerConfig {
  /** Sandbox configuration from project.json */
  sandboxConfig: SandboxConfig;
  /** Working directory (repo root) to mount as /workspace */
  cwd: string;
  /** Emit events for logging/debugging */
  emit?: ((event: SandboxRunnerEvent) => void) | undefined;
  /** Emit HUD messages for streaming output to UI */
  emitHud?: ((message: HudMessage) => void) | undefined;
  /** Execution context for UI grouping (default: "verification") */
  context?: ExecutionContext | undefined;
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

interface SandboxBackendRunOptions {
  command: string[];
  cwd: string;
  sandboxConfig: SandboxConfig;
  env?: Record<string, string>;
  hudAdapter?: SandboxHudAdapter;
}

interface SandboxBackend {
  name: string;
  sandboxed: boolean;
  run: (
    options: SandboxBackendRunOptions,
  ) => Effect.Effect<CommandResult, Error, never>;
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
 * Run a command on the host (no sandbox) with optional streaming callbacks.
 */
const runOnHostWithCallbacks = (
  command: string[],
  cwd: string,
  options?: {
    timeoutMs?: number | undefined;
    onStdout?: (chunk: string) => void;
    onStderr?: (chunk: string) => void;
    env?: Record<string, string>;
  }
): Effect.Effect<CommandResult, Error, never> =>
  Effect.async((resume) => {
    // Join command array into shell command string
    const cmdString = command.join(" ");
    const proc = spawn(cmdString, {
      cwd,
      shell: true,
      stdio: ["pipe", "pipe", "pipe"],
      env: options?.env
        ? { ...process.env, ...options.env }
        : process.env,
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
// Backend Implementations
// ─────────────────────────────────────────────────────────────────────────────

const hostBackend: SandboxBackend = {
  name: "host",
  sandboxed: false,
  run: ({ command, cwd, sandboxConfig, hudAdapter, env }) =>
    runOnHostWithCallbacks(command, cwd, {
      timeoutMs: sandboxConfig.timeoutMs,
      ...(hudAdapter?.callbacks.onStdout ? { onStdout: hudAdapter.callbacks.onStdout } : {}),
      ...(hudAdapter?.callbacks.onStderr ? { onStderr: hudAdapter.callbacks.onStderr } : {}),
      ...(env ? { env } : {}),
    }),
};

const createContainerBackend = (): SandboxBackend => ({
  name: "container",
  sandboxed: true,
  run: ({ command, cwd, sandboxConfig, hudAdapter, env }) =>
    Effect.gen(function* () {
      const credentialMount: CredentialMount | null = yield* createCredentialMount()
        .pipe(
          Effect.provide(BunContext.layer),
          Effect.catchAll((err) => {
            console.warn(`[sandbox] Credential injection skipped: ${err.message}`);
            return Effect.succeed(null);
          }),
        );

      const volumeMounts = credentialMount ? [credentialMount.volumeMount] : [];
      const containerConfig = buildContainerConfig(sandboxConfig, cwd, {
        ...(env ? { env } : {}),
        ...(volumeMounts.length > 0 ? { volumeMounts } : {}),
      });

      const result = yield* runInSandboxContainer(command, containerConfig, hudAdapter?.callbacks).pipe(
        Effect.ensuring(
          credentialMount
            ? cleanupCredentialMount(credentialMount).pipe(
                Effect.provide(BunContext.layer),
                Effect.catchAll(() => Effect.void),
              )
            : Effect.void,
        ),
      );

      return result;
    }),
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
    // Validate working directory exists before attempting to run
    // This prevents cryptic container errors when worktrees are corrupted/removed
    if (!nodeFs.existsSync(config.cwd)) {
      config.emit?.({
        type: "sandbox_fallback",
        reason: `Working directory does not exist: ${config.cwd}`,
      });
      return yield* Effect.fail(
        new Error(`Working directory does not exist: ${config.cwd}. The worktree may have been corrupted or removed during execution.`),
      );
    }

    const sandboxAvailable = yield* checkSandboxAvailable(
      config.sandboxConfig,
      config.emit,
    );

    const backend = sandboxAvailable ? createContainerBackend() : hostBackend;
    const hudAdapter = createSandboxHudAdapter(config.emitHud);

    config.emit?.({
      type: "sandbox_command_start",
      command,
      inContainer: backend.sandboxed,
    });

    hudAdapter.emitStart({
      command,
      sandboxed: backend.sandboxed,
      image: backend.sandboxed
        ? config.sandboxConfig.image ?? DEFAULT_SANDBOX_IMAGE
        : "host",
      workdir: config.cwd,
      context: config.context ?? "verification",
    });

  const runWithBackend = (target: SandboxBackend) =>
    target.run({
      command,
      cwd: config.cwd,
      sandboxConfig: config.sandboxConfig,
      ...(env ? { env } : {}),
      hudAdapter,
    });

    const startTime = Date.now();

    const result = yield* (
      backend.sandboxed
        ? runWithBackend(backend).pipe(
            Effect.catchAll((error) => {
              config.emit?.({ type: "sandbox_fallback", reason: error.message });
              hudAdapter.setSandboxed(false);
              return runWithBackend(hostBackend);
            }),
          )
        : runWithBackend(backend)
    );

    const durationMs = Date.now() - startTime;

    config.emit?.({
      type: "sandbox_command_complete",
      command,
      exitCode: result.exitCode,
      durationMs,
    });

    hudAdapter.emitComplete(result.exitCode, durationMs);

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
