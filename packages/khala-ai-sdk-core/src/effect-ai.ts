/**
 * STREAM-01 (#9129): `effect/unstable/ai` as the model-call substrate.
 *
 * This module adds the Effect AI `LanguageModel` path beside the existing
 * Vercel-shaped runtime, additively:
 *
 * - `khalaEffectAiLanguageModelLayer` satisfies the Effect AI `LanguageModel`
 *   service backed by this package's existing provider transport (the
 *   injectable Vercel `streamText` call), so provider choice stays a Layer
 *   swap while the wire transport is unchanged.
 * - `khalaAiSdkTextStreamPartFromEffectAiStreamPart` maps Effect AI
 *   `Response.StreamPart` values onto the existing
 *   `KhalaRuntimeAiSdkTextStreamPart` ingestion vocabulary, so the shipped
 *   `khalaRuntimeEventFromAiSdkTextStreamPart` projection stays the single
 *   `KhalaRuntimeEvent` emission point.
 * - `runKhalaEffectAiCoreRuntime` is the `LanguageModel.streamText`-shaped run
 *   path that collects a turn into `KhalaRuntimeEvent`s.
 *
 * Failure classification for the fleet vocabulary lives in
 * `@openagentsinc/harness-conformance` (`ai-error-failure-class.ts`), because
 * that private package owns `HarnessFailureClass` and this package publishes
 * to npm.
 */
import {
  khalaRuntimeEventFromAiSdkTextStreamPart,
  type KhalaRuntimeAiSdkTextStreamPart,
  type KhalaRuntimeAiSdkUsage,
  type KhalaRuntimeEvent,
  type KhalaRuntimeSource,
  type KhalaRuntimeToolAuthority,
} from "@openagentsinc/agent-runtime-schema"
import { streamText as vercelStreamText } from "ai"
import { Effect, Layer, Stream } from "effect"
import { AiError, LanguageModel, Response } from "effect/unstable/ai"
import type { Prompt } from "effect/unstable/ai"
import {
  buildKhalaAiSdkCoreStreamTextOptions,
  collectKhalaAiSdkCoreEventsFromStream,
  normalizeAiSdkTextStreamPart,
  type KhalaAiSdkCoreRunResult,
  type KhalaAiSdkCoreStreamText,
  type KhalaAiSdkProviderProfile,
} from "./index.js"

type EffectAiUsageEncoded = (typeof Response.Usage)["Encoded"]

type EffectAiStreamPartInput = Response.StreamPartEncoded | Response.AnyPart

const effectAiFinishReasons: ReadonlySet<string> = new Set([
  "stop",
  "length",
  "content-filter",
  "tool-calls",
  "error",
  "pause",
  "other",
  "unknown",
])

/**
 * Map one part of the existing Vercel-shaped ingestion vocabulary onto the
 * Effect AI `Response.StreamPartEncoded` vocabulary. Parts with no Effect AI
 * equivalent (`start-step`, `finish-step`, `tool-approval-response`, `raw`,
 * `custom`, `reasoning-file`) return `undefined` and are dropped by the
 * adapter; raw sidecar retention stays on the existing ingestion path.
 */
