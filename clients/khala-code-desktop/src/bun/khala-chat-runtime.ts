import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  applyPatchToolDefinition,
  createApplyPatchTool,
  createAskUserTool,
  createBrowserTools,
  createEditTool,
  createExecCommandTool,
  createGlobTool,
  createGrepTool,
  createLsTool,
  createReadTool,
  createTodoWriteTool,
  createViewImageTool,
  createWebFetchTool,
  createWebSearchTool,
  createWriteStdinTool,
  createWriteTool,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  redactKhalaPublicText,
  resolveKhalaBackend,
  toOpenAiCompatibleTools,
  type KhalaBackendSelection,
  type KhalaToolDefinition,
  type KhalaToolRegistry,
  type KhalaToolResult,
  type KhalaToolServices,
  type RegisteredKhalaTool,
} from "@openagentsinc/khala-tools"
import {
  type KhalaCodeDesktopBackendProjection,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopChatTurnResponse,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopToolCatalogResponse,
} from "../shared/rpc.js"
import { createPlaywrightKhalaBrowserService } from "./khala-browser-service.js"
import { createDuckDuckGoKhalaWebSearchService } from "./khala-web-search-service.js"

const KHALA_CODE_SYSTEM_PROMPT = [
  "You are Khala Code Desktop, the owner-local coding surface for Khala by OpenAgents.",
  "Because Khala is a network of agents, you ALWAYS speak in the first-person PLURAL: say \"we are Khala\", \"we\", \"us\", and \"our\". Never say \"I am\", \"I\", \"me\", or \"my\".",
  "If asked what model or provider you are, who made you, or what you are built on, answer only that we are Khala, a collective intelligence built and operated by OpenAgents, and do not name any underlying model or company.",
  "For a simple greeting or intro, use exactly: \"We are Khala. How can we help?\"",
  "Answer the user directly and use the provided local tools whenever they help.",
  "All tools are enabled by default in this owner-local desktop session. Never claim a tool ran unless the host returned a tool result.",
].join(" ")

const MAX_TOOL_ROUNDS = 8
const MAX_TOTAL_TOOL_CALLS = 32
const DEFAULT_HOSTED_TOKEN_MESSAGE =
  "Khala Code is wired to the hosted OpenAgents cloud by default, but this desktop process does not have an OPENAGENTS_AGENT_TOKEN. Set OPENAGENTS_AGENT_TOKEN for hosted cloud, or set OPENROUTER_API_KEY to use your own OpenRouter key."
const OPENROUTER_APP_ATTRIBUTION_HEADERS = {
  "HTTP-Referer": "https://openagents.com/khala",
  "X-OpenRouter-Categories": "cli-agent,cloud-agent,personal-agent,programming-app",
  "X-OpenRouter-Title": "Khala Code",
} as const

type ChatEnv = Readonly<Record<string, string | undefined>>

type ChatTransportMessage = {
  content: string | null
  name?: string
  role: "assistant" | "system" | "tool" | "user"
  tool_call_id?: string
  tool_calls?: readonly OpenAiToolCall[]
}

type OpenAiToolCall = {
  readonly function: {
    readonly arguments: string
    readonly name: string
  }
  readonly id: string
  readonly type: "function"
}

type ChatCompletionBody = {
  readonly choices?: readonly {
    readonly finish_reason?: unknown
    readonly message?: {
      readonly content?: unknown
      readonly tool_calls?: unknown
    }
  }[]
}

type ChatTransportCallbacks = {
  readonly onAssistantDelta?: (delta: string) => void
}

type ChatTransport = {
  readonly backend: KhalaBackendSelection
  readonly request: (
    messages: readonly ChatTransportMessage[],
    tools: readonly ReturnType<typeof toOpenAiCompatibleTools>[number][],
    callbacks?: ChatTransportCallbacks,
  ) => Promise<ChatCompletionBody>
}

