import { Args, Command, Options } from "@effect/cli";
import * as BunRuntime from "@effect/platform-bun/BunRuntime";
import * as Console from "effect/Console";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import { OpenRouterClient, openRouterLive } from "./openrouter.js";

const promptArg = Args.text({ name: "prompt", description: "User message to send to the model" });
const modelOption = Options.text("model").pipe(
  Options.withAlias("m"),
  Options.withDescription("Override the default OpenRouter model"),
);
const jsonOption = Options.boolean("json").pipe(
  Options.withDescription("Print the raw JSON response instead of just the text"),
);

const chatCommand = Command.make("chat", { prompt: promptArg, model: modelOption, json: jsonOption }, (input) =>
  Effect.gen(function* (_) {
    const client = yield* _(OpenRouterClient);
    const response = yield* _(
      client.chat({
        model: input.model,
        messages: [{ role: "user", content: input.prompt }],
      }),
    );

    if (input.json) {
      yield* _(Console.log(JSON.stringify(response, null, 2)));
      return;
    }

    const text = response.choices[0]?.message.content ?? "";
    const toolCalls = response.choices[0]?.message.tool_calls ?? [];

    if (text) {
      yield* _(Console.log(text));
    }
    if (toolCalls.length > 0) {
      yield* _(Console.log("\nTool calls:"));
      for (const call of toolCalls) {
        yield* _(Console.log(`- ${call.name}(${call.arguments}) [${call.id}]`));
      }
    }
  }),
);

const cli = Command.run(chatCommand, { name: "openrouter", version: "0.0.1" });

const app = cli(process.argv);

Layer.toRuntime(openRouterLive)
  .pipe(
    Effect.flatMap((runtime) => runtime.runPromise(app)),
    Effect.tapError(Console.error),
  )
  .pipe(BunRuntime.runMain);
