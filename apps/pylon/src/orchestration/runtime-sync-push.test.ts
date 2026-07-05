import { describe, expect, test } from "bun:test"
import {
  KHALA_SYNC_PUSH_ROUTE_PATH,
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  pushKhalaSyncMutation,
  runtimeSyncClientForTurn,
} from "./runtime-sync-push.js"

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

describe("pushKhalaSyncMutation", () => {
  test("posts the wire-shaped PushRequest with the agent bearer and the named mutator", async () => {
    const { calls, fetchImpl } = fakeFetch(() =>
      new Response(
        JSON.stringify({
          lastMutationId: 1,
          protocolVersion: 1,
          results: [{ mutationId: 1, status: "applied" }],
        }),
        { status: 200 },
      ),
    )
    const result = await pushKhalaSyncMutation({
      agentToken: "agent-secret",
      args: { kind: "turn.started" },
      baseUrl: "https://openagents.com",
      clientGroupId: "cg-1",
      clientId: "c-1",
      fetchImpl,
      mutationId: 1,
      name: RUNTIME_RECORD_EVENT_MUTATOR_NAME,
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.result.status).toBe("applied")
      expect(result.result.mutationId).toBe(1)
    }
    expect(calls).toHaveLength(1)
    expect(new URL(calls[0]!.url).pathname).toBe(KHALA_SYNC_PUSH_ROUTE_PATH)
    expect(new Headers(calls[0]!.init?.headers).get("authorization")).toBe("Bearer agent-secret")
    const body = JSON.parse(String(calls[0]!.init?.body)) as {
      clientGroupId: string
      clientId: string
      mutations: Array<{ name: string; mutationId: number; argsJson: string }>
    }
    expect(body.clientGroupId).toBe("cg-1")
    expect(body.clientId).toBe("c-1")
    expect(body.mutations).toHaveLength(1)
    expect(body.mutations[0]!.name).toBe(RUNTIME_RECORD_EVENT_MUTATOR_NAME)
    expect(body.mutations[0]!.mutationId).toBe(1)
    expect(JSON.parse(body.mutations[0]!.argsJson)).toEqual({ kind: "turn.started" })
  })

  test("surfaces a rejected mutation result in-band as ok:true", async () => {
    const { fetchImpl } = fakeFetch(() =>
      new Response(
        JSON.stringify({
          lastMutationId: 0,
          protocolVersion: 1,
          results: [{ errorCode: "runtime_turn_not_found", errorMessageSafe: "no such turn", mutationId: 1, status: "rejected" }],
        }),
        { status: 200 },
      ),
    )
    const result = await pushKhalaSyncMutation({
      agentToken: "agent-secret",
      args: {},
      baseUrl: "https://openagents.com",
      clientGroupId: "cg-1",
      clientId: "c-1",
      fetchImpl,
      mutationId: 1,
      name: RUNTIME_RECORD_EVENT_MUTATOR_NAME,
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.result.status).toBe("rejected")
      expect(result.result.errorCode).toBe("runtime_turn_not_found")
    }
  })

  test("typed failures: 401 / 400 / bad response never leak the token", async () => {
    const cases: Array<{ response: Response; error: string }> = [
      { error: "unauthorized", response: new Response("{}", { status: 401 }) },
      {
        error: "invalid_request",
        response: new Response(JSON.stringify({ reason: "bad args" }), { status: 400 }),
      },
      { error: "bad_response", response: new Response("not json", { status: 200 }) },
      { error: "bad_response", response: new Response(JSON.stringify({ results: [] }), { status: 200 }) },
    ]
    for (const item of cases) {
      const { fetchImpl } = fakeFetch(() => item.response)
      const result = await pushKhalaSyncMutation({
        agentToken: "agent-secret",
        args: {},
        baseUrl: "https://openagents.com",
        clientGroupId: "cg-1",
        clientId: "c-1",
        fetchImpl,
        mutationId: 1,
        name: RUNTIME_RECORD_EVENT_MUTATOR_NAME,
      })
      expect(result.ok).toBe(false)
      if (!result.ok) {
        expect(result.error).toBe(item.error as never)
        expect(JSON.stringify(result)).not.toContain("agent-secret")
      }
    }
  })

  test("network failure never throws and never leaks the token", async () => {
    const fetchImpl = (() => Promise.reject(new Error("connect ECONNREFUSED"))) as unknown as typeof globalThis.fetch
    const result = await pushKhalaSyncMutation({
      agentToken: "agent-secret",
      args: {},
      baseUrl: "https://openagents.com",
      clientGroupId: "cg-1",
      clientId: "c-1",
      fetchImpl,
      mutationId: 1,
      name: RUNTIME_RECORD_EVENT_MUTATOR_NAME,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error).toBe("network_failed")
      expect(JSON.stringify(result)).not.toContain("agent-secret")
    }
  })
})

describe("runtimeSyncClientForTurn", () => {
  test("produces a fresh clientGroupId/clientId pair scoped to the Pylon and turn", () => {
    const a = runtimeSyncClientForTurn({ pylonRef: "pylon.fixture.1", random: "aaa", turnId: "turn-1" })
    const b = runtimeSyncClientForTurn({ pylonRef: "pylon.fixture.1", random: "bbb", turnId: "turn-1" })
    expect(a.clientGroupId).toBe("khala-pylon-runtime.pylon.fixture.1")
    expect(a.clientId).toBe("runtime-turn.turn-1.aaa")
    // Distinct random suffixes always mint distinct clientIds — the push
    // engine's dense-ordering ledger starts clean at mutationId=1 for each.
    expect(a.clientId).not.toBe(b.clientId)
  })
})
