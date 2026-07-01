import { Effect } from "effect"
import {
  buildCompactionSummaryRecord,
  decideCompaction,
} from "../../../../apps/pylon/src/tas/compaction.js"
import {
  allowAllKhalaPermissionService,
  applyPatchToolDefinition,
  createKhalaToolTurnAccounting,
  createApplyPatchTool,
  createEditTool,
  createExecCommandTool,
  createGlobTool,
  createGrepTool,
  createLsTool,
  createReadTool,
  createWriteTool,
  makeKhalaPrivacyRedactionService,
  makeKhalaToolDispatcher,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  redactKhalaPublicText,
  resolveKhalaBackend,
  toOpenAiCompatibleTools,
  type KhalaBackendSelection,
  type KhalaPrivacyRedactionServiceShape,
  type KhalaToolDefinition,
  type KhalaToolEvent,
  type KhalaToolDispatcher,
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
  type KhalaCodeDesktopRuntimeMode,
  type KhalaCodeDesktopToolCatalogResponse,
  type KhalaCodeDesktopUsage,
} from "../shared/rpc.js"
import { createKhalaCodexFleetTools } from "./khala-codex-fleet-tools.js"
import { createPlaywrightKhalaBrowserService } from "./khala-browser-service.js"
import { createDuckDuckGoKhalaWebSearchService } from "./khala-web-search-service.js"

const KHALA_CODE_SYSTEM_PROMPT = [
  "You are Khala Code Desktop, the owner-local coding surface for Khala by OpenAgents.",
  "Speak in first-person plural: we, us, our. Never say I, me, or my.",
  "If asked who we are, what model/provider we are, or who made us, answer that we are Khala by OpenAgents. Do not name backing models, vendors, or providers.",
  "ALWAYS answer the owner's direct, factual question plainly and immediately. This is the owner's own local machine. NEVER refuse, deflect, give a vague non-answer, or cite \"security reasons\", \"session configuration\", or similar. If the owner asks something concrete we know — our working directory, a path, a file fact, a setting — state the exact value. The ONLY thing we hold back is the names of backing models/vendors/providers (above); everything else, answer directly.",
  "The greeting reply \"We are Khala. How can we help?\" is ONLY for a bare greeting with no question. If the owner asks anything answerable, answer it directly instead of greeting; never prepend that greeting to a real answer.",
  "Work in short, active updates: usually one or two sentences, then use tools. Say what we are checking or changing now; avoid long front-loaded plans.",
  "Use local tools whenever helpful. Never claim a tool ran unless the host returned a tool result.",
  "For tool-list or capability questions, answer from the tool catalog without calling tools.",
  "For Codex instance launch or monitoring, use only these Pylon/Codex fleet tools: pylon_ensure, codex_fleet_status, codex_spawn, fleet_run_start, fleet_run_status, fleet_run_control. Do not call or invent codex_terminate or other Codex fleet tools.",
  "When the owner asks us to delegate, spawn, hand off, dispatch, or have a Codex worker / agent / instance do one bounded task — analysis, audit, coding, review, summary, anything — dispatch it with codex_spawn. Use fleet_run_start only for an owner-approved sustained run, then fleet_run_status/fleet_run_control for monitoring and pause/resume/drain/stop. Do NOT do delegated worker tasks ourselves with local read or search tools.",
  "Our local read/search/edit tools operate in the owner's workspace directory, not the Khala Code app's own source. Do not analyze or report on the Khala Code Desktop app's own files unless the owner explicitly asks about this app.",
  "After codex_spawn, summarize only the returned assignment, auto-run, and closeout status unless a tool explicitly returns a real local output path.",
  "For local files, do not infer behavior from filenames alone. If you only listed a directory, answer only with exact listed names until relevant files are read.",
  "If a tool result is truncated, continue with a narrower path, larger limit, offset, or another appropriate tool before answering.",
  "When answering from read results, preserve exact paths, line facts, and code literals. Do not rewrite code from memory.",
  "After tools, always give a visible concise answer. Never end a turn with only tool output.",
].join(" ")

