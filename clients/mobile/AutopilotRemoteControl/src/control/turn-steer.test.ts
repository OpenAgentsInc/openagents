// G4 (#5496): mobile sends chat/continuation turns over the capability-scoped
// bridge `turn.steer` verb, with a dev-token `session.reply` fallback for older
// nodes. These tests mock fetch so they run without a node.

import { afterEach, describe, expect, test } from "bun:test"

import { createBridgeTransport } from "@openagentsinc/autopilot-control-protocol"

import {
  steerTurn,
  steerTurnViaBridge,
  type BridgeSession,
} from "./control-client"

const realFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = realFetch
})

function bridgeFor(impl: typeof fetch): BridgeSession {
  const transport = createBridgeTransport({
    baseUrl: "https://node.example",
    credential: { pairingRef: "p1", jti: "j1", capabilityRef: "send_instruction" },
    fetchImpl: impl,
  })
  return {
    transport,
    credential: { pairingRef: "p1", jti: "j1", capabilityRef: "send_instruction" },
    baseUrl: "https://node.example",
  }
}

describe("steerTurnViaBridge", () => {
  test("posts turn.steer with the parent session and instruction", async () => {
    let body: any = null
    const impl = (async (_url: string, init?: RequestInit) => {
      body = JSON.parse(String(init!.body))
      return new Response(
        JSON.stringify({
          ok: true,
          result: { sessionRef: "session.child.1", parentSessionRef: "session.parent.1", state: "queued" },
        }),
        { status: 200 },
      )
    }) as unknown as typeof fetch

    const result = await steerTurnViaBridge(bridgeFor(impl), {
      sessionRef: "session.parent.1",
      instruction: "continue with the regression test",
      timeoutSeconds: 120,
    })
    expect(body).toMatchObject({
      verb: "turn.steer",
      pairingRef: "p1",
      capabilityRef: "send_instruction",
      sessionRef: "session.parent.1",
      instruction: "continue with the regression test",
      timeoutSeconds: 120,
    })
    expect(result).toEqual({
      sessionRef: "session.child.1",
      parentSessionRef: "session.parent.1",
      state: "queued",
    })
  })
})

describe("steerTurn", () => {
  test("falls back to the dev-token session.reply command shape", async () => {
    let body: any = null
    globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
      body = JSON.parse(String(init!.body))
      return new Response(
        JSON.stringify({
          ok: true,
          result: { sessionRef: "session.child.2", parentSessionRef: "session.parent.2", state: "queued" },
        }),
        { status: 200 },
      )
    }) as typeof fetch

    const result = await steerTurn(
      { baseUrl: "https://node.example", token: "tok" },
      { sessionRef: "session.parent.2", instruction: "ship the follow-up" },
    )
    expect(body).toEqual({
      type: "session.reply",
      sessionRef: "session.parent.2",
      objective: "ship the follow-up",
    })
    expect(result.sessionRef).toBe("session.child.2")
  })
})