export function effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart(
  part: KhalaRuntimeAiSdkTextStreamPart,
): Response.StreamPartEncoded | undefined {
  switch (part.type) {
    case "start":
      return {
        id: undefined,
        modelId: undefined,
        request: undefined,
        timestamp: undefined,
        type: "response-metadata",
      }
    case "text-start":
      return { id: part.id, type: "text-start" }
    case "text-delta":
      return { delta: part.text, id: part.id, type: "text-delta" }
    case "text-end":
      return { id: part.id, type: "text-end" }
    case "reasoning-start":
      return { id: part.id, type: "reasoning-start" }
    case "reasoning-delta":
      return { delta: part.text, id: part.id, type: "reasoning-delta" }
    case "reasoning-end":
      return { id: part.id, type: "reasoning-end" }
    case "tool-input-start":
      return { id: part.id, name: part.toolName, type: "tool-params-start" }
    case "tool-input-delta":
      return { delta: part.delta, id: part.id, type: "tool-params-delta" }
    case "tool-input-end":
      return { id: part.id, type: "tool-params-end" }
    case "tool-call":
      return {
        id: part.toolCallId,
        name: part.toolName,
        params: part.input ?? {},
        type: "tool-call",
        ...(part.providerExecuted === undefined
          ? {}
          : { providerExecuted: part.providerExecuted }),
      }
    case "tool-result":
      return {
        id: part.toolCallId,
        isFailure: false,
        name: part.toolName,
        result: part.output ?? null,
        type: "tool-result",
        ...(part.providerExecuted === undefined
          ? {}
          : { providerExecuted: part.providerExecuted }),
      }
    case "tool-error":
      return {
        id: part.toolCallId,
        isFailure: true,
        name: part.toolName,
        result: describeUnknownForModel(part.error),
        type: "tool-result",
        ...(part.providerExecuted === undefined
          ? {}
          : { providerExecuted: part.providerExecuted }),
      }
    case "tool-output-denied":
      return {
        id: part.toolCallId,
        isFailure: true,
        name: part.toolName,
        result: "tool output denied",
        type: "tool-result",
      }
    case "tool-approval-request":
      return {
        approvalId: `approval.${part.toolCallId}`,
        toolCallId: part.toolCallId,
        type: "tool-approval-request",
      }
    case "finish":
      return {
        reason: effectAiFinishReason(part.finishReason),
        response: undefined,
        type: "finish",
        usage: effectAiUsageEncodedFromKhalaUsage(part.totalUsage),
      }
    case "abort":
      return {
        reason: "other",
        response: undefined,
        type: "finish",
        usage: effectAiUsageEncodedFromKhalaUsage(undefined),
      }
    case "error":
      return { error: part.error, type: "error" }
    default:
      return undefined
  }
}

/**
 * Map one Effect AI response stream part onto the existing Vercel-shaped
 * ingestion vocabulary. This is the emission analogue of
 * `KhalaRuntimeAiSdkTextStreamPart` ingestion: composing it with
 * `khalaRuntimeEventFromAiSdkTextStreamPart` yields `KhalaRuntimeEvent`s.
 */
export function khalaAiSdkTextStreamPartFromEffectAiStreamPart(
  part: EffectAiStreamPartInput,
): KhalaRuntimeAiSdkTextStreamPart {
  switch (part.type) {
    case "response-metadata":
      return { type: "start" }
    case "text-start":
      return { id: part.id, type: "text-start" }
    case "text-delta":
      return { id: part.id, text: part.delta, type: "text-delta" }
    case "text-end":
      return { id: part.id, type: "text-end" }
    case "reasoning-start":
      return { id: part.id, type: "reasoning-start" }
    case "reasoning-delta":
      return { id: part.id, text: part.delta, type: "reasoning-delta" }
    case "reasoning-end":
      return { id: part.id, type: "reasoning-end" }
    case "tool-params-start":
      return { id: part.id, toolName: part.name, type: "tool-input-start" }
    case "tool-params-delta":
      return { delta: part.delta, id: part.id, type: "tool-input-delta" }
    case "tool-params-end":
      return { id: part.id, type: "tool-input-end" }
    case "tool-call":
      return {
        input: part.params,
        toolCallId: part.id,
        toolName: part.name,
        type: "tool-call",
        ...(part.providerExecuted === undefined
          ? {}
          : { providerExecuted: part.providerExecuted }),
      }
    case "tool-result":
      return part.isFailure
        ? {
            error: part.result,
            toolCallId: part.id,
            toolName: part.name,
            type: "tool-error",
            ...(part.providerExecuted === undefined
              ? {}
              : { providerExecuted: part.providerExecuted }),
          }
        : {
            output: part.result,
            toolCallId: part.id,
            toolName: part.name,
            type: "tool-result",
            ...(part.providerExecuted === undefined
              ? {}
              : { providerExecuted: part.providerExecuted }),
          }
    case "tool-approval-request":
      return {
        toolCallId: part.toolCallId,
        toolName: "unknown",
        type: "tool-approval-request",
      }
    case "file":
      return { type: "file" }
    case "source":
      return { type: "source" }
    case "finish":
      return {
        finishReason: part.reason,
        totalUsage: khalaUsageFromEffectAiUsage(part.usage),
        type: "finish",
      }
    case "error":
      return { error: part.error, type: "error" }
    default:
      return { rawValue: part, type: "raw" }
  }
}

