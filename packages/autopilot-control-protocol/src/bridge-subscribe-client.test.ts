import { describe, expect, test } from "bun:test"

import { buildSubscribeEnvelope, parseBridgeEventBatch, parseEventBatchResponse } from "./bridge-subscribe-client.js"
import { CONTROL_SCHEMA_TAG } from "./control.js"
import { sessionEventStreamFixture } from "./fixtures.js"

const baseInput = {
  pairingRef: "pairing.fixture.0001",
  capabilityRef: "capability.fixture.observe_public",
  sessionRef: "session.fixture.0001",
  clientRequestId: "client.request.fixture.0001",
}

describe("bridge subscribe client", () => {
  test("buildSubscribeEnvelope builds a session.subscribe envelope", () => {
    expect(buildSubscribeEnvelope(baseInput)).toEqual({
      verb: "session.subscribe",
      ...baseInput,
      idempotencyKey: baseInput.clientRequestId,
    })
  })

  test("buildSubscribeEnvelope includes cursor when supplied", () => {
    expect(buildSubscribeEnvelope({ ...baseInput, cursor: 12 })).toEqual({
      verb: "session.subscribe",
      ...baseInput,
      cursor: 12,
      idempotencyKey: baseInput.clientRequestId,
    })
  })

  test("buildSubscribeEnvelope omits cursor when not supplied", () => {
    expect(buildSubscribeEnvelope(baseInput)).not.toHaveProperty("cursor")
  })

  test("parseEventBatchResponse decodes event batch fixtures", () => {
    expect(parseEventBatchResponse(sessionEventStreamFixture)).toEqual(sessionEventStreamFixture)
  })

  test("parseEventBatchResponse accepts an empty event batch", () => {
    expect(parseEventBatchResponse([])).toEqual([])
  })

  test("parseEventBatchResponse rejects non-array responses", () => {
    expect(() => parseEventBatchResponse({ events: sessionEventStreamFixture })).toThrow(
      "Expected event batch response to be an array",
    )
  })

  test("parseBridgeEventBatch extracts the node events() projection tail", () => {
    const batch = parseBridgeEventBatch({
      sessionRef: "session.fixture.0001",
      eventsPath: "/sessions/session.fixture.0001/events",
      state: "running",
      recentEvents: [
        { sessionRef: "session.fixture.0001", eventIndex: 0, phase: "started", state: "running", observedAt: "t0" },
        {
          sessionRef: "session.fixture.0001",
          eventIndex: 1,
          phase: "composer_event",
          state: "running",
          observedAt: "t1",
          messageText: "editing file",
        },
      ],
    })
    expect(batch.sessionRef).toBe("session.fixture.0001")
    expect(batch.state).toBe("running")
    expect(batch.events.length).toBe(2)
    expect(batch.events[1]?.messageText).toBe("editing file")
    expect(batch.cursor).toBe(1)
  })

  test("parseBridgeEventBatch resumes from a cursor and dedups already-seen rows", () => {
    const projection = {
      sessionRef: "session.fixture.0001",
      state: "running",
      recentEvents: [
        { eventIndex: 0, phase: "started", state: "running", observedAt: "t0" },
        { eventIndex: 1, phase: "composer_event", state: "running", observedAt: "t1" },
        { eventIndex: 2, phase: "completed", state: "completed", observedAt: "t2", artifactRef: "art.1", resultRef: "rcpt.1" },
      ],
    }
    const batch = parseBridgeEventBatch(projection, 1)
    expect(batch.events.map((e) => e.eventIndex)).toEqual([2])
    expect(batch.events[0]?.artifactRef).toBe("art.1")
    expect(batch.events[0]?.resultRef).toBe("rcpt.1")
    expect(batch.cursor).toBe(2)
  })

  test("parseBridgeEventBatch passes through unknown phases (e.g. future cloud lane events) and sorts ascending", () => {
    const batch = parseBridgeEventBatch({
      sessionRef: "session.cloud.0001",
      state: "running",
      recentEvents: [
        { eventIndex: 5, phase: "cloud.gce.provisioned", state: "running", observedAt: "t5" },
        { eventIndex: 4, phase: "cloud.gce.leased", state: "running", observedAt: "t4" },
      ],
    })
    expect(batch.events.map((e) => e.phase)).toEqual(["cloud.gce.leased", "cloud.gce.provisioned"])
    expect(batch.cursor).toBe(5)
  })

  test("parseBridgeEventBatch renders cloud lane events with the same timeline row shape as local composer events", () => {
    const local = parseBridgeEventBatch({
      sessionRef: "session.local.0001",
      state: "running",
      recentEvents: [
        {
          eventIndex: 1,
          phase: "composer_event",
          state: "running",
          observedAt: "t1",
          messageText: "editing file",
          artifactRef: "artifact.local.diff",
          resultRef: "receipt.local.usage",
        },
      ],
    })
    const cloud = parseBridgeEventBatch({
      sessionRef: "session.cloud.0001",
      state: "running",
      recentEvents: [
        {
          eventIndex: 1,
          phase: "composer_event",
          state: "running",
          observedAt: "t1",
          messageText: "cloud GCE VM provisioned",
          artifactRef: "artifact.cloud.diff",
          resultRef: "receipt.cloud.gce.resource_usage",
        },
      ],
    })

    expect(Object.keys(cloud.events[0]!).sort()).toEqual(Object.keys(local.events[0]!).sort())
    expect(cloud.events[0]).toMatchObject({
      sessionRef: "session.cloud.0001",
      eventIndex: 1,
      phase: "composer_event",
      state: "running",
      messageText: "cloud GCE VM provisioned",
      artifactRef: "artifact.cloud.diff",
      resultRef: "receipt.cloud.gce.resource_usage",
    })
  })

  test("parseBridgeEventBatch tolerates a malformed/empty projection", () => {
    expect(parseBridgeEventBatch(null)).toEqual({ sessionRef: "", state: "unknown", events: [], cursor: -1 })
    expect(parseBridgeEventBatch({ sessionRef: "s", state: "running" }, 7).cursor).toBe(7)
  })

  test("parseEventBatchResponse rejects malformed events", () => {
    expect(() => parseEventBatchResponse([
      {
        schema: CONTROL_SCHEMA_TAG,
        sessionRef: "session.fixture.0001",
        eventId: "evt.fixture.bad",
        sequence: 1,
        phase: "not_a_phase",
        projectionLevel: "public_safe",
        observedAt: "2026-06-13T12:00:00.000Z",
      },
    ])).toThrow()
  })
})
