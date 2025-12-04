import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  defaultProjectConfig,
  loadProjectConfig,
  projectConfigPath,
  saveProjectConfig,
} from "./project.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const setup = () =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const dir = yield* fs.makeTempDirectory({ prefix: "project-service" });
    const configPath = yield* projectConfigPath(dir);
    return { dir, configPath };
  });

describe("ProjectService", () => {
  test("returns null when config is missing", async () => {
    const config = await runWithBun(
      Effect.gen(function* () {
        const { dir } = yield* setup();
        return yield* loadProjectConfig(dir);
      }),
    );

    expect(config).toBeNull();
  });

  test("saves and loads config with defaults applied", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { dir, configPath } = yield* setup();
        const config = defaultProjectConfig("openagents");

        yield* saveProjectConfig(dir, config);
        const loaded = yield* loadProjectConfig(dir);
        const fs = yield* FileSystem.FileSystem;
        const raw = yield* fs.readFileString(configPath);

        return { loaded, raw };
      }),
    );

    expect(result.loaded?.projectId).toBe("openagents");
    expect(result.loaded?.defaultBranch).toBe("main");
    expect(result.loaded?.typecheckCommands).toEqual([]);
    expect(result.loaded?.allowPush).toBe(true);
    expect(result.loaded?.parallelExecution?.installTimeoutMs).toBe(15 * 60 * 1000);
    expect(result.loaded?.parallelExecution?.installArgs).toEqual(["--frozen-lockfile"]);
    expect(result.raw).toContain("\"projectId\": \"openagents\"");
  });

  test("overrides defaults when provided", async () => {
    const config = await runWithBun(
      Effect.gen(function* () {
        const { dir } = yield* setup();
        yield* saveProjectConfig(dir, {
          ...defaultProjectConfig("custom"),
          defaultBranch: "develop",
          typecheckCommands: ["bun run typecheck"],
          testCommands: ["bun test"],
          allowPush: false,
        });

        return yield* loadProjectConfig(dir);
      }),
    );

    expect(config?.defaultBranch).toBe("develop");
    expect(config?.typecheckCommands).toEqual(["bun run typecheck"]);
    expect(config?.testCommands).toEqual(["bun test"]);
    expect(config?.allowPush).toBe(false);
  });

  test("applies claudeCode defaults when missing", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { dir } = yield* setup();
        yield* saveProjectConfig(dir, defaultProjectConfig("claude-defaults"));
        return yield* loadProjectConfig(dir);
      }),
    );

    expect(result?.claudeCode?.enabled).toBe(true);
    expect(result?.claudeCode?.preferForComplexTasks).toBe(true);
    expect(result?.claudeCode?.maxTurnsPerSubtask).toBe(300);
    expect(result?.claudeCode?.permissionMode).toBe("bypassPermissions");
    expect(result?.claudeCode?.fallbackToMinimal).toBe(true);
  });

  test("persists claudeCode overrides", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { dir } = yield* setup();
        const overrides = {
          ...defaultProjectConfig("claude-overrides"),
          claudeCode: {
            enabled: false,
            preferForComplexTasks: false,
            maxTurnsPerSubtask: 15,
            permissionMode: "dontAsk" as const,
            fallbackToMinimal: false,
          },
        };

        yield* saveProjectConfig(dir, overrides);
        return yield* loadProjectConfig(dir);
      }),
    );

    expect(result?.claudeCode?.enabled).toBe(false);
    expect(result?.claudeCode?.preferForComplexTasks).toBe(false);
    expect(result?.claudeCode?.maxTurnsPerSubtask).toBe(15);
    expect(result?.claudeCode?.permissionMode).toBe("dontAsk");
    expect(result?.claudeCode?.fallbackToMinimal).toBe(false);
  });

  // Regression tests for loadProjectConfig path handling
  // Bug fixed: overnight.ts was passing openagentsDir instead of workDir,
  // causing loadProjectConfig to look for .openagents/.openagents/project.json

  test("loadProjectConfig expects root dir, not .openagents dir", async () => {
    // This test documents the API contract: pass the PROJECT ROOT, not .openagents
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;

        // Create a temp directory structure: /tmp/xxx/.openagents/project.json
        const rootDir = yield* fs.makeTempDirectory({ prefix: "project-root" });
        const openagentsDir = path.join(rootDir, ".openagents");
        yield* fs.makeDirectory(openagentsDir, { recursive: true });

        // Save config to rootDir (which creates rootDir/.openagents/project.json)
        yield* saveProjectConfig(rootDir, {
          ...defaultProjectConfig("path-test"),
          typecheckCommands: ["npm run types"],
        });

        // Correct usage: pass rootDir
        const correctResult = yield* loadProjectConfig(rootDir);

        // Incorrect usage (the bug): passing openagentsDir would look for
        // .openagents/.openagents/project.json which doesn't exist
        const incorrectResult = yield* loadProjectConfig(openagentsDir);

        return { correctResult, incorrectResult };
      }),
    );

    // Correct usage should find the config
    expect(result.correctResult).not.toBeNull();
    expect(result.correctResult?.projectId).toBe("path-test");
    expect(result.correctResult?.typecheckCommands).toEqual(["npm run types"]);

    // Incorrect usage (passing .openagents dir) should return null
    // because it looks for .openagents/.openagents/project.json
    expect(result.incorrectResult).toBeNull();
  });

  test("projectConfigPath constructs path with .openagents subdirectory", async () => {
    // Verify projectConfigPath adds .openagents to the provided root
    const configPath = await runWithBun(
      Effect.gen(function* () {
        return yield* projectConfigPath("/some/project/root");
      }),
    );

    // Should be: /some/project/root/.openagents/project.json
    expect(configPath).toBe("/some/project/root/.openagents/project.json");
    expect(configPath).toContain(".openagents");
    expect(configPath).not.toContain(".openagents/.openagents");
  });
});
