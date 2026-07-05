import { createHash } from "node:crypto"
import {
  jsonSchema,
  streamText,
  tool,
} from "ai"
import {
  assertKhalaRuntimePublicEventSafe,
  decodeKhalaRuntimeEvent,
  khalaRuntimeEventFromAgentRuntimeEvent,
  khalaRuntimeEventFromAiSdkTextStreamPart,
  type AgentRuntimeEvent,
  type KhalaRuntimeAiSdkTextStreamPart,
  type KhalaRuntimeEvent,
  type KhalaRuntimeSource,
  type KhalaRuntimeToolAuthority,
} from "@openagentsinc/agent-runtime-schema"
import {
  makeKhalaToolDispatcher,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaToolDispatcherOptions,
  type KhalaToolRegistry,
  type KhalaToolResult,
  type KhalaToolServices,
  type RegisteredKhalaTool,
} from "@openagentsinc/khala-tools"
import { Effect } from "effect"

export type KhalaAiSdkCoreStreamTextResult = Readonly<{
  stream: AsyncIterable<unknown>
}>

export type KhalaAiSdkCoreStreamText = (
  options: Record<string, unknown>,
) => KhalaAiSdkCoreStreamTextResult | Promise<KhalaAiSdkCoreStreamTextResult>

export type KhalaAiSdkReasoningOptions = Readonly<{
  effort?: "minimal" | "low" | "medium" | "high"
  enabled?: boolean
  maxTokens?: number
}>

export type KhalaAiSdkPromptCacheOptions = Readonly<{
  cacheKey?: string
  cacheControl?: "ephemeral" | "persistent"
}>

export type KhalaAiSdkProviderProfile = Readonly<{
  providerRef: string
  modelRef: string
  headers?: Readonly<Record<string, string>>
  providerOptions?: Readonly<Record<string, unknown>>
  reasoning?: KhalaAiSdkReasoningOptions
  promptCache?: KhalaAiSdkPromptCacheOptions
  schemaLowering?: "json_schema" | "loose_object"
  strictToolSchemas?: boolean
}>

export type KhalaAiSdkCoreToolBridgeOptions = Readonly<{
  dispatcherOptions?: KhalaToolDispatcherOptions
  registry?: KhalaToolRegistry
  services?: KhalaToolServices
  sessionId?: string
  telemetryTags?: Readonly<Record<string, string | number | boolean>>
  tools?: ReadonlyArray<RegisteredKhalaTool>
}>

export type KhalaAiSdkCoreRunInput = Readonly<{
  model: unknown
  threadId: string
  turnId: string
  messages?: unknown
  prompt?: string
  headers?: Readonly<Record<string, string>>
  provider?: KhalaAiSdkProviderProfile
  providerOptions?: Readonly<Record<string, unknown>>
  streamText?: KhalaAiSdkCoreStreamText
  tools?: Record<string, unknown>
  source?: KhalaRuntimeSource
  eventVisibility?: KhalaRuntimeEvent["visibility"]
  observedAt?: () => string
  toolAuthority?: (toolName: string) => KhalaRuntimeToolAuthority
  rawMode?: "private_sidecar_ref" | "discard"
  onPrivateRawPart?: (
    input: Readonly<{ rawEventRef: string; part: unknown }>,
  ) => void | Promise<void>
}>

export type KhalaAiSdkCoreRunResult = Readonly<{
  events: ReadonlyArray<KhalaRuntimeEvent>
  rawSidecars: ReadonlyArray<KhalaAiSdkPrivateRawSidecar>
}>

export type KhalaAiSdkPrivateRawSidecar = Readonly<{
  rawEventRef: string
  partType: string
  retained: boolean
}>

export type KhalaRuntimeTranscriptProjection = Readonly<{
  textByMessageId: Readonly<Record<string, string>>
  reasoningByMessageId: Readonly<Record<string, string>>
  toolStates: Readonly<Record<string, "called" | "completed" | "failed">>
  turnState: "idle" | "running" | "interrupted" | "completed" | "failed"
}>

