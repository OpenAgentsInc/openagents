import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { findTool } from "./find.js";
import { runTool, ToolExecutionError, isTextContent } from "./schema.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("findTool", () => {
  it("finds files matching pattern", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "find-tool" });
        yield* fs.makeDirectory(path.join(dir, "src"), { recursive: true });
        yield* fs.writeFileString(path.join(dir, "src", "alpha.txt"), "a");
        yield* fs.writeFileString(path.join(dir, "beta.txt"), "b");

        return yield* runTool(findTool, { path: dir, pattern: "a" });
      }),
    );

    const text = result.content.find(isTextContent)?.text ?? "";
    expect(text).toContain("alpha.txt");
    expect(text).toContain("beta.txt"); // pattern matches filename containing "a"
  });

  it("limits results", async () => {
    const text = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "find-tool" });
        yield* fs.writeFileString(path.join(dir, "one.txt"), "1");
        yield* fs.writeFileString(path.join(dir, "two.txt"), "2");
        const result = yield* runTool(findTool, { path: dir, maxResults: 1 });
        return result.content.find(isTextContent)?.text ?? "";
      }),
    );

    const lines = text.split("\n").filter((l) => l.trim().length > 0);
    expect(lines.length).toBe(1);
  });

  it("fails on missing path", async () => {
    const error = await runWithBun(
      runTool(findTool, { path: "/no/such/path" }).pipe(Effect.flip),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("not_found");
  });
});
