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
    expect(result.loaded?.allowPush).toBe(true);
    expect(result.raw).toContain("\"projectId\": \"openagents\"");
  });

  test("overrides defaults when provided", async () => {
    const config = await runWithBun(
      Effect.gen(function* () {
        const { dir } = yield* setup();
        yield* saveProjectConfig(dir, {
          ...defaultProjectConfig("custom"),
          defaultBranch: "develop",
          testCommands: ["bun test"],
          allowPush: false,
        });

        return yield* loadProjectConfig(dir);
      }),
    );

    expect(config?.defaultBranch).toBe("develop");
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
    expect(result?.claudeCode?.maxTurnsPerSubtask).toBe(30);
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
});