export function buildKhalaAiSdkCoreStreamTextOptions(
  input: Omit<
    KhalaAiSdkCoreRunInput,
    | "eventVisibility"
    | "observedAt"
    | "onPrivateRawPart"
    | "rawMode"
    | "source"
    | "streamText"
    | "threadId"
    | "toolAuthority"
    | "turnId"
  >,
): Record<string, unknown> {
  return withoutUndefined({
    headers: {
      ...(input.provider?.headers ?? {}),
      ...(input.headers ?? {}),
    },
    messages: input.messages,
    model: input.model,
    prompt: input.prompt,
    providerOptions: lowerKhalaAiSdkProviderOptions({
      provider: input.provider,
      providerOptions: input.providerOptions,
    }),
    tools: input.tools,
  })
}

export function lowerKhalaAiSdkProviderOptions(input: Readonly<{
  provider?: KhalaAiSdkProviderProfile | undefined
  providerOptions?: Readonly<Record<string, unknown>> | undefined
}>): Record<string, unknown> {
  const provider = input.provider
  const merged: Record<string, unknown> = {
    ...(input.providerOptions ?? {}),
  }
  if (provider === undefined) return merged

  const providerKey = provider.providerRef
  const existing = isRecord(merged[providerKey])
    ? merged[providerKey]
    : {}
  merged[providerKey] = withoutUndefined({
    ...existing,
    ...(provider.providerOptions ?? {}),
    promptCache: provider.promptCache,
    reasoning: provider.reasoning,
    schemaLowering: provider.schemaLowering,
    strictToolSchemas: provider.strictToolSchemas,
  })
  return merged
}

export function khalaToolsToAiSdkTools(
  input: KhalaAiSdkCoreToolBridgeOptions,
): Record<string, unknown> {
  const registry = input.registry ?? makeKhalaToolRegistry(input.tools ?? [])
  const dispatcher = makeKhalaToolDispatcher(input.dispatcherOptions)
  const services = input.services ?? makeKhalaToolServices()
  const sessionId = input.sessionId ?? "session.ai_sdk_core"
  const aiTools: Record<string, unknown> = {}

  for (const definition of registry.list()) {
    aiTools[definition.name] = tool({
      description: definition.description,
      inputSchema: jsonSchema(definition.inputSchema),
      metadata: {
        khalaToolInternalId: definition.internalId,
        khalaToolAuthority: definition.authority,
      },
      title: definition.label,
      execute: async (args: unknown, options: unknown) => {
        const toolCallId = toolCallIdFromOptions(options, definition.name, args)
        const dispatched = await Effect.runPromise(
          dispatcher.dispatch({
            invocation: {
              arguments: isRecord(args) ? args : {},
              id: toolCallId,
              name: definition.name,
              sessionId,
            },
            registry,
            services,
            telemetryTags: {
              lane: "ai_sdk_core",
              ...(input.telemetryTags ?? {}),
            },
          }),
        )
        return khalaToolResultForModel(dispatched.result)
      },
    })
  }

  return aiTools
}

export async function runKhalaAiSdkCoreRuntime(
  input: KhalaAiSdkCoreRunInput,
): Promise<KhalaAiSdkCoreRunResult> {
  const callStreamText = input.streamText ?? ((options) => streamText(options as never) as never)
  const result = await callStreamText(
    buildKhalaAiSdkCoreStreamTextOptions(input),
  )
  return collectKhalaAiSdkCoreEventsFromStream({
    source: input.source ?? sourceFromProvider(input.provider),
    stream: result.stream,
    threadId: input.threadId,
    turnId: input.turnId,
    ...(input.eventVisibility === undefined
      ? {}
      : { eventVisibility: input.eventVisibility }),
    ...(input.observedAt === undefined ? {} : { observedAt: input.observedAt }),
    ...(input.onPrivateRawPart === undefined
      ? {}
      : { onPrivateRawPart: input.onPrivateRawPart }),
    ...(input.rawMode === undefined ? {} : { rawMode: input.rawMode }),
    ...(input.toolAuthority === undefined
      ? {}
      : { toolAuthority: input.toolAuthority }),
  })
}