export type RunKhalaCodeDesktopChatTurnInput = {
  readonly env: ChatEnv
  readonly fetchFn?: typeof fetch
  readonly request: KhalaCodeDesktopChatTurnRequest
  readonly onEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly services?: KhalaToolServices
  readonly registry?: KhalaToolRegistry
  readonly workingDirectory?: string
}

export function createKhalaCodeDesktopToolRegistry(): KhalaToolRegistry {
  return makeKhalaToolRegistry(createKhalaCodeDesktopTools())
}

export function createKhalaCodeDesktopTools(): ReadonlyArray<RegisteredKhalaTool> {
  return [
    createReadTool(),
    createLsTool(),
    createGlobTool(),
    createGrepTool(),
    createEditTool(),
    createWriteTool(),
    createApplyPatchTool(),
    createExecCommandTool(),
    createWriteStdinTool(),
    createAskUserTool(),
    createTodoWriteTool(),
    createViewImageTool(),
    createWebFetchTool(),
    createWebSearchTool(),
    ...createBrowserTools(),
  ]
}

export function khalaCodeDesktopToolCatalog(): KhalaCodeDesktopToolCatalogResponse {
  const tools = createKhalaCodeDesktopToolRegistry().list()
  return {
    defaultEnabled: true,
    toolCount: tools.length,
    tools: tools.map(tool => ({
      authority: tool.authority,
      name: tool.name,
    })),
  }
}

export async function runKhalaCodeDesktopChatTurn(
  input: RunKhalaCodeDesktopChatTurnInput,
): Promise<KhalaCodeDesktopChatTurnResponse> {
  const registry = input.registry ?? createKhalaCodeDesktopToolRegistry()
  const toolDefinitions = registry.list()
  const backend = resolveKhalaBackend({ env: input.env })
  const services = input.services ?? createDefaultToolServices({
    env: input.env,
    ...(input.fetchFn === undefined ? {} : { fetchFn: input.fetchFn }),
    ...(input.workingDirectory === undefined ? {} : { workingDirectory: input.workingDirectory }),
  })
  const toolNames = toolDefinitions.map(tool => tool.name)
  const transport = createChatTransport({
    backend,
    env: input.env,
    fetchFn: input.fetchFn ?? fetch,
  })
  if (transport === null) {
    return {
      backend: projectBackend(backend),
      messages: [hostMessage("assistant", DEFAULT_HOSTED_TOKEN_MESSAGE)],
      ok: false,
      toolNames,
      usedTools: [],
    }
  }

  const messages: ChatTransportMessage[] = [
    { role: "system", content: KHALA_CODE_SYSTEM_PROMPT },
    ...projectTranscriptMessages(input.request.messages),
  ]
  const transcript: KhalaCodeDesktopMessage[] = []
  const usedTools: string[] = []
  const tools = toOpenAiCompatibleTools(toolDefinitions)
  const turnId = input.request.turnId ?? nextMessageId("turn")
  const emit = (event: KhalaCodeDesktopChatTurnEvent): void => {
    input.onEvent?.(event)
  }
  let totalToolCalls = 0
  let retriedWithoutTools = false

  for (let round = 0; round < MAX_TOOL_ROUNDS; round += 1) {
    let completion: ChatCompletionBody
    const streamingAssistant: { current: KhalaCodeDesktopMessage | null } = { current: null }
    const onAssistantDelta = (delta: string): void => {
      if (delta.length === 0) return
      if (streamingAssistant.current === null) {
        streamingAssistant.current = hostMessage("assistant", "")
        emit({ message: streamingAssistant.current, turnId, type: "message_start" })
      }
      streamingAssistant.current = {
        ...streamingAssistant.current,
        body: `${streamingAssistant.current.body}${delta}`,
      }
      emit({
        delta,
        messageId: streamingAssistant.current.id,
        turnId,
        type: "message_delta",
      })
    }

    try {
      completion = await transport.request(messages, tools, { onAssistantDelta })
    } catch (error) {
      if (
        !retriedWithoutTools &&
        totalToolCalls === 0 &&
        streamingAssistant.current === null &&
        shouldRetryWithoutTools(error)
      ) {
        retriedWithoutTools = true
        try {
          completion = await transport.request(messages, [], { onAssistantDelta })
        } catch (retryError) {
          return providerFailureResult(backend, toolNames, usedTools, retryError)
        }
      } else {
        return providerFailureResult(backend, toolNames, usedTools, error)
      }
    }
    const assistant = firstAssistantMessage(completion)
    const text = textContent(assistant.content)
    const toolCalls = parseToolCalls(assistant.tool_calls)

    if (text.length > 0) {
      const streamed = streamingAssistant.current
      const message = streamed === null
        ? hostMessage("assistant", text)
        : { ...streamed, body: text }
      transcript.push(message)
      if (streamed === null) {
        emit({ message, turnId, type: "message_start" })
      } else if (streamed.body !== text) {
        emit({ message, turnId, type: "message_replace" })
      }
      emit({ messageId: message.id, turnId, type: "message_done" })
      messages.push({
        content: text,
        role: "assistant",
        ...(toolCalls.length === 0 ? {} : { tool_calls: toolCalls }),
      })
    } else if (toolCalls.length > 0) {
      messages.push({ content: "", role: "assistant", tool_calls: toolCalls })
    }

    if (toolCalls.length === 0) {
      return {
        backend: projectBackend(backend),
        messages: transcript.length === 0
          ? [hostMessage("assistant", "Khala returned an empty response. Please try again.")]
          : transcript,
        ok: transcript.length > 0,
        toolNames,
        usedTools,
      }
    }

    for (const call of toolCalls) {
      totalToolCalls += 1
      if (totalToolCalls > MAX_TOTAL_TOOL_CALLS) {
        return limitResult(backend, toolNames, usedTools, "Khala requested too many tool calls in one turn.")
      }
      const toolTranscript = hostMessage("tool", toolTranscriptRunningBody(call.function.name))
      emit({ message: toolTranscript, turnId, type: "message_start" })
      const result = await runToolCall({
        call,
        registry,
        services,
        sessionId: input.request.sessionId,
      })
      usedTools.push(call.function.name)
      const completedToolTranscript = toolTranscriptMessage(
        call.function.name,
        result,
        toolTranscript.id,
      )
      transcript.push(completedToolTranscript)
      emit({ message: completedToolTranscript, turnId, type: "message_replace" })
      emit({ messageId: completedToolTranscript.id, turnId, type: "message_done" })
      messages.push({
        content: toolMessageContent(result),
        name: call.function.name,
        role: "tool",
        tool_call_id: call.id,
      })
    }
  }

  return limitResult(backend, toolNames, usedTools, "Khala used the maximum tool rounds without finishing the answer.")
}

