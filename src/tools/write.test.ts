import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { writeTool } from "./write.js";
import { runTool, ToolExecutionError } from "./schema.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("writeTool", () => {
  it("writes and overwrites files", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "write-tool" });
        const file = path.join(dir, "sample.txt");

        yield* runTool(writeTool, { file_path: file, content: "hello" });
        yield* runTool(writeTool, { file_path: file, content: "updated" });

        const content = yield* fs.readFileString(file);
        return { result: content };
      }),
    );

    expect(result.result).toBe("updated");
  });

  it("creates parent directories", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "write-tool" });
        const nested = path.join(dir, "deep/nested/sample.txt");

        yield* runTool(writeTool, { file_path: nested, content: "data" });
        const content = yield* fs.readFileString(nested);
        return content;
      }),
    );

    expect(result).toBe("data");
  });

  it("requires a path or file_path", async () => {
    const error = await runWithBun(runTool(writeTool, { content: "x" }).pipe(Effect.flip));

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("invalid_arguments");
  });

  it("fails on invalid path", async () => {
    const error = await runWithBun(
      runTool(writeTool, { file_path: "/root/forbidden.txt", content: "x" }).pipe(Effect.flip),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("command_failed");
  });
});
