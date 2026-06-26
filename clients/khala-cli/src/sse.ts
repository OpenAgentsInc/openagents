import { Schema as S } from "effect"
import {
  KhalaCliError,
  OpenAiStreamPayload,
  PublicDeltaPayload,
  PublicDonePayload,
  PublicErrorPayload,
  PublicMetaPayload,
  PublicReasoningPayload,
  type KhalaStreamUsage,
  type PublicMetaPayload as PublicMetaPayloadType,
} from "./types.js"

export interface SseFrame {
  readonly event?: string
  readonly data: string
}

export function parseSseFramesFromText(input: string): ReadonlyArray<SseFrame> {
  const normalized = input.replace(/\r\n/g, "\n").replace(/\r/g, "\n")
  const blocks = normalized.split(/\n\n+/)
  return blocks.flatMap((block) => {
    const frame = parseSseBlock(block)
    return frame === null ? [] : [frame]
  })
}

export async function* readSseFrames(body: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame> {
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
    const finalFrame = parseSseBlock(buffer)
    if (finalFrame !== null) yield finalFrame
  } finally {
    reader.releaseLock()
  }
}

export type DecodedStreamFrame =
  | { readonly kind: "delta"; readonly metadata?: StreamFrameMetadata | undefined; readonly reasoningText?: string | undefined; readonly text: string }
  | { readonly kind: "done" }
  | { readonly kind: "meta"; readonly metadata: StreamFrameMetadata }

export interface StreamFrameMetadata {
  readonly adapterRouteMetadata?: unknown
  readonly fallbackReason?: string | null | undefined
  readonly finishReason?: string | undefined
  readonly id?: string | undefined
  readonly primaryAdapterId?: string | undefined
  readonly requestedModel?: string | undefined
  readonly servedAdapterId?: string | undefined
  readonly servedModel?: string | undefined
  readonly traceRef?: string | undefined
  readonly traceUrl?: string | undefined
  readonly traceUuid?: string | undefined
  readonly usage?: KhalaStreamUsage | undefined
}

export function decodePublicFrame(frame: SseFrame): DecodedStreamFrame {
  const data = parseJson(frame.data, "public Khala stream frame")
  if (frame.event === "delta") {
    const payload = S.decodeUnknownSync(PublicDeltaPayload)(data)
    return { kind: "delta", text: payload.text }
  }
  if (frame.event === "reasoning") {
    const payload = S.decodeUnknownSync(PublicReasoningPayload)(data)
    return { kind: "delta", reasoningText: payload.text, text: "" }
  }
  if (frame.event === "meta") {
    const payload = S.decodeUnknownSync(PublicMetaPayload)(data)
    return { kind: "meta", metadata: publicMetadata(payload) }
  }
  if (frame.event === "done") {
    S.decodeUnknownSync(PublicDonePayload)(data)
    return { kind: "done" }
  }
  if (frame.event === "error") {
    const payload = S.decodeUnknownSync(PublicErrorPayload)(data)
    throw new KhalaCliError({
      reason: payload.reason ?? payload.error,
      code: payload.code ?? "stream_error",
      traceRef: payload.traceRef,
    })
  }
  throw new KhalaCliError({ reason: `Unknown public Khala stream event: ${frame.event ?? "(none)"}`, code: "unknown_stream_event" })
}

export function decodeOpenAiFrame(frame: SseFrame): DecodedStreamFrame {
  if (frame.data.trim() === "[DONE]") {
    return { kind: "done" }
  }
  const data = parseJson(frame.data, "OpenAI-compatible stream frame")
  const payload = S.decodeUnknownSync(OpenAiStreamPayload)(data)
  const text = payload.choices.map((choice) => choice.delta.content ?? "").join("")
  const reasoningText = payload.choices
    .map((choice) => choice.delta.reasoning_content ?? choice.delta.reasoning ?? "")
    .join("")
  const openagents = extractOpenAgentsReceipt(payload.openagents)
  return {
    kind: "delta",
    metadata: {
      adapterRouteMetadata: openagents?.routing,
      fallbackReason: openagents?.fallbackReason,
      id: payload.id,
      primaryAdapterId: openagents?.primaryAdapterId,
      requestedModel: payload.model,
      servedAdapterId: openagents?.servedAdapterId,
      servedModel: openagents?.servedModel ?? payload.model,
      usage: payload.usage === undefined
        ? undefined
        : {
            cachedPromptTokens: payload.usage.cached_tokens,
            completionTokens: payload.usage.completion_tokens,
            promptTokens: payload.usage.prompt_tokens,
            totalTokens: payload.usage.total_tokens,
          },
    },
    ...(reasoningText.length === 0 ? {} : { reasoningText }),
    text,
  }
}

function extractOpenAgentsReceipt(value: unknown): {
  readonly fallbackReason?: string | null | undefined
  readonly primaryAdapterId?: string | undefined
  readonly routing?: unknown
  readonly servedAdapterId?: string | undefined
  readonly servedModel?: string | undefined
} | undefined {
  if (value === null || typeof value !== "object") return undefined
  const record = value as Record<string, unknown>
  const routing = record.routing
  const routingRecord = routing !== null && typeof routing === "object"
    ? routing as Record<string, unknown>
    : undefined
  const fallbackReason = routingRecord?.fallback_reason
  return {
    ...(typeof fallbackReason === "string" || fallbackReason === null
      ? { fallbackReason }
      : {}),
    primaryAdapterId: typeof record.primary_worker === "string" ? record.primary_worker : undefined,
    routing,
    servedAdapterId: typeof record.worker === "string" ? record.worker : undefined,
    servedModel: typeof record.served_model === "string" ? record.served_model : undefined,
  }
}

function publicMetadata(payload: PublicMetaPayloadType): StreamFrameMetadata {
  return {
    adapterRouteMetadata: payload.adapterRouteMetadata,
    fallbackReason: payload.fallbackReason,
    finishReason: payload.finishReason,
    primaryAdapterId: payload.primaryAdapterId,
    requestedModel: payload.requestedModel,
    servedAdapterId: payload.servedAdapterId,
    servedModel: payload.servedModel,
    traceRef: payload.traceRef,
    traceUrl: payload.traceUrl,
    traceUuid: payload.traceUuid,
    usage: payload.usage,
  }
}

function parseSseBlock(block: string): SseFrame | null {
  const lines = block.split("\n")
  let event: string | undefined
  const data: Array<string> = []

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

function parseJson(data: string, label: string): unknown {
  try {
    return JSON.parse(data)
  } catch (error) {
    throw new KhalaCliError({
      reason: `Malformed ${label}: ${error instanceof Error ? error.message : String(error)}`,
      code: "malformed_stream_json",
    })
  }
}
