import { describe, expect, test } from "bun:test"

import {
  buildCloudDispatchRequest,
  buildCloudSessionListRequest,
  parseCloudSessionList,
} from "./cloud-coordinator-client.js"
import type { SessionSummary } from "./control.js"

const baseSession: SessionSummary = {
  sessionRef: "cloud.session.fixture.0001",
  adapter: "codex",
  state: "running",
  objectiveRef: "objective.fixture.0001",
  workspaceRef: "workspace.fixture.0001",
  accountRefHash: "account.hash.fixture",
  lastProgressRef: "progress.fixture.0001",
  latestActivity: "running cloud coordinator smoke",
  updatedAt: "2026-06-13T12:00:00.000Z",
}

describe("cloud coordinator request builders", () => {
  test("buildCloudSessionListRequest carries coordinator refs", () => {
    expect(buildCloudSessionListRequest({
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "capability.fixture.cloud",
      clientRequestId: "client.request.fixture.0001",
    })).toEqual({
      verb: "cloud.session.list",
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "capability.fixture.cloud",
      clientRequestId: "client.request.fixture.0001",
      idempotencyKey: "client.request.fixture.0001",
    })
  })

  test("buildCloudDispatchRequest carries objective and idempotency key", () => {
    expect(buildCloudDispatchRequest({
      objective: "ship the shared cloud coordinator client",
      clientRequestId: "client.request.fixture.0002",
    })).toEqual({
      verb: "cloud.dispatch",
      objective: "ship the shared cloud coordinator client",
      clientRequestId: "client.request.fixture.0002",
      idempotencyKey: "client.request.fixture.0002",
    })
  })
})

describe("cloud coordinator session list parser", () => {
  test("parseCloudSessionList accepts session-summary-like rows", () => {
    const rows = [
      baseSession,
      {
        sessionRef: "cloud.session.fixture.0002",
        adapter: "claude_agent",
        state: "completed",
        accountRefHash: null,
        updatedAt: "2026-06-13T12:05:00.000Z",
      },
    ]

    expect(parseCloudSessionList(rows)).toEqual(rows as ReturnType<typeof parseCloudSessionList>)
  })

  test("parseCloudSessionList preserves optional external-agent fields", () => {
    const row = {
      ...baseSession,
      parentRef: "session.parent.fixture",
      agentKind: "external",
      pylonManaged: false,
    }

    expect(parseCloudSessionList([row])).toEqual([row])
  })

  test("parseCloudSessionList rejects non-array payloads", () => {
    expect(() => parseCloudSessionList({ sessions: [baseSession] })).toThrow(
      "Expected cloud coordinator session list response to be an array",
    )
  })

  test("parseCloudSessionList rejects non-object rows", () => {
    expect(() => parseCloudSessionList(["cloud.session.fixture.0001"])).toThrow(
      "Expected cloud session to be an object",
    )
  })

  test("parseCloudSessionList rejects invalid adapters", () => {
    expect(() => parseCloudSessionList([{ ...baseSession, adapter: "unknown" }])).toThrow(
      "Expected cloud session adapter to be valid",
    )
  })

  test("parseCloudSessionList rejects invalid states", () => {
    expect(() => parseCloudSessionList([{ ...baseSession, state: "lost" }])).toThrow(
      "Expected cloud session state to be valid",
    )
  })

  test("parseCloudSessionList rejects malformed optional fields", () => {
    expect(() => parseCloudSessionList([{ ...baseSession, latestActivity: 42 }])).toThrow(
      "Expected cloud session latestActivity to be a string",
    )
  })
})
