import { describe, expect, test } from "bun:test"
import {
  healthFixture,
  sessionListFixture,
} from "@openagentsinc/autopilot-control-protocol/fixtures"
import { fetchNodeState, readControlToken } from "../src/bun/pylon-control.ts"

const bearerToken = "local-bearer-token-fixture"

describe("Pylon control client", () => {
  test("fetches health and decodes session list rows", async () => {
    const fetchFn: typeof fetch = async (input, init) => {
      const url = String(input)
      if (url.endsWith("/health")) return Response.json(healthFixture)
      if (url.endsWith("/command")) {
        expect(init?.method).toBe("POST")
        expect((init?.headers as Record<string, string>).Authorization).toBe(`Bearer ${bearerToken}`)
        expect(JSON.parse(String(init?.body))).toEqual({ type: "session.list" })
        return Response.json({ ok: true, result: sessionListFixture })
      }
      return Response.json({ error: "not found" }, { status: 404 })
    }

    const state = await fetchNodeState({
      baseUrl: "http://127.0.0.1:4716",
      token: bearerToken,
      fetchFn,
    })

    expect(state.ok).toBe(true)
    expect(state.schema).toBe(healthFixture.schema)
    expect(state.sessions).toHaveLength(2)
    expect(state.sessions.map((session) => session.sessionRef)).toEqual(
      sessionListFixture.map((session) => session.sessionRef),
    )

    // The decoded node-state projection (what crosses the RPC to the webview)
    // must never carry the bearer token used to fetch it.
    expect(JSON.stringify(state)).not.toContain(bearerToken)
  })

  test("returns null for a missing control token path", () => {
    expect(readControlToken("/tmp/openagents-autopilot-desktop-missing-token")).toBeNull()
  })
})
