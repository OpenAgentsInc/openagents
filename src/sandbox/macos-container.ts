import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { Effect, Layer, Stream } from "effect";
import { ContainerBackendTag, type ContainerBackend } from "./backend.js";
import {
  ContainerError,
  type ContainerConfig,
  type ContainerRunResult,
} from "./schema.js";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const CONTAINER_CLI = "container";
const MAX_OUTPUT_SIZE = 10 * 1024 * 1024; // 10 MB (same as bash tool)
const DEFAULT_TIMEOUT_MS = 300_000; // 5 minutes

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Collect stream with size limit (from bash.ts pattern)
// ─────────────────────────────────────────────────────────────────────────────

const collectLimited = (
  stream: Stream.Stream<Uint8Array, unknown, unknown>,
): Effect.Effect<string, unknown, unknown> =>
  Stream.runFold(stream, "", (acc, chunk) => {
    if (acc.length >= MAX_OUTPUT_SIZE) {
      return acc;
    }
    const remaining = MAX_OUTPUT_SIZE - acc.length;
    const text = Buffer.from(chunk.subarray(0, remaining)).toString("utf-8");
    return acc + text;
  });

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

  const run: ContainerBackend["run"] = (command, config, _options) =>
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

        // Collect output
        const baseCollect = Effect.all([
          collectLimited(process.stdout as any),
          collectLimited(process.stderr as any),
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

        const [_stdout, stderr, exitCode] = yield* Effect.all([
          collectLimited(process.stdout as any),
          collectLimited(process.stderr as any),
          process.exitCode,
        ] as const).pipe(
          Effect.mapError(
            (e) => new ContainerError("execution_failed", String(e)),
          ),
        );

        if (Number(exitCode) !== 0) {
          yield* Effect.fail(
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

/** Layer with CommandExecutor dependency provided (for standalone use) */
export const macOSContainerLive = Layer.provide(
  macOSContainerLayer,
  CommandExecutor.layer,
);
