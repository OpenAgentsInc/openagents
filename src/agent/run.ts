#!/usr/bin/env bun
/**
 * Run the agent with a user message and tools.
 *
 * Flags:
 *   --print        Show composed prompt + tool list and exit
 *   --export <p>   Export a session JSONL file to HTML transcript
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect, Layer } from "effect";
import { agentLoop } from "./loop.js";
import { BASE_SYSTEM_PROMPT } from "./prompts.js";
import { readTool } from "../tools/read.js";
import { editTool } from "../tools/edit.js";
import { bashTool } from "../tools/bash.js";
import { writeTool } from "../tools/write.js";
import { openRouterLive } from "../llm/openrouter.js";
import { parseArgs } from "../cli/parser.js";
import { buildSystemPromptWithContext } from "../cli/context-loader.js";
import { exportSessionToHtml } from "../sessions/export-html.js";

const TOOL_MAP = {
  read: readTool,
  edit: editTool,
  bash: bashTool,
  write: writeTool,
};

const resolveTools = (names?: string[]) => {
  if (!names || names.length === 0) return Object.values(TOOL_MAP);
  const resolved = names.map((name) => TOOL_MAP[name as keyof typeof TOOL_MAP]).filter(Boolean);
  return resolved.length > 0 ? resolved : Object.values(TOOL_MAP);
};

const printPromptPreview = (systemPrompt: string, userMessage: string, tools: { name: string }[], files: string[]) => {
  console.log("Prompt Preview\n==============\n");
  console.log("System prompt:\n");
  console.log(systemPrompt);
  console.log("\nUser message:\n");
  console.log(userMessage);
  console.log("\nTools:");
  for (const tool of tools) {
    console.log(`- ${tool.name}`);
  }
  if (files.length > 0) {
    console.log("\nAttached files:");
    for (const file of files) {
      console.log(`- ${file}`);
    }
  }
};

const runAgent = (userMessage: string, tools: any[], systemPrompt: string) =>
  Effect.gen(function* () {
    console.log("User:", userMessage);
    console.log("---");

    const result = yield* agentLoop(userMessage, tools, {
      systemPrompt,
      maxTurns: 10,
    });

    console.log(`\nCompleted in ${result.totalTurns} turn(s)`);

    for (const turn of result.turns) {
      if (turn.content) {
        console.log("\nAssistant:", turn.content);
      }
      if (turn.toolCalls) {
        for (const call of turn.toolCalls) {
          console.log(`\nTool call: ${call.name}`);
          console.log("Args:", call.arguments);
        }
      }
      if (turn.toolResults) {
        for (const res of turn.toolResults) {
          console.log(`\nTool result (${res.name}):`, res.isError ? "ERROR" : "SUCCESS");
          for (const content of res.result.content) {
            if (content.type === "text") {
              const text = content.text;
              if (text.length > 500) {
                console.log(text.slice(0, 500) + `\n... (${text.length - 500} more chars)`);
              } else {
                console.log(text);
              }
            }
          }
        }
      }
    }

    if (result.finalMessage) {
      console.log("\n---");
      console.log("Final:", result.finalMessage);
    }
  });

const main = async () => {
  const args = parseArgs(process.argv.slice(2));
  const userMessage = args.messages.join(" ").trim() || "What tools do you have available?";
  const systemPrompt = buildSystemPromptWithContext(BASE_SYSTEM_PROMPT);
  const selectedTools = resolveTools(args.tools);
  const filePaths = args.files ?? [];

  if (args.print) {
    printPromptPreview(systemPrompt, userMessage, selectedTools, filePaths);
    return;
  }

  if (args.export) {
    const htmlPath = exportSessionToHtml(args.export);
    console.log(`Exported HTML transcript to ${htmlPath}`);
    return;
  }

  const program = runAgent(userMessage, selectedTools, systemPrompt);
  const liveLayer = Layer.mergeAll(openRouterLive, BunContext.layer);
  await Effect.runPromise(program.pipe(Effect.provide(liveLayer)));
};

if (import.meta.main) {
  main().catch((err) => {
    console.error(err);
    process.exit(1);
  });
}
