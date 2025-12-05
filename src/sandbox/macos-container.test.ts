import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import { ContainerBackendTag } from "./backend.js";
import { macOSContainerLive } from "./macos-container.js";
import { autoDetectLayer } from "./detect.js";

const runWithMacOSContainer = <A, E>(
  effect: Effect.Effect<A, E, ContainerBackendTag>,
) => Effect.runPromise(effect.pipe(Effect.provide(macOSContainerLive)));

const runWithAutoDetect = <A, E>(
  effect: Effect.Effect<A, E, ContainerBackendTag>,
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

  test("run respects environment variables", async () => {
    const result = await runWithMacOSContainer(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        const available = yield* backend.isAvailable();
        if (!available) {
          return { skipped: true };
        }
        return yield* backend.run(["sh", "-c", "echo $TEST_VAR"], {
          image: "alpine:latest",
          workspaceDir: process.cwd(),
          env: { TEST_VAR: "hello_from_env" },
        });
      }),
    );

    if ("skipped" in result) {
      return;
    }

    expect(result.exitCode).toBe(0);
    expect(result.stdout.trim()).toBe("hello_from_env");
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

    // Should return one of supported backends
    expect(["macos-container", "docker", "none"]).toContain(result);
  });

  test("autoDetectLayer provides a working backend", async () => {
    const result = await runWithAutoDetect(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        const available = yield* backend.isAvailable();
        return { name: backend.name, available };
      }),
    );

    expect(typeof result.name).toBe("string");
    expect(typeof result.available).toBe("boolean");

    // If backend is "none", available should be false
    if (result.name === "none") {
      expect(result.available).toBe(false);
    }
  });
});
