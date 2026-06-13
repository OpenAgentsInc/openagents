import {
  decodeSessionEvent,
  type SessionEvent,
} from "@openagentsinc/autopilot-control-protocol"

export type ParsedSessionEventChunk = {
  events: SessionEvent[]
  remainder: string
}

export function parseSessionEventStream(raw: string): SessionEvent[] {
  return parseSessionEventStreamChunk({ buffer: "", chunk: raw, flush: true }).events
}

export function parseSessionEventStreamChunk(input: {
  buffer: string
  chunk: string
  flush?: boolean
}): ParsedSessionEventChunk {
  const normalized = `${input.buffer}${input.chunk}`.replaceAll("\r\n", "\n").replaceAll("\r", "\n")
  const frames = normalized.split("\n\n")
  const remainder = input.flush === true ? "" : (frames.pop() ?? "")

  return {
    events: frames.flatMap(parseFrame),
    remainder,
  }
}

export function parseSessionEventStreamChunks(chunks: string[]): SessionEvent[] {
  let buffer = ""
  const events: SessionEvent[] = []

  for (const chunk of chunks) {
    const parsed = parseSessionEventStreamChunk({ buffer, chunk })
    buffer = parsed.remainder
    events.push(...parsed.events)
  }

  const flushed = parseSessionEventStreamChunk({ buffer, chunk: "", flush: true })
  events.push(...flushed.events)
  return events
}

function parseFrame(frame: string): SessionEvent[] {
  const dataLines = frame
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trimStart())

  if (dataLines.length === 0) return []

  return [decodeSessionEvent(JSON.parse(dataLines.join("\n")))]
}
