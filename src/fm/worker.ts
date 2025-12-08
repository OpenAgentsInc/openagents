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
// FM has 4096 tokens (~16K chars) context window, so we can be more verbose

const WORKER_SYSTEM = `You are a coding assistant. Respond ONLY with a tool call in this exact format:
<tool_call>{"name":"TOOL","arguments":{"key":"value"}}</tool_call>

Available tools:
- write_file: {"name":"write_file","arguments":{"path":"file.txt","content":"text"}}
- read_file: {"name":"read_file","arguments":{"path":"file.txt"}}
- run_command: {"name":"run_command","arguments":{"command":"ls -la"}}
- edit_file: {"name":"edit_file","arguments":{"path":"file.txt","old_text":"old","new_text":"new"}}

IMPORTANT: Output ONLY the tool call, nothing else.`;

export interface WorkerPromptInput extends WorkerInput {
  taskDescription?: string | undefined;
}

export function buildWorkerPrompt(input: WorkerPromptInput): string {
  const taskSection = input.taskDescription 
    ? `Original Task: ${input.taskDescription}\n\nCurrent Step: ${input.action}`
    : `Task: ${input.action}`;
  
  return `${WORKER_SYSTEM}

${taskSection}
Context: ${input.context}
${input.previous !== "none" ? `Previous: ${input.previous}` : ""}

Respond with a single tool call:`;
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
  input: WorkerPromptInput,
  log?: (msg: string) => void,
): Promise<WorkerOutput> {
  const prompt = buildWorkerPrompt(input);

  const messages: Array<{ role: "user" | "system" | "assistant" | "tool"; content: string }> = [
    { role: "user", content: prompt },
  ];

  // Log the exact prompt being sent
  const jsonSize = JSON.stringify(messages).length;
  log?.(`[FM] Prompt: "${prompt}" (${prompt.length} chars, JSON: ${jsonSize} chars)`);

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

  // FM has 4096 tokens (~16K chars) - be conservative with ~8K limit
  if (size > 8000) {
    return {
      valid: false,
      size,
      error: `Prompt too large: ${size} chars (max 8000)`,
    };
  }

  return { valid: true, size };
}
