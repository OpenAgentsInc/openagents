import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import * as BunContext from "@effect/platform-bun/BunContext";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { editTool } from "./edit.js";
import { runTool, ToolExecutionError } from "./schema.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("editTool", () => {
  it("replaces a unique match and emits a diff", async () => {
    const { updated, diff } = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const pathService = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "edit-tool" });
        const file = pathService.join(dir, "sample.txt");
        const original = "alpha\nbravo\ncharlie\n";

        yield* fs.writeFileString(file, original);

        const result = yield* runTool(editTool, { path: file, oldText: "bravo", newText: "delta" });
        const updated = yield* fs.readFileString(file);

        return { updated, diff: result.details?.diff ?? "" };
      }),
    );

    expect(updated).toContain("delta");
    expect(diff).toContain("+");
    expect(diff).toContain("-");
  });

  it("fails when the match is not unique", async () => {
    const error = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const pathService = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "edit-tool" });
        const file = pathService.join(dir, "dupe.txt");

        yield* fs.writeFileString(file, "repeat repeat");

        return yield* runTool(editTool, { path: file, oldText: "repeat", newText: "once" }).pipe(Effect.flip);
      }),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("not_unique");
  });

  it("fails when the old text is missing", async () => {
    const error = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const pathService = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "edit-tool" });
        const file = pathService.join(dir, "missing.txt");

        yield* fs.writeFileString(file, "hello world");

        return yield* runTool(editTool, { path: file, oldText: "absent", newText: "present" }).pipe(Effect.flip);
      }),
    );

    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("missing_old_text");
  });
});
