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

export interface AgentResult {
  turns: AgentTurn[];
  finalMessage: string | null;
  totalTurns: number;
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

const executeToolCall = <R>(
  tools: Tool<any, any, R, any>[],
  toolCall: { id: string; name: string; arguments: string },
  signal?: AbortSignal,
): Effect.Effect<
  { toolCallId: string; name: string; result: ToolResult; isError: boolean },
  never,
  R
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

    if (config.systemPrompt) {
      messages.push({ role: "system", content: config.systemPrompt });
    }
    messages.push({ role: "user", content: userMessage });

    let turnCount = 0;
    let continueLoop = true;

    while (continueLoop && turnCount < maxTurns) {
      turnCount++;

      const request: ChatRequest = {
        model: config.model,
        messages,
        tools,
        temperature: config.temperature,
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
        toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
      };

      if (toolCalls.length === 0) {
        turns.push(turn);
        continueLoop = false;
      } else {
        const toolResults: AgentTurn["toolResults"] = [];

        for (const toolCall of toolCalls) {
          const result = yield* executeToolCall(tools, toolCall);
          toolResults.push(result);

          messages.push({
            role: "tool",
            tool_call_id: toolCall.id,
            name: toolCall.name,
            content:
              result.result.content
                .filter((c): c is { type: "text"; text: string } => c.type === "text")
                .map((c) => c.text)
                .join("\n") || (result.isError ? "Error" : "Success"),
          });
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
    };
  });
