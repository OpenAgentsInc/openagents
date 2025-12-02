// @ts-nocheck
/**
 * Demo: ask Grok (OpenRouter) to use read-only tools and execute the returned tool calls.
 * Command: bun --bun src/llm/grok-readonly-demo.ts
 */
import { Console, Effect } from "effect";
import { OpenRouter } from "@openrouter/sdk";
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

const systemPrompt =
  "You are a cautious coding agent. Use only the provided read-only tools (read) to inspect files; do not guess.";

const targetFile = "docs/scratchpad/demo.txt";

const main = Effect.gen(function* (_) {
  // Build OpenRouter client from env
  const config = loadOpenRouterEnv();
  const client: OpenRouter = createOpenRouterClient(config);

  yield* _(Console.log(colors.bold("== Grok read-only tool demo ==")));
  yield* _(
    Console.log(
      `Model: x-ai/grok-4.1-fast | File: ${targetFile}\nTools: read (read-only)\nSystem: ${systemPrompt}`,
    ),
  );

  const tools = [toolToOpenRouterDefinition(readTool)];

  const response = yield* _(
    Effect.tryPromise({
      try: () =>
        client.chat.send({
          model: "x-ai/grok-4.1-fast",
          tools,
          toolChoice: "auto",
          messages: [
            { role: "system", content: systemPrompt },
            {
              role: "user",
              content: `Read ${targetFile} and return the second line. Use the tool; do not guess.`,
            },
          ],
        }),
      catch: (cause) => new Error(`OpenRouter request failed: ${String(cause)}`),
    }),
  );

  const choice = response.choices?.[0];
  const toolCalls = choice?.message.toolCalls ?? [];

  if (toolCalls.length === 0) {
    yield* _(
      Console.log(
        `${colors.red("✖")} No tool calls returned. Assistant said: ${choice?.message.content ?? "<empty>"}`,
      ),
    );
    return;
  }

  yield* _(Console.log(colors.bold("\n== Tool calls ==")));
  for (const call of toolCalls) {
    yield* _(Console.log(`${colors.cyan("•")} ${call.function.name} ${call.id}`));
    yield* _(Console.log(`  args: ${call.function.arguments}`));

    const args = JSON.parse(call.function.arguments);
    const result = yield* _(runTool(readTool, args));
    const text = result.content.find((c) => c.type === "text")?.text ?? "";
    yield* _(Console.log(colors.green("Result (text):")));
    yield* _(Console.log(text));
  }
});

Effect.runPromise(main).catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});
