import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { writeTool } from "./write.js";
import { runTool, ToolExecutionError } from "./schema.js";

const runWithBun = <A>(program: Effect.Effect<A>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("writeTool", () => {
  it("writes and overwrites files", async () => {
    const result = await runWithBun(
      Effect.gen(function* (_) {
        const fs = yield* _(FileSystem.FileSystem);
        const path = yield* _(Path.Path);
        const dir = yield* _(fs.makeTempDirectory({ prefix: "write-tool" }));
        const file = path.join(dir, "sample.txt");

        yield* _(runTool(writeTool, { path: file, content: "hello" }));
        yield* _(runTool(writeTool, { path: file, content: "updated" }));

        const content = yield* _(fs.readFileString(file));
        return { result: content };
      }),
    );

    expect(result.result).toBe("updated");
  });

  it("creates parent directories", async () => {
    const result = await runWithBun(
      Effect.gen(function* (_) {
        const fs = yield* _(FileSystem.FileSystem);
        const path = yield* _(Path.Path);
        const dir = yield* _(fs.makeTempDirectory({ prefix: "write-tool" }));
        const nested = path.join(dir, "deep/nested/sample.txt");

        yield* _(runTool(writeTool, { path: nested, content: "data" }));
        const content = yield* _(fs.readFileString(nested));
        return content;
      }),
    );

    expect(result).toBe("data");
  });

  it("fails on invalid path", async () => {
    const error = await runWithBun(
      Effect.gen(function* (_) {
        return yield* _(runTool(writeTool, { path: "/root/forbidden.txt", content: "x" }).pipe(Effect.flip));
      }),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("command_failed");
  });
});
