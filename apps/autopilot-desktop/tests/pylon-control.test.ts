import { describe, expect, test } from "bun:test"
import {
  healthFixture,
  sessionListFixture,
} from "@openagentsinc/autopilot-control-protocol/fixtures"
import { fetchNodeState, probeControlToken, readControlToken } from "../src/bun/pylon-control.ts"

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

  // CL-45b: the control-server token probe used to fall through a stale token.
  describe("probeControlToken", () => {
    const baseUrl = "http://127.0.0.1:4716"

    test("rejects a 401 (stale/wrong token)", async () => {
      const fetchFn: typeof fetch = async (input, init) => {
        if (String(input).endsWith("/health")) return Response.json(healthFixture)
        expect(String(input)).toBe(`${baseUrl}/command`)
        expect(init?.method).toBe("POST")
        expect((init?.headers as Record<string, string>).Authorization).toBe("Bearer stale")
        expect(JSON.parse(String(init?.body))).toEqual({ type: "session.list" })
        return new Response("unauthorized", { status: 401 })
      }
      expect(await probeControlToken({ baseUrl, token: "stale", fetchFn })).toBe(false)
    })

    test("accepts a 200 (server authenticated the token)", async () => {
      const fetchFn: typeof fetch = async (input) =>
        String(input).endsWith("/health")
          ? Response.json(healthFixture)
          : Response.json({ ok: true, result: sessionListFixture })
      expect(await probeControlToken({ baseUrl, token: "good", fetchFn })).toBe(true)
    })

    test("accepts any non-401 status (e.g. 500) — reachable + authenticated", async () => {
      const fetchFn: typeof fetch = async (input) =>
        String(input).endsWith("/health")
          ? Response.json(healthFixture)
          : new Response("boom", { status: 500 })
      expect(await probeControlToken({ baseUrl, token: "good", fetchFn })).toBe(true)
    })

    test("rejects an otherwise reachable old node without desktop capabilities", async () => {
      const fetchFn: typeof fetch = async (input) =>
        String(input).endsWith("/health")
          ? Response.json({ ok: true, schema: "openagents.pylon.control.v0.3" })
          : Response.json({ ok: true, result: sessionListFixture })
      expect(await probeControlToken({ baseUrl, token: "good", fetchFn })).toBe(false)
    })

    test("treats a transport error as not-accepted", async () => {
      const fetchFn: typeof fetch = async () => {
        throw new Error("ECONNREFUSED")
      }
      expect(await probeControlToken({ baseUrl, token: "good", fetchFn })).toBe(false)
    })

    test("never echoes the token in any thrown/returned value", async () => {
      const fetchFn: typeof fetch = async () => new Response(null, { status: 401 })
      const result = await probeControlToken({ baseUrl, token: "super-secret", fetchFn })
      expect(JSON.stringify(result)).not.toContain("super-secret")
    })
  })
})
