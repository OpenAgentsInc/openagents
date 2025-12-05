/**
 * Minimal Coding Subagent
 * 
 * Implements one subtask at a time with a minimal prompt.
 * Following pi-mono's insight: models are RL-trained for coding,
 * they don't need 10K tokens of instructions.
 */
import { Effect } from "effect";
import { agentLoop } from "../loop.js";
import { OpenRouterClient } from "../../llm/openrouter.js";
import type { Tool } from "../../tools/schema.js";
import {
  type Subtask,
  type SubagentConfig,
  type SubagentResult,
  SUBAGENT_SYSTEM_PROMPT,
  buildSubagentPrompt,
} from "./types.js";

/**
 * Detect if the subagent has completed its subtask.
 * Looks for "SUBTASK_COMPLETE" in the final message.
 */
const detectSubtaskComplete = (finalMessage: string | null): boolean => {
  if (!finalMessage) return false;
  return finalMessage.includes("SUBTASK_COMPLETE");
};

/**
 * Extract files modified from tool results.
 * Looks for edit/write tool calls and extracts file paths.
 */
const extractFilesModified = (
  turns: Array<{
    toolCalls?: Array<{ name: string; arguments: string }>;
  }>
): string[] => {
  const files = new Set<string>();
  
  for (const turn of turns) {
    if (!turn.toolCalls) continue;
    
    for (const call of turn.toolCalls) {
      if (call.name === "edit" || call.name === "write") {
        try {
          const args = JSON.parse(call.arguments);
          if (args.path) {
            files.add(args.path);
          }
        } catch {
          // Ignore parse errors
        }
      }
    }
  }
  
  return Array.from(files);
};

/**
 * Run the minimal coding subagent to complete a single subtask.
 * 
 * The subagent:
 * - Gets a minimal system prompt (~50 tokens)
 * - Has 4 tools: read, write, edit, bash
 * - Works on one subtask at a time
 * - Outputs SUBTASK_COMPLETE when done
 */
export const runSubagent = (
  config: SubagentConfig
): Effect.Effect<SubagentResult, Error, OpenRouterClient> =>
  Effect.gen(function* () {
    const { subtask, tools, model, maxTurns = 15 } = config;

    const userPrompt = buildSubagentPrompt(subtask);

    const loopConfig = {
      systemPrompt: SUBAGENT_SYSTEM_PROMPT,
      maxTurns,
      onEvent: (event: any) => {
        if (event.type === "tool_call") {
          console.log(`[Subagent] Tool: ${event.tool}`);
        }
      },
      ...(config.onOutput ? { onOutput: config.onOutput } : {}),
      ...(model ? { model } : {}),
    };

    const result = yield* agentLoop(userPrompt, tools, loopConfig).pipe(
      Effect.catchAll((error) =>
        Effect.succeed({
          turns: [],
          finalMessage: null,
          totalTurns: 0,
          verifyState: {
            dirtySinceVerify: false,
            typecheckOk: false,
            testsOk: false,
          },
          error: error.message,
        })
      )
    );

    // Check if subtask was completed successfully
    const completed = detectSubtaskComplete(result.finalMessage);
    const filesModified = extractFilesModified(result.turns);

    if ("error" in result && result.error) {
      return {
        success: false,
        subtaskId: subtask.id,
        filesModified,
        error: result.error,
        turns: result.totalTurns,
        agent: "minimal",
      };
    }

    if (!completed) {
      return {
        success: false,
        subtaskId: subtask.id,
        filesModified,
        error: "Subtask did not complete - SUBTASK_COMPLETE not found in output",
        turns: result.totalTurns,
        agent: "minimal",
      };
    }

    return {
      success: true,
      subtaskId: subtask.id,
      filesModified,
      turns: result.totalTurns,
      agent: "minimal",
    };
  });

/**
 * Create a subagent configuration with minimal tools.
 */
export const createSubagentConfig = (
  subtask: Subtask,
  cwd: string,
  tools: Tool<any, any, any, any>[],
  options?: {
    model?: string;
    maxTurns?: number;
    signal?: AbortSignal;
    onOutput?: (text: string) => void;
  }
): SubagentConfig => ({
  subtask,
  cwd,
  tools,
  maxTurns: options?.maxTurns ?? 15,
  ...(options?.model ? { model: options.model } : {}),
  ...(options?.signal ? { signal: options.signal } : {}),
  ...(options?.onOutput ? { onOutput: options.onOutput } : {}),
});
