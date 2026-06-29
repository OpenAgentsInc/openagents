import { Effect, Schema as S } from "effect";
import {
  type ProbeLlmContentPart,
  type ProbeLlmMessage,
  type ProbeLlmRequest,
  stringifyToolResult,
} from "../../llm/index.js";
import { ProbeLlmEvents, type ProbeLlmEvent, type ProbeLlmFinishReason } from "../../llm/events.js";
import { type ProbeLlmToolDefinition } from "../../llm/tool.js";
import { makeProbeLlmUsage, type ProbeLlmUsage } from "../../llm/usage.js";

export interface PsionicOpenAiTool {
  readonly type: "function";
  readonly function: {
    readonly name: string;
    readonly description: string;
    readonly parameters: Record<string, unknown>;
  };
}

export interface PsionicOpenAiChatBody {
  readonly model: string;
  readonly messages: ReadonlyArray<Record<string, unknown>>;
  readonly tools?: ReadonlyArray<PsionicOpenAiTool>;
  readonly tool_choice?: unknown;
  readonly max_tokens?: number;
  readonly temperature?: number;
  readonly top_p?: number;
  readonly stop?: ReadonlyArray<string>;
  readonly stream?: boolean;
  readonly stream_options?: {
    readonly include_usage: boolean;
  };
}

export class PsionicOpenAiProtocolError extends S.TaggedErrorClass<PsionicOpenAiProtocolError>()(
  "PsionicOpenAiProtocolError",
  {
    reason: S.String,
    failureClass: S.String,
  },
) {}

export function lowerProbeLlmRequestToPsionicOpenAiBody(
  request: ProbeLlmRequest,
  input: { readonly stream?: boolean } = {},
): PsionicOpenAiChatBody {
  const toolsEnabled = request.tools.length > 0 && request.toolChoice?.type !== "none";

  return dropUndefined({
    model: request.model.model,
    messages: [...lowerSystemMessages(request.system), ...request.messages.flatMap(lowerMessage)],
    tools: toolsEnabled ? request.tools.map(lowerTool) : undefined,
    tool_choice: toolsEnabled ? lowerToolChoice(request.toolChoice) : undefined,
    max_tokens: request.generation?.maxTokens,
    temperature: request.generation?.temperature,
    top_p: request.generation?.topP,
    stop: request.generation?.stop,
    stream: input.stream === true ? true : undefined,
    stream_options: input.stream === true ? { include_usage: true } : undefined,
  });
}