/**
 * Project one Effect AI stream part directly into a `KhalaRuntimeEvent`.
 */
export function khalaRuntimeEventFromEffectAiStreamPart(input: {
  readonly part: EffectAiStreamPartInput
  readonly eventId: string
  readonly threadId: string
  readonly turnId: string
  readonly sequence: number
  readonly observedAt: string
  readonly source?: KhalaRuntimeSource
  readonly messageId?: string
  readonly stepId?: string
  readonly authority?: KhalaRuntimeToolAuthority
  readonly rawEventRef?: string
}): KhalaRuntimeEvent {
  const { part, ...rest } = input
  return khalaRuntimeEventFromAiSdkTextStreamPart({
    ...rest,
    part: khalaAiSdkTextStreamPartFromEffectAiStreamPart(part),
  })
}

/**
 * Collect an Effect AI response-part stream into ordered, decoded
 * `KhalaRuntimeEvent`s plus raw sidecar refs. The stream is materialized
 * before projection; stream failures (for example `AiError`) surface in the
 * returned Effect's error channel.
 */
export function collectKhalaAiSdkCoreEventsFromEffectAiStream<E, R>(
  input: Readonly<{
    stream: Stream.Stream<EffectAiStreamPartInput, E, R>
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
  }>,
): Effect.Effect<KhalaAiSdkCoreRunResult, E, R> {
  return Effect.flatMap(Stream.runCollect(input.stream), parts =>
    Effect.promise(() =>
      collectKhalaAiSdkCoreEventsFromStream({
        stream: asyncIterableOfParts(
          parts.map(khalaAiSdkTextStreamPartFromEffectAiStreamPart),
        ),
        threadId: input.threadId,
        turnId: input.turnId,
        ...(input.source === undefined ? {} : { source: input.source }),
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
      }),
    ))
}

export type KhalaEffectAiLanguageModelOptions = Readonly<{
  model?: unknown
  headers?: Readonly<Record<string, string>>
  provider?: KhalaAiSdkProviderProfile
  providerOptions?: Readonly<Record<string, unknown>>
  tools?: Record<string, unknown>
  streamText?: KhalaAiSdkCoreStreamText
}>

/**
 * Build the Effect AI `LanguageModel` service on top of this package's
 * existing provider transport. The transport stays the injectable
 * Vercel-shaped `streamText` call; its full-stream parts are normalized and
 * re-encoded as Effect AI `Response.StreamPartEncoded` values, which the
 * Effect AI framework then decodes and returns as typed stream parts.
 */
