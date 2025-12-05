import * as JSONSchema from "effect/JSONSchema";
import type { Tool } from "../tools/schema.js";
import type { ChatMessage, ChatRequest } from "./openrouter-types.js";

export const DEFAULT_OPENROUTER_MODEL = "x-ai/grok-4.1-fast:free";

export const toolToOpenRouterDefinition = (tool: Tool<any>): Record<string, unknown> => {
  const schema = JSONSchema.make(tool.schema) as unknown as Record<string, unknown>;
  const { $schema, ...parameters } = schema;

  return {
    type: "function",
    function: {
      name: tool.name,
      description: tool.description,
      parameters,
      strict: true,
    },
  };
};

export const makeRequestBody = (request: ChatRequest) => {
  const tools = request.tools?.map(toolToOpenRouterDefinition);

  const messages: Array<ChatMessage | { role: "tool"; toolCallId: string }> = request.messages.map(
    (msg) => {
      if (msg.role === "tool" && msg.tool_call_id) {
        return {
          role: "tool" as const,
          toolCallId: msg.tool_call_id,
          content: msg.content,
          ...(msg.name ? { name: msg.name } : {}),
        };
      }
      return msg;
    },
  );

  return {
    model: request.model ?? DEFAULT_OPENROUTER_MODEL,
    messages,
    tools,
    tool_choice: request.toolChoice ?? (tools && tools.length > 0 ? "auto" : undefined),
    temperature: request.temperature,
    max_tokens: request.maxTokens,
    stream: false,
  };
};
