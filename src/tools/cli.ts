import { Args, Command, Options } from "@effect/cli";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as BunRuntime from "@effect/platform-bun/BunRuntime";
import { Console, Effect } from "effect";
import { editTool } from "./edit.js";
import { runTool, ToolExecutionError, isTextContent } from "./schema.js";

const pathArg = Args.text({ name: "path" });
const oldTextOption = Options.text("old-text");
const newTextOption = Options.text("new-text");

const editCommand = Command.make(
  "edit",
  { path: pathArg, oldText: oldTextOption, newText: newTextOption },
  ({ path, oldText, newText }) =>
    Effect.gen(function* (_) {
      const result = yield* _(runTool(editTool, { path, oldText, newText }));
      const body = result.content
        .map((block) => (isTextContent(block) ? block.text : "[image content omitted]"))
        .join("\n\n");

      yield* _(Console.log(body));

      if (result.details?.diff) {
        yield* _(Console.log("\n--- diff ---"));
        yield* _(Console.log(result.details.diff));
      }
    }),
);

const cli = Command.run(editCommand, {
  name: "openagents-tools",
  version: "0.1.0",
});

cli(process.argv).pipe(
  Effect.catchAll((error) =>
    Console.error(
      error instanceof ToolExecutionError || error instanceof Error ? error.message : String(error),
    ),
  ),
  Effect.provide(BunContext.layer),
  BunRuntime.runMain,
);
