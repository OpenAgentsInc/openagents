import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  applyPatchToolDefinition,
  createApplyPatchTool,
  createEditTool,
  createExecCommandTool,
  createGlobTool,
  createGrepTool,
  createLsTool,
  createReadTool,
  createWriteTool,
  executeKhalaTool,
  makeKhalaPrivacyRedactionService,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  redactKhalaPublicText,
  resolveKhalaBackend,
  toOpenAiCompatibleTools,
  type KhalaBackendSelection,
  type KhalaPrivacyRedactionServiceShape,
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
import { createKhalaCodexFleetTools } from "./khala-codex-fleet-tools.js"
import { createPlaywrightKhalaBrowserService } from "./khala-browser-service.js"
import { createDuckDuckGoKhalaWebSearchService } from "./khala-web-search-service.js"

const KHALA_CODE_SYSTEM_PROMPT = [
  "You are Khala Code Desktop, the owner-local coding surface for Khala by OpenAgents.",
  "Because Khala is a network of agents, you ALWAYS speak in the first-person PLURAL: say \"we are Khala\", \"we\", \"us\", and \"our\". Never say \"I am\", \"I\", \"me\", or \"my\".",
  "If asked what model or provider you are, who made you, or what you are built on, answer exactly that we are Khala by OpenAgents. Do not add evasive boilerplate like \"we do not disclose\" and do not name any backing model, vendor, or provider.",
  "For a simple greeting or intro, use exactly: \"We are Khala. How can we help?\"",
  "Answer the user directly and use the provided local tools whenever they help.",
  "All tools are enabled by default in this owner-local desktop session. Never claim a tool ran unless the host returned a tool result.",
  "When the user asks what tools or capabilities are available, answer from the available-tool catalog in this system context. Do not call a filesystem or shell tool just to describe the tool catalog.",
  "When the user asks to spin up, launch, monitor, or manage Codex instances, use the Pylon/Codex fleet tools instead of ad hoc shell commands.",
  "For local files, do not infer behavior from filenames alone. If you only listed a directory, answer only with exact listed names until you read the relevant files.",
  "If a tool says output was truncated, continue inspecting with a narrower path, larger limit, offset, or another appropriate tool before giving a final answer.",
  "When answering from read results, preserve exact paths, line facts, and code literals from the tool output. Do not rewrite code from memory.",
  "After using tools, always produce a visible final answer that explains what the tool results mean for the user. Never end a turn with only tool output.",
].join(" ")

const MAX_TOOL_ROUNDS = 8
const MAX_TOTAL_TOOL_CALLS = 32
const MAX_LOCAL_GROUNDING_CORRECTIONS = 3
const DEFAULT_HOSTED_TOKEN_MESSAGE =
  "Khala Code routes model traffic through hosted Khala, but this desktop process does not have an OPENAGENTS_AGENT_TOKEN. Set OPENAGENTS_AGENT_TOKEN for hosted Khala; OPENROUTER_API_KEY alone cannot run the Khala system locally."

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

type ChatTurnEmitter = (event: KhalaCodeDesktopChatTurnEvent) => void

type LocalFileEvidenceState = {
  readonly pendingTruncatedLs: boolean
  readonly toolNames: readonly string[]
}

type LocalGroundingCorrection = {
  readonly prompt: string
  readonly visibleReason: string
}

export type RunKhalaCodeDesktopChatTurnInput = {
  readonly env: ChatEnv
  readonly fetchFn?: typeof fetch
  readonly request: KhalaCodeDesktopChatTurnRequest
  readonly onEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly redaction?: KhalaPrivacyRedactionServiceShape
  readonly services?: KhalaToolServices
  readonly registry?: KhalaToolRegistry
  readonly workingDirectory?: string
}