export function makeKhalaEffectAiLanguageModel(
  options: KhalaEffectAiLanguageModelOptions,
): Effect.Effect<LanguageModel.Service> {
  const callStreamText =
    options.streamText ??
    (streamOptions => vercelStreamText(streamOptions as never) as never)

  const openStream = (
    providerOptions: LanguageModel.ProviderOptions,
  ): Stream.Stream<Response.StreamPartEncoded, AiError.AiError> =>
    Stream.unwrap(
      Effect.map(
        Effect.tryPromise({
          catch: cause => khalaEffectAiTransportError("streamText", cause),
          try: async () => {
            const result = await callStreamText(
              buildKhalaAiSdkCoreStreamTextOptions({
                messages: khalaMessagesFromEffectAiPrompt(providerOptions.prompt),
                model: options.model,
                ...(options.headers === undefined ? {} : { headers: options.headers }),
                ...(options.provider === undefined
                  ? {}
                  : { provider: options.provider }),
                ...(options.providerOptions === undefined
                  ? {}
                  : { providerOptions: options.providerOptions }),
                ...(options.tools === undefined ? {} : { tools: options.tools }),
              }),
            )
            return result.stream
          },
        }),
        stream =>
          Stream.fromAsyncIterable(stream, cause =>
            khalaEffectAiTransportError("streamText", cause)).pipe(
            Stream.map(rawPart =>
              effectAiStreamPartEncodedFromKhalaAiSdkTextStreamPart(
                normalizeAiSdkTextStreamPart(rawPart),
              )),
            Stream.filter(
              (part): part is Response.StreamPartEncoded => part !== undefined,
            ),
          ),
      ),
    )

  return LanguageModel.make({
    generateText: providerOptions =>
      collectEffectAiPartsForGenerate(openStream(providerOptions)),
    streamText: openStream,
  })
}

/**
 * Layer form of `makeKhalaEffectAiLanguageModel`. Provider choice for the
 * Effect AI path is a Layer swap (`Model.make(provider, model, layer)` can
 * wrap this layer once real Effect AI provider packages exist).
 */
export function khalaEffectAiLanguageModelLayer(
  options: KhalaEffectAiLanguageModelOptions,
): Layer.Layer<LanguageModel.LanguageModel> {
  return Layer.effect(
    LanguageModel.LanguageModel,
    makeKhalaEffectAiLanguageModel(options),
  )
}

export type KhalaEffectAiRunInput = Readonly<{
  prompt: Prompt.RawInput
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
}>

/**
 * The `LanguageModel.streamText`-shaped run path: stream one model turn
 * through whichever `LanguageModel` Layer is provided and collect it into
 * `KhalaRuntimeEvent`s.
 */
export function runKhalaEffectAiCoreRuntime(
  input: KhalaEffectAiRunInput,
): Effect.Effect<
  KhalaAiSdkCoreRunResult,
  AiError.AiError,
  LanguageModel.LanguageModel
