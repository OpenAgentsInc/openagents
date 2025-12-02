import { Command, Options } from "@effect/cli";
import * as Console from "effect/Console";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import { runOpenRouterChat, dotenvLocalLayer } from "./openrouter.js";
import * as ManagedRuntime from "effect/ManagedRuntime";
import * as DefaultServices from "effect/DefaultServices";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FetchHttpClient from "@effect/platform/FetchHttpClient";

const promptOption = Options.text("prompt").pipe(
  Options.withAlias("p"),
  Options.withDescription("User message to send to the model"),
);
const modelOption = Options.optional(
  Options.text("model").pipe(
    Options.withAlias("m"),
    Options.withDescription("Override the default OpenRouter model"),
  ),
);
const jsonOption = Options.boolean("json").pipe(
  Options.withDescription("Print the raw JSON response instead of just the text"),
);

const chatCommand = Command.make("chat", { prompt: promptOption, model: modelOption, json: jsonOption }, (input) =>
  Effect.gen(function* () {
    const model = typeof input.model === "string" ? input.model : "x-ai/grok-4.1-fast";
    const response = yield* runOpenRouterChat({
      model,
      messages: [{ role: "user", content: input.prompt }],
    });

    if (input.json) {
      yield* Console.log(JSON.stringify(response, null, 2));
      return;
    }

    const text = response.choices[0]?.message.content ?? "";
    const toolCalls = response.choices[0]?.message.tool_calls ?? [];

    if (text) {
      yield* Console.log(text);
    }
    if (toolCalls.length > 0) {
      yield* Console.log("\nTool calls:");
      for (const call of toolCalls) {
        yield* Console.log(`- ${call.name}(${call.arguments}) [${call.id}]`);
      }
    }
  }),
);

const cli = Command.run(chatCommand, { name: "openrouter", version: "0.0.1" });

const app = cli(process.argv);

// Base runtime: default services, Bun platform (FS/Path), fetch HTTP client, and .env.local loader.
const platformLayer = Layer.mergeAll(
  Layer.syncContext(() => DefaultServices.liveServices),
  BunContext.layer,
  FetchHttpClient.layer,
);
const envLayer = dotenvLocalLayer.pipe(Layer.provideMerge(platformLayer));
const runtime = ManagedRuntime.make(Layer.mergeAll(platformLayer, envLayer));

runtime
  .runPromise(app)
  .then(() => runtime.dispose(), async (err) => {
    await runtime.runPromise(Console.error(String(err)));
    await runtime.dispose();
    process.exit(1);
  });
