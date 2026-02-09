import { AiError, LanguageModel, Prompt, Response, Tool } from "@effect/ai"
import { Effect, Stream } from "effect"

type UsageEncoded = typeof Response.Usage.Encoded

type WorkersAiUsage = {
  readonly prompt_tokens?: number
  readonly completion_tokens?: number
}

type WorkersAiNonStreamOutput = {
  readonly choices?: ReadonlyArray<{
    readonly message?: { readonly content?: string; readonly reasoning_content?: string; readonly tool_calls?: unknown }
    readonly finish_reason?: string | null
  }>
  readonly tool_calls?: unknown
  readonly finish_reason?: string | null
  readonly response?: unknown
  readonly usage?: WorkersAiUsage
}

type WorkersAiStreamChunk = {
  readonly response?: string
  readonly usage?: WorkersAiUsage
  readonly tool_calls?: unknown
  readonly finish_reason?: string | null
  readonly choices?: ReadonlyArray<{
    readonly delta?: { readonly content?: string; readonly reasoning_content?: string }
    readonly finish_reason?: string | null
  }>
}

type WorkersAiChatMessage =
  | { readonly role: "system"; readonly content: string }
  | { readonly role: "user"; readonly content: string }
  | {
    readonly role: "assistant"
    readonly content: string
    readonly tool_calls?: ReadonlyArray<{
      readonly id: string
      readonly type: "function"
      readonly function: { readonly name: string; readonly arguments: string }
    }>
  }
  | { readonly role: "tool"; readonly name: string; readonly content: string; readonly tool_call_id: string }

type WorkersAiTool = {
  readonly type: "function"
  readonly function: {
    readonly name: string
    readonly description?: string
    readonly parameters: unknown
  }
}

type WorkersAiToolChoice = "auto" | "none" | "any"

function mapFinishReason(raw: unknown): Response.FinishReason {
  const r = typeof raw === "string" ? raw : ""
  switch (r) {
    case "stop":
      return "stop"
    case "length":
    case "model_length":
      return "length"
    case "tool_calls":
      return "tool-calls"
    case "error":
      return "error"
    case "unknown":
      return "unknown"
    case "other":
      return "other"
    default:
      return "stop"
  }
}

function toUsage(u: WorkersAiUsage | undefined): UsageEncoded {
  const input = typeof u?.prompt_tokens === "number" ? u.prompt_tokens : undefined
  const output =
    typeof u?.completion_tokens === "number" ? u.completion_tokens : undefined
  const total = input !== undefined && output !== undefined ? input + output : undefined
  return {
    inputTokens: input,
    outputTokens: output,
    totalTokens: total,
  }
}

function getToolCalls(output: WorkersAiNonStreamOutput): unknown[] {
  const top = output.tool_calls
  if (Array.isArray(top)) return top as unknown[]

  const nested = output.choices?.[0]?.message?.tool_calls
  if (Array.isArray(nested)) return nested as unknown[]

  return []
}

function parseToolCallParams(raw: unknown): unknown {
  if (raw == null) return {}
  if (typeof raw === "object") return raw
  if (typeof raw === "string") {
    try {
      return JSON.parse(raw)
    } catch {
      return {}
    }
  }
  return {}
}

type WorkersAiToolCallRaw = {
  id?: string
  type?: string
  name?: string
  arguments?: string
  function?: { name?: string; arguments?: string }
}

function toToolCallParts(rawToolCalls: unknown[]): Array<Response.ToolCallPartEncoded> {
  const out: Array<Response.ToolCallPartEncoded> = []

  for (let idx = 0; idx < rawToolCalls.length; idx++) {
    const raw = rawToolCalls[idx]
    if (!raw || typeof raw !== "object") continue

    const record = raw as WorkersAiToolCallRaw
    const fallbackId = `call_${idx}`

    // OpenAI-style: { id, type, function: { name, arguments } }
    if (record.function && typeof record.function === "object") {
      const fn = record.function
      const id = typeof record.id === "string" && record.id.length > 0 ? record.id : fallbackId
      const name = typeof fn.name === "string" ? fn.name : "tool"
      const params = parseToolCallParams(fn.arguments)
      out.push({
        type: "tool-call",
        id,
        name,
        params,
        providerExecuted: false,
      })
      continue
    }

    // Legacy-ish: { id, name, arguments }
    if (typeof record.name === "string") {
      const id = typeof record.id === "string" && record.id.length > 0 ? record.id : fallbackId
      const name = record.name
      const params = parseToolCallParams(record.arguments)
      out.push({
        type: "tool-call",
        id,
        name,
        params,
        providerExecuted: false,
      })
      continue
    }
  }

  return out
}

