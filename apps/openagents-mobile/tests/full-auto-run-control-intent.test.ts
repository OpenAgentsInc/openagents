import { describe, expect, test } from "vite-plus/test"

import { makeFullAutoRunControlDispatcher } from "../src/full-auto/full-auto-run-control-intent"

const RUN_REF = "run.full-auto.fixture-0001"

const noopSleep = async (_ms: number): Promise<void> => undefined

describe("makeFullAutoRunControlDispatcher", () => {
  test("dispatches and polls until the intent is applied, surfacing the resultLifecycleState", async () => {
    let listCalls = 0
    const dispatcher = makeFullAutoRunControlDispatcher({
      baseUrl: "https://openagents.com",
      accessToken: () => "token-a",
      sleep: noopSleep,
      pollAttempts: 3,
      fetchImpl: (async (input, init) => {
        if (init?.method === "POST") {
          const body = JSON.parse(String(init.body))
          expect(body.intent.action).toBe("pause")
          return Response.json({
            ok: true,
            intent: {
              schema: "full_auto_run.control_intent.v1", intentId: body.intent.intentId,
              idempotencyKey: body.intent.idempotencyKey, runRef: RUN_REF, action: "pause",
              surface: "mobile", createdAt: "2026-07-18T02:00:00.000Z", status: "pending",
              appliedAt: null, rejectionReason: null, resultLifecycleState: null,
            },
          })
        }
        listCalls += 1
        // First GET: still pending. Second GET: applied.
        const status = listCalls === 1 ? "pending" : "applied"
        return Response.json({
          ok: true,
          intents: [{
            schema: "full_auto_run.control_intent.v1", intentId: "intent.mobile.fixed",
            idempotencyKey: "idem.mobile.fixed", runRef: RUN_REF, action: "pause", surface: "mobile",
            createdAt: "2026-07-18T02:00:00.000Z", status,
            appliedAt: status === "applied" ? "2026-07-18T02:00:05.000Z" : null,
            rejectionReason: null,
            resultLifecycleState: status === "applied" ? "paused" : null,
          }],
        })
      }) as typeof fetch,
      randomId: () => "fixed",
    })
    const outcome = await dispatcher({ runRef: RUN_REF, action: "pause" })
    expect(outcome).toEqual({ state: "applied", resultLifecycleState: "paused" })
    expect(listCalls).toBe(2)
  })

  test("surfaces a rejected outcome with its typed reason, never treating it as success", async () => {
    const dispatcher = makeFullAutoRunControlDispatcher({
      baseUrl: "https://openagents.com",
      accessToken: () => "token-a",
      sleep: noopSleep,
      pollAttempts: 3,
      fetchImpl: (async (_input, init) => {
        if (init?.method === "POST") {
          return Response.json({
            ok: true,
            intent: {
              schema: "full_auto_run.control_intent.v1", intentId: "intent.mobile.fixed",
              idempotencyKey: "idem.mobile.fixed", runRef: RUN_REF, action: "resume",
              surface: "mobile", createdAt: "2026-07-18T02:00:00.000Z", status: "pending",
              appliedAt: null, rejectionReason: null, resultLifecycleState: null,
            },
          })
        }
        return Response.json({
          ok: true,
          intents: [{
            schema: "full_auto_run.control_intent.v1", intentId: "intent.mobile.fixed",
            idempotencyKey: "idem.mobile.fixed", runRef: RUN_REF, action: "resume", surface: "mobile",
            createdAt: "2026-07-18T02:00:00.000Z", status: "rejected", appliedAt: null,
            rejectionReason: "illegal_transition", resultLifecycleState: null,
          }],
        })
      }) as typeof fetch,
    })
    const outcome = await dispatcher({ runRef: RUN_REF, action: "resume" })
    expect(outcome).toEqual({ state: "rejected", reason: "illegal_transition" })
  })

  test("honestly reports pending (never success) when the poll deadline elapses before Desktop applies it", async () => {
    const dispatcher = makeFullAutoRunControlDispatcher({
      baseUrl: "https://openagents.com",
      accessToken: () => "token-a",
      sleep: noopSleep,
      pollAttempts: 2,
      fetchImpl: (async (_input, init) => {
        if (init?.method === "POST") {
          return Response.json({
            ok: true,
            intent: {
              schema: "full_auto_run.control_intent.v1", intentId: "intent.mobile.fixed",
              idempotencyKey: "idem.mobile.fixed", runRef: RUN_REF, action: "stop",
              surface: "mobile", createdAt: "2026-07-18T02:00:00.000Z", status: "pending",
              appliedAt: null, rejectionReason: null, resultLifecycleState: null,
            },
          })
        }
        return Response.json({
          ok: true,
          intents: [{
            schema: "full_auto_run.control_intent.v1", intentId: "intent.mobile.fixed",
            idempotencyKey: "idem.mobile.fixed", runRef: RUN_REF, action: "stop", surface: "mobile",
            createdAt: "2026-07-18T02:00:00.000Z", status: "pending",
            appliedAt: null, rejectionReason: null, resultLifecycleState: null,
          }],
        })
      }) as typeof fetch,
    })
    const outcome = await dispatcher({ runRef: RUN_REF, action: "stop" })
    expect(outcome).toEqual({ state: "pending" })
  })

  test("fails closed with unauthorized when there is no access token, never dispatching", async () => {
    let fetchCalls = 0
    const dispatcher = makeFullAutoRunControlDispatcher({
      baseUrl: "https://openagents.com",
      accessToken: () => null,
      sleep: noopSleep,
      fetchImpl: (async () => {
        fetchCalls += 1
        return Response.json({ ok: true, intents: [] })
      }) as typeof fetch,
    })
    const outcome = await dispatcher({ runRef: RUN_REF, action: "pause" })
    expect(outcome).toEqual({ state: "unauthorized" })
    expect(fetchCalls).toBe(0)
  })

  test("surfaces a typed rejection code from the dispatch call itself (e.g. a bounded-run-limit refusal)", async () => {
    const dispatcher = makeFullAutoRunControlDispatcher({
      baseUrl: "https://openagents.com",
      accessToken: () => "token-a",
      sleep: noopSleep,
      fetchImpl: (async () => Response.json({ ok: false, error: { code: "fleet_intent_run_limit" } }, { status: 409 })) as typeof fetch,
    })
    const outcome = await dispatcher({ runRef: RUN_REF, action: "pause" })
    expect(outcome).toEqual({ state: "rejected_at_dispatch", code: "fleet_intent_run_limit" })
  })
})
