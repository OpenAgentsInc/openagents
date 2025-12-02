import * as BunContext from "@effect/platform-bun/BunContext";
import * as DefaultServices from "effect/DefaultServices";
import * as Console from "effect/Console";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { editTool } from "./edit.js";
import { runTool } from "./schema.js";

const colors = {
  cyan: (s: string) => `\x1b[36m${s}\x1b[0m`,
  green: (s: string) => `\x1b[32m${s}\x1b[0m`,
  yellow: (s: string) => `\x1b[33m${s}\x1b[0m`,
  bold: (s: string) => `\x1b[1m${s}\x1b[0m`,
};

const systemPrompt = `You are an agent with file tools. Use them to perform precise edits and report diffs back.`;

const program = Effect.gen(function* (_) {
  const fs = yield* _(FileSystem.FileSystem);
  const path = yield* _(Path.Path);

  const scratchDir = path.join(process.cwd(), "docs", "scratchpad");
  const targetFile = path.join(scratchDir, "demo.txt");

  yield* _(Console.log(colors.bold("== System Prompt ==")));
  yield* _(Console.log(systemPrompt));

  yield* _(Console.log(`\n${colors.bold("== Preparing scratch file ==")}`));
  yield* _(fs.makeDirectory(scratchDir, { recursive: true }));
  yield* _(fs.writeFileString(targetFile, "This is a demo scratchpad file.\nThe agent will edit this line.\nKeep calm and let tools handle it.\n"));
  yield* _(Console.log(`${colors.green("✔")} Wrote ${targetFile}`));

  yield* _(Console.log(`\n${colors.bold("== Running edit tool ==")}`));
  yield* _(
    runTool(editTool, {
      path: targetFile,
      oldText: "The agent will edit this line.",
      newText: "The agent has edited this line successfully.",
    }).pipe(
      Effect.flatMap((result) =>
        Console.log(
          `${colors.green("✔")} Edit applied:\n${colors.yellow(result.details?.diff ?? "(no diff)")}`,
        ),
      ),
    ),
  );
});

const platform = Layer.mergeAll(
  Layer.syncContext(() => DefaultServices.liveServices),
  BunContext.layer,
);

program.pipe(Effect.provide(platform), Effect.runPromise).catch((error) => {
  // eslint-disable-next-line no-console
  console.error(error);
  process.exit(1);
});
