#!/usr/bin/env bun
/**
 * Run the agent with a user message and tools.
 * Usage: bun src/agent/run.ts "Your message here"
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

const tools = [readTool, editTool, bashTool, writeTool];

const userMessage = process.argv[2] || "What tools do you have available?";

const program = Effect.gen(function* () {
  console.log("User:", userMessage);
  console.log("---");

  const result = yield* agentLoop(userMessage, tools as any, {
    systemPrompt: BASE_SYSTEM_PROMPT,
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

const liveLayer = Layer.mergeAll(openRouterLive, BunContext.layer);

Effect.runPromise(program.pipe(Effect.provide(liveLayer))).catch(console.error);
