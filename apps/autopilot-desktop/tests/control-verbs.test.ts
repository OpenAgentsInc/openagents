import { describe, expect, test } from "bun:test"
import {
  cancelSession,
  fetchNodeState,
  resolveApproval,
  setCoordinatorPaused,
  spawnSession,
  submitIntent,
} from "../src/bun/pylon-control"

// A tiny fetch stub that routes by the POSTed command `type` (and GET /health).
function stubFetch(routes: Record<string, unknown>): typeof fetch {
  return (async (url: string, init?: RequestInit) => {
    if (String(url).endsWith("/health")) {
      return new Response(JSON.stringify(routes["__health"] ?? { ok: true, schema: "openagents.pylon.control.v0.3" }), {
        status: 200,
      })
    }
    const body = init?.body ? JSON.parse(String(init.body)) : {}
    const payload = routes[body.type as string]
    if (payload === undefined) return new Response(JSON.stringify({ ok: true, result: [] }), { status: 200 })
    return new Response(JSON.stringify(payload), { status: 200 })
  }) as unknown as typeof fetch
}

const base = { baseUrl: "http://127.0.0.1:4716", token: "tok" }

describe("CL-46 control verbs", () => {
  test("submitIntent returns the ship status", async () => {
    const r = await submitIntent({
      ...base,
      title: "ship it",
      body: "",
      fetchFn: stubFetch({ "intent.submit": { ok: true, result: { status: "received" } } }),
    })
    expect(r).toEqual({ ok: true, status: "received" })
  })

  test("submitIntent surfaces a node failure honestly", async () => {
    const r = await submitIntent({
      ...base,
      title: "x",
      body: "",
      fetchFn: stubFetch({ "intent.submit": { ok: false } }),
    })
    expect(r.ok).toBe(false)
  })

  test("resolveApproval reports applied + duplicate", async () => {
    const r = await resolveApproval({
      ...base,
      approvalRef: "a1",
      decision: "approve",
      fetchFn: stubFetch({ "approvals.resolve": { ok: true, result: { applied: true, duplicate: false, decision: "approve" } } }),
    })
    expect(r).toEqual({ applied: true, duplicate: false, decision: "approve" })
  })

  test("setCoordinatorPaused echoes the resulting flag", async () => {
    const r = await setCoordinatorPaused({
      ...base,
      paused: true,
      fetchFn: stubFetch({ "coordinator.pause": { ok: true, result: { paused: true } } }),
    })
    expect(r.paused).toBe(true)
  })

  test("cancelSession returns the new state", async () => {
    const r = await cancelSession({
      ...base,
      sessionRef: "s1",
      fetchFn: stubFetch({ "session.cancel": { ok: true, result: { state: "cancelled" } } }),
    })
    expect(r).toEqual({ ok: true, state: "cancelled" })
  })

  test("spawnSession returns the new session ref", async () => {
    const r = await spawnSession({
      ...base,
      adapter: "codex",
      objective: "do the thing",
      fetchFn: stubFetch({ "session.spawn": { ok: true, result: { sessionRef: "sess-123" } } }),
    })
    expect(r).toEqual({ ok: true, sessionRef: "sess-123" })
  })

  test("fetchNodeState aggregates the parity surfaces", async () => {
    const state = await fetchNodeState({
      ...base,
      fetchFn: stubFetch({
        "session.list": { ok: true, result: [] },
        "intent.list": { ok: true, result: { intents: [{ intentId: "i1", title: "t", status: "shipping" }] } },
        "approvals.list": { ok: true, result: { approvals: [{ approvalRef: "ap1", kind: "exec", prompt: "ok?" }] } },
        "wallet.status": { ok: true, result: { configured: true, daemonOnline: true, balanceSats: 1234, readiness: "ready" } },
        "assignments.poll": { ok: true, result: [{ assignmentRef: "as1", leaseRef: "l1", goal: "g", paymentMode: "credits" }] },
        "coordinator.status": { ok: true, result: { paused: true } },
        "deploy.status": { ok: true, result: { state: "deployed", message: "live" } },
        "accounts.list": { ok: true, result: { accounts: [] } },
      }),
    })
    expect(state.intents[0]?.status).toBe("shipping")
    expect(state.approvals[0]?.approvalRef).toBe("ap1")
    expect(state.wallet?.balanceSats).toBe(1234)
    expect(state.assignments[0]?.paymentMode).toBe("credits")
    expect(state.coordinatorPaused).toBe(true)
  })
})