export async function collectKhalaAiSdkCoreEventsFromStream(input: Readonly<{
  stream: AsyncIterable<unknown>
  threadId: string
  turnId: string
  source?: KhalaRuntimeSource
  eventVisibility?: KhalaRuntimeEvent["visibility"]
  observedAt?: () => string
  toolAuthority?: (toolName: string) => KhalaRuntimeToolAuthority
  rawMode?: "private_sidecar_ref" | "discard"
  onPrivateRawPart?: (
    input: Readonly<{ rawEventRef: string; part: unknown }>,
  ) => void | Promise<void>
}>): Promise<KhalaAiSdkCoreRunResult> {
  const events: KhalaRuntimeEvent[] = []
  const rawSidecars: KhalaAiSdkPrivateRawSidecar[] = []
  let sequence = 1

  for await (const rawPart of input.stream) {
    const part = normalizeAiSdkTextStreamPart(rawPart)
    const rawEventRef = `raw.${stableRef([
      input.threadId,
      input.turnId,
      String(sequence),
      part.type,
    ])}`

    if (part.type === "raw") {
      if (input.rawMode !== "discard") {
        await input.onPrivateRawPart?.({
          part: rawPart,
          rawEventRef,
        })
        rawSidecars.push({
          partType: part.type,
          rawEventRef,
          retained: input.onPrivateRawPart !== undefined,
        })
      }
    }

    if (part.type !== "raw" || input.rawMode !== "discard") {
      const authority = toolAuthorityForPart(part, input.toolAuthority)
      const mapped = khalaRuntimeEventFromAiSdkTextStreamPart({
        eventId: `event.${stableRef([
          input.threadId,
          input.turnId,
          String(sequence),
          part.type,
        ])}`,
        observedAt: input.observedAt?.() ?? new Date().toISOString(),
        part,
        rawEventRef,
        sequence,
        source: input.source ?? { lane: "ai_sdk_core", surface: "server" },
        threadId: input.threadId,
        turnId: input.turnId,
        ...(authority === undefined ? {} : { authority }),
      })
      events.push(
        withVisibility(mapped, input.eventVisibility ?? "private"),
      )
      sequence += 1
    }
  }

  return { events, rawSidecars }
}

export function reduceKhalaRuntimeTranscript(
  events: ReadonlyArray<KhalaRuntimeEvent>,
): KhalaRuntimeTranscriptProjection {
  const textByMessageId: Record<string, string> = {}
  const reasoningByMessageId: Record<string, string> = {}
  const toolStates: Record<string, "called" | "completed" | "failed"> = {}
  let turnState: KhalaRuntimeTranscriptProjection["turnState"] = "idle"

  for (const event of events) {
    switch (event.kind) {
      case "turn.started":
        turnState = "running"
        break
      case "turn.interrupted":
        turnState = "interrupted"
        break
      case "turn.finished":
        turnState = event.finishReason === "error" ? "failed" : "completed"
        break
      case "text.delta":
        textByMessageId[event.messageId] =
          (textByMessageId[event.messageId] ?? "") + event.text
        break
      case "reasoning.delta":
        reasoningByMessageId[event.messageId] =
          (reasoningByMessageId[event.messageId] ?? "") + event.text
        break
      case "tool.call":
      case "tool.input.delta":
      case "tool.input.completed":
        toolStates[event.toolCallId] = "called"
        break
      case "tool.result":
        toolStates[event.toolCallId] = "completed"
        break
      case "tool.error":
        toolStates[event.toolCallId] = "failed"
        break
      default:
        break
    }
  }

  return {
    reasoningByMessageId,
    textByMessageId,
    toolStates,
    turnState,
  }
}

export function reduceAgentRuntimeEventsAsKhalaTranscript(input: Readonly<{
  events: ReadonlyArray<AgentRuntimeEvent>
  source: KhalaRuntimeSource
  threadId: string
  turnId: string
  authority?: KhalaRuntimeToolAuthority
}>): KhalaRuntimeTranscriptProjection {
  return reduceKhalaRuntimeTranscript(
    input.events.map(event =>
      khalaRuntimeEventFromAgentRuntimeEvent({
        event,
        source: input.source,
        threadId: input.threadId,
        turnId: input.turnId,
        ...(input.authority === undefined ? {} : { authority: input.authority }),
      }),
    ),
  )
}