function promptToWorkersAiMessages(prompt: Prompt.Prompt): Array<WorkersAiChatMessage> {
  const out: Array<WorkersAiChatMessage> = []

  for (const message of prompt.content) {
    switch (message.role) {
      case "system": {
        out.push({ role: "system", content: message.content })
        break
      }
      case "user": {
        const text = message.content
          .map((p) => (p.type === "text" ? p.text : ""))
          .join("\n")
        out.push({ role: "user", content: text })
        break
      }
      case "assistant": {
        let text = ""
        const tool_calls: Array<{
          id: string
          type: "function"
          function: { name: string; arguments: string }
        }> = []

        for (const p of message.content) {
          switch (p.type) {
            case "text":
              text += p.text
              break
            case "reasoning":
              text += p.text
              break
            case "tool-call":
              tool_calls.push({
                id: p.id,
                type: "function",
                function: { name: p.name, arguments: JSON.stringify(p.params ?? {}) },
              })
              break
          }
        }

        out.push({
          role: "assistant",
          content: text,
          ...(tool_calls.length > 0 ? { tool_calls } : {}),
        })
        break
      }
      case "tool": {
        for (const p of message.content) {
          if (p.type !== "tool-result") continue
          // Workers AI expects tool_call_id to match the assistant message tool_calls ids.
          out.push({
            role: "tool",
            name: p.name,
            tool_call_id: p.id,
            content: typeof p.result === "string" ? p.result : JSON.stringify(p.result),
          })
        }
        break
      }
    }
  }

  return out
}

function mapTools(tools: ReadonlyArray<Tool.Any>): Array<WorkersAiTool> {
  return tools.map((tool) => {
    const description =
      typeof tool.description === "string" ? tool.description : undefined

    return {
      type: "function",
      function: {
        name: tool.name,
        ...(description ? { description } : {}),
        parameters: Tool.getJsonSchemaFromSchemaAst(tool.parametersSchema.ast),
      },
    }
  })
}

function mapToolChoice(choice: LanguageModel.ProviderOptions["toolChoice"]): WorkersAiToolChoice {
  if (choice === "none") return "none"
  if (choice === "required") return "any"
  return "auto"
}

function toolChoiceSingleToolName(choice: LanguageModel.ProviderOptions["toolChoice"]): string | null {
  if (choice && typeof choice === "object" && "tool" in choice) {
    const toolName = (choice as { tool: unknown }).tool
    return typeof toolName === "string" ? toolName : null
  }
  return null
}

type PartialToolCall = {
  readonly index: number
  readonly id?: string
  readonly type?: string
  readonly function?: { readonly name?: string; readonly arguments?: string }
}

type MergedToolCall = {
  id: string
  type: string
  function: { name: string; arguments: string }
}

function mergePartialToolCalls(partialCalls: ReadonlyArray<PartialToolCall>): unknown[] {
  const mergedByIndex: Record<number, MergedToolCall> = {}

  for (const call of partialCalls) {
    const idx = call.index
    if (typeof idx !== "number") continue

    if (!mergedByIndex[idx]) {
      mergedByIndex[idx] = {
        function: {
          arguments: "",
          name: call.function?.name ?? "",
        },
        id: call.id ?? "",
        type: call.type ?? "function",
      }
    } else {
      if (call.id) mergedByIndex[idx].id = call.id
      if (call.type) mergedByIndex[idx].type = call.type
      if (call.function?.name) mergedByIndex[idx].function.name = call.function.name
    }

    if (call.function?.arguments) {
      mergedByIndex[idx].function.arguments += call.function.arguments
    }
  }

  return Object.values(mergedByIndex)
}

