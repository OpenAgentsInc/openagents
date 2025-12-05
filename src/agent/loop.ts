import { Effect } from "effect";
import type { Tool, ToolResult } from "../tools/schema.js";
import { ToolExecutionError, runTool } from "../tools/schema.js";
import type { ChatMessage, ChatRequest } from "../llm/openrouter.js";
import { OpenRouterClient } from "../llm/openrouter.js";

// Event types that can be emitted during the loop
export type LoopEvent = 
  | { type: "turn_start"; turn: number }
  | { type: "llm_request"; turn: number; messages: ChatMessage[]; toolNames: string[] }
  | { type: "llm_response"; turn: number; hasToolCalls: boolean; message: ChatMessage; toolCalls: Array<{ id: string; name: string; arguments: string }> }
  | { type: "tool_call"; tool: string; toolCallId: string; args: string }
  | { type: "tool_result"; tool: string; toolCallId: string; ok: boolean; result: ToolResult }
  | { type: "tool_output"; tool: string; toolCallId: string; chunk: string }
  | { type: "edit_detected"; tool: string };

export interface AgentConfig {
  model?: string;
  systemPrompt?: string;
  maxTurns?: number;
  temperature?: number;
  onEvent?: (event: LoopEvent) => void;  // Callback for streaming events DURING the loop
  /** Streaming tool output (stdout/stderr) */
  onOutput?: (text: string) => void;
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
  onOutput?: (text: string) => void,
  emit?: (event: LoopEvent) => void,
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

    const result = yield* runTool(tool, args, {
      ...(signal ? { signal } : {}),
      ...(onOutput
        ? {
            onStream: (chunk) => {
              if (chunk.type === "text") {
                onOutput(chunk.text);
                emit?.({ type: "tool_output", tool: tool.name, toolCallId: toolCall.id, chunk: chunk.text });
              }
            },
          }
        : {}),
    }).pipe(
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
    
    // Event emitter - call synchronously for immediate flush
    const emit = config.onEvent ?? (() => {});

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
      
      // EMIT turn_start BEFORE calling LLM
      emit({ type: "turn_start", turn: turnCount });

      const request: ChatRequest = {
        messages,
        tools: tools as unknown as Tool<any>[],
        ...(config.model ? { model: config.model } : {}),
        ...(config.temperature !== undefined ? { temperature: config.temperature } : {}),
      };

      // EMIT llm_request BEFORE calling provider
      emit({ type: "llm_request", turn: turnCount, messages, toolNames: tools.map((t) => t.name) });

      const response = yield* client.chat(request).pipe(
        Effect.mapError((e: Error) => new AgentLoopError("llm_error", e.message)),
      );

      const choice = response.choices[0];
      if (!choice) {
        return yield* Effect.fail(new AgentLoopError("no_response", "No response from LLM"));
      }

      const assistantMessage = choice.message;
      const toolCalls = assistantMessage.tool_calls ?? [];
      const assistantContent =
        typeof assistantMessage.content === "string"
          ? assistantMessage.content
          : "";
      const safeAssistantMessage: ChatMessage = {
        ...assistantMessage,
        content: assistantContent,
      };
      
      // EMIT llm_response AFTER LLM returns
      emit({
        type: "llm_response",
        turn: turnCount,
        hasToolCalls: toolCalls.length > 0,
        message: safeAssistantMessage,
        toolCalls,
      });

      messages.push({
        role: "assistant",
        content: safeAssistantMessage.content,
      });

      const turnContent =
        typeof safeAssistantMessage.content === "string" ? safeAssistantMessage.content : "";
      const turn: AgentTurn = {
        role: "assistant",
        content: turnContent,
        ...(toolCalls.length > 0 ? { toolCalls } : {}),
      };

      if (toolCalls.length === 0) {
        turns.push(turn);
        continueLoop = false;
      } else {
        const toolResults: AgentTurn["toolResults"] = [];

        for (const toolCall of toolCalls) {
          // EMIT tool_call BEFORE executing
          emit({ type: "tool_call", tool: toolCall.name, toolCallId: toolCall.id, args: toolCall.arguments });
          
          const result = yield* executeToolCall(tools, toolCall, undefined, config.onOutput, emit);
          toolResults.push(result);
          
          // EMIT tool_result AFTER executing
          emit({ type: "tool_result", tool: toolCall.name, toolCallId: toolCall.id, ok: !result.isError, result: result.result });

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
            // EMIT edit_detected
            emit({ type: "edit_detected", tool: toolName });
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
    const finalMessage = lastTurn?.content ?? null;
    return {
      turns,
      finalMessage,
      totalTurns: turnCount,
      verifyState,
    };
  });
