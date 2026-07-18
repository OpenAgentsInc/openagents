import { describe, expect, test } from "vite-plus/test"

import {
  dispatchFullAutoRunControlIntent,
  fetchFullAutoRunControlIntents,
  reportFullAutoRunControlIntentOutcome,
} from "./full-auto-run-control-intent.js"

const timestamp = "2026-07-18T02:00:00.000Z"
const pendingIntent = {
  schema: "full_auto_run.control_intent.v1" as const,
  intentId: "intent.mobile.abc123",
  idempotencyKey: "idem.mobile.abc123",
  runRef: "run.full-auto.abc123.def456",
  action: "pause" as const,
  surface: "mobile" as const,
  createdAt: timestamp,
  status: "pending" as const,
  appliedAt: null,
  rejectionReason: null,
  resultLifecycleState: null,
}

describe("dispatchFullAutoRunControlIntent", () => {
  test("POSTs the typed intent and returns it pending", async () => {
    const result = await dispatchFullAutoRunControlIntent({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      intentId: pendingIntent.intentId,
      idempotencyKey: pendingIntent.idempotencyKey,
      runRef: pendingIntent.runRef,
      action: "pause",
      fetchImpl: async (input, init) => {
        expect(String(input)).toBe("https://openagents.com/api/full-auto-runs/control-intents")
        expect(init?.method).toBe("POST")
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer host-owned")
        const body = JSON.parse(String(init?.body))
        expect(body.intent.action).toBe("pause")
        return Response.json({ ok: true, intent: pendingIntent })
      },
    })
    expect(result).toEqual({ state: "dispatched", intent: pendingIntent })
  })

  test("fails closed on 401", async () => {
    expect(await dispatchFullAutoRunControlIntent({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      intentId: pendingIntent.intentId,
      idempotencyKey: pendingIntent.idempotencyKey,
      runRef: pendingIntent.runRef,
      action: "pause",
      fetchImpl: async () => new Response(null, { status: 401 }),
    })).toEqual({ state: "unauthorized" })
  })

  test("surfaces a typed rejection code rather than a generic failure", async () => {
    expect(await dispatchFullAutoRunControlIntent({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      intentId: pendingIntent.intentId,
      idempotencyKey: pendingIntent.idempotencyKey,
      runRef: pendingIntent.runRef,
      action: "pause",
      fetchImpl: async () =>
        Response.json({ ok: false, error: { code: "fleet_intent_run_limit" } }, { status: 409 }),
    })).toEqual({ state: "rejected", code: "fleet_intent_run_limit" })
  })
})

describe("fetchFullAutoRunControlIntents", () => {
  test("lists intents for the signed-in owner", async () => {
    const result = await fetchFullAutoRunControlIntents({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async (input, init) => {
        expect(String(input)).toBe("https://openagents.com/api/full-auto-runs/control-intents")
        expect(init?.method).toBe("GET")
        return Response.json({ ok: true, intents: [pendingIntent] })
      },
    })
    expect(result).toEqual({ state: "available", intents: [pendingIntent] })
  })
})

describe("reportFullAutoRunControlIntentOutcome", () => {
  test("POSTs an applied outcome", async () => {
    const applied = { ...pendingIntent, status: "applied" as const, appliedAt: timestamp, resultLifecycleState: "paused" as const }
    const result = await reportFullAutoRunControlIntentOutcome({
      baseUrl: "https://openagents.com",
      accessToken: "desktop-owned",
      outcome: { intentId: pendingIntent.intentId, status: "applied", resultLifecycleState: "paused" },
      fetchImpl: async (_input, init) => {
        const body = JSON.parse(String(init?.body))
        expect(body.outcome.status).toBe("applied")
        return Response.json({ ok: true, intent: applied })
      },
    })
    expect(result).toEqual({ state: "reported", intent: applied })
  })
})
