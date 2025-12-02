import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { readTool } from "./read.js";
import { runTool, ToolExecutionError } from "./schema.js";

const runWithBun = <A>(program: Effect.Effect<A>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("readTool", () => {
  it("reads text content", async () => {
    const result = await runWithBun(
      Effect.gen(function* (_) {
        const fs = yield* _(FileSystem.FileSystem);
        const path = yield* _(Path.Path);
        const dir = yield* _(fs.makeTempDirectory({ prefix: "read-tool" }));
        const file = path.join(dir, "sample.txt");
        yield* _(fs.writeFileString(file, "alpha\nbravo\ncharlie\n"));

        return yield* _(runTool(readTool, { path: file }));
      }),
    );

    expect(result.content[0]?.text).toContain("alpha");
    expect(result.content[0]?.text).toContain("charlie");
  });

  it("respects offset and limit with notice", async () => {
    const result = await runWithBun(
      Effect.gen(function* (_) {
        const fs = yield* _(FileSystem.FileSystem);
        const path = yield* _(Path.Path);
        const dir = yield* _(fs.makeTempDirectory({ prefix: "read-tool" }));
        const file = path.join(dir, "paged.txt");
        yield* _(fs.writeFileString(file, "one\ntwo\nthree\nfour\n"));

        return yield* _(runTool(readTool, { path: file, offset: 2, limit: 1 }));
      }),
    );

    const text = result.content[0]?.text ?? "";
    expect(text).toContain("two");
    expect(text).toContain("more lines not shown");
    expect(text).toContain("offset=3");
  });

  it("fails when file is missing", async () => {
    const error = await runWithBun(
      Effect.gen(function* (_) {
        return yield* _(runTool(readTool, { path: "/no/such/file.txt" }).pipe(Effect.flip));
      }),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("not_found");
  });
});
