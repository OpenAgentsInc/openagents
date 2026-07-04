import { describe, expect, test } from "bun:test"
import {
  FLEET_INTENTS_ROUTE_PATH,
  readPendingFleetIntents,
} from "./fleet-intents.js"

const okBody = {
  intents: [
    {
      createdAt: "2026-07-04T15:20:11.412Z",
      desiredSlots: 4,
      flagRef: null,
      id: 11,
      intent: "set_desired_slots",
      mutationRef: "mutation:cg-1:c-1:1",
      requestedByUserId: "user-1",
      runId: "fleet-run.pylon.supervisor.abc123",
      scope: "scope.fleet_run.fleet-run.pylon.supervisor.abc123",
      workerId: null,
    },
    {
      createdAt: "2026-07-04T15:21:00.000Z",
      desiredSlots: null,
      flagRef: null,
      id: 12,
      intent: "pause_worker",
      mutationRef: "mutation:cg-1:c-1:2",
      requestedByUserId: "user-1",
      runId: "fleet-run.pylon.supervisor.abc123",
      scope: "scope.fleet_run.fleet-run.pylon.supervisor.abc123",
      workerId: "dispatch-context.pylon.supervisor.9ab31c44",
    },
  ],
  nextAfter: 12,
  ok: true,
  routeRef: "route.internal.khala_sync.fleet_intents.v0_1",
  upToDate: true,
}

const fakeFetch = (
  handler: (url: string, init: RequestInit | undefined) => Response,
): { calls: Array<{ url: string; init: RequestInit | undefined }>; fetchImpl: typeof globalThis.fetch } => {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = []
  const fetchImpl = ((input: string | URL | Request, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
    calls.push({ init, url })
    return Promise.resolve(handler(url, init))
  }) as typeof globalThis.fetch
  return { calls, fetchImpl }
}

describe("readPendingFleetIntents (pylon poller)", () => {
  test("polls the internal route with the admin bearer and decodes typed rows", async () => {
    const { calls, fetchImpl } = fakeFetch(() =>
      new Response(JSON.stringify(okBody), {
        headers: { "content-type": "application/json" },
        status: 200,
      }),
    )
    const result = await readPendingFleetIntents({
      adminToken: "admin-secret",
      after: 10,
      baseUrl: "https://openagents.com",
      fetchImpl,
      limit: 50,
      scope: "scope.fleet_run.fleet-run.pylon.supervisor.abc123",
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.intents.map((i) => i.intent)).toEqual([
        "set_desired_slots",
        "pause_worker",
      ])
      expect(result.intents[1]!.workerId).toBe(
        "dispatch-context.pylon.supervisor.9ab31c44",
      )
      expect(result.nextAfter).toBe(12)
      expect(result.upToDate).toBe(true)
    }
    expect(calls).toHaveLength(1)
    const requested = new URL(calls[0]!.url)
    expect(requested.pathname).toBe(FLEET_INTENTS_ROUTE_PATH)
    expect(requested.searchParams.get("after")).toBe("10")
    expect(requested.searchParams.get("limit")).toBe("50")
    expect(requested.searchParams.get("scope")).toBe(
      "scope.fleet_run.fleet-run.pylon.supervisor.abc123",
    )
    expect(
      new Headers(calls[0]!.init?.headers).get("authorization"),
    ).toBe("Bearer admin-secret")
  })

  test("typed failures: 401 / 400 / 503 / non-JSON / enablement gap / bad shape", async () => {
    const cases: Array<{
      response: Response
      error: string
    }> = [
      {
        error: "unauthorized",
        response: new Response("{}", { status: 401 }),
      },
      {
        error: "invalid_request",
        response: new Response("{}", { status: 400 }),
      },
      {
        error: "storage_unavailable",
        response: new Response("{}", { status: 503 }),
      },
      {
        error: "bad_response",
        response: new Response("not json", { status: 200 }),
      },
      {
        error: "not_enabled",
        response: new Response(
          JSON.stringify({ ok: false, reason: "binding absent" }),
          { status: 200 },
        ),
      },
      {
        error: "bad_response",
        response: new Response(JSON.stringify({ ok: true }), { status: 200 }),
      },
      {
        error: "bad_response",
        response: new Response(
          JSON.stringify({
            intents: [{ id: "not-a-row" }],
            nextAfter: 1,
            ok: true,
          }),
          { status: 200 },
        ),
      },
    ]
    for (const item of cases) {
      const { fetchImpl } = fakeFetch(() => item.response)
      const result = await readPendingFleetIntents({
        adminToken: "admin-secret",
        baseUrl: "https://openagents.com",
        fetchImpl,
      })
      expect(result.ok).toBe(false)
      if (!result.ok) {
        expect(result.error).toBe(item.error as never)
        expect(JSON.stringify(result)).not.toContain("admin-secret")
      }
    }
  })

  test("network failure never throws and never leaks the token", async () => {
    const fetchImpl = (() =>
      Promise.reject(new Error("connect ECONNREFUSED"))) as unknown as typeof globalThis.fetch
    const result = await readPendingFleetIntents({
      adminToken: "admin-secret",
      baseUrl: "https://openagents.com",
      fetchImpl,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error).toBe("network_failed")
      expect(JSON.stringify(result)).not.toContain("admin-secret")
    }
  })
})