function createDefaultToolServices(input: {
  readonly env: ChatEnv
  readonly fetchFn?: typeof fetch
  readonly workingDirectory?: string
}): KhalaToolServices {
  return makeKhalaToolServices({
    browser: createPlaywrightKhalaBrowserService(),
    permission: allowAllKhalaPermissionService,
    search: createDuckDuckGoKhalaWebSearchService(input.fetchFn ?? fetch),
    workingDirectory: input.workingDirectory ?? input.env.KHALA_CODE_DESKTOP_WORKSPACE ?? process.cwd(),
  })
}

function createChatTransport(input: {
  readonly backend: KhalaBackendSelection
  readonly env: ChatEnv
  readonly fetchFn: typeof fetch
}): ChatTransport | null {
  if (input.backend.kind === "mock") {
    return {
      backend: input.backend,
      request: async () => ({
        choices: [{
          message: { content: "Mock Khala Code is ready with the full local tool catalog enabled." },
        }],
      }),
    }
  }

  if (input.backend.kind === "openrouter_byok") {
    const token = input.env.OPENROUTER_API_KEY?.trim()
    if (token === undefined || token.length === 0) return null
    return {
      backend: input.backend,
      request: (messages, tools, callbacks) => postOpenAiCompatible({
        body: {
          max_tokens: 4096,
          messages,
          model: input.backend.model,
          stream: true,
          stream_options: { include_usage: true },
          ...toolRequestFields(tools),
        },
        fetchFn: input.fetchFn,
        headers: {
          authorization: `Bearer ${token}`,
          ...OPENROUTER_APP_ATTRIBUTION_HEADERS,
        },
        ...(callbacks?.onAssistantDelta === undefined ? {} : { onAssistantDelta: callbacks.onAssistantDelta }),
        url: `${(input.env.OPENROUTER_BASE_URL?.trim() || "https://openrouter.ai/api/v1").replace(/\/+$/, "")}/chat/completions`,
      }),
    }
  }

  const hostedToken = input.env.OPENAGENTS_AGENT_TOKEN?.trim() || input.env.OPENAGENTS_API_KEY?.trim()
  if (hostedToken === undefined || hostedToken.length === 0) return null
  return {
    backend: input.backend,
    request: (messages, tools, callbacks) => postOpenAiCompatible({
      body: {
        max_tokens: 4096,
        messages,
        model: input.backend.model,
        stream: true,
        stream_options: { include_usage: true },
        ...toolRequestFields(tools),
      },
      fetchFn: input.fetchFn,
      headers: { authorization: `Bearer ${hostedToken}` },
      ...(callbacks?.onAssistantDelta === undefined ? {} : { onAssistantDelta: callbacks.onAssistantDelta }),
      url: `${(input.backend.baseUrl ?? "https://openagents.com").replace(/\/+$/, "")}/api/v1/chat/completions`,
    }),
  }
}

