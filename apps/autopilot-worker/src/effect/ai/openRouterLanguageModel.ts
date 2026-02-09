/**
 * OpenRouter LanguageModel for Effect AI.
 * Uses https://openrouter.ai/api/v1/chat/completions (OpenAI-compatible).
 */
import { AiError, LanguageModel, Prompt, Response, Tool } from "@effect/ai"
import { Effect, Stream } from "effect"

type UsageEncoded = typeof Response.Usage.Encoded

type OpenRouterMessage =
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

type OpenRouterTool = {
  readonly type: "function"
  readonly function: {
    readonly name: string
    readonly description?: string
    readonly parameters: unknown
  }
}

function promptToMessages(prompt: Prompt.Prompt): Array<OpenRouterMessage> {
  const out: Array<OpenRouterMessage> = []
  for (const message of prompt.content) {
    switch (message.role) {
      case "system":
        out.push({ role: "system", content: message.content })
        break
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
        out.push({ role: "assistant", content: text, ...(tool_calls.length > 0 ? { tool_calls } : {}) })
        break
      }
      case "tool":
        for (const p of message.content) {
          if (p.type !== "tool-result") continue
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
  return out
}

function mapTools(tools: ReadonlyArray<Tool.Any>): Array<OpenRouterTool> {
  return tools.map((tool) => ({
    type: "function",
    function: {
      name: tool.name,
      ...(typeof tool.description === "string" ? { description: tool.description } : {}),
      parameters: Tool.getJsonSchemaFromSchemaAst(tool.parametersSchema.ast),
    },
  }))
}

function mapFinishReason(raw: unknown): Response.FinishReason {
  const r = typeof raw === "string" ? raw : ""
  switch (r) {
    case "stop":
      return "stop"
    case "length":
      return "length"
    case "tool_calls":
      return "tool-calls"
    default:
      return "stop"
  }
}

function toUsage(usage: { prompt_tokens?: number; completion_tokens?: number } | undefined): UsageEncoded {
  const input = typeof usage?.prompt_tokens === "number" ? usage.prompt_tokens : undefined
  const output = typeof usage?.completion_tokens === "number" ? usage.completion_tokens : undefined
  return {
    inputTokens: input,
    outputTokens: output,
    totalTokens: input !== undefined && output !== undefined ? input + output : undefined,
  }
}

type OpenRouterToolCallRaw = {
  id?: string
  type?: string
  function?: { name?: string; arguments?: string }
}

function toToolCallParts(rawToolCalls: unknown[]): Array<Response.ToolCallPartEncoded> {
  const out: Array<Response.ToolCallPartEncoded> = []
  for (let idx = 0; idx < rawToolCalls.length; idx++) {
    const raw = rawToolCalls[idx]
    if (!raw || typeof raw !== "object") continue
    const record = raw as OpenRouterToolCallRaw
    const fn = record.function
    if (fn && typeof fn === "object") {
      const id = typeof record.id === "string" && record.id.length > 0 ? record.id : `call_${idx}`
      const name = typeof fn.name === "string" ? fn.name : "tool"
      let params: unknown = {}
      try {
        params = typeof fn.arguments === "string" ? JSON.parse(fn.arguments) : fn.arguments ?? {}
      } catch {
        params = {}
      }
      out.push({ type: "tool-call", id, name, params, providerExecuted: false })
    }
  }
  return out
}

function toolChoiceSingleToolName(choice: LanguageModel.ProviderOptions["toolChoice"]): string | null {
  if (choice && typeof choice === "object" && "tool" in choice) {
    const toolName = (choice as { tool: unknown }).tool
    return typeof toolName === "string" ? toolName : null
  }
  return null
}

const OPENROUTER_BASE = "https://openrouter.ai/api/v1"

type OpenRouterCompletionResponse = {
  choices?: Array<{
    message?: { content?: string; tool_calls?: unknown }
    finish_reason?: string
  }>
  usage?: { prompt_tokens?: number; completion_tokens?: number }
}

export const makeOpenRouterLanguageModel = (options: {
  readonly apiKey: string
  readonly model: string
  readonly maxOutputTokens: number
  readonly temperature?: number
  readonly topP?: number
  readonly fetch?: typeof fetch
}) => {
  const doFetch = options.fetch ?? fetch
  return LanguageModel.make({
    generateText: (providerOptions) =>
      Effect.tryPromise({
        try: async () => {
          const messages = promptToMessages(providerOptions.prompt)
          let tools = providerOptions.tools
          const onlyTool = toolChoiceSingleToolName(providerOptions.toolChoice)
          if (onlyTool) tools = tools.filter((t) => t.name === onlyTool)

          const body: Record<string, unknown> = {
            model: options.model,
            max_tokens: options.maxOutputTokens,
            messages,
            ...(typeof options.temperature === "number" ? { temperature: options.temperature } : {}),
            ...(typeof options.topP === "number" ? { top_p: options.topP } : {}),
          }
          if (tools.length > 0) {
            body.tools = mapTools(tools)
            body.tool_choice = providerOptions.toolChoice === "none" ? "none" : providerOptions.toolChoice === "required" ? "required" : "auto"
          }

          const res = await doFetch(`${OPENROUTER_BASE}/chat/completions`, {
            method: "POST",
            headers: {
              Authorization: `Bearer ${options.apiKey}`,
              "Content-Type": "application/json",
            },
            body: JSON.stringify(body),
          })
          if (!res.ok) {
            const text = await res.text()
            throw new Error(`OpenRouter API error ${res.status}: ${text}`)
          }
          const output = (await res.json()) as OpenRouterCompletionResponse
          const text = output?.choices?.[0]?.message?.content ?? ""
          const rawToolCalls = Array.isArray(output?.choices?.[0]?.message?.tool_calls)
            ? (output.choices[0].message!.tool_calls as unknown[])
            : []
          const toolCalls = toToolCallParts(rawToolCalls)
          const finishReason = mapFinishReason(output?.choices?.[0]?.finish_reason ?? "stop")
          const usage = toUsage(output?.usage)

          const parts: Array<Response.PartEncoded> = [
            { type: "text", text: typeof text === "string" ? text : String(text), metadata: {} },
            ...toolCalls.map((p) => ({ ...p, metadata: {} })),
            { type: "finish", reason: finishReason, usage, metadata: {} },
          ]
          return parts
        },
        catch: (cause) =>
          new AiError.UnknownError({
            module: "OpenRouter",
            method: "generateText",
            description: "OpenRouter generateText failed",
            cause,
          }),
      }),

    streamText: (providerOptions) => {
      const onError = (cause: unknown) =>
        new AiError.UnknownError({
          module: "OpenRouter",
          method: "streamText",
          description: "OpenRouter streamText failed",
          cause,
        })
      const iter = async function* (): AsyncGenerator<Response.StreamPartEncoded> {
        const messages = promptToMessages(providerOptions.prompt)
        let tools = providerOptions.tools
        const onlyTool = toolChoiceSingleToolName(providerOptions.toolChoice)
        if (onlyTool) tools = tools.filter((t) => t.name === onlyTool)

        const body: Record<string, unknown> = {
          model: options.model,
          max_tokens: options.maxOutputTokens,
          messages,
          stream: true,
          ...(typeof options.temperature === "number" ? { temperature: options.temperature } : {}),
          ...(typeof options.topP === "number" ? { top_p: options.topP } : {}),
        }
        if (tools.length > 0) {
          body.tools = mapTools(tools)
          body.tool_choice = providerOptions.toolChoice === "none" ? "none" : providerOptions.toolChoice === "required" ? "required" : "auto"
        }

        const res = await doFetch(`${OPENROUTER_BASE}/chat/completions`, {
          method: "POST",
          headers: {
            Authorization: `Bearer ${options.apiKey}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify(body),
        })
        if (!res.ok) {
          const text = await res.text()
          throw new Error(`OpenRouter API error ${res.status}: ${text}`)
        }
        const reader = res.body?.getReader()
        if (!reader) throw new Error("OpenRouter stream: no body")
        const decoder = new TextDecoder()
        let buffer = ""
        let usage: UsageEncoded = { inputTokens: undefined, outputTokens: undefined, totalTokens: undefined }
        let finishReasonRaw: unknown = "stop"
        const partialToolCalls: unknown[] = []
        let textId: string | null = null

        const ensureText = () => {
          if (!textId) {
            textId = crypto.randomUUID()
            return { type: "text-start" as const, id: textId!, metadata: {} }
          }
          return null
        }

        while (true) {
          const { done, value } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          for (;;) {
            const nl = buffer.indexOf("\n")
            if (nl < 0) break
            const line = buffer.slice(0, nl).replace(/\r$/, "")
            buffer = buffer.slice(nl + 1)
            if (!line.startsWith("data:")) continue
            const data = line.slice(5).trim()
            if (!data || data === "[DONE]") continue
            let chunk: OpenRouterCompletionResponse & {
              choices?: Array<{ delta?: { content?: string; tool_calls?: unknown }; finish_reason?: string }>
            }
            try {
              chunk = JSON.parse(data) as typeof chunk
            } catch {
              continue
            }
            if (chunk?.usage) usage = toUsage(chunk.usage)
            if (chunk?.choices?.[0]?.finish_reason != null) finishReasonRaw = chunk.choices[0].finish_reason
            if (Array.isArray(chunk?.choices?.[0]?.delta?.tool_calls)) partialToolCalls.push(...chunk.choices[0].delta.tool_calls)
            const delta = chunk?.choices?.[0]?.delta?.content
            if (typeof delta === "string" && delta.length > 0) {
              const start = ensureText()
              if (start) yield start
              yield { type: "text-delta", id: textId!, delta, metadata: {} }
            }
          }
        }

        type StreamToolCallDelta = { index?: number; id?: string; function?: { name?: string; arguments?: string } }
        type MergedToolCall = { id: string; type: string; function: { name: string; arguments: string } }
        const rawToolCalls = partialToolCalls.filter((c): c is StreamToolCallDelta => Boolean(c) && typeof c === "object")
        const byIndex: Record<number, MergedToolCall> = {}
        for (const c of rawToolCalls) {
          const idx = c.index ?? 0
          if (!byIndex[idx]) byIndex[idx] = { id: c.id ?? "", type: "function", function: { name: c.function?.name ?? "", arguments: "" } }
          if (c.function?.arguments) byIndex[idx].function.arguments += c.function.arguments
        }
        const merged: unknown[] = Object.values(byIndex)
        for (const tc of toToolCallParts(merged)) {
          yield { ...tc, metadata: {} }
        }
        if (textId) yield { type: "text-end", id: textId, metadata: {} }
        yield { type: "finish", reason: mapFinishReason(finishReasonRaw), usage, metadata: {} }
      }
      return Stream.fromAsyncIterable(iter(), onError)
    },
  }).pipe(
    Effect.catchAll((error) => {
      if (AiError.isAiError(error)) return Effect.fail(error)
      return Effect.fail(
        new AiError.UnknownError({
          module: "OpenRouter",
          method: "makeLanguageModel",
          description: "Failed to create OpenRouter LanguageModel",
          cause: error,
        }),
      )
    }),
  )
}
