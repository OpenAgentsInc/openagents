import * as BunContext from "@effect/platform-bun/BunContext";
import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { Effect, Layer, Stream } from "effect";
import { ContainerBackendTag, type ContainerBackend } from "./backend.js";
import {
  ContainerError,
  type ContainerRunResult,
} from "./schema.js";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const CONTAINER_CLI = "container";
const MAX_OUTPUT_SIZE = 10 * 1024 * 1024; // 10 MB (same as bash tool)
const DEFAULT_TIMEOUT_MS = 300_000; // 5 minutes

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Collect stream with size limit and optional callback (from bash.ts pattern)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Collect a stream into a string with size limit, optionally streaming chunks
 * to a callback as they arrive.
 *
 * @param stream - The byte stream to collect
 * @param onChunk - Optional callback to receive each chunk as it arrives
 */
const collectWithCallback = <E, R>(
  stream: Stream.Stream<Uint8Array, E, R>,
  onChunk?: (text: string) => void,
): Effect.Effect<string, E, R> =>
  Stream.runFold(stream, "", (acc, chunk) => {
    const text = Buffer.from(chunk).toString("utf-8");

    // Stream to callback if provided
    if (onChunk && text.length > 0) {
      onChunk(text);
    }

    // Accumulate with size limit
    if (acc.length >= MAX_OUTPUT_SIZE) {
      return acc;
    }
    const remaining = MAX_OUTPUT_SIZE - acc.length;
    return acc + text.slice(0, remaining);
  });

/** @deprecated Use collectWithCallback instead */
const collectLimited = <E, R>(
  stream: Stream.Stream<Uint8Array, E, R>,
): Effect.Effect<string, E, R> => collectWithCallback(stream);

// ─────────────────────────────────────────────────────────────────────────────
// Implementation
// ─────────────────────────────────────────────────────────────────────────────

