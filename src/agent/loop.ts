import { Effect } from "effect";
import type { Tool, ToolResult } from "../tools/schema.js";
import { ToolExecutionError, runTool } from "../tools/schema.js";
import type { ChatMessage, ChatRequest } from "../llm/openrouter.js";
import { OpenRouterClient } from "../llm/openrouter.js";

export interface AgentConfig {
  model?: string;
  systemPrompt?: string;
  maxTurns?: number;
  temperature?: number;
}

export interface AgentTurn {
  role: "assistant" | "tool";
  content: string | null;
  toolCalls?: Array<{ id: string; name: string; arguments: string }>;
  toolResults?: Array<{ toolCallId: string; name: string; result: ToolResult; isError: boolean }>;
}

export interface VerifyState {
  dirtySinceVerify: boolean;  // any edit/write since last successful typecheck+tests
  typecheckOk: boolean;
  testsOk: boolean;
}

export interface AgentResult {
  turns: AgentTurn[];
  finalMessage: string | null;
  totalTurns: number;
  verifyState: VerifyState;
}

export class AgentLoopError extends Error {
  readonly _tag = "AgentLoopError";
  constructor(
    readonly reason: "max_turns_exceeded" | "no_response" | "llm_error",
    message: string,
  ) {
    super(message);
    this.name = "AgentLoopError";
  }
}

const executeToolCall = (
  tools: Tool<any, any, any, any>[],
  toolCall: { id: string; name: string; arguments: string },
  signal?: AbortSignal,
): Effect.Effect<
  { toolCallId: string; name: string; result: ToolResult; isError: boolean },
  never,
  any
> =>
  Effect.gen(function* () {
    const tool = tools.find((t) => t.name === toolCall.name);

    if (!tool) {
      return {
        toolCallId: toolCall.id,
        name: toolCall.name,
        result: {
          content: [{ type: "text" as const, text: `Tool "${toolCall.name}" not found` }],
        },
        isError: true,
      };
    }

    const args = yield* Effect.try({
      try: () => JSON.parse(toolCall.arguments),
      catch: () => new ToolExecutionError("invalid_arguments", "Failed to parse tool arguments"),
    }).pipe(Effect.catchAll((e) => Effect.succeed({ __parseError: e.message })));

    if ("__parseError" in args) {
      return {
        toolCallId: toolCall.id,
        name: toolCall.name,
        result: {
          content: [{ type: "text" as const, text: `Invalid arguments: ${args.__parseError}` }],
        },
        isError: true,
      };
    }

    const result = yield* runTool(tool, args, signal ? { signal } : undefined).pipe(
      Effect.catchAll((error) =>
        Effect.succeed({
          content: [{ type: "text" as const, text: error.message }],
        } as ToolResult),
      ),
    );

    return {
      toolCallId: toolCall.id,
      name: toolCall.name,
      result,
      isError: false,
    };
  });

// Helper to detect if a bash command is a verification command
const isTypecheckCommand = (args: string): boolean => {
  return args.includes("bun run typecheck") || args.includes("tsc");
};

const isTestCommand = (args: string): boolean => {
  return args.includes("bun test") || args.includes("bun run test");
};

const isTypecheckSuccess = (output: string): boolean => {
  // No "error TS" and exit code 0 (no "exited with code 1/2")
  return !output.includes("error TS") && 
         !output.includes("exited with code 1") && 
         !output.includes("exited with code 2");
};

const isTestSuccess = (output: string): boolean => {
  // Contains "pass" and "0 fail" or no "fail" count
  return output.includes("pass") && 
         (output.includes("0 fail") || !output.match(/\d+ fail/));
};

export const agentLoop = (
  userMessage: string,
  tools: Tool<any, any, any, any>[],
  config: AgentConfig = {},
): Effect.Effect<AgentResult, AgentLoopError, OpenRouterClient> =>
  Effect.gen(function* () {
    const client = yield* OpenRouterClient;
    const maxTurns = config.maxTurns ?? 10;
    const turns: AgentTurn[] = [];
    const messages: ChatMessage[] = [];

    // Verification state tracking
    const verifyState: VerifyState = {
      dirtySinceVerify: false,
      typecheckOk: false,
      testsOk: false,
    };

    if (config.systemPrompt) {
      messages.push({ role: "system", content: config.systemPrompt });
    }
    messages.push({ role: "user", content: userMessage });

    let turnCount = 0;
    let continueLoop = true;

    while (continueLoop && turnCount < maxTurns) {
      turnCount++;

      const request: ChatRequest = {
        messages,
        tools: tools as unknown as Tool<any>[],
        ...(config.model ? { model: config.model } : {}),
        ...(config.temperature !== undefined ? { temperature: config.temperature } : {}),
      };

      const response = yield* client.chat(request).pipe(
        Effect.mapError((e: Error) => new AgentLoopError("llm_error", e.message)),
      );

      const choice = response.choices[0];
      if (!choice) {
        return yield* Effect.fail(new AgentLoopError("no_response", "No response from LLM"));
      }

      const assistantMessage = choice.message;
      const toolCalls = assistantMessage.tool_calls ?? [];

      messages.push({
        role: "assistant",
        content: assistantMessage.content ?? "",
      });

      const turn: AgentTurn = {
        role: "assistant",
        content: assistantMessage.content,
        ...(toolCalls.length > 0 ? { toolCalls } : {}),
      };

      if (toolCalls.length === 0) {
        turns.push(turn);
        continueLoop = false;
      } else {
        const toolResults: AgentTurn["toolResults"] = [];

        for (const toolCall of toolCalls) {
          const result = yield* executeToolCall(tools, toolCall);
          toolResults.push(result);

          const toolOutput = result.result.content
            .filter((c): c is { type: "text"; text: string } => c.type === "text")
            .map((c) => c.text)
            .join("\n") || (result.isError ? "Error" : "Success");

          messages.push({
            role: "tool",
            tool_call_id: toolCall.id,
            name: toolCall.name,
            content: toolOutput,
          });

          // Track verification state based on tool calls
          const toolName = toolCall.name.toLowerCase();
          
          // edit/write marks as dirty
          if ((toolName === "edit" || toolName === "write") && !result.isError) {
            verifyState.dirtySinceVerify = true;
            verifyState.typecheckOk = false;
            verifyState.testsOk = false;
          }
          
          // bash commands: check for typecheck/test commands
          if (toolName === "bash" && !result.isError) {
            const args = toolCall.arguments;
            if (isTypecheckCommand(args)) {
              if (isTypecheckSuccess(toolOutput)) {
                verifyState.typecheckOk = true;
              }
            }
            if (isTestCommand(args)) {
              if (isTestSuccess(toolOutput)) {
                verifyState.testsOk = true;
              }
            }
            // If both pass after edits, we're no longer dirty
            if (verifyState.typecheckOk && verifyState.testsOk) {
              verifyState.dirtySinceVerify = false;
            }
          }
        }

        turn.toolResults = toolResults;
        turns.push(turn);
      }
    }

    if (turnCount >= maxTurns && continueLoop) {
      return yield* Effect.fail(
        new AgentLoopError("max_turns_exceeded", `Exceeded maximum of ${maxTurns} turns`),
      );
    }

    const lastTurn = turns[turns.length - 1];
    return {
      turns,
      finalMessage: lastTurn?.content ?? null,
      totalTurns: turnCount,
      verifyState,
    };
  });