> {
  return collectKhalaAiSdkCoreEventsFromEffectAiStream({
    stream: LanguageModel.streamText({ prompt: input.prompt }),
    threadId: input.threadId,
    turnId: input.turnId,
    ...(input.source === undefined ? {} : { source: input.source }),
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

/**
 * Render the normalized Effect AI prompt into the plain role/content message
 * list the existing Vercel-shaped transport accepts. Text parts are joined;
 * non-text parts and tool messages are outside this bounded adapter and are
 * skipped.
 */
export function khalaMessagesFromEffectAiPrompt(
  prompt: Prompt.Prompt,
): ReadonlyArray<{ readonly role: string; readonly content: string }> {
  const messages: Array<{ readonly role: string; readonly content: string }> = []
  for (const message of prompt.content) {
    if (message.role === "tool") continue
    messages.push({
      content:
        typeof message.content === "string"
          ? message.content
          : message.content
              .map(part =>
                typeof part === "object" &&
                part !== null &&
                "text" in part &&
                part.type === "text" &&
                typeof part.text === "string"
                  ? part.text
                  : "")
              .join(""),
      role: message.role,
    })
  }
  return messages
}

function collectEffectAiPartsForGenerate<R>(
  stream: Stream.Stream<Response.StreamPartEncoded, AiError.AiError, R>,
): Effect.Effect<Array<Response.PartEncoded>, AiError.AiError, R> {
  return Effect.flatMap(Stream.runCollect(stream), parts => {
    const textOrder: Array<string> = []
    const textById = new Map<string, string>()
    const reasoningOrder: Array<string> = []
    const reasoningById = new Map<string, string>()
    const passthrough: Array<Response.PartEncoded> = []
    let errorPart: Response.ErrorPartEncoded | undefined

    for (const part of parts) {
      switch (part.type) {
        case "text-start":
          if (!textById.has(part.id)) {
            textById.set(part.id, "")
            textOrder.push(part.id)
          }
          break
        case "text-delta":
          if (!textById.has(part.id)) textOrder.push(part.id)
          textById.set(part.id, (textById.get(part.id) ?? "") + part.delta)
          break
        case "reasoning-start":
          if (!reasoningById.has(part.id)) {
            reasoningById.set(part.id, "")
            reasoningOrder.push(part.id)
          }
          break
        case "reasoning-delta":
          if (!reasoningById.has(part.id)) reasoningOrder.push(part.id)
          reasoningById.set(
            part.id,
            (reasoningById.get(part.id) ?? "") + part.delta,
          )
          break
        case "tool-call":
        case "tool-result":
        case "tool-approval-request":
        case "file":
        case "source":
        case "response-metadata":
        case "finish":
          passthrough.push(part)
          break
        case "error":
          errorPart = part
          break
        default:
          break
      }
    }

    if (errorPart !== undefined) {
      return Effect.fail(
        AiError.make({
          method: "generateText",
          module: "KhalaAiSdkCore",
          reason: new AiError.UnknownError({
            description: describeUnknownForModel(errorPart.error),
          }),
        }),
      )
    }

    return Effect.succeed([
      ...reasoningOrder.map(
        (id): Response.PartEncoded => ({
          text: reasoningById.get(id) ?? "",
          type: "reasoning",
        }),
      ),
      ...textOrder.map(
        (id): Response.PartEncoded => ({
          text: textById.get(id) ?? "",
          type: "text",
        }),
      ),
      ...passthrough,
    ])
  })
}

function khalaEffectAiTransportError(
  method: string,
  cause: unknown,
): AiError.AiError {
  if (AiError.isAiError(cause)) return cause
  return AiError.make({
    method,
    module: "KhalaAiSdkCore",
    reason: AiError.isAiErrorReason(cause)
      ? cause
      : new AiError.UnknownError({
          description: describeUnknownForModel(cause),
        }),
  })
}

function effectAiFinishReason(
  finishReason: string,
): (typeof Response.FinishReason)["Type"] {
  return effectAiFinishReasons.has(finishReason)
    ? (finishReason as (typeof Response.FinishReason)["Type"])
    : "unknown"
}

function effectAiUsageEncodedFromKhalaUsage(
  usage: KhalaRuntimeAiSdkUsage | undefined,
): EffectAiUsageEncoded {
  return {
    inputTokens: {
      cacheRead: usage?.inputTokenDetails?.cacheReadTokens,
      cacheWrite: usage?.inputTokenDetails?.cacheWriteTokens,
      total: usage?.inputTokens,
      uncached: undefined,
    },
    outputTokens: {
      reasoning: usage?.outputTokenDetails?.reasoningTokens,
      text: undefined,
      total: usage?.outputTokens,
    },
  }
}

function khalaUsageFromEffectAiUsage(
  usage: EffectAiUsageEncoded | Response.Usage,
): KhalaRuntimeAiSdkUsage {
  const inputTotal = usage.inputTokens.total
  const outputTotal = usage.outputTokens.total
  return {
    inputTokenDetails: {
      cacheReadTokens: usage.inputTokens.cacheRead,
      cacheWriteTokens: usage.inputTokens.cacheWrite,
    },
    inputTokens: inputTotal,
    outputTokenDetails: {
      reasoningTokens: usage.outputTokens.reasoning,
    },
    outputTokens: outputTotal,
    totalTokens:
      inputTotal === undefined && outputTotal === undefined
        ? undefined
        : (inputTotal ?? 0) + (outputTotal ?? 0),
  }
}

function describeUnknownForModel(value: unknown): string {
  if (value instanceof Error) return value.message
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value) ?? String(value)
  } catch {
    return String(value)
  }
}

async function* asyncIterableOfParts(
  parts: ReadonlyArray<KhalaRuntimeAiSdkTextStreamPart>,
): AsyncIterable<unknown> {
  for (const part of parts) yield part
}
