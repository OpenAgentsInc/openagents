import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { lsTool } from "./ls.js";
import { runTool, ToolExecutionError, isTextContent } from "./schema.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("lsTool", () => {
  it("lists entries", async () => {
    const text = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "ls-tool" });
        yield* fs.makeDirectory(path.join(dir, "sub"), { recursive: true });
        yield* fs.writeFileString(path.join(dir, "file.txt"), "data");

        const result = yield* runTool(lsTool, { path: dir });
        return result.content.find(isTextContent)?.text ?? "";
      }),
    );

    expect(text).toContain("file.txt");
    expect(text).toContain("sub/");
  });

  it("supports SDK-style file_path alias", async () => {
    const text = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "ls-tool" });
        yield* fs.writeFileString(path.join(dir, "sdk.txt"), "data");

        const result = yield* runTool(lsTool, { file_path: dir });
        return result.content.find(isTextContent)?.text ?? "";
      }),
    );

    expect(text).toContain("sdk.txt");
  });

  it("respects recursion and hidden toggle", async () => {
    const text = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "ls-tool" });
        const nestedDir = path.join(dir, ".hidden");
        yield* fs.makeDirectory(nestedDir, { recursive: true });
        yield* fs.writeFileString(path.join(nestedDir, "inner.txt"), "x");

        const result = yield* runTool(lsTool, {
          path: dir,
          recursive: true,
          includeHidden: false,
        });
        return result.content.find(isTextContent)?.text ?? "";
      }),
    );

    expect(text).not.toContain(".hidden");

    const withHidden = await runWithBun(
      Effect.gen(function* () {
        const result = yield* runTool(lsTool, {
          path: ".",
          recursive: false,
          includeHidden: true,
          maxResults: 1,
        });
        return result.content.find(isTextContent)?.text ?? "";
      }),
    );

    expect(withHidden.length).toBeGreaterThan(0);
  });

  it("fails on missing path", async () => {
    const error = await runWithBun(runTool(lsTool, { path: "/no/such/path" }).pipe(Effect.flip));
    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("not_found");
  });
});