function normalizeStreamToolCalls(partials: ReadonlyArray<unknown>): unknown[] {
  // Workers AI sometimes emits partial tool_calls with an `index` field.
  // We merge those by index into complete calls at end-of-stream.
  const partialCalls = partials.filter((c): c is PartialToolCall => Boolean(c) && typeof c === "object") as Array<PartialToolCall>
  return mergePartialToolCalls(partialCalls)
}

export const makeWorkersAiLanguageModel = (options: {
  readonly binding: Ai
  readonly model: string
  readonly maxOutputTokens: number
  readonly temperature?: number
  readonly topP?: number
}) =>
  LanguageModel.make({
    generateText: (providerOptions) =>
      Effect.tryPromise({
        try: async () => {
          const messages = promptToWorkersAiMessages(providerOptions.prompt)

          let tools = providerOptions.tools
          const onlyTool = toolChoiceSingleToolName(providerOptions.toolChoice)
          if (onlyTool) {
            tools = tools.filter((t) => t.name === onlyTool)
          }

          const output = (await options.binding.run(
            options.model as Parameters<Ai["run"]>[0],
            {
              model: options.model,
              max_tokens: options.maxOutputTokens,
              messages,
              ...(tools.length > 0 ? { tools: mapTools(tools) } : {}),
              ...(tools.length > 0
                ? { tool_choice: mapToolChoice(providerOptions.toolChoice) }
                : {}),
              ...(typeof options.temperature === "number" ? { temperature: options.temperature } : {}),
              ...(typeof options.topP === "number" ? { top_p: options.topP } : {}),
            } as Parameters<Ai["run"]>[1],
            {},
          )) as WorkersAiNonStreamOutput

          const reasoning = output.choices?.[0]?.message?.reasoning_content
          const text =
            output.choices?.[0]?.message?.content ??
            (typeof output.response === "string"
              ? output.response
              : JSON.stringify(output.response ?? ""))

          const rawToolCalls = getToolCalls(output)
          const toolCalls = toToolCallParts(rawToolCalls)

          const finishReason = mapFinishReason(output.choices?.[0]?.finish_reason ?? output.finish_reason)
          const usage = toUsage(output.usage)

          const parts: Array<Response.PartEncoded> = []
          if (typeof reasoning === "string" && reasoning.length > 0) {
            parts.push({ type: "reasoning", text: reasoning, metadata: {} })
          }
          parts.push({ type: "text", text: typeof text === "string" ? text : String(text), metadata: {} })
          parts.push(...toolCalls.map((p) => ({ ...p, metadata: {} })))
          parts.push({ type: "finish", reason: finishReason, usage, metadata: {} })

          return parts
        },
        catch: (cause) =>
          new AiError.UnknownError({
            module: "WorkersAI",
            method: "generateText",
            description: "Workers AI generateText failed",
            cause,
          }),
      }),

    streamText: (providerOptions) => {
      const onError = (cause: unknown) =>
        new AiError.UnknownError({
          module: "WorkersAI",
          method: "streamText",
          description: "Workers AI streamText failed",
          cause,
        })

      const iter = async function* (): AsyncGenerator<Response.StreamPartEncoded> {
        const messages = promptToWorkersAiMessages(providerOptions.prompt)

        let tools = providerOptions.tools
        const onlyTool = toolChoiceSingleToolName(providerOptions.toolChoice)
        if (onlyTool) {
          tools = tools.filter((t) => t.name === onlyTool)
        }

        const response = await options.binding.run(
          options.model as Parameters<Ai["run"]>[0],
          {
            model: options.model,
            max_tokens: options.maxOutputTokens,
            messages,
            stream: true,
            ...(tools.length > 0 ? { tools: mapTools(tools) } : {}),
            ...(tools.length > 0
              ? { tool_choice: mapToolChoice(providerOptions.toolChoice) }
              : {}),
            ...(typeof options.temperature === "number" ? { temperature: options.temperature } : {}),
            ...(typeof options.topP === "number" ? { top_p: options.topP } : {}),
          } as Parameters<Ai["run"]>[1],
          {},
        )

        if (!(response instanceof ReadableStream)) {
          throw new Error("Expected ReadableStream from Workers AI when stream=true")
        }

        const reader = response.getReader()
        const decoder = new TextDecoder()

        let buffer = ""
        let usage: UsageEncoded = { inputTokens: undefined, outputTokens: undefined, totalTokens: undefined }
        const partialToolCalls: Array<unknown> = []
        let finishReasonRaw: unknown = null

        let textId: string | null = null
        let reasoningId: string | null = null

        const ensureText = () => {
          if (!textId) {
            textId = crypto.randomUUID()
            return { type: "text-start" as const, id: textId, metadata: {} }
          }
          return null
        }

        const ensureReasoning = () => {
          if (!reasoningId) {
            reasoningId = crypto.randomUUID()
            return { type: "reasoning-start" as const, id: reasoningId, metadata: {} }
          }
          return null
        }

        while (true) {
          const { done, value } = await reader.read()
          if (done) break

          buffer += decoder.decode(value, { stream: true })

          while (true) {
            const nl = buffer.indexOf("\n")
            if (nl < 0) break
            const line = buffer.slice(0, nl).replace(/\r$/, "")
            buffer = buffer.slice(nl + 1)

            if (!line.startsWith("data:")) continue
            const data = line.slice("data:".length).trim()
            if (!data) continue
            if (data === "[DONE]") {
              buffer = ""
              break
            }

            let chunk: WorkersAiStreamChunk
            try {
              chunk = JSON.parse(data) as WorkersAiStreamChunk
            } catch {
              continue
            }

            if (chunk.usage) {
              usage = toUsage(chunk.usage)
            }
            if (typeof chunk.finish_reason === "string" || chunk.finish_reason === null) {
              finishReasonRaw = chunk.finish_reason
            }
            if (
              typeof chunk.choices?.[0]?.finish_reason === "string" ||
              chunk.choices?.[0]?.finish_reason === null
            ) {
              finishReasonRaw = chunk.choices[0]!.finish_reason
            }
            if (Array.isArray(chunk.tool_calls)) {
              partialToolCalls.push(...chunk.tool_calls)
            }

            const responseText = typeof chunk.response === "string" ? chunk.response : ""
            if (responseText.length > 0) {
              const start = ensureText()
              if (start) yield start
              yield { type: "text-delta", id: textId!, delta: responseText, metadata: {} }
            }

            const reasoningDelta =
              typeof chunk.choices?.[0]?.delta?.reasoning_content === "string"
                ? chunk.choices[0]!.delta!.reasoning_content!
                : ""
            if (reasoningDelta.length > 0) {
              const start = ensureReasoning()
              if (start) yield start
              yield { type: "reasoning-delta", id: reasoningId!, delta: reasoningDelta, metadata: {} }
            }

            const textDelta =
              typeof chunk.choices?.[0]?.delta?.content === "string"
                ? chunk.choices[0]!.delta!.content!
                : ""
            if (textDelta.length > 0) {
              const start = ensureText()
              if (start) yield start
              yield { type: "text-delta", id: textId!, delta: textDelta, metadata: {} }
            }
          }
        }

        // Flush tool calls after stream ends.
        const mergedToolCalls = normalizeStreamToolCalls(partialToolCalls)
        const toolCalls = toToolCallParts(mergedToolCalls).map((p) => ({ ...p, metadata: {} }))
        for (const tc of toolCalls) {
          yield tc
        }

        if (reasoningId) {
          yield { type: "reasoning-end", id: reasoningId, metadata: {} }
        }
        if (textId) {
          yield { type: "text-end", id: textId, metadata: {} }
        }

        const finishReason: Response.FinishReason =
          typeof finishReasonRaw === "string"
            ? mapFinishReason(finishReasonRaw)
            : toolCalls.length > 0
              ? "tool-calls"
              : "stop"
        yield { type: "finish", reason: finishReason, usage, metadata: {} }
      }

      return Stream.fromAsyncIterable(iter(), onError)
    },
  }).pipe(
    Effect.catchAll((error) => {
      // LanguageModel.make can fail only if it can't build defaults; keep the
      // error in AiError space for callers.
      if (AiError.isAiError(error)) return Effect.fail(error)
      return Effect.fail(
        new AiError.UnknownError({
          module: "WorkersAI",
          method: "makeLanguageModel",
          description: "Failed to create Workers AI LanguageModel",
          cause: error,
        }),
      )
    }),
  )
