import { describe, expect, test } from "bun:test"

import { sessionEventStreamFixture } from "@openagentsinc/autopilot-control-protocol/fixtures"

import {
  parseSessionEventStream,
  parseSessionEventStreamChunk,
  parseSessionEventStreamChunks,
} from "./session-stream"

describe("session stream parser", () => {
  test("decodes shared fixture events from raw SSE text", () => {
    const raw = sessionEventStreamFixture.map((event) => `event: session\ndata: ${JSON.stringify(event)}\n\n`).join("")

    expect(parseSessionEventStream(raw)).toEqual(sessionEventStreamFixture)
  })

  test("decodes shared fixture events split across chunks", () => {
    const raw = sessionEventStreamFixture.map((event) => `id: ${event.eventId}\ndata: ${JSON.stringify(event)}\n\n`).join("")
    const chunks = [raw.slice(0, 17), raw.slice(17, 121), raw.slice(121)]

    expect(parseSessionEventStreamChunks(chunks)).toEqual(sessionEventStreamFixture)
  })

  test("retains an incomplete frame until more data arrives", () => {
    const firstEvent = sessionEventStreamFixture[0]!
    const raw = `data: ${JSON.stringify(firstEvent)}`

    const pending = parseSessionEventStreamChunk({ buffer: "", chunk: raw })
    expect(pending.events).toEqual([])
    expect(pending.remainder).toBe(raw)

    const complete = parseSessionEventStreamChunk({ buffer: pending.remainder, chunk: "\n\n" })
    expect(complete.events).toEqual([firstEvent])
    expect(complete.remainder).toBe("")
  })

  test("rejects data that is not a protocol session event", () => {
    expect(() => parseSessionEventStream("data: {\"type\":\"not-session-event\"}\n\n")).toThrow()
  })
})
