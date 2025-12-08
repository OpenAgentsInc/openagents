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
- task_complete: {"name":"task_complete","arguments":{}} - Call this when the task is finished

IMPORTANT: Output ONLY the tool call, nothing else. Call task_complete when done.`;

export interface WorkerPromptInput extends WorkerInput {
  taskDescription?: string | undefined;
}

export function buildWorkerPrompt(input: WorkerPromptInput): string {
  const taskSection = input.taskDescription 
    ? `Original Task: ${input.taskDescription}\n\nCurrent Step: ${input.action}`
    : `Task: ${input.action}`;
  
  // Add workflow hints based on previous actions and task type
  let hint = "";
  
  // Check if task requires special handling
  const taskLower = (input.taskDescription ?? "").toLowerCase();
  const needsWordCount = taskLower.includes("count") && taskLower.includes("word");
  const needsExactCopy = (taskLower.includes("exact same content") || 
                          taskLower.includes("exact content") ||
                          taskLower.includes("copy it exactly") ||
                          (taskLower.includes("read") && taskLower.includes("create") && taskLower.includes("same content")));
  const needsReadFirst = !needsWordCount && !needsExactCopy && (
                         taskLower.includes("count") || 
                         taskLower.includes("number of") ||
                         taskLower.includes("read") && taskLower.includes("write"));
  
  if (input.previous && input.previous !== "none") {
    if (input.previous.includes(" contains:")) {
      // Just read a file - hint to use the content with exact preservation
      hint = "\nHint: You just read file content. Write it EXACTLY to the target file, preserving all newlines (use \\n).";
    } else if (input.previous.includes("Command output:")) {
      // Just ran a command - hint to save the output
      hint = "\nHint: You have command output. Save it to a file if needed.";
    }
  } else if (needsExactCopy) {
    // Exact file copy - use cp command to preserve content exactly
    hint = "\nHint: To copy a file exactly, use run_command with: cp source.txt destination.txt";
  } else if (needsWordCount) {
    // Word counting task - use wc command
    hint = "\nHint: To count words, use run_command with: wc -w filename.txt | awk '{print $1}'";
  } else if (needsReadFirst) {
    // First turn and task needs reading - hint to read first
    hint = "\nHint: This task requires reading a file first. Use read_file before writing.";
  }
  
  return `${WORKER_SYSTEM}

${taskSection}
Context: ${input.context}
${input.previous !== "none" ? `Previous: ${input.previous}` : ""}${hint}

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