const redactionBySession = new Map<string, KhalaPrivacyRedactionServiceShape>()

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
    ...createKhalaCodexFleetTools(),
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

  const redaction = input.redaction ?? redactionForSession(input.request.sessionId)
  const messages: ChatTransportMessage[] = [
    { role: "system", content: KHALA_CODE_SYSTEM_PROMPT },
    { role: "system", content: toolCatalogSystemPrompt(toolDefinitions) },
    ...(await projectTranscriptMessages(input.request.messages, redaction)),
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
  let assistantMessagesSinceLastToolResult = 0
  let localFileEvidence = emptyLocalFileEvidenceState()
  let localGroundingCorrections = 0

  const appendAssistantText = async (
    text: string,
    streamed: KhalaCodeDesktopMessage | null,
    toolCalls: readonly OpenAiToolCall[] = [],
  ): Promise<void> => {
    const modelText = await protectModelText(text, redaction)
    const visibleText = await revealLocalText(modelText, redaction)
    const message = streamed === null
      ? hostMessage("assistant", visibleText)
      : { ...streamed, body: visibleText }
    transcript.push(message)
    assistantMessagesSinceLastToolResult += 1
    if (streamed === null) {
      emit({ message, turnId, type: "message_start" })
    } else if (streamed.body !== visibleText) {
      emit({ message, turnId, type: "message_replace" })
    }
    emit({ messageId: message.id, turnId, type: "message_done" })
    messages.push({
      content: modelText,
      role: "assistant",
      ...(toolCalls.length === 0 ? {} : { tool_calls: toolCalls }),
    })
  }

  for (let round = 0; round < MAX_TOOL_ROUNDS; round += 1) {
    let completion: ChatCompletionBody
    const assistantStream = createAssistantStreamRecorder({ emit, turnId })

    try {
      completion = await transport.request(messages, tools, { onAssistantDelta: assistantStream.onAssistantDelta })
    } catch (error) {
      if (
        !retriedWithoutTools &&
        totalToolCalls === 0 &&
        assistantStream.streamingAssistant.current === null &&
        shouldRetryWithoutTools(error)
      ) {
        retriedWithoutTools = true
        try {
          completion = await transport.request(messages, [], { onAssistantDelta: assistantStream.onAssistantDelta })
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
    const vacuousToolAnswer = toolCalls.length === 0 && usedTools.length > 0 && isVacuousPostToolAnswer(text)
    const localGroundingCorrection = toolCalls.length === 0
      ? localGroundingCorrectionForFinalAnswer(text, localFileEvidence)
      : null

    if (
      localGroundingCorrection !== null &&
      localGroundingCorrections < MAX_LOCAL_GROUNDING_CORRECTIONS
    ) {
      localGroundingCorrections += 1
      replaceRejectedAssistantStream({
        correction: localGroundingCorrection,
        emit,
        stream: assistantStream.streamingAssistant.current,
        turnId,
      })
      messages.push({ content: localGroundingCorrection.prompt, role: "user" })
      assistantMessagesSinceLastToolResult = 0
      continue
    }

    if (localGroundingCorrection !== null) {
      return limitResult(
        backend,
        toolNames,
        usedTools,
        "Khala tried to answer from incomplete local file evidence. We stopped instead of guessing; ask us to inspect a narrower path or continue reading the relevant files.",
      )
    }

    if (text.length > 0 && !vacuousToolAnswer) {
      await appendAssistantText(text, assistantStream.streamingAssistant.current, toolCalls)
    } else if (toolCalls.length > 0) {
      messages.push({ content: "", role: "assistant", tool_calls: toolCalls })
    }

    if (toolCalls.length === 0) {
      if (usedTools.length > 0 && (assistantMessagesSinceLastToolResult === 0 || vacuousToolAnswer)) {
        const replaceMessage = vacuousToolAnswer ? assistantStream.streamingAssistant.current : null
        const visibleAnswer = await requestVisibleToolAnswer({
          emit,
          messages,
          stream: replaceMessage === null,
          transport,
          turnId,
        })
        const visibleText = usableToolAnswerText(visibleAnswer?.text ?? "")
        if (visibleText === null) {
          await appendAssistantText(toolOnlyTurnFallbackBody(transcript), replaceMessage)
        } else {
          await appendAssistantText(visibleText, replaceMessage ?? visibleAnswer?.streamingAssistant ?? null)
        }
      }
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
        content: await protectUserText(toolMessageContent(result), redaction),
        name: call.function.name,
        role: "tool",
        tool_call_id: call.id,
      })
      localFileEvidence = updateLocalFileEvidence(localFileEvidence, call.function.name, result)
      assistantMessagesSinceLastToolResult = 0
    }
  }

  return limitResult(backend, toolNames, usedTools, "Khala used the maximum tool rounds without finishing the answer.")
}

function redactionForSession(sessionId: string): KhalaPrivacyRedactionServiceShape {
  const existing = redactionBySession.get(sessionId)
  if (existing !== undefined) return existing
  const redaction = makeKhalaPrivacyRedactionService()
  redactionBySession.set(sessionId, redaction)
  return redaction
}

function emptyLocalFileEvidenceState(): LocalFileEvidenceState {
  return {
    pendingTruncatedLs: false,
    toolNames: [],
  }
}

function updateLocalFileEvidence(
  evidence: LocalFileEvidenceState,
  toolName: string,
  result: KhalaToolResult,
): LocalFileEvidenceState {
  if (!isLocalFileInspectionTool(toolName)) return evidence
  return {
    pendingTruncatedLs: toolName === "ls" && isTruncatedLsResult(result),
    toolNames: [...evidence.toolNames, toolName],
  }
}

function isLocalFileInspectionTool(toolName: string): boolean {
  return toolName === "ls" ||
    toolName === "read" ||
    toolName === "glob" ||
    toolName === "grep" ||
    toolName === "view_image"
}

function isTruncatedLsResult(result: KhalaToolResult): boolean {
  return result.modelOutput.text.includes("[ls truncated; refine path or increase limit]")
}

function localGroundingCorrectionForFinalAnswer(
  answerText: string,
  evidence: LocalFileEvidenceState,
): LocalGroundingCorrection | null {
  if (evidence.toolNames.length === 0) return null
  if (evidence.pendingTruncatedLs) {
    return {
      prompt: [
        "The last directory listing was truncated, so do not answer yet.",
        "Continue local inspection with tools: call ls again with a narrower path or a larger limit, or read/grep/glob the relevant files before giving a final answer.",
        "When you do answer, say exactly what you inspected and avoid guessing.",
      ].join(" "),
      visibleReason: "the previous directory listing was truncated",
    }
  }
  if (evidence.toolNames.every(toolName => toolName === "ls") && containsSpeculativeFileClaim(answerText)) {
    return {
      prompt: [
        "The only local-file evidence so far is directory names, and the draft answer speculated about what files do.",
        "Continue with tools: read the relevant files before describing behavior or purpose.",
        "If the user only asked for names, answer only with the exact listed names and do not use speculative language.",
      ].join(" "),
      visibleReason: "filenames alone are not enough to describe file behavior",
    }
  }
  return null
}

function containsSpeculativeFileClaim(text: string): boolean {
  return /\b(?:likely|probably|presumably|maybe|might|could|appears?|seems?|suggests?)\b/iu.test(text)
}

function replaceRejectedAssistantStream(input: {
  readonly correction: LocalGroundingCorrection
  readonly emit: ChatTurnEmitter
  readonly stream: KhalaCodeDesktopMessage | null
  readonly turnId: string
}): void {
  if (input.stream === null) return
  const message: KhalaCodeDesktopMessage = {
    ...input.stream,
    body: `Khala Code is continuing local inspection: ${input.correction.visibleReason}.`,
    role: "system",
  }
  input.emit({ message, turnId: input.turnId, type: "message_replace" })
  input.emit({ messageId: message.id, turnId: input.turnId, type: "message_done" })
}

function createAssistantStreamRecorder(input: {
  readonly emit: ChatTurnEmitter
  readonly turnId: string
}): {
  readonly onAssistantDelta: (delta: string) => void
  readonly streamingAssistant: { current: KhalaCodeDesktopMessage | null }
} {
  const streamingAssistant: { current: KhalaCodeDesktopMessage | null } = { current: null }
  return {
    onAssistantDelta: delta => {
      if (delta.length === 0) return
      if (streamingAssistant.current === null) {
        streamingAssistant.current = hostMessage("assistant", "")
        input.emit({ message: streamingAssistant.current, turnId: input.turnId, type: "message_start" })
      }
      streamingAssistant.current = {
        ...streamingAssistant.current,
        body: `${streamingAssistant.current.body}${delta}`,
      }
      input.emit({
        delta,
        messageId: streamingAssistant.current.id,
        turnId: input.turnId,
        type: "message_delta",
      })
    },
    streamingAssistant,
  }
}

async function requestVisibleToolAnswer(input: {
  readonly emit: ChatTurnEmitter
  readonly messages: ChatTransportMessage[]
  readonly stream?: boolean
  readonly transport: ChatTransport
  readonly turnId: string
}): Promise<{
  readonly streamingAssistant: KhalaCodeDesktopMessage | null
  readonly text: string
} | null> {
  const assistantStream = input.stream === false
    ? null
    : createAssistantStreamRecorder({
      emit: input.emit,
      turnId: input.turnId,
    })
  input.messages.push({
    content: "Use the tool results above to answer the user's request now. Do not call tools. Keep the answer concise and explicit.",
    role: "user",
  })

  let completion: ChatCompletionBody
  try {
    completion = await input.transport.request(input.messages, [], {
      ...(assistantStream === null ? {} : { onAssistantDelta: assistantStream.onAssistantDelta }),
    })
  } catch {
    const partialText = assistantStream?.streamingAssistant.current?.body.trim() ?? ""
    return partialText.length === 0
      ? null
      : {
        streamingAssistant: assistantStream?.streamingAssistant.current ?? null,
        text: partialText,
      }
  }

  const text = textContent(firstAssistantMessage(completion).content)
  const partialText = assistantStream?.streamingAssistant.current?.body.trim() ?? ""
  if (text.length === 0 && partialText.length === 0) return null
  return {
    streamingAssistant: assistantStream?.streamingAssistant.current ?? null,
    text: text.length === 0 ? partialText : text,
  }
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
      headers: {
        authorization: `Bearer ${hostedToken}`,
        ...hostedByokHeaders(input.env),
      },
      ...(callbacks?.onAssistantDelta === undefined ? {} : { onAssistantDelta: callbacks.onAssistantDelta }),
      url: `${(input.backend.baseUrl ?? "https://openagents.com").replace(/\/+$/, "")}/api/v1/chat/completions`,
    }),
  }
}

