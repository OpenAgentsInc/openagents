import { describe, expect, test } from "bun:test"

import { buildSubscribeEnvelope, parseEventBatchResponse } from "./bridge-subscribe-client"
import { CONTROL_SCHEMA_TAG } from "./control"
import { sessionEventStreamFixture } from "./fixtures"

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
