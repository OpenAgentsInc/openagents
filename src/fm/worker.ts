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
import type { Skill } from "../skills/schema.js";

// --- Worker Prompt Template ---
// FM has 4096 tokens (~16K chars) context window, but task descriptions can be very long
// We truncate task descriptions to stay within limits

const MAX_TASK_CHARS = 600; // Conservative limit for FM's tiny context

/**
 * Truncate task description to fit in FM's context window.
 * The full description is too verbose for FM; it just needs the gist.
 */
function truncateTaskDescription(description: string): string {
  if (description.length <= MAX_TASK_CHARS) {
    return description;
  }
  return description.slice(0, MAX_TASK_CHARS) + "\n...[truncated]";
}

const WORKER_SYSTEM = `You are a coding assistant. Respond ONLY with a tool call in this exact format:
<tool_call>{"name":"TOOL","arguments":{"key":"value"}}</tool_call>

PATH RULES:
- Your workspace is the current directory (.)
- When the task mentions "/app/foo", use "foo" or "./foo" (relative path)
- Never use absolute /app/ paths in commands or file operations
- Example: "/app/output.txt" → "output.txt" or "./output.txt"

JSON RULES:
- Keep file content reasonably short when possible
- Ensure valid JSON: escape newlines (\\n) and quotes (\\")
- If writing large files, consider breaking into smaller chunks

Available tools:
- write_file: {"name":"write_file","arguments":{"path":"file.txt","content":"text"}}
- read_file: {"name":"read_file","arguments":{"path":"file.txt"}}
- run_command: {"name":"run_command","arguments":{"command":"ls -la"}}
- edit_file: {"name":"edit_file","arguments":{"path":"file.txt","old_text":"old","new_text":"new"}}
- verify_progress: {"name":"verify_progress","arguments":{}} - Check how many tests are passing
- task_complete: {"name":"task_complete","arguments":{}} - Call this when the task is finished

IMPORTANT: The ONLY tools you may call are:
- write_file
- read_file
- run_command
- edit_file
- verify_progress
- task_complete

If you put any other name in the "name" field, it will fail. Skills/approaches listed above are for inspiration only - they are NOT callable tools.

IMPORTANT: Output ONLY the tool call, nothing else. Call task_complete when done.`;

export interface WorkerPromptInput extends WorkerInput {
  taskDescription?: string | undefined;
  skills?: Skill[] | undefined;
  hint?: string | undefined;
  /** Verification feedback from last test run (e.g., "3/9 tests passing. Failures: ...") */
  verificationFeedback?: string | undefined;
}

export function buildWorkerPrompt(input: WorkerPromptInput): string {
  const taskSection = input.taskDescription
    ? `Original Task: ${truncateTaskDescription(input.taskDescription)}\n\nCurrent Step: ${input.action}`
    : `Task: ${input.action}`;

  // Use suite-aware hint if provided (from orchestrator)
  const hint = input.hint ? `\n${input.hint}` : "";

  // Format skills section if provided (clearly not callable)
  let skillsSection = "";
  if (input.skills && input.skills.length > 0) {
    const approaches = input.skills.map(s => {
      // Use description only, without the skill name
      const desc = s.description.slice(0, 80);
      return `  • ${desc}`;
    }).join("\n");

    skillsSection = `
Example approaches (for reference only, NOT callable tools):
${approaches}
`;
  }

  // Format verification feedback if provided (KEY for MAP architecture)
  // This is what allows FM to iterate based on test results
  let verificationSection = "";
  if (input.verificationFeedback) {
    verificationSection = `
VERIFICATION STATUS:
${input.verificationFeedback}
`;
  }

  return `${WORKER_SYSTEM}
${skillsSection}
${taskSection}
Context: ${input.context}
${input.previous !== "none" ? `Previous: ${input.previous}` : ""}${verificationSection}${hint}

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
  log?.(`[FM] Prompt length: ${prompt.length} chars (JSON: ${jsonSize} chars)`);

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
