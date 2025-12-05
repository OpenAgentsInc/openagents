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
    const { updated, diff, details } = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const pathService = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "edit-tool" });
        const file = pathService.join(dir, "sample.txt");
        const original = "alpha\nbravo\ncharlie\n";

        yield* fs.writeFileString(file, original);

        const result = yield* runTool(editTool, { file_path: file, old_string: "bravo", new_string: "delta" });
        const updated = yield* fs.readFileString(file);

        return { updated, diff: result.details?.diff ?? "", details: result.details };
      }),
    );

    expect(updated).toContain("delta");
    expect(diff).toContain("+");
    expect(diff).toContain("-");
    expect(details?.linesAdded).toBeGreaterThan(0);
    expect(details?.linesRemoved).toBeGreaterThan(0);
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

  it("replaces all occurrences when replace_all is true", async () => {
    const updated = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const pathService = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "edit-tool" });
        const file = pathService.join(dir, "multi.txt");

        yield* fs.writeFileString(file, "x y x y");

        yield* runTool(editTool, {
          path: file,
          old_string: "x",
          new_string: "z",
          replace_all: true,
        });

        return yield* fs.readFileString(file);
      }),
    );

    expect(updated).toBe("z y z y");
  });
});
