# Container Abstraction Implementation Plan

> Concrete implementation plan for container support in MechaCoder

## Overview

Create an Effect-based container abstraction layer that:
1. Defines a generic `ContainerBackend` interface
2. Implements macOS Container (Apple's `container` CLI) as the first backend
3. Integrates with MechaCoder's execution flow
4. Follows existing OpenAgents Effect patterns

## Architecture

```
src/sandbox/
├── schema.ts           # ContainerConfig schema, error types
├── backend.ts          # ContainerBackend Context.Tag + interface
├── macos-container.ts  # Apple Container implementation
├── detect.ts           # Auto-detection of available backends
├── index.ts            # Public exports + convenience functions
└── *.test.ts           # Tests for each module
```

---

## File 1: `src/sandbox/schema.ts`

Define schemas and error types following existing patterns.

```typescript
import * as S from "@effect/schema/Schema";

// ─────────────────────────────────────────────────────────────────────────────
// Error Types (following ToolExecutionError pattern from src/tools/schema.ts)
// ─────────────────────────────────────────────────────────────────────────────

export type ContainerErrorReason =
  | "not_available"      // Container runtime not installed/running
  | "image_not_found"    // Specified image doesn't exist
  | "start_failed"       // Container failed to start
  | "execution_failed"   // Command inside container failed
  | "timeout"            // Operation timed out
  | "aborted";           // User/signal aborted

export class ContainerError extends Error {
  readonly _tag = "ContainerError";
  constructor(
    readonly reason: ContainerErrorReason,
    message: string,
    readonly exitCode?: number,
  ) {
    super(message);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Schema
// ─────────────────────────────────────────────────────────────────────────────

export const ContainerConfigSchema = S.Struct({
  /** Image to run (e.g., "mechacoder:latest" or "oven/bun:latest") */
  image: S.String,

  /** Host directory to mount as /workspace inside container */
  workspaceDir: S.String,

  /** Working directory inside container (default: /workspace) */
  workdir: S.optional(S.String),

  /** Memory limit with optional suffix K/M/G (e.g., "4G") */
  memoryLimit: S.optional(S.String),

  /** Number of CPUs to allocate */
  cpuLimit: S.optional(S.Number),

  /** Environment variables to set */
  env: S.optional(S.Record({ key: S.String, value: S.String })),

  /** Timeout in milliseconds for the entire operation */
  timeoutMs: S.optional(S.Number),

  /** Remove container after it exits (default: true) */
  autoRemove: S.optional(S.Boolean),
});

export type ContainerConfig = S.Schema.Type<typeof ContainerConfigSchema>;

// ─────────────────────────────────────────────────────────────────────────────
// Execution Result
// ─────────────────────────────────────────────────────────────────────────────

export interface ContainerRunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  /** Container ID (useful for debugging) */
  containerId?: string;
}
```

---

## File 2: `src/sandbox/backend.ts`

Define the abstract backend interface as a Context.Tag.

```typescript
import { Context, Effect } from "effect";
import type { ContainerConfig, ContainerRunResult, ContainerError } from "./schema.js";

// ─────────────────────────────────────────────────────────────────────────────
// Backend Interface
// ─────────────────────────────────────────────────────────────────────────────

export interface ContainerBackend {
  /** Human-readable name for this backend */
  readonly name: string;

  /** Check if this backend is available on the current system */
  isAvailable: () => Effect.Effect<boolean, never, never>;

  /**
   * Run a command inside a container.
   *
   * @param command - Command and arguments to run
   * @param config - Container configuration
   * @param options - Optional abort signal
   */
  run: (
    command: string[],
    config: ContainerConfig,
    options?: { signal?: AbortSignal },
  ) => Effect.Effect<ContainerRunResult, ContainerError, never>;

  /**
   * Build an image from a Dockerfile/Containerfile.
   *
   * @param contextDir - Build context directory
   * @param tag - Tag for the built image
   * @param options - Build options
   */
  build: (
    contextDir: string,
    tag: string,
    options?: {
      file?: string;        // Path to Dockerfile/Containerfile
      memoryLimit?: string; // Builder memory limit
      cpuLimit?: number;    // Builder CPU limit
    },
  ) => Effect.Effect<void, ContainerError, never>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Context.Tag (modern class-based pattern)
// ─────────────────────────────────────────────────────────────────────────────

export class ContainerBackendTag extends Context.Tag("ContainerBackend")<
  ContainerBackendTag,
  ContainerBackend
>() {}
```

---

## File 3: `src/sandbox/macos-container.ts`

Implement Apple Container backend using `container` CLI.

```typescript
import { Effect, Layer } from "effect";
import { Command, CommandExecutor } from "@effect/platform";
import { ContainerBackendTag, type ContainerBackend } from "./backend.js";
import { ContainerError, type ContainerConfig, type ContainerRunResult } from "./schema.js";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const CONTAINER_CLI = "container";
const MAX_OUTPUT_SIZE = 10 * 1024 * 1024; // 10 MB (same as bash tool)
const DEFAULT_TIMEOUT_MS = 300_000; // 5 minutes

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Collect stream with size limit (from bash.ts pattern)
// ─────────────────────────────────────────────────────────────────────────────

const collectStream = (stream: Stream.Stream<Uint8Array>): Effect.Effect<string> =>
  Stream.runFold(stream, [] as Uint8Array[], (chunks, chunk) => {
    const totalSize = chunks.reduce((sum, c) => sum + c.length, 0);
    if (totalSize + chunk.length > MAX_OUTPUT_SIZE) {
      return chunks; // Stop collecting if we hit size limit
    }
    return [...chunks, chunk];
  }).pipe(
    Effect.map((chunks) => Buffer.concat(chunks).toString("utf-8")),
  );

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
        const args: string[] = ["run", "--rm"];

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
            proc.kill("SIGKILL").pipe(
              Effect.catchAll(() => Effect.void),
            ),
        );

        // Handle abort signal
        if (options?.signal) {
          options.signal.addEventListener("abort", () => {
            Effect.runSync(
              process.kill("SIGTERM").pipe(Effect.catchAll(() => Effect.void)),
            );
          });
        }

        // Collect output
        const [stdout, stderr, exitCode] = yield* Effect.all([
          collectStream(process.stdout),
          collectStream(process.stderr),
          process.exitCode,
        ]);

        return { exitCode, stdout, stderr } satisfies ContainerRunResult;
      }),
    ).pipe(
      // Apply timeout
      Effect.timeoutFail({
        duration: config.timeoutMs ?? DEFAULT_TIMEOUT_MS,
        onTimeout: () => new ContainerError("timeout", "Container execution timed out"),
      }),
      // Map other errors
      Effect.catchAll((error) => {
        if (error instanceof ContainerError) {
          return Effect.fail(error);
        }
        return Effect.fail(
          new ContainerError("execution_failed", String(error)),
        );
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
        const process = yield* executor.start(cmd);
        const exitCode = yield* process.exitCode;

        if (exitCode !== 0) {
          const stderr = yield* collectStream(process.stderr);
          yield* Effect.fail(
            new ContainerError("execution_failed", `Build failed: ${stderr}`, exitCode),
          );
        }
      }),
    ).pipe(
      Effect.catchAll((error) => {
        if (error instanceof ContainerError) {
          return Effect.fail(error);
        }
        return Effect.fail(
          new ContainerError("execution_failed", `Build failed: ${String(error)}`),
        );
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
```

---

## File 4: `src/sandbox/detect.ts`

Auto-detect the best available backend.

```typescript
import { Effect, Layer } from "effect";
import { ContainerBackendTag, type ContainerBackend } from "./backend.js";
import { ContainerError } from "./schema.js";
import { macOSContainerLive } from "./macos-container.js";

// ─────────────────────────────────────────────────────────────────────────────
// NoOp Backend (when no container runtime is available)
// ─────────────────────────────────────────────────────────────────────────────

const noopBackend: ContainerBackend = {
  name: "none",
  isAvailable: () => Effect.succeed(false),
  run: () =>
    Effect.fail(
      new ContainerError("not_available", "No container runtime available"),
    ),
  build: () =>
    Effect.fail(
      new ContainerError("not_available", "No container runtime available"),
    ),
};

// ─────────────────────────────────────────────────────────────────────────────
// Auto-detection
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Detect and return the best available container backend.
 *
 * Priority:
 * 1. macOS Container (if on macOS 26+ with `container` CLI)
 * 2. (Future: Docker, Seatbelt, etc.)
 * 3. NoOp backend (no sandboxing available)
 */
export const detectBackend = Effect.gen(function* () {
  // Try macOS Container
  if (process.platform === "darwin") {
    const macosBackend = yield* Effect.provide(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        const available = yield* backend.isAvailable();
        return available ? backend : null;
      }),
      macOSContainerLive,
    );
    if (macosBackend) {
      return macosBackend;
    }
  }

  // TODO: Add Docker backend check here
  // TODO: Add Seatbelt backend check here

  // Fallback to noop
  return noopBackend;
});

/**
 * Layer that auto-detects the best backend.
 * Use this when you want automatic backend selection.
 */
export const autoDetectLayer = Layer.effect(
  ContainerBackendTag,
  detectBackend,
);
```

---

## File 5: `src/sandbox/index.ts`

Public API exports and convenience functions.

```typescript
// Re-export types
export { ContainerError, type ContainerConfig, type ContainerRunResult } from "./schema.js";
export { ContainerBackendTag, type ContainerBackend } from "./backend.js";

// Re-export implementations
export { macOSContainerLayer, macOSContainerLive } from "./macos-container.js";
export { detectBackend, autoDetectLayer } from "./detect.js";

// ─────────────────────────────────────────────────────────────────────────────
// Convenience Functions
// ─────────────────────────────────────────────────────────────────────────────

import { Effect } from "effect";
import { ContainerBackendTag } from "./backend.js";
import type { ContainerConfig } from "./schema.js";

/**
 * Run a command in a sandboxed container.
 *
 * @example
 * ```typescript
 * const result = yield* runInContainer(
 *   ["bun", "test"],
 *   { image: "mechacoder:latest", workspaceDir: "/path/to/project" }
 * );
 * ```
 */
export const runInContainer = (
  command: string[],
  config: ContainerConfig,
  options?: { signal?: AbortSignal },
) =>
  Effect.gen(function* () {
    const backend = yield* ContainerBackendTag;
    return yield* backend.run(command, config, options);
  });

/**
 * Build a container image.
 *
 * @example
 * ```typescript
 * yield* buildImage("/path/to/project", "mechacoder:latest");
 * ```
 */
export const buildImage = (
  contextDir: string,
  tag: string,
  options?: {
    file?: string;
    memoryLimit?: string;
    cpuLimit?: number;
  },
) =>
  Effect.gen(function* () {
    const backend = yield* ContainerBackendTag;
    return yield* backend.build(contextDir, tag, options);
  });

/**
 * Check if any container backend is available.
 */
export const isContainerAvailable = () =>
  Effect.gen(function* () {
    const backend = yield* ContainerBackendTag;
    return yield* backend.isAvailable();
  });
```

---

## File 6: `src/sandbox/macos-container.test.ts`

Tests for the macOS Container backend.

```typescript
import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import { ContainerBackendTag } from "./backend.js";
import { macOSContainerLive } from "./macos-container.js";
import { autoDetectLayer } from "./detect.js";

const runWithMacOSContainer = <A, E>(
  effect: Effect.Effect<A, E, typeof ContainerBackendTag>,
) => Effect.runPromise(effect.pipe(Effect.provide(macOSContainerLive)));

const runWithAutoDetect = <A, E>(
  effect: Effect.Effect<A, E, typeof ContainerBackendTag>,
) => Effect.runPromise(effect.pipe(Effect.provide(autoDetectLayer)));

describe("macOS Container Backend", () => {
  test("isAvailable returns boolean", async () => {
    const result = await runWithMacOSContainer(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        return yield* backend.isAvailable();
      }),
    );
    expect(typeof result).toBe("boolean");
  });

  test("run executes command in container", async () => {
    const result = await runWithMacOSContainer(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        const available = yield* backend.isAvailable();
        if (!available) {
          return { skipped: true };
        }
        return yield* backend.run(["echo", "hello"], {
          image: "alpine:latest",
          workspaceDir: process.cwd(),
        });
      }),
    );

    if ("skipped" in result) {
      console.log("Skipped: macOS Container not available");
      return;
    }

    expect(result.exitCode).toBe(0);
    expect(result.stdout.trim()).toBe("hello");
  });

  test("run mounts workspace correctly", async () => {
    const result = await runWithMacOSContainer(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        const available = yield* backend.isAvailable();
        if (!available) {
          return { skipped: true };
        }
        // Check that /workspace exists and is mounted
        return yield* backend.run(["ls", "-la", "/workspace"], {
          image: "alpine:latest",
          workspaceDir: process.cwd(),
        });
      }),
    );

    if ("skipped" in result) {
      return;
    }

    expect(result.exitCode).toBe(0);
    // Should see files from the mounted directory
    expect(result.stdout).toContain("package.json");
  });
});

describe("Auto-detect", () => {
  test("detectBackend returns a backend", async () => {
    const result = await runWithAutoDetect(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        return backend.name;
      }),
    );

    // Should return either "macos-container" or "none"
    expect(["macos-container", "none"]).toContain(result);
  });
});
```

---

## Integration: MechaCoder Changes

### Update `.openagents/project.json` schema

Add to `src/tasks/project-service.ts`:

```typescript
// Add to ProjectConfig interface
sandbox?: {
  enabled: boolean;
  backend?: "macos-container" | "docker" | "auto";
  image: string;
  memoryLimit?: string;
  cpuLimit?: number;
};
```

### Update agent execution

In `src/agent/orchestrator/orchestrator.ts`, wrap tool execution:

```typescript
import { runInContainer, autoDetectLayer, isContainerAvailable } from "../sandbox/index.js";

// Before executing subagent tools, check if sandboxing is enabled
const executeInSandbox = (command: string[], config: ProjectConfig) =>
  Effect.gen(function* () {
    if (!config.sandbox?.enabled) {
      // Run directly (existing behavior)
      return yield* executeDirectly(command);
    }

    const available = yield* isContainerAvailable();
    if (!available) {
      console.warn("Sandbox requested but no container runtime available");
      return yield* executeDirectly(command);
    }

    return yield* runInContainer(command, {
      image: config.sandbox.image,
      workspaceDir: projectDir,
      memoryLimit: config.sandbox.memoryLimit,
      cpuLimit: config.sandbox.cpuLimit,
    });
  });
```

---

## Example Configuration

`.openagents/project.json`:
```json
{
  "version": 1,
  "projectId": "openagents",
  "defaultBranch": "main",
  "testCommands": ["bun test"],
  "allowPush": true,
  "sandbox": {
    "enabled": true,
    "backend": "macos-container",
    "image": "oven/bun:latest",
    "memoryLimit": "4G",
    "cpuLimit": 4
  }
}
```

---

## Implementation Order

1. **`src/sandbox/schema.ts`** - Types and errors
2. **`src/sandbox/backend.ts`** - Context.Tag interface
3. **`src/sandbox/macos-container.ts`** - Apple Container implementation
4. **`src/sandbox/detect.ts`** - Auto-detection
5. **`src/sandbox/index.ts`** - Public exports
6. **`src/sandbox/macos-container.test.ts`** - Tests
7. **Update `src/tasks/project-service.ts`** - Add sandbox config to schema
8. **Update `src/agent/orchestrator/`** - Integrate sandboxed execution
9. **Update `docs/claude/plans/containers.md`** - Add link to implementation, note that abstraction is complete

---

## Files to Modify

| File | Change |
|------|--------|
| `src/tasks/project-service.ts` | Add `sandbox` to ProjectConfig |
| `src/agent/orchestrator/orchestrator.ts` | Add sandboxed execution path |
| `.openagents/project.json` | Add sandbox config (optional) |

## Files to Create

| File | Purpose |
|------|---------|
| `src/sandbox/schema.ts` | Types, schemas, errors |
| `src/sandbox/backend.ts` | ContainerBackend interface |
| `src/sandbox/macos-container.ts` | Apple Container impl |
| `src/sandbox/detect.ts` | Auto-detection |
| `src/sandbox/index.ts` | Public API |
| `src/sandbox/macos-container.test.ts` | Tests |

---

## Testing Strategy

1. **Unit tests** for each module (schema validation, backend interface)
2. **Integration tests** with real Apple Container (skip if not available)
3. **E2E test**: Run `bun test` inside container against openagents repo