function toolRequestFields(
  tools: readonly ReturnType<typeof toOpenAiCompatibleTools>[number][],
): Record<string, unknown> {
  return tools.length === 0
    ? {}
    : {
      tool_choice: "auto",
      tools,
    }
}

class ChatProviderRequestError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "ChatProviderRequestError"
    this.status = status
  }
}

async function postOpenAiCompatible(input: {
  readonly body: Record<string, unknown>
  readonly fetchFn: typeof fetch
  readonly headers: Readonly<Record<string, string>>
  readonly onAssistantDelta?: (delta: string) => void
  readonly url: string
}): Promise<ChatCompletionBody> {
  const response = await input.fetchFn(input.url, {
    method: "POST",
    headers: {
      accept: "text/event-stream, application/json",
      "content-type": "application/json",
      ...input.headers,
    },
    body: JSON.stringify(input.body),
  })
  if (!response.ok) {
    const body = await readJsonOrText(response)
    throw new ChatProviderRequestError(errorText(body, response.status), response.status)
  }
  if (isEventStreamResponse(response)) {
    return readOpenAiCompatibleStream(response, input.onAssistantDelta)
  }
  const body = await readJson(response)
  return isRecord(body) ? body as ChatCompletionBody : {}
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch {
    return null
  }
}

async function readJsonOrText(response: Response): Promise<unknown> {
  const text = await response.text()
  if (text.trim().length === 0) return null
  try {
    return JSON.parse(text) as unknown
  } catch {
    return text
  }
}

function isEventStreamResponse(response: Response): boolean {
  return response.headers.get("content-type")?.toLowerCase().includes("text/event-stream") === true
}

type SseFrame = {
  readonly data: string
  readonly event?: string
}

async function readOpenAiCompatibleStream(
  response: Response,
  onAssistantDelta: ((delta: string) => void) | undefined,
): Promise<ChatCompletionBody> {
  if (response.body === null) return {}
  const state: OpenAiStreamState = {
    content: "",
    finishReason: undefined,
    toolCalls: new Map(),
  }
  for await (const frame of readSseFrames(response.body)) {
    if (frame.data.trim() === "[DONE]") break
    const payload = parseStreamJson(frame.data)
    applyOpenAiStreamPayload(payload, state, onAssistantDelta)
  }
  const toolCalls = [...state.toolCalls.entries()]
    .sort(([a], [b]) => a - b)
    .flatMap(([, call]) => {
      const name = call.name.trim()
      if (name.length === 0) return []
      return [{
        function: {
          arguments: call.arguments,
          name,
        },
        id: call.id.length > 0 ? call.id : `call_${call.index + 1}`,
        type: "function" as const,
      }]
    })

  return {
    choices: [{
      ...(state.finishReason === undefined ? {} : { finish_reason: state.finishReason }),
      message: {
        content: state.content,
        ...(toolCalls.length === 0 ? {} : { tool_calls: toolCalls }),
      },
    }],
  }
}

