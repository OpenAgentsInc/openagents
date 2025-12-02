// @ts-nocheck
/**
 * Demo: chain 3 read-only Grok tool calls with commentary.
 * Command: bun --bun src/llm/grok-readonly-chain.ts
 */
import { Console, Effect, Layer } from "effect";
import { OpenRouter } from "@openrouter/sdk";
import * as BunContext from "@effect/platform-bun/BunContext";
import { readTool } from "../tools/read.js";
import { runTool } from "../tools/schema.js";
import { createOpenRouterClient, loadOpenRouterEnv, toolToOpenRouterDefinition } from "./openrouter.js";

const colors = {
  bold: (s: string) => `\x1b[1m${s}\x1b[0m`,
  green: (s: string) => `\x1b[32m${s}\x1b[0m`,
  cyan: (s: string) => `\x1b[36m${s}\x1b[0m`,
  yellow: (s: string) => `\x1b[33m${s}\x1b[0m`,
  red: (s: string) => `\x1b[31m${s}\x1b[0m`,
};

const targetFile = "docs/scratchpad/demo.txt";
const steps = [
  { label: "Step 1", offset: 1, limit: 1 },
  { label: "Step 2", offset: 2, limit: 1 },
  { label: "Step 3", offset: 3, limit: 1 },
];

const systemPrompt =
  "You are a cautious coding agent. Use only the provided read-only tool (read). Do not guess. Provide commentary text in your message content alongside any tool calls.";

const platformLayer = BunContext.layer;

const runReadTool = (args: any) =>
  runTool(readTool, args).pipe(
    Effect.provide(platformLayer),
    Effect.mapBoth({
      onFailure: (err) => new Error(err.message ?? String(err)),
      onSuccess: (res) => res,
    }),
  );

const callGrok = (client: OpenRouter, offset: number, limit: number) =>
  Effect.tryPromise({
    try: () =>
      client.chat.send({
        model: "x-ai/grok-4.1-fast",
        tools: [toolToOpenRouterDefinition(readTool)],
        toolChoice: "auto",
        messages: [
          { role: "system", content: systemPrompt },
          {
            role: "user",
            content: `Use the read tool to fetch offset=${offset}, limit=${limit} from ${targetFile}. Always include commentary text in your response content (even after tool calls).`,
          },
        ],
      }),
    catch: (cause) => new Error(`OpenRouter request failed: ${String(cause)}`),
  });

const main = Effect.gen(function* () {
  const client = createOpenRouterClient(loadOpenRouterEnv());

  yield* Console.log(colors.bold("== Grok read-only chain demo =="));
  yield* Console.log(
    `File: ${targetFile} | Steps: ${steps.length} | Model: x-ai/grok-4.1-fast\nSystem: ${systemPrompt}`,
  );

  for (const step of steps) {
    yield* Console.log(colors.bold(`\n${step.label}`));

    const response = yield* callGrok(client, step.offset, step.limit);
    const toolCalls = response.choices?.[0]?.message.toolCalls ?? [];
    const commentary = response.choices?.[0]?.message.content ?? "";

    if (toolCalls.length === 0) {
      yield* Console.log(`${colors.red("✖")} No tool call. Commentary: ${commentary}`);
      continue;
    }

    for (const call of toolCalls) {
      yield* Console.log(`${colors.cyan("•")} ${call.function.name} args: ${call.function.arguments}`);
      const args = JSON.parse(call.function.arguments);
      const result = yield* runReadTool(args);
      const text = result.content.find((c) => c.type === "text")?.text ?? "";
      yield* Console.log(colors.green("Result:"));
      yield* Console.log(text);
    }

    yield* Console.log(colors.yellow(`Commentary: ${commentary || "<none provided>"}`));
  }

  yield* Console.log(colors.bold("\n== Done =="));
  yield* Console.log("Each step above includes the assistant commentary alongside tool results.");
});

Effect.runPromise(main).catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});
