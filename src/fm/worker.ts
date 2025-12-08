/**
 * Single-Turn FM Worker
 *
 * Calls FM as a pure function: tiny prompt -> single tool call.
 * No conversation history, no state, no retries for context overflow.
 * If the prompt doesn't fit, it's a bug in the caller.
 */

import { Effect } from "effect";
import type { WorkerInput, WorkerOutput } from "./micro-task-types.js";
import { parseToolCalls } from "../bench/model-adapter.js";

// --- Worker Prompt Template ---

const WORKER_PROMPT_TEMPLATE = `Tools: read_file(p), write_file(p,c), edit_file(p,o,n), run_command(c)
Action: {ACTION}
Context: {CONTEXT}
Previous: {PREVIOUS}
<tool_call>`;

export function buildWorkerPrompt(input: WorkerInput): string {
  return WORKER_PROMPT_TEMPLATE
    .replace("{ACTION}", input.action)
    .replace("{CONTEXT}", input.context || "none")
    .replace("{PREVIOUS}", input.previous || "none");
}

// --- Worker Call ---

export interface FMClientLike {
  chat: (opts: { messages: Array<{ role: "user" | "system" | "assistant" | "tool"; content: string }> }) => Effect.Effect<{
    choices: Array<{ message: { content: string | null } }>;
    usage?: { total_tokens?: number | undefined };
  }, unknown, never>;
}

export async function callFMWorker(
  client: FMClientLike,
  input: WorkerInput,
): Promise<WorkerOutput> {
  const prompt = buildWorkerPrompt(input);

  const messages: Array<{ role: "user" | "system" | "assistant" | "tool"; content: string }> = [
    { role: "user", content: prompt },
  ];

  const response = await Effect.runPromise(
    client.chat({ messages }),
  );

  const content = response.choices[0]?.message.content ?? "";

  const toolCalls = parseToolCalls(content);

  if (toolCalls.length === 0) {
    return {
      toolName: "",
      toolArgs: {},
      raw: content,
    };
  }

  const firstCall = toolCalls[0];
  return {
    toolName: firstCall.name,
    toolArgs: firstCall.arguments,
    raw: content,
  };
}

// --- Prompt Size Validation ---

export function validatePromptSize(input: WorkerInput): { valid: boolean; size: number; error?: string } {
  const prompt = buildWorkerPrompt(input);
  const size = prompt.length;

  if (size > 180) {
    return {
      valid: false,
      size,
      error: `Prompt too large: ${size} chars (max 180)`,
    };
  }

  return { valid: true, size };
}
