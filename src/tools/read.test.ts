import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { readTool } from "./read.js";
import { runTool, ToolExecutionError, isTextContent } from "./schema.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("readTool", () => {
  it("reads text content via file_path alias", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "read-tool" });
        const file = path.join(dir, "sample.txt");
        yield* fs.writeFileString(file, "alpha\nbravo\ncharlie\n");

        return yield* runTool(readTool, { file_path: file });
      }),
    );

    const textBlock = result.content.find(isTextContent);
    expect(textBlock?.text).toContain("alpha");
    expect(textBlock?.text).toContain("charlie");
  });

  it("respects offset and limit with notice", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "read-tool" });
        const file = path.join(dir, "paged.txt");
        yield* fs.writeFileString(file, "one\ntwo\nthree\nfour\n");

        return yield* runTool(readTool, { path: file, offset: 2, limit: 1 });
      }),
    );

    const textBlock = result.content.find(isTextContent);
    const text = textBlock?.text ?? "";
    expect(text).toContain("two");
    expect(text).toContain("more lines not shown");
    expect(text).toContain("offset=3");
  });

  it("fails when file is missing", async () => {
    const error = await runWithBun(
      runTool(readTool, { file_path: "/no/such/file.txt" }).pipe(Effect.flip),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("not_found");
  });
});
