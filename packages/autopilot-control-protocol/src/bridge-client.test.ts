import { describe, expect, test } from "bun:test"

import {
  buildHistoryRequest,
  buildListRequest,
  buildSnapshotRequest,
  buildSubscribeRequest,
  parseEventBatch,
  parseListResponse,
} from "./bridge-client.js"
import {
  sessionEventStreamFixture,
  sessionListFixture,
} from "./fixtures.js"

const baseRequest = {
  pairingRef: "pairing.fixture.0001",
  capabilityRef: "capability.fixture.observe_public",
  clientRequestId: "client.request.fixture.0001",
  idempotencyKey: "idem.fixture.0001",
}

describe("bridge client read request builders", () => {
  test("buildListRequest carries verb and bridge refs", () => {
    expect(buildListRequest(baseRequest)).toEqual({
      verb: "session.list",
      ...baseRequest,
    })
  })

  test("buildSubscribeRequest carries sessionRef and cursor", () => {
    expect(buildSubscribeRequest({
      ...baseRequest,
      sessionRef: "session.fixture.0001",
      cursor: 12,
    })).toEqual({
      verb: "session.subscribe",
      ...baseRequest,
      sessionRef: "session.fixture.0001",
      cursor: 12,
    })
  })

  test("buildSnapshotRequest carries the snapshot verb", () => {
    expect(buildSnapshotRequest({
      ...baseRequest,
      sessionRef: "session.fixture.0001",
    })).toEqual({
      verb: "session.snapshot",
      ...baseRequest,
      sessionRef: "session.fixture.0001",
    })
  })

  test("buildHistoryRequest carries sessionRef and cursor", () => {
    expect(buildHistoryRequest({
      ...baseRequest,
      sessionRef: "session.fixture.0001",
      cursor: 3,
    })).toEqual({
      verb: "session.history",
      ...baseRequest,
      sessionRef: "session.fixture.0001",
      cursor: 3,
    })
  })
})

describe("bridge client response parsers", () => {
  test("parseListResponse decodes session summary fixtures", () => {
    expect(parseListResponse(sessionListFixture)).toEqual(sessionListFixture)
  })

  test("parseEventBatch decodes session event fixtures", () => {
    expect(parseEventBatch(sessionEventStreamFixture)).toEqual(sessionEventStreamFixture)
  })
})