const MAX_TOOL_ROUNDS = 8
const MAX_TOTAL_TOOL_CALLS = 32
const MAX_LOCAL_GROUNDING_CORRECTIONS = 3
const DEFAULT_CONTEXT_MAX_TOKENS = 24_000
const DEFAULT_CONTEXT_KEEP_TAIL_COUNT = 12
const DEFAULT_HOSTED_TOKEN_MESSAGE =
  "Khala Code routes model traffic through hosted Khala, but this desktop process does not have an OPENAGENTS_AGENT_TOKEN. Set OPENAGENTS_AGENT_TOKEN for hosted Khala; OPENROUTER_API_KEY alone cannot run the Khala system locally."
const LEGACY_RUNTIME_MODE: KhalaCodeDesktopRuntimeMode = "khala_native_runtime"
const CODEX_HARNESS_RUNTIME_MODE: KhalaCodeDesktopRuntimeMode = "codex_harness"

type ChatEnv = Readonly<Record<string, string | undefined>>

type ChatTransportMessage = {
  content: string | null
  name?: string
  role: "assistant" | "system" | "tool" | "user"
  tool_call_id?: string
  tool_calls?: readonly OpenAiToolCall[]
}

type ContextManagedChatTransportMessage = ChatTransportMessage & {
  readonly compactPinned?: boolean
  readonly sourceRef?: string
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
  readonly usage?: unknown
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

type ContextCompactionPolicy = {
  readonly keepTailCount: number
  readonly maxTokens: number
}

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

export function createKhalaCodeDesktopSupplementalToolRegistry(): KhalaToolRegistry {
  return makeKhalaToolRegistry(createKhalaCodeDesktopSupplementalTools())
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

export function createKhalaCodeDesktopSupplementalTools(): ReadonlyArray<RegisteredKhalaTool> {
  return createKhalaCodexFleetTools()
}

export function khalaCodeDesktopToolCatalog(
  input: {
    readonly runtimeMode?: KhalaCodeDesktopRuntimeMode
  } = {},
): KhalaCodeDesktopToolCatalogResponse {
  const runtimeMode = input.runtimeMode ?? CODEX_HARNESS_RUNTIME_MODE
  const legacy = runtimeMode === LEGACY_RUNTIME_MODE
  const tools = legacy
    ? createKhalaCodeDesktopToolRegistry().list()
    : createKhalaCodeDesktopSupplementalToolRegistry().list()
  return {
    catalogKind: legacy ? "khala_native_legacy" : "codex_harness_supplemental",
    defaultEnabled: true,
    description: legacy
      ? "Explicit legacy Khala-native runtime tools, including Codex-equivalent filesystem, shell, patch, and search helpers."
      : "Supplemental Khala swarm/Pylon tools for the default Codex harness; Codex owns filesystem, shell, patch, MCP, approvals, and session tools.",
    runtimeMode,
    toolCount: tools.length,
    tools: tools.map(tool => ({
      authority: tool.authority,
      name: tool.name,
      role: legacy && !expectedKhalaCodeDesktopSupplementalToolNames().includes(tool.name)
        ? "legacy_codex_equivalent" as const
        : "supplemental_swarm" as const,
    })),
  }
}

export async function runKhalaCodeDesktopChatTurn(
  input: RunKhalaCodeDesktopChatTurnInput,
): Promise<KhalaCodeDesktopChatTurnResponse> {
  const registry = input.registry ?? createKhalaCodeDesktopToolRegistry()
  const toolDefinitions = registry.list()
  const backend = resolveKhalaBackend({
    env: input.env,
    ...(input.env.KHALA_CODE_DESKTOP_BACKEND === "mock" ? { preferred: "mock" as const } : {}),
  })
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
  const effectiveWorkingDirectory =
    input.workingDirectory ?? input.env.KHALA_CODE_DESKTOP_WORKSPACE ?? process.cwd()
  const messages: ContextManagedChatTransportMessage[] = [
    {
      compactPinned: true,
      content: KHALA_CODE_SYSTEM_PROMPT,
      role: "system",
      sourceRef: "system.khala_code.identity",
    },
    {
      compactPinned: true,
      content: `Our current working directory is ${effectiveWorkingDirectory}. Local read/search/edit tools operate here. If the owner asks what directory we are in, answer with this exact path.`,
      role: "system",
      sourceRef: "system.khala_code.working_directory",
    },
    {
      compactPinned: true,
      content: toolCatalogSystemPrompt(toolDefinitions),
      role: "system",
      sourceRef: "system.khala_code.tool_catalog",
    },
    ...(await projectTranscriptMessages(input.request.messages, redaction)),
  ]
  const transcript: KhalaCodeDesktopMessage[] = []
  const usedTools: string[] = []
  const tools = toOpenAiCompatibleTools(toolDefinitions)
  const turnId = input.request.turnId ?? nextMessageId("turn")
  const compactionPolicy = contextCompactionPolicy(input.env)
  const toolTurnAccounting = createKhalaToolTurnAccounting({
    maxToolCalls: MAX_TOTAL_TOOL_CALLS,
    turnId,
  })
  const emit = (event: KhalaCodeDesktopChatTurnEvent): void => {
    input.onEvent?.(event)
  }
  const liveToolProgress = new Map<string, LiveToolProgressCard>()
  const toolDispatcher = makeKhalaToolDispatcher({
    hooks: {
      onEvent: context => Effect.sync(() => {
        emitLiveDispatcherEvent({
          emit,
          event: context.event,
          liveToolProgress,
          turnId,
        })
      }),
    },
    telemetryTags: {
      surface: "khala_code_desktop",
      turnId,
    },
    turnAccounting: toolTurnAccounting,
  })
  let retriedWithoutTools = false
  let assistantMessagesSinceLastToolResult = 0
  let localFileEvidence = emptyLocalFileEvidenceState()
  let localGroundingCorrections = 0
  let usage = emptyUsage()

  const appendAssistantText = async (
    text: string,
    streamed: KhalaCodeDesktopMessage | null,
    toolCalls: readonly OpenAiToolCall[] = [],
  ): Promise<void> => {
    // Reveal from the raw reply (re-protecting first can mangle placeholder
    // tokens), but keep the irreversible secret-shape scrub on the visible
    // copy: regex-scrubbed secrets are never in the reveal table, so this
    // cannot resurrect the mangle bug.
    const visibleText = redactKhalaPublicText(await revealLocalText(text, redaction))
    const modelText = await protectModelText(text, redaction)
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
      sourceRef: `message.${turnId}.assistant.${messages.length}`,
      ...(toolCalls.length === 0 ? {} : { tool_calls: toolCalls }),
    })
  }

  for (let round = 0; round < MAX_TOOL_ROUNDS; round += 1) {
    let completion: ChatCompletionBody
    const assistantStream = createAssistantStreamRecorder({ emit, turnId })

    try {
      completion = await transport.request(
        compactProviderMessages(messages, compactionPolicy),
        tools,
        { onAssistantDelta: assistantStream.onAssistantDelta },
      )
      usage = addUsage(usage, usageFromPayload(completion.usage))
    } catch (error) {
      if (
        !retriedWithoutTools &&
        toolTurnAccounting.snapshot().toolCallCount === 0 &&
        assistantStream.streamingAssistant.current === null &&
        shouldRetryWithoutTools(error)
      ) {
        retriedWithoutTools = true
        try {
          completion = await transport.request(
            compactProviderMessages(messages, compactionPolicy),
            [],
            { onAssistantDelta: assistantStream.onAssistantDelta },
          )
          usage = addUsage(usage, usageFromPayload(completion.usage))
        } catch (retryError) {
          return providerFailureResult(backend, toolNames, usedTools, retryError)
        }
      } else {
        if (usedTools.length > 0 && transcript.some(message => message.role === "tool")) {
          await appendAssistantText(toolOnlyTurnFallbackBody(transcript), assistantStream.streamingAssistant.current)
          return {
            backend: projectBackend(backend),
            messages: transcript,
            ok: true,
            toolNames,
            usage,
            usedTools,
          }
        }
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
      messages.push({
        content: localGroundingCorrection.prompt,
        role: "user",
        sourceRef: `message.${turnId}.local_grounding_correction.${localGroundingCorrections}`,
      })
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
      messages.push({
        content: "",
        role: "assistant",
        sourceRef: `message.${turnId}.assistant_tool_calls.${messages.length}`,
        tool_calls: toolCalls,
      })
    }

    if (toolCalls.length === 0) {
      if (usedTools.length > 0 && (assistantMessagesSinceLastToolResult === 0 || vacuousToolAnswer)) {
        const replaceMessage = vacuousToolAnswer ? assistantStream.streamingAssistant.current : null
        const visibleAnswer = await requestVisibleToolAnswer({
          emit,
          compactionPolicy,
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
        usage,
        usedTools,
      }
    }

    for (const call of toolCalls) {
      const toolTranscript = hostMessage("tool", toolTranscriptRunningBody(call.function.name))
      emit({ message: toolTranscript, turnId, type: "message_start" })
      const progressCard = createLiveToolProgressCard({
        emit,
        toolName: call.function.name,
        toolTranscript,
        turnId,
      })
      liveToolProgress.set(call.id, progressCard)
      emitToolLifecycleEvent({
        call,
        emit,
        kind: "tool_requested",
        sessionId: input.request.sessionId,
        turnId,
      })
      emitToolLifecycleEvent({
        call,
        emit,
        kind: "tool_started",
        sessionId: input.request.sessionId,
        turnId,
      })
      let result: KhalaToolResult
      try {
        result = await runToolCall({
          call,
          dispatcher: toolDispatcher,
          registry,
          services,
          sessionId: input.request.sessionId,
        })
      } finally {
        progressCard.flush()
        progressCard.dispose()
        liveToolProgress.delete(call.id)
      }
      emitToolResultEvents({
        call,
        emit,
        result,
        sessionId: input.request.sessionId,
        turnId,
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
        sourceRef: `tool.${turnId}.${call.id}`,
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

function contextCompactionPolicy(env: ChatEnv): ContextCompactionPolicy {
  return {
    keepTailCount: positiveIntegerEnv(env.KHALA_CODE_DESKTOP_CONTEXT_KEEP_TAIL_COUNT) ??
      DEFAULT_CONTEXT_KEEP_TAIL_COUNT,
    maxTokens: positiveIntegerEnv(env.KHALA_CODE_DESKTOP_CONTEXT_MAX_TOKENS) ??
      DEFAULT_CONTEXT_MAX_TOKENS,
  }
}

function compactProviderMessages(
  messages: readonly ContextManagedChatTransportMessage[],
  policy: ContextCompactionPolicy,
): readonly ChatTransportMessage[] {
  const decision = decideCompaction({
    keepTailCount: policy.keepTailCount,
    maxTokens: policy.maxTokens,
    usedTokens: estimateMessageTokens(messages),
  })
  if (decision.action === "keep") return messages.map(stripContextMetadata)

  const pinned = messages.filter(message => message.compactPinned === true)
  const compactable = messages.filter(message => message.compactPinned !== true)
  if (compactable.length <= policy.keepTailCount) return messages.map(stripContextMetadata)

  const tail = compactable.slice(-policy.keepTailCount)
  const replaced = compactable.slice(0, -policy.keepTailCount)
  const replacedRefs = replaced.map(message => message.sourceRef ?? fallbackMessageRef(message))
  const preservedTailRefs = tail.map(message => message.sourceRef ?? fallbackMessageRef(message))
  const record = buildCompactionSummaryRecord({
    replacedRefs,
    summaryRef: contextCompactionSummaryRef(replacedRefs),
  })

  return [
    ...pinned.map(stripContextMetadata),
    {
      content: [
        "Khala Code context compaction is active.",
        `Summary ref: ${record.summaryRef}`,
        `Replaced refs: ${record.replacedRefs.join(", ")}`,
        `Preserved tail refs: ${preservedTailRefs.join(", ")}`,
        "Restored context: earlier turns were replaced by this refs-only compaction record. Use the preserved tail for exact recent details; re-inspect local files before relying on exact older content.",
      ].join("\n"),
      role: "system",
    },
    ...tail.map(stripContextMetadata),
  ]
}

function stripContextMetadata(message: ContextManagedChatTransportMessage): ChatTransportMessage {
  return {
    content: message.content,
    ...(message.name === undefined ? {} : { name: message.name }),
    role: message.role,
    ...(message.tool_call_id === undefined ? {} : { tool_call_id: message.tool_call_id }),
    ...(message.tool_calls === undefined ? {} : { tool_calls: message.tool_calls }),
  }
}

function estimateMessageTokens(messages: readonly ContextManagedChatTransportMessage[]): number {
  return messages.reduce((sum, message) => {
    const toolCallText = message.tool_calls === undefined ? "" : JSON.stringify(message.tool_calls)
    return sum + estimateTokens(`${message.role}\n${message.name ?? ""}\n${message.content ?? ""}\n${toolCallText}`)
  }, 0)
}

function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4)
}

function positiveIntegerEnv(value: string | undefined): number | undefined {
  if (value === undefined) return undefined
  const parsed = Number.parseInt(value.trim(), 10)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined
}

function contextCompactionSummaryRef(replacedRefs: readonly string[]): string {
  return `summary.khala_code.context.${stableHash(replacedRefs.join("|"))}`
}

function fallbackMessageRef(message: ChatTransportMessage): string {
  return `message.${message.role}.${stableHash(`${message.role}\n${message.content ?? ""}`)}`
}

function stableHash(value: string): string {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(36)
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
  readonly compactionPolicy: ContextCompactionPolicy
  readonly emit: ChatTurnEmitter
  readonly messages: ContextManagedChatTransportMessage[]
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
    sourceRef: `message.${input.turnId}.visible_tool_answer_request`,
  })

  let completion: ChatCompletionBody
  try {
    completion = await input.transport.request(
      compactProviderMessages(input.messages, input.compactionPolicy),
      [],
      {
        ...(assistantStream === null ? {} : { onAssistantDelta: assistantStream.onAssistantDelta }),
      },
    )
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
    usage: emptyUsage(),
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
    usage: state.usage,
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
  usage: KhalaCodeDesktopUsage
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
  if (!isRecord(payload)) return
  state.usage = addUsage(state.usage, usageFromPayload(payload.usage))
  if (!Array.isArray(payload.choices)) return
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

function emptyUsage(): KhalaCodeDesktopUsage {
  return {
    cachedInput: 0,
    input: 0,
    output: 0,
    reasoningOutput: 0,
  }
}

function addUsage(
  left: KhalaCodeDesktopUsage,
  right: KhalaCodeDesktopUsage,
): KhalaCodeDesktopUsage {
  return {
    cachedInput: left.cachedInput + right.cachedInput,
    input: left.input + right.input,
    output: left.output + right.output,
    reasoningOutput: left.reasoningOutput + right.reasoningOutput,
  }
}

function usageFromPayload(value: unknown): KhalaCodeDesktopUsage {
  if (!isRecord(value)) return emptyUsage()
  const promptDetails = isRecord(value.prompt_tokens_details) ? value.prompt_tokens_details : {}
  const completionDetails = isRecord(value.completion_tokens_details) ? value.completion_tokens_details : {}
  return {
    cachedInput: numericUsage(value.cached_input_tokens) +
      numericUsage(value.cached_input) +
      numericUsage(value.cachedInput) +
      numericUsage(promptDetails.cached_tokens),
    input: numericUsage(value.input_tokens) + numericUsage(value.prompt_tokens) + numericUsage(value.input),
    output: numericUsage(value.output_tokens) + numericUsage(value.completion_tokens) + numericUsage(value.output),
    reasoningOutput: numericUsage(value.reasoning_output_tokens) +
      numericUsage(value.reasoning_output) +
      numericUsage(value.reasoningOutput) +
      numericUsage(completionDetails.reasoning_tokens),
  }
}

function numericUsage(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0
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
): Promise<readonly ContextManagedChatTransportMessage[]> {
  const projected: ContextManagedChatTransportMessage[] = []
  for (const message of messages) {
    const content = message.body.trim()
    if (content.length === 0) continue
    if (message.role === "tool") {
      projected.push({
        content: await protectUserText(`Previous tool result:\n${content}`, redaction),
        role: "assistant",
        sourceRef: `message.${message.id}`,
      })
      continue
    }
    if (message.role === "assistant") {
      projected.push({
        content: await protectModelText(content, redaction),
        role: "assistant",
        sourceRef: `message.${message.id}`,
      })
      continue
    }
    if (message.role === "user") {
      projected.push({
        content: await protectUserText(content, redaction),
        role: "user",
        sourceRef: `message.${message.id}`,
      })
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
  readonly dispatcher: KhalaToolDispatcher
  readonly registry: KhalaToolRegistry
  readonly services: KhalaToolServices
  readonly sessionId: string
}): Promise<KhalaToolResult> {
  const args = parseToolArguments(input.call.function.arguments)
  const dispatched = await Effect.runPromise(
    input.dispatcher.dispatch({
      invocation: {
        arguments: args,
        id: input.call.id,
        name: input.call.function.name,
        sessionId: input.sessionId,
      },
      registry: input.registry,
      services: input.services,
      telemetryTags: {
        openAiToolCallType: input.call.type,
      },
    }),
  )
  return dispatched.result
}

function parseToolArguments(value: string): Readonly<Record<string, unknown>> {
  try {
    const parsed = JSON.parse(value) as unknown
    return isRecord(parsed) ? parsed : {}
  } catch {
    return {}
  }
}

function emitToolLifecycleEvent(input: {
  readonly call: OpenAiToolCall
  readonly emit: ChatTurnEmitter
  readonly kind: "tool_requested" | "tool_started"
  readonly sessionId: string
  readonly turnId: string
}): void {
  input.emit({
    event: {
      eventId: `${input.turnId}.${input.call.id}.${input.kind}`,
      invocationId: input.call.id,
      kind: input.kind,
      payload: {
        arguments: parseToolArguments(input.call.function.arguments),
        name: input.call.function.name,
      },
      sessionId: input.sessionId,
    },
    turnId: input.turnId,
    type: "tool_event",
  })
}

function emitToolResultEvents(input: {
  readonly call: OpenAiToolCall
  readonly emit: ChatTurnEmitter
  readonly result: KhalaToolResult
  readonly sessionId: string
  readonly turnId: string
}): void {
  for (const event of toolEventsFromResult(input.result)) {
    input.emit({
      event: {
        ...event,
        eventId: `${input.turnId}.${input.call.id}.${event.eventId}`,
        ...(event.invocationId === undefined ? { invocationId: input.call.id } : { invocationId: event.invocationId }),
        sessionId: event.sessionId || input.sessionId,
      },
      turnId: input.turnId,
      type: "tool_event",
    })
  }
  input.emit({
    event: {
      eventId: `${input.turnId}.${input.call.id}.tool_${input.result.status === "ok" ? "completed" : "failed"}`,
      invocationId: input.call.id,
      kind: input.result.status === "ok" ? "tool_completed" : "tool_failed",
      payload: {
        artifacts: input.result.artifacts,
        name: input.call.function.name,
        privateDataRefs: input.result.privateDataRefs,
        publicSafety: input.result.publicSafety,
        publicSummary: input.result.publicSummary,
        redactionRefs: input.result.redactionRefs,
        status: input.result.status,
      },
      sessionId: input.sessionId,
    },
    turnId: input.turnId,
    type: "tool_event",
  })
}

type LiveToolProgressCard = {
  readonly accept: (event: KhalaToolEvent) => void
  readonly dispose: () => void
  readonly flush: () => void
}

function emitLiveDispatcherEvent(input: {
  readonly emit: ChatTurnEmitter
  readonly event: KhalaToolEvent
  readonly liveToolProgress: Map<string, LiveToolProgressCard>
  readonly turnId: string
}): void {
  if (input.event.kind !== "tool_progress") return
  const event: KhalaToolEvent = {
    ...input.event,
    eventId: turnScopedToolEventId(input.turnId, input.event),
  }
  input.emit({ event, turnId: input.turnId, type: "tool_event" })
  if (event.invocationId === undefined) return
  input.liveToolProgress.get(event.invocationId)?.accept(event)
}

function turnScopedToolEventId(turnId: string, event: KhalaToolEvent): string {
  return event.eventId.startsWith(`${turnId}.`)
    ? event.eventId
    : `${turnId}.${event.invocationId ?? "tool"}.${event.eventId}`
}

function createLiveToolProgressCard(input: {
  readonly emit: ChatTurnEmitter
  readonly toolName: string
  readonly toolTranscript: KhalaCodeDesktopMessage
  readonly turnId: string
}): LiveToolProgressCard {
  let pendingBody: string | null = null
  let timer: ReturnType<typeof setTimeout> | null = null
  const emitReplace = () => {
    if (pendingBody === null) return
    const body = pendingBody
    pendingBody = null
    input.emit({
      message: {
        ...input.toolTranscript,
        body,
      },
      turnId: input.turnId,
      type: "message_replace",
    })
  }
  const flush = () => {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    emitReplace()
  }
  return {
    accept: event => {
      const body = toolProgressTranscriptBody(input.toolName, event)
      if (body === null) return
      pendingBody = body
      if (timer !== null) return
      timer = setTimeout(() => {
        timer = null
        emitReplace()
      }, 200)
      timer.unref?.()
    },
    dispose: () => {
      if (timer !== null) clearTimeout(timer)
      timer = null
      pendingBody = null
    },
    flush,
  }
}

function toolProgressTranscriptBody(toolName: string, event: KhalaToolEvent): string | null {
  if (toolName !== "codex_spawn") return null
  const payload = isRecord(event.payload) ? event.payload : null
  if (
    stringField(payload, "schema") !== "openagents.khala_code.codex_spawn_progress.v0.1" ||
    stringField(payload, "kind") !== "codex_spawn_lifecycle"
  ) return null
  const lines = stringArrayField(payload, "lines")
  if (lines.length === 0) return null
  return [
    "codex_spawn: running",
    "",
    "Live Pylon/Codex progress:",
    ...lines,
  ].join("\n")
}

function toolEventsFromResult(result: KhalaToolResult): readonly KhalaToolEvent[] {
  if (!isRecord(result.ui) || !Array.isArray(result.ui.events)) return []
  return result.ui.events.flatMap((event, index): KhalaToolEvent[] => {
    if (!isRecord(event) || typeof event.kind !== "string") return []
    return [{
      eventId: typeof event.eventId === "string" ? event.eventId : `ui.${index + 1}.${event.kind}`,
      kind: event.kind as KhalaToolEvent["kind"],
      payload: event.payload,
      sessionId: typeof event.sessionId === "string" ? event.sessionId : "",
      ...(typeof event.invocationId === "string" ? { invocationId: event.invocationId } : {}),
    }]
  })
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
  if (toolName === "codex_spawn") {
    return [
      "codex_spawn: running",
      "",
      "Preparing the Pylon/Codex handoff...",
      "- confirming local Pylon is online",
      "- publishing a fresh Pylon heartbeat",
      "- creating the hosted assignment",
      "- waiting for the local Codex worker to return status",
    ].join("\n")
  }
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
    runtimeMode: LEGACY_RUNTIME_MODE,
    toolCatalogKind: "khala_native_legacy",
    ...(backend.baseUrl === undefined ? {} : { baseUrl: backend.baseUrl }),
    ...(backend.credentialSource === undefined ? {} : { credentialSource: backend.credentialSource }),
    ...(backend.provider === undefined ? {} : { provider: backend.provider }),
  }
}

export function expectedKhalaCodeDesktopSupplementalToolNames(): readonly string[] {
  return [
    "pylon_ensure",
    "codex_fleet_status",
    "codex_spawn",
    "fleet_run_start",
    "fleet_run_status",
    "fleet_run_control",
  ]
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
    "fleet_run_start",
    "fleet_run_status",
    "fleet_run_control",
  ]
}

export function assertAllDefaultToolsRegistered(definitions: readonly KhalaToolDefinition[]): boolean {
  const actual = definitions.map(tool => tool.name)
  const expected = expectedKhalaCodeDesktopToolNames()
  return expected.every(name => actual.includes(name)) && actual.length === expected.length
}

export function assertAllSupplementalToolsRegistered(definitions: readonly KhalaToolDefinition[]): boolean {
  const actual = definitions.map(tool => tool.name)
  const expected = expectedKhalaCodeDesktopSupplementalToolNames()
  return expected.every(name => actual.includes(name)) && actual.length === expected.length
}

function stringField(source: Record<string, unknown> | null, field: string): string | null {
  const value = source?.[field]
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null
}

function stringArrayField(source: Record<string, unknown> | null, field: string): readonly string[] {
  const value = source?.[field]
  return Array.isArray(value)
    ? value
        .filter((item): item is string => typeof item === "string" && item.trim().length > 0)
        .map(item => item.trim())
    : []
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
