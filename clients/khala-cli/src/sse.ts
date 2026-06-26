import { Schema as S } from "effect"
import {
  KhalaCliError,
  OpenAiStreamPayload,
  PublicDeltaPayload,
  PublicDonePayload,
  PublicErrorPayload,
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

export function decodePublicFrame(frame: SseFrame): { readonly kind: "delta"; readonly text: string } | { readonly kind: "done" } {
  const data = parseJson(frame.data, "public Khala stream frame")
  if (frame.event === "delta") {
    const payload = S.decodeUnknownSync(PublicDeltaPayload)(data)
    return { kind: "delta", text: payload.text }
  }
  if (frame.event === "done") {
    S.decodeUnknownSync(PublicDonePayload)(data)
    return { kind: "done" }
  }
  if (frame.event === "error") {
    const payload = S.decodeUnknownSync(PublicErrorPayload)(data)
    throw new KhalaCliError({ reason: payload.error, code: payload.code ?? "stream_error" })
  }
  throw new KhalaCliError({ reason: `Unknown public Khala stream event: ${frame.event ?? "(none)"}`, code: "unknown_stream_event" })
}

export function decodeOpenAiFrame(frame: SseFrame): { readonly kind: "delta"; readonly text: string } | { readonly kind: "done" } {
  if (frame.data.trim() === "[DONE]") {
    return { kind: "done" }
  }
  const data = parseJson(frame.data, "OpenAI-compatible stream frame")
  const payload = S.decodeUnknownSync(OpenAiStreamPayload)(data)
  const text = payload.choices.map((choice) => choice.delta.content ?? "").join("")
  return { kind: "delta", text }
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
