import { Args, Command, Options } from "@effect/cli";
import * as Console from "effect/Console";
import * as Effect from "effect/Effect";
import * as ManagedRuntime from "effect/ManagedRuntime";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { editTool } from "./edit.js";
import { runTool } from "./schema.js";
import {
  createOpenRouterClient,
  loadOpenRouterEnv,
  toolToOpenRouterDefinition,
  dotenvLocalLayer,
} from "../llm/openrouter.js";
import type { ChatMessageToolCall } from "@openrouter/sdk/esm/models/index.js";
import * as Layer from "effect/Layer";
import * as DefaultServices from "effect/DefaultServices";
import * as BunContext from "@effect/platform-bun/BunContext";

const colors = {
  cyan: (s: string) => `\x1b[36m${s}\x1b[0m`,
  green: (s: string) => `\x1b[32m${s}\x1b[0m`,
  yellow: (s: string) => `\x1b[33m${s}\x1b[0m`,
  bold: (s: string) => `\x1b[1m${s}\x1b[0m`,
  red: (s: string) => `\x1b[31m${s}\x1b[0m`,
};

const systemPrompt =
  "You are an agent with file tools. Call the provided tool to edit files exactly as requested and report back.";

const cliCommand = Command.make(
  "openrouter-edit-demo",
  {
    file: Options.text("file").pipe(
      Options.withAlias("f"),
      Options.withDescription("Target file to edit (default: docs/scratchpad/demo.txt)"),
      Options.withDefault("docs/scratchpad/demo.txt"),
    ),
  },
  ({ file }) =>
    Effect.gen(function* (_) {
    const fs = yield* _(FileSystem.FileSystem);
    const path = yield* _(Path.Path);
    const client = createOpenRouterClient(loadOpenRouterEnv());

      const targetPath = path.resolve(file);
      const dir = path.dirname(targetPath);

      yield* _(Console.log(colors.bold("== System Prompt ==")));
      yield* _(Console.log(systemPrompt));

      yield* _(Console.log(`\n${colors.bold("== Preparing scratch file ==")}`));
      yield* _(fs.makeDirectory(dir, { recursive: true }));
      yield* _(
        fs.writeFileString(
          targetPath,
          "This is a demo scratchpad file.\nThe agent will edit this line.\nKeep calm and let tools handle it.\n",
        ),
      );
      yield* _(Console.log(`${colors.green("✔")} Wrote ${targetPath}`));

      yield* _(Console.log(`\n${colors.bold("== Calling OpenRouter with tools ==")}`));
      const response = yield* _(
        Effect.tryPromise({
          try: () =>
            client.chat.send({
              model: "x-ai/grok-4.1-fast",
              tools: [toolToOpenRouterDefinition(editTool)],
              toolChoice: "required",
              messages: [
                { role: "system", content: systemPrompt },
                {
                  role: "user",
                  content: [
                    "Use the edit tool now. Do not ask for confirmation.",
                    `File path: ${targetPath}`,
                    "Replace the line exactly:",
                    `'The agent will edit this line.'`,
                    "with:",
                    `'Edited via OpenRouter tool demo.'`,
                  ].join("\n"),
                },
              ],
            }),
          catch: (cause) => new Error(`OpenRouter request failed: ${String(cause)}`),
        }),
      );

      const toolCalls: ChatMessageToolCall[] = response.choices[0]?.message.toolCalls ?? [];

      if (toolCalls.length === 0) {
        yield* _(
          Console.log(
            `${colors.red("✖")} No tool calls returned by the model. Response message: ${
              response.choices[0]?.message.content ?? "<empty>"
            }`,
          ),
        );
        return;
      }

      for (const call of toolCalls) {
        yield* _(
          Console.log(
            `${colors.cyan("•")} Tool call ${colors.bold(call.id)} -> ${call.function.name}(${call.function.arguments})`,
          ),
        );

        const parsedArgs = yield* _(
          Effect.try({
            try: () => JSON.parse(call.function.arguments) as unknown,
            catch: (cause) =>
              new Error(`Failed to parse arguments JSON for ${call.id}: ${String(cause)}`),
          }),
        );

        const result = yield* _(runTool(editTool, parsedArgs));

        const diff = result.details?.diff ?? "(no diff)";
        yield* _(
          Console.log(
            `${colors.green("✔")} Edit applied via tool ${call.function.name}:\n${colors.yellow(diff)}`,
          ),
        );
      }
    }),
);

const platformLayer = Layer.mergeAll(
  Layer.syncContext(() => DefaultServices.liveServices),
  BunContext.layer,
);
const envLayer = dotenvLocalLayer.pipe(Layer.provideMerge(platformLayer));
const runtimeLayer = Layer.mergeAll(platformLayer, envLayer);

const app = Command.run(cliCommand, { name: "openrouter-edit-demo", version: "0.0.1" })(
  process.argv,
);

const runtime = ManagedRuntime.make(runtimeLayer);

runtime
  .runPromise(app)
  .then(
    () => runtime.dispose(),
    async (err) => {
      await runtime.dispose();
      // eslint-disable-next-line no-console
      console.error(err);
      process.exit(1);
    },
  );