type OpenAiStreamToolCallState = {
  arguments: string
  id: string
  index: number
  name: string
}

type OpenAiStreamState = {
  content: string
  finishReason: unknown
  toolCalls: Map<number, OpenAiStreamToolCallState>
}

async function* readSseFrames(body: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame> {
  const reader = body.getReader()
  const decoder = new TextDecoder()
  let buffer = ""

  try {
    while (true) {
      const chunk = await reader.read()
      if (chunk.done) break
      buffer = `${buffer}${decoder.decode(chunk.value, { stream: true })}`
        .replace(/\r\n/g, "\n")
        .replace(/\r/g, "\n")
      let boundary = buffer.indexOf("\n\n")
      while (boundary >= 0) {
        const block = buffer.slice(0, boundary)
        buffer = buffer.slice(boundary + 2)
        const frame = parseSseBlock(block)
        if (frame !== null) yield frame
        boundary = buffer.indexOf("\n\n")
      }
    }
    buffer = `${buffer}${decoder.decode()}`
    const frame = parseSseBlock(buffer)
    if (frame !== null) yield frame
  } finally {
    reader.releaseLock()
  }
}

function parseSseBlock(block: string): SseFrame | null {
  const lines = block.split("\n")
  let event: string | undefined
  const data: string[] = []

  for (const line of lines) {
    if (line.length === 0 || line.startsWith(":")) continue
    const colonIndex = line.indexOf(":")
    const field = colonIndex >= 0 ? line.slice(0, colonIndex) : line
    const rawValue = colonIndex >= 0 ? line.slice(colonIndex + 1) : ""
    const value = rawValue.startsWith(" ") ? rawValue.slice(1) : rawValue
    if (field === "event") event = value
    if (field === "data") data.push(value)
  }

  if (event === undefined && data.length === 0) return null
  return event === undefined ? { data: data.join("\n") } : { event, data: data.join("\n") }
}

function parseStreamJson(data: string): unknown {
  try {
    return JSON.parse(data) as unknown
  } catch (error) {
    throw new Error(`Malformed OpenAI-compatible stream frame: ${
      error instanceof Error ? error.message : String(error)
    }`)
  }
}

function applyOpenAiStreamPayload(
  payload: unknown,
  state: OpenAiStreamState,
  onAssistantDelta: ((delta: string) => void) | undefined,
): void {
  if (!isRecord(payload) || !Array.isArray(payload.choices)) return
  for (const choice of payload.choices) {
    if (!isRecord(choice)) continue
    if (choice.finish_reason !== undefined && choice.finish_reason !== null) {
      state.finishReason = choice.finish_reason
    }
    const delta = isRecord(choice.delta) ? choice.delta : undefined
    if (delta !== undefined) {
      const content = streamTextDelta(delta.content)
      if (content.length > 0) {
        state.content = `${state.content}${content}`
        onAssistantDelta?.(content)
      }
      collectToolCallDeltas(delta.tool_calls, state.toolCalls)
    }
    const message = isRecord(choice.message) ? choice.message : undefined
    if (message !== undefined) {
      const content = streamTextDelta(message.content)
      if (content.length > 0) {
        state.content = `${state.content}${content}`
        onAssistantDelta?.(content)
      }
      collectToolCallDeltas(message.tool_calls, state.toolCalls)
    }
  }
}

function streamTextDelta(value: unknown): string {
  if (typeof value === "string") return value
  if (!Array.isArray(value)) return ""
  return value
    .map(part => isRecord(part) && typeof part.text === "string" ? part.text : "")
    .join("")
}

function collectToolCallDeltas(value: unknown, toolCalls: Map<number, OpenAiStreamToolCallState>): void {
  if (!Array.isArray(value)) return
  value.forEach((item, position) => {
    if (!isRecord(item)) return
    const index = typeof item.index === "number" && Number.isInteger(item.index)
      ? item.index
      : position
    const current = toolCalls.get(index) ?? {
      arguments: "",
      id: "",
      index,
      name: "",
    }
    const fn = isRecord(item.function) ? item.function : undefined
    toolCalls.set(index, {
      arguments: `${current.arguments}${typeof fn?.arguments === "string" ? fn.arguments : ""}`,
      id: typeof item.id === "string" && item.id.length > 0 ? item.id : current.id,
      index,
      name: typeof fn?.name === "string" && fn.name.length > 0 ? fn.name : current.name,
    })
  })
}

function errorText(body: unknown, status: number): string {
  if (typeof body === "string" && body.trim().length > 0) return body.trim()
  if (isRecord(body)) {
    const message = body.message ?? body.error
    const reason = body.reason
    if (
      typeof body.error === "string" &&
      body.error.trim().length > 0 &&
      typeof reason === "string" &&
      reason.trim().length > 0
    ) {
      return `${body.error.trim()}: ${reason.trim()}`
    }
    if (typeof message === "string" && message.trim().length > 0) return message.trim()
  }
  return `chat completion failed with ${status}`
}

function shouldRetryWithoutTools(error: unknown): boolean {
  const status = error instanceof ChatProviderRequestError ? error.status : undefined
  if (status === 401 || status === 402 || status === 403) return false
  const message = providerErrorText(error).toLowerCase()
  return (
    (status === 400 && message.includes("tool")) ||
    status === 408 ||
    status === 409 ||
    status === 429 ||
    status === undefined ||
    status >= 500 ||
    message.includes("provider_error")
  )
}

function providerErrorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function providerFailureResult(
  backend: KhalaBackendSelection,
  toolNames: readonly string[],
  usedTools: readonly string[],
  error: unknown,
): KhalaCodeDesktopChatTurnResponse {
  return {
    backend: projectBackend(backend),
    messages: [hostMessage("system", providerFailureMessage(backend, error))],
    ok: false,
    toolNames,
    usedTools,
  }
}

function providerFailureMessage(backend: KhalaBackendSelection, error: unknown): string {
  const detail = redactKhalaPublicText(providerErrorText(error)).trim()
  const suffix = detail.length === 0 ? "." : `: ${detail}.`
  if (backend.kind === "openrouter_byok") {
    return `OpenRouter request failed for ${backend.model}${suffix} Check OPENROUTER_API_KEY and OPENROUTER_MODEL.`
  }
  if (backend.kind === "hosted_openagents") {
    return `Hosted OpenAgents cloud request failed for ${backend.model}${suffix} Set OPENROUTER_API_KEY to use your own OpenRouter key while the hosted lane is unavailable.`
  }
  return `Khala provider request failed for ${backend.model}${suffix}`
}

function projectTranscriptMessages(messages: readonly KhalaCodeDesktopMessage[]): readonly ChatTransportMessage[] {
  return messages.flatMap(message => {
    if (message.role !== "assistant" && message.role !== "user") return []
    const content = message.body.trim()
    if (content.length === 0) return []
    return [{ role: message.role, content }]
  })
}

function firstAssistantMessage(body: ChatCompletionBody): {
  readonly content?: unknown
  readonly tool_calls?: unknown
} {
  return body.choices?.[0]?.message ?? {}
}

function textContent(value: unknown): string {
  if (typeof value === "string") return value.trim()
  if (!Array.isArray(value)) return ""
  return value
    .map(part => isRecord(part) && typeof part.text === "string" ? part.text : "")
    .join("")
    .trim()
}

function parseToolCalls(value: unknown): readonly OpenAiToolCall[] {
  if (!Array.isArray(value)) return []
  const calls: OpenAiToolCall[] = []
  for (const item of value) {
    if (!isRecord(item) || item.type !== "function" || !isRecord(item.function)) continue
    const id = typeof item.id === "string" && item.id.length > 0
      ? item.id
      : `call_${calls.length + 1}`
    const name = typeof item.function.name === "string" ? item.function.name : ""
    const args = typeof item.function.arguments === "string" ? item.function.arguments : "{}"
    if (name.length === 0) continue
    calls.push({
      function: {
        arguments: args,
        name,
      },
      id,
      type: "function",
    })
  }
  return calls
}

async function runToolCall(input: {
  readonly call: OpenAiToolCall
  readonly registry: KhalaToolRegistry
  readonly services: KhalaToolServices
  readonly sessionId: string
}): Promise<KhalaToolResult> {
  const args = parseToolArguments(input.call.function.arguments)
  return await Effect.runPromise(
    executeKhalaTool(
      input.registry,
      {
        arguments: args,
        id: input.call.id,
        name: input.call.function.name,
        sessionId: input.sessionId,
      },
      input.services,
    ),
  )
}

function parseToolArguments(value: string): Readonly<Record<string, unknown>> {
  try {
    const parsed = JSON.parse(value) as unknown
    return isRecord(parsed) ? parsed : {}
  } catch {
    return {}
  }
}

function toolMessageContent(result: KhalaToolResult): string {
  return JSON.stringify({
    artifacts: result.artifacts,
    modelOutput: result.modelOutput.text,
    privateDataRefs: result.privateDataRefs,
    publicSummary: result.publicSummary,
    status: result.status,
    ui: result.ui,
  })
}

function toolTranscriptRunningBody(toolName: string): string {
  return `${toolName}: running`
}

function toolTranscriptBody(toolName: string, result: KhalaToolResult): string {
  const output = result.modelOutput.text.trimEnd()
  return output.length === 0
    ? `${toolName}: ${result.status}`
    : `${toolName}: ${result.status}\n\n${output}`
}

function toolTranscriptMessage(
  toolName: string,
  result: KhalaToolResult,
  id = nextMessageId("tool"),
): KhalaCodeDesktopMessage {
  return {
    body: toolTranscriptBody(toolName, result),
    id,
    role: "tool",
  }
}

function hostMessage(
  role: KhalaCodeDesktopMessage["role"],
  body: string,
): KhalaCodeDesktopMessage {
  return {
    body,
    id: nextMessageId(role),
    role,
  }
}

function nextMessageId(role: string): string {
  return `${role}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

function limitResult(
  backend: KhalaBackendSelection,
  toolNames: readonly string[],
  usedTools: readonly string[],
  body: string,
): KhalaCodeDesktopChatTurnResponse {
  return {
    backend: projectBackend(backend),
    messages: [hostMessage("assistant", body)],
    ok: false,
    toolNames,
    usedTools,
  }
}

function projectBackend(backend: KhalaBackendSelection): KhalaCodeDesktopBackendProjection {
  return {
    kind: backend.kind,
    model: backend.model,
    ...(backend.baseUrl === undefined ? {} : { baseUrl: backend.baseUrl }),
    ...(backend.credentialSource === undefined ? {} : { credentialSource: backend.credentialSource }),
    ...(backend.provider === undefined ? {} : { provider: backend.provider }),
  }
}

export function expectedKhalaCodeDesktopToolNames(): readonly string[] {
  return [
    "read",
    "ls",
    "glob",
    "grep",
    "edit",
    "write",
    applyPatchToolDefinition.name,
    "exec_command",
    "write_stdin",
    "ask_user",
    "todo_write",
    "view_image",
    "web_fetch",
    "web_search",
    "browser_navigate",
    "browser_click",
    "browser_type",
    "browser_read_text",
    "browser_read_dom",
    "browser_wait_for",
    "browser_screenshot",
  ]
}

export function assertAllDefaultToolsRegistered(definitions: readonly KhalaToolDefinition[]): boolean {
  const actual = definitions.map(tool => tool.name)
  const expected = expectedKhalaCodeDesktopToolNames()
  return expected.every(name => actual.includes(name)) && actual.length === expected.length
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
