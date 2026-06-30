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
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopChatTurnResponse,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopToolCatalogResponse,
} from "../shared/rpc.js"
import { createPlaywrightKhalaBrowserService } from "./khala-browser-service.js"
import { createDuckDuckGoKhalaWebSearchService } from "./khala-web-search-service.js"

const KHALA_CODE_SYSTEM_PROMPT =
  "You are Khala Code Desktop. Answer the user directly and use the provided local tools whenever they help. " +
  "All tools are enabled by default in this owner-local desktop session. Never claim a tool ran unless the host returned a tool result."

const MAX_TOOL_ROUNDS = 8
const MAX_TOTAL_TOOL_CALLS = 32
const DEFAULT_HOSTED_TOKEN_MESSAGE =
  "Khala Code is wired to the hosted OpenAgents cloud by default, but this desktop process does not have an OPENAGENTS_AGENT_TOKEN. Set OPENAGENTS_AGENT_TOKEN for hosted cloud, or set OPENROUTER_API_KEY to use your own OpenRouter key."

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

type ChatTransport = {
  readonly backend: KhalaBackendSelection
  readonly request: (
    messages: readonly ChatTransportMessage[],
    tools: readonly ReturnType<typeof toOpenAiCompatibleTools>[number][],
  ) => Promise<ChatCompletionBody>
}

export type RunKhalaCodeDesktopChatTurnInput = {
  readonly env: ChatEnv
  readonly fetchFn?: typeof fetch
  readonly request: KhalaCodeDesktopChatTurnRequest
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
  let totalToolCalls = 0
  let retriedWithoutTools = false

  for (let round = 0; round < MAX_TOOL_ROUNDS; round += 1) {
    let completion: ChatCompletionBody
    try {
      completion = await transport.request(messages, tools)
    } catch (error) {
      if (
        !retriedWithoutTools &&
        totalToolCalls === 0 &&
        shouldRetryWithoutTools(error)
      ) {
        retriedWithoutTools = true
        try {
          completion = await transport.request(messages, [])
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
      transcript.push(hostMessage("assistant", text))
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
      const result = await runToolCall({
        call,
        registry,
        services,
        sessionId: input.request.sessionId,
      })
      usedTools.push(call.function.name)
      transcript.push(toolTranscriptMessage(call.function.name, result))
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
      request: (messages, tools) => postOpenAiCompatible({
        body: {
          max_tokens: 4096,
          messages,
          model: input.backend.model,
          stream: false,
          ...toolRequestFields(tools),
        },
        fetchFn: input.fetchFn,
        headers: { authorization: `Bearer ${token}` },
        url: `${(input.env.OPENROUTER_BASE_URL?.trim() || "https://openrouter.ai/api/v1").replace(/\/+$/, "")}/chat/completions`,
      }),
    }
  }

  const hostedToken = input.env.OPENAGENTS_AGENT_TOKEN?.trim() || input.env.OPENAGENTS_API_KEY?.trim()
  if (hostedToken === undefined || hostedToken.length === 0) return null
  return {
    backend: input.backend,
    request: (messages, tools) => postOpenAiCompatible({
      body: {
        max_tokens: 4096,
        messages,
        model: input.backend.model,
        stream: false,
        ...toolRequestFields(tools),
      },
      fetchFn: input.fetchFn,
      headers: { authorization: `Bearer ${hostedToken}` },
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
  readonly url: string
}): Promise<ChatCompletionBody> {
  const response = await input.fetchFn(input.url, {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
      ...input.headers,
    },
    body: JSON.stringify(input.body),
  })
  const body = await readJson(response)
  if (!response.ok) {
    throw new ChatProviderRequestError(errorText(body, response.status), response.status)
  }
  return isRecord(body) ? body as ChatCompletionBody : {}
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch {
    return null
  }
}

function errorText(body: unknown, status: number): string {
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

function toolTranscriptMessage(toolName: string, result: KhalaToolResult): KhalaCodeDesktopMessage {
  return hostMessage("tool", `${toolName}: ${result.status}\n${result.modelOutput.text}`)
}

function hostMessage(
  role: KhalaCodeDesktopMessage["role"],
  body: string,
): KhalaCodeDesktopMessage {
  return {
    body,
    id: `${role}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
    role,
  }
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
