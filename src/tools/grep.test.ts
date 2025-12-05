import * as BunContext from "@effect/platform-bun/BunContext";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { grepTool } from "./grep.js";
import { runTool, ToolExecutionError, isTextContent } from "./schema.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, CommandExecutor.CommandExecutor | FileSystem.FileSystem | Path.Path>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("grepTool", () => {
  it("finds matches in files", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "grep-tool" });
        const file = path.join(dir, "sample.txt");
        yield* fs.writeFileString(file, "hello\nworld\nhello world");

        return yield* runTool(grepTool, { pattern: "hello", path: dir, fixed: true });
      }),
    );

    const text = result.content.find(isTextContent)?.text ?? "";
    expect(text).toContain("sample.txt:1");
    expect(text).toContain("sample.txt:3");
    expect(result.details?.matches).toBe(2);
    expect(result.details?.exitCode).toBe(0);
  });

  it("respects maxResults", async () => {
    const text = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "grep-tool" });
        const file = path.join(dir, "sample.txt");
        yield* fs.writeFileString(file, ["one", "one", "one"].join("\n"));

        const result = yield* runTool(grepTool, { pattern: "one", path: dir, maxResults: 1, fixed: true });
        return result.content[0]?.type === "text" ? result.content[0]?.text : "";
      }),
    );

    const lines = text.split("\n").filter((l) => l.trim().length > 0);
    expect(lines).toHaveLength(1);
  });

  it("fails on missing path", async () => {
    const error = await runWithBun(
      runTool(grepTool, { pattern: "x", path: "/does/not/exist" }).pipe(Effect.flip),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("not_found");
  });
});