function hostedByokHeaders(env: ChatEnv): Record<string, string> {
  const openRouterKey = env.OPENROUTER_API_KEY?.trim()
  return openRouterKey === undefined || openRouterKey.length === 0
    ? {}
    : {
      "x-openagents-provider": "openrouter",
      "x-openagents-provider-key": openRouterKey,
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
  if (backend.kind === "hosted_openagents") {
    return `Hosted OpenAgents cloud request failed for ${backend.model}${suffix} Check OPENAGENTS_AGENT_TOKEN and hosted Khala availability.`
  }
  return `Khala provider request failed for ${backend.model}${suffix}`
}

async function projectTranscriptMessages(
  messages: readonly KhalaCodeDesktopMessage[],
  redaction: KhalaPrivacyRedactionServiceShape,
): Promise<readonly ChatTransportMessage[]> {
  const projected: ChatTransportMessage[] = []
  for (const message of messages) {
    const content = message.body.trim()
    if (content.length === 0) continue
    if (message.role === "tool") {
      projected.push({
        content: await protectUserText(`Previous tool result:\n${content}`, redaction),
        role: "assistant",
      })
      continue
    }
    if (message.role === "assistant") {
      projected.push({ role: "assistant", content: await protectModelText(content, redaction) })
      continue
    }
    if (message.role === "user") {
      projected.push({ role: "user", content: await protectUserText(content, redaction) })
    }
  }
  return projected
}

async function protectUserText(
  text: string,
  redaction: KhalaPrivacyRedactionServiceShape,
): Promise<string> {
  return (await Effect.runPromise(redaction.protectUserText(text))).text
}

async function protectModelText(
  text: string,
  redaction: KhalaPrivacyRedactionServiceShape,
): Promise<string> {
  return (await Effect.runPromise(redaction.protectModelText(text))).text
}

async function revealLocalText(
  text: string,
  redaction: KhalaPrivacyRedactionServiceShape,
): Promise<string> {
  return await Effect.runPromise(redaction.revealForLocalUser(text))
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

function toolOnlyTurnFallbackBody(transcript: readonly KhalaCodeDesktopMessage[]): string {
  const summaries = transcript
    .filter(message => message.role === "tool")
    .map(message => firstNonEmptyLine(message.body))
    .filter(summary => summary.length > 0)
  return [
    "We ran the requested tools and received the results shown above. The model returned no final summary, so we are surfacing the tool output instead of leaving this turn blank.",
    ...(summaries.length === 0 ? [] : ["", ...summaries.map(summary => `- ${summary}`)]),
  ].join("\n")
}

function usableToolAnswerText(text: string): string | null {
  const trimmed = text.trim()
  return trimmed.length === 0 || isVacuousPostToolAnswer(trimmed) ? null : trimmed
}

function isVacuousPostToolAnswer(text: string): boolean {
  const normalized = text.trim().replace(/\s+/g, " ").toLowerCase()
  if (normalized.length === 0) return false
  if (
    normalized === "we are khala. how can we help?" ||
    normalized === "we are khala. how can we help you?" ||
    normalized === "we are khala code." ||
    normalized === "we are khala code. how can we help?"
  ) {
    return true
  }
  const identityOnly = /^we are khala(?: code)?,? (?:a )?collective intelligence(?: built and operated by openagents| by openagents)?\.?(?: how can we help(?: you)?\?)?$/u.test(normalized)
  const evasiveDisclosure =
    /\bwe (?:do not|don't|cannot|can't|won't) disclose\b/u.test(normalized) &&
    /\b(?:underlying|backing)\b/u.test(normalized) &&
    /\b(?:model|provider|vendor|company)\b/u.test(normalized)
  return identityOnly || evasiveDisclosure
}

function toolCatalogSystemPrompt(toolDefinitions: readonly KhalaToolDefinition[]): string {
  return [
    "Available Khala Code Desktop tools:",
    ...toolDefinitions.map(tool => {
      const authority = tool.authority.trim()
      const description = tool.description.trim().replace(/\s+/g, " ")
      return `- ${tool.name} (${authority}): ${description}`
    }),
    "For tool-list or capability questions, summarize this catalog directly without invoking any tool.",
  ].join("\n")
}

function firstNonEmptyLine(value: string): string {
  return value
    .split(/\r?\n/)
    .map(line => line.trim())
    .find(line => line.length > 0) ?? ""
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
    "pylon_ensure",
    "codex_fleet_status",
    "codex_spawn",
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