const makeMacOSContainerBackend = Effect.gen(function* () {
  const executor = yield* CommandExecutor.CommandExecutor;

  const isAvailable: ContainerBackend["isAvailable"] = () =>
    Effect.gen(function* () {
      // Check 1: Is macOS?
      if (process.platform !== "darwin") {
        return false;
      }

      // Check 2: Is `container` CLI available?
      const whichCmd = Command.make("which", CONTAINER_CLI);
      const whichResult = yield* Effect.either(
        Effect.scoped(
          Effect.gen(function* () {
            const proc = yield* executor.start(whichCmd);
            return yield* proc.exitCode;
          }),
        ),
      );
      if (whichResult._tag === "Left" || whichResult.right !== 0) {
        return false;
      }

      // Check 3: Is container system running?
      const statusCmd = Command.make(CONTAINER_CLI, "system", "status");
      const statusResult = yield* Effect.either(
        Effect.scoped(
          Effect.gen(function* () {
            const proc = yield* executor.start(statusCmd);
            return yield* proc.exitCode;
          }),
        ),
      );
      return statusResult._tag === "Right" && statusResult.right === 0;
    });

  const run: ContainerBackend["run"] = (command, config, options) =>
    Effect.scoped(
      Effect.gen(function* () {
        // Build argument list
        const args: string[] = ["run"];

        // Auto-remove unless explicitly disabled
        if (config.autoRemove !== false) {
          args.push("--rm");
        }

        // Volume mount: host:container
        args.push("-v", `${config.workspaceDir}:/workspace`);

        // Additional volume mounts (for credentials, etc.)
        if (config.volumeMounts) {
          for (const mount of config.volumeMounts) {
            args.push("-v", mount);
          }
        }

        // Working directory
        args.push("-w", config.workdir ?? "/workspace");

        // Resource limits
        if (config.memoryLimit) {
          args.push("--memory", config.memoryLimit);
        }
        if (config.cpuLimit) {
          args.push("--cpus", String(config.cpuLimit));
        }

        // Environment variables
        if (config.env) {
          for (const [key, value] of Object.entries(config.env)) {
            args.push("-e", `${key}=${value}`);
          }
        }

        // Image and command
        args.push(config.image, ...command);

        // Create and run command
        const cmd = Command.make(CONTAINER_CLI, ...args);

        const process = yield* Effect.acquireRelease(
          executor.start(cmd),
          (proc) =>
            proc.isRunning.pipe(
              Effect.flatMap((running) =>
                running ? proc.kill("SIGKILL") : Effect.void,
              ),
              Effect.orElse(() => Effect.void),
            ),
        ).pipe(
          Effect.mapError(
            (e) => new ContainerError("start_failed", String(e)),
          ),
        );

        // Collect output with streaming callbacks (cast streams to proper type)
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const stdoutStream = process.stdout as Stream.Stream<Uint8Array, never, never>;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const stderrStream = process.stderr as Stream.Stream<Uint8Array, never, never>;

        const baseCollect = Effect.all([
          collectWithCallback(stdoutStream, options?.onStdout),
          collectWithCallback(stderrStream, options?.onStderr),
          process.exitCode,
        ] as const).pipe(
          Effect.mapError(
            (e) => new ContainerError("execution_failed", String(e)),
          ),
        );

        // Apply timeout
        const timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS;
        const withTimeout = Effect.timeoutFail(baseCollect, {
          duration: timeoutMs,
          onTimeout: () =>
            new ContainerError(
              "timeout",
              `Container execution timed out after ${timeoutMs}ms`,
            ),
        });

        const [stdout, stderr, exitCode] = yield* withTimeout;

        return {
          exitCode: Number(exitCode),
          stdout,
          stderr,
        } satisfies ContainerRunResult;
      }),
    );

  const build: ContainerBackend["build"] = (contextDir, tag, options) =>
    Effect.scoped(
      Effect.gen(function* () {
        const args: string[] = ["build", "-t", tag];

        if (options?.file) {
          args.push("-f", options.file);
        }
        if (options?.memoryLimit) {
          args.push("--memory", options.memoryLimit);
        }
        if (options?.cpuLimit) {
          args.push("--cpus", String(options.cpuLimit));
        }

        args.push(contextDir);

        const cmd = Command.make(CONTAINER_CLI, ...args);

        const process = yield* Effect.acquireRelease(
          executor.start(cmd),
          (proc) =>
            proc.isRunning.pipe(
              Effect.flatMap((running) =>
                running ? proc.kill("SIGKILL") : Effect.void,
              ),
              Effect.orElse(() => Effect.void),
            ),
        ).pipe(
          Effect.mapError(
            (e) => new ContainerError("start_failed", String(e)),
          ),
        );

        // Cast streams to proper type
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const stdoutStream = process.stdout as Stream.Stream<Uint8Array, never, never>;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const stderrStream = process.stderr as Stream.Stream<Uint8Array, never, never>;

        const [_stdout, stderr, exitCode] = yield* Effect.all([
          collectLimited(stdoutStream),
          collectLimited(stderrStream),
          process.exitCode,
        ] as const).pipe(
          Effect.mapError(
            (e) => new ContainerError("execution_failed", String(e)),
          ),
        );

        if (Number(exitCode) !== 0) {
          return yield* Effect.fail(
            new ContainerError(
              "execution_failed",
              `Build failed: ${stderr}`,
              Number(exitCode),
            ),
          );
        }
      }),
    );

  return {
    name: "macos-container",
    isAvailable,
    run,
    build,
  } satisfies ContainerBackend;
});

// ─────────────────────────────────────────────────────────────────────────────
// Layer
// ─────────────────────────────────────────────────────────────────────────────

export const macOSContainerLayer = Layer.effect(
  ContainerBackendTag,
  makeMacOSContainerBackend,
);

/** Layer with BunContext dependency provided (for standalone use) */
export const macOSContainerLive = Layer.provide(
  macOSContainerLayer,
  BunContext.layer,
);