function normalizeAiSdkTextStreamPart(
  part: unknown,
): KhalaRuntimeAiSdkTextStreamPart {
  if (!isRecord(part) || typeof part.type !== "string") {
    return { rawValue: part, type: "raw" }
  }
  switch (part.type) {
    case "start":
    case "start-step":
    case "text-start":
    case "text-delta":
    case "text-end":
    case "reasoning-start":
    case "reasoning-delta":
    case "reasoning-end":
    case "tool-input-start":
    case "tool-input-delta":
    case "tool-input-end":
    case "tool-call":
    case "tool-result":
    case "tool-error":
    case "tool-output-denied":
    case "tool-approval-request":
    case "tool-approval-response":
    case "finish-step":
    case "finish":
    case "abort":
    case "error":
    case "raw":
    case "custom":
    case "source":
    case "file":
    case "reasoning-file":
      return part as KhalaRuntimeAiSdkTextStreamPart
    default:
      return { rawValue: part, type: "raw" }
  }
}

function toolAuthorityForPart(
  part: KhalaRuntimeAiSdkTextStreamPart,
  toolAuthority: ((toolName: string) => KhalaRuntimeToolAuthority) | undefined,
): KhalaRuntimeToolAuthority | undefined {
  const toolName = toolNameFromPart(part)
  if (toolName === undefined) return undefined
  return toolAuthority?.(toolName) ?? defaultToolAuthority(toolName)
}

function toolNameFromPart(
  part: KhalaRuntimeAiSdkTextStreamPart,
): string | undefined {
  switch (part.type) {
    case "tool-call":
    case "tool-result":
    case "tool-error":
    case "tool-output-denied":
    case "tool-approval-request":
    case "tool-approval-response":
    case "tool-input-start":
      return part.toolName
    default:
      return undefined
  }
}

function defaultToolAuthority(toolName: string): KhalaRuntimeToolAuthority {
  return {
    allowed: true,
    authorityRef: `authority.${stableRef([toolName])}`,
    blockerRefs: [],
    decisionRef: `decision.${stableRef(["allow", toolName])}`,
    policyRef: "policy.private.ai_sdk_core.tool_dispatcher",
    status: "allowed",
    toolRef: `tool.openagents.khala.${safeRefSegment(toolName)}`,
  }
}

function khalaToolResultForModel(result: KhalaToolResult): Record<string, unknown> {
  return {
    artifactRefs: result.artifacts.map(artifact => artifact.artifactRef),
    privateDataRefs: [...result.privateDataRefs],
    publicSafety: result.publicSafety,
    publicSummary: result.publicSummary,
    redactionRefs: [...result.redactionRefs],
    status: result.status,
    text: result.modelOutput.text,
    ui: result.ui,
  }
}

function sourceFromProvider(
  provider: KhalaAiSdkProviderProfile | undefined,
): KhalaRuntimeSource {
  return {
    lane: "ai_sdk_core",
    surface: "server",
    ...(provider?.providerRef === undefined ? {} : { providerRef: provider.providerRef }),
    ...(provider?.modelRef === undefined ? {} : { modelRef: provider.modelRef }),
  }
}

function withVisibility(
  event: KhalaRuntimeEvent,
  visibility: KhalaRuntimeEvent["visibility"],
): KhalaRuntimeEvent {
  const next = decodeKhalaRuntimeEvent({
    ...event,
    redactionClass: visibility === "public" ? event.redactionClass : "private_ref",
    visibility,
  })
  return visibility === "public"
    ? assertKhalaRuntimePublicEventSafe(next)
    : next
}

function toolCallIdFromOptions(
  options: unknown,
  toolName: string,
  args: unknown,
): string {
  if (isRecord(options) && typeof options.toolCallId === "string") {
    return options.toolCallId
  }
  return `tool_call.${stableRef([toolName, JSON.stringify(args)])}`
}

function withoutUndefined(
  value: Record<string, unknown>,
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(value).filter(([, entry]) => entry !== undefined),
  )
}

function stableRef(parts: ReadonlyArray<string>): string {
  return createHash("sha256").update(parts.join("\0")).digest("hex").slice(0, 24)
}

function safeRefSegment(value: string): string {
  return /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value)
    ? value
    : stableRef([value])
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