export function parsePsionicOpenAiChatCompletion(
  raw: unknown,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, PsionicOpenAiProtocolError> {
  return Effect.gen(function* () {
    if (!isRecord(raw)) {
      return yield* protocolError("Psionic chat response was not an object", "malformed_response");
    }

    const choice = Array.isArray(raw.choices) && isRecord(raw.choices[0]) ? raw.choices[0] : undefined;
    const message = isRecord(choice?.message) ? choice.message : undefined;

    if (message === undefined) {
      return yield* protocolError("Psionic chat response did not include choices[0].message", "malformed_response");
    }

    const events: ProbeLlmEvent[] = [ProbeLlmEvents.stepStart(0)];
    const content = typeof message.content === "string" ? message.content : "";

    if (content.length > 0) {
      events.push(ProbeLlmEvents.textDelta({ id: "text-0", text: content }));
    }

    for (const toolCall of parseOpenAiToolCalls(message.tool_calls)) {
      const event = yield* toolCall;
      events.push(event);
    }

    const reason = mapOpenAiFinishReason(typeof choice?.finish_reason === "string" ? choice.finish_reason : undefined, events);
    const usage = mapOpenAiUsage(raw.usage);
    events.push(ProbeLlmEvents.stepFinish({ index: 0, reason, usage }));
    events.push(ProbeLlmEvents.finish({ reason, usage }));

    return events;
  });
}

export interface PsionicOpenAiStreamState {
  readonly text: string;
  readonly toolCalls: Readonly<Record<number, MutableToolCall>>;
  readonly finishReason?: string;
  readonly usage?: ProbeLlmUsage;
}

interface MutableToolCall {
  id?: string;
  name?: string;
  arguments: string;
}

export function makePsionicOpenAiStreamState(): PsionicOpenAiStreamState {
  return {
    text: "",
    toolCalls: {},
  };
}

export function parsePsionicOpenAiSsePayload(
  payload: string,
  state: PsionicOpenAiStreamState,
): Effect.Effect<{ readonly state: PsionicOpenAiStreamState; readonly events: ReadonlyArray<ProbeLlmEvent> }, PsionicOpenAiProtocolError> {
  return Effect.gen(function* () {
    const raw = yield* parseJson(payload, "Psionic stream payload was not JSON");

    if (!isRecord(raw)) {
      return yield* protocolError("Psionic stream payload was not an object", "malformed_response");
    }

    const choice = Array.isArray(raw.choices) && isRecord(raw.choices[0]) ? raw.choices[0] : undefined;
    const delta = isRecord(choice?.delta) ? choice.delta : undefined;
    const events: ProbeLlmEvent[] = [];
    let text = state.text;
    let toolCalls = cloneToolCalls(state.toolCalls);
    const usage = raw.usage === undefined ? state.usage : mapOpenAiUsage(raw.usage);

    if (delta !== undefined) {
      if (typeof delta.content === "string" && delta.content.length > 0) {
        text += delta.content;
        events.push(ProbeLlmEvents.textDelta({ id: "text-0", text: delta.content }));
      }

      for (const deltaToolCall of Array.isArray(delta.tool_calls) ? delta.tool_calls : []) {
        if (!isRecord(deltaToolCall)) {
          continue;
        }

        const index = typeof deltaToolCall.index === "number" ? deltaToolCall.index : 0;
        const current = toolCalls[index] ?? { arguments: "" };
        const fn = isRecord(deltaToolCall.function) ? deltaToolCall.function : {};
        toolCalls[index] = {
          id: typeof deltaToolCall.id === "string" ? deltaToolCall.id : current.id,
          name: typeof fn.name === "string" ? fn.name : current.name,
          arguments: current.arguments + (typeof fn.arguments === "string" ? fn.arguments : ""),
        };
      }
    }

    return {
      state: {
        text,
        toolCalls,
        finishReason: typeof choice?.finish_reason === "string" ? choice.finish_reason : state.finishReason,
        usage,
      },
      events,
    };
  });
}

export function finishPsionicOpenAiStreamState(
  state: PsionicOpenAiStreamState,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, PsionicOpenAiProtocolError> {
  return Effect.gen(function* () {
    const events: ProbeLlmEvent[] = [];

    for (const [index, toolCall] of Object.entries(state.toolCalls)) {
      if (toolCall.name === undefined) {
        return yield* protocolError(`Psionic stream tool call ${index} was missing a function name`, "malformed_tool_call");
      }

      const input = yield* parseToolArguments(toolCall.arguments);
      events.push(ProbeLlmEvents.toolCall({
        id: toolCall.id ?? `tool_${index}`,
        name: toolCall.name,
        input,
      }));
    }

    const reason = mapOpenAiFinishReason(state.finishReason, events);
    events.push(ProbeLlmEvents.stepFinish({ index: 0, reason, usage: state.usage }));
    events.push(ProbeLlmEvents.finish({ reason, usage: state.usage }));

    return events;
  });
}

function lowerSystemMessages(messages: ReadonlyArray<ProbeLlmMessage>): ReadonlyArray<Record<string, unknown>> {
  const content = messages.flatMap(messageTextParts).join("\n\n");
  return content.length === 0 ? [] : [{ role: "system", content }];
}

function lowerMessage(message: ProbeLlmMessage): ReadonlyArray<Record<string, unknown>> {
  if (message.role === "system") {
    return lowerSystemMessages([message]);
  }

  if (message.role === "assistant") {
    const content = messageTextParts(message).join("");
    const toolCalls = message.content.flatMap(lowerAssistantToolCallPart);
    return [
      dropUndefined({
        role: "assistant",
        content: content.length > 0 ? content : toolCalls.length === 0 ? "" : null,
        tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
      }),
    ];
  }

  if (message.role === "tool") {
    return message.content.flatMap(lowerToolResultPart);
  }

  return [{ role: "user", content: message.content.flatMap(partText).join("\n") }];
}

function lowerAssistantToolCallPart(part: ProbeLlmContentPart): ReadonlyArray<Record<string, unknown>> {
  if (part.type !== "tool-call") {
    return [];
  }

  return [
    {
      id: part.id,
      type: "function",
      function: {
        name: part.name,
        arguments: JSON.stringify(part.input ?? {}),
      },
    },
  ];
}

function lowerToolResultPart(part: ProbeLlmContentPart): ReadonlyArray<Record<string, unknown>> {
  if (part.type !== "tool-result") {
    return [];
  }

  return [
    {
      role: "tool",
      tool_call_id: part.id,
      name: part.name,
      content: stringifyToolResult(part.result.value),
    },
  ];
}

function lowerTool(tool: ProbeLlmToolDefinition): PsionicOpenAiTool {
  return {
    type: "function",
    function: {
      name: tool.name,
      description: tool.description,
      parameters: tool.inputSchema,
    },
  };
}

function lowerToolChoice(toolChoice: ProbeLlmRequest["toolChoice"]): unknown {
  if (toolChoice === undefined || toolChoice.type === "auto") {
    return "auto";
  }

  if (toolChoice.type === "none") {
    return "none";
  }

  if (toolChoice.type === "required") {
    return "required";
  }

  return {
    type: "function",
    function: {
      name: toolChoice.name,
    },
  };
}

function parseOpenAiToolCalls(raw: unknown): ReadonlyArray<Effect.Effect<ProbeLlmEvent, PsionicOpenAiProtocolError>> {
  if (!Array.isArray(raw)) {
    return [];
  }

  return raw.map((toolCall, index) =>
    Effect.gen(function* () {
      if (!isRecord(toolCall) || !isRecord(toolCall.function)) {
        return yield* protocolError(`Psionic tool call ${index} was malformed`, "malformed_tool_call");
      }

      const name = toolCall.function.name;

      if (typeof name !== "string" || name.length === 0) {
        return yield* protocolError(`Psionic tool call ${index} was missing a function name`, "malformed_tool_call");
      }

      const input = yield* parseToolArguments(toolCall.function.arguments);

      return ProbeLlmEvents.toolCall({
        id: typeof toolCall.id === "string" ? toolCall.id : `tool_${index}`,
        name,
        input,
      });
    })
  );
}

function parseToolArguments(value: unknown): Effect.Effect<unknown, PsionicOpenAiProtocolError> {
  if (value === undefined || value === "") {
    return Effect.succeed({});
  }

  if (isRecord(value)) {
    return Effect.succeed(value);
  }

  if (typeof value !== "string") {
    return protocolError("Psionic tool call arguments were not a JSON string", "malformed_tool_arguments");
  }

  return parseJson(value, "Psionic tool call arguments were not valid JSON");
}

function parseJson(value: string, message: string): Effect.Effect<unknown, PsionicOpenAiProtocolError> {
  return Effect.try({
    try: () => JSON.parse(value),
    catch: () => new PsionicOpenAiProtocolError({ reason: message, failureClass: "malformed_tool_arguments" }),
  });
}

function mapOpenAiUsage(raw: unknown): ProbeLlmUsage | undefined {
  if (!isRecord(raw)) {
    return undefined;
  }

  return makeProbeLlmUsage({
    inputTokens: typeof raw.prompt_tokens === "number" ? raw.prompt_tokens : undefined,
    outputTokens: typeof raw.completion_tokens === "number" ? raw.completion_tokens : undefined,
    totalTokens: typeof raw.total_tokens === "number" ? raw.total_tokens : undefined,
  });
}

function mapOpenAiFinishReason(raw: string | undefined, events: ReadonlyArray<ProbeLlmEvent>): ProbeLlmFinishReason {
  if (events.some(ProbeLlmEvents.isToolCall)) {
    return "tool_calls";
  }

  if (raw === "stop") {
    return "stop";
  }

  if (raw === "length") {
    return "length";
  }

  if (raw === "content_filter") {
    return "content_filter";
  }

  return raw === undefined ? "unknown" : "unknown";
}

function messageTextParts(message: ProbeLlmMessage): ReadonlyArray<string> {
  return message.content.flatMap(partText);
}

function partText(part: ProbeLlmContentPart): ReadonlyArray<string> {
  if (part.type === "text") {
    return [part.text];
  }

  if (part.type === "tool-result") {
    return [stringifyToolResult(part.result.value)];
  }

  return [];
}

function cloneToolCalls(input: Readonly<Record<number, MutableToolCall>>): Record<number, MutableToolCall> {
  return Object.fromEntries(
    Object.entries(input).map(([index, toolCall]) => [index, { ...toolCall }]),
  ) as Record<number, MutableToolCall>;
}

function dropUndefined<T extends Record<string, unknown>>(input: T): T {
  return Object.fromEntries(Object.entries(input).filter(([, value]) => value !== undefined)) as T;
}

function protocolError(reason: string, failureClass: string): Effect.Effect<never, PsionicOpenAiProtocolError> {
  return Effect.fail(new PsionicOpenAiProtocolError({ reason, failureClass }));
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
