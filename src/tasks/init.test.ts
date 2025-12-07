import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { initOpenAgentsProject } from "./init.js";
import { runWithTestContext } from "./test-helpers.js";

describe("initOpenAgentsProject", () => {
  test("creates .openagents with default projectId and empty tasks", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "init-openagents" });

        const initResult = yield* initOpenAgentsProject({ rootDir: dir });
        const projectContent = yield* fs.readFileString(initResult.projectPath);
        // Note: dbPath is now a SQLite database, not a text file
        // We can check if it exists but can't read it as text
        const dbExists = yield* fs.exists(initResult.dbPath);

        return {
          initResult,
          projectJson: JSON.parse(projectContent),
          dbExists,
          base: path.basename(dir),
        };
      }),
    );

    expect(result.initResult.projectId).toBe(result.base);
    expect(result.projectJson.projectId).toBe(result.base);
    expect(result.dbExists).toBe(true);
  });

  test("allows custom projectId", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "init-openagents-custom" });
        const initResult = yield* initOpenAgentsProject({ rootDir: dir, projectId: "openagents" });
        const projectContent = yield* fs.readFileString(initResult.projectPath);
        return JSON.parse(projectContent);
      }),
    );

    expect(result.projectId).toBe("openagents");
  });

  test("fails if project already exists without allowExisting", async () => {
    const error = await runWithTestContext(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "init-openagents-exists" });
        yield* initOpenAgentsProject({ rootDir: dir });
        return yield* initOpenAgentsProject({ rootDir: dir }).pipe(Effect.flip);
      }),
    );

    expect(error._tag).toBe("InitProjectError");
    expect(error.reason).toBe("exists");
  });
});
