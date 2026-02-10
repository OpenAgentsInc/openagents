import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test"
import { describe, expect, it, vi } from "vitest"

import type { WorkerEnv } from "../../src/effuse-host/env"

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  authUserId: "user_dse_admin",
  queryCalls: [] as any[],
}))

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<any>()
  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => ({
        auth: {
          user: { id: state.authUserId, email: "admin@example.com", firstName: "A", lastName: "D" },
          sessionId: "sess-1",
          accessToken: "token-1",
        },
        refreshedSessionData: undefined,
      }),
      saveSession: async (_auth: unknown, _sessionData: string) => ({ headers: {} as Record<string, string> }),
    }),
  }
})

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<any>()
  const { Effect, Layer, Stream } = await import("effect")

  const isRecord = (u: unknown): u is Record<string, any> => Boolean(u) && typeof u === "object"

  const query = (ref: any, args: any) =>
    Effect.sync(() => {
      state.queryCalls.push({ ref, args })

      if (isRecord(args) && typeof args.signatureId === "string") {
        return {
          ok: true,
          receipts: [
            {
              receiptId: "r1",
              signatureId: args.signatureId,
              compiled_id: "sha256:c1",
              threadId: "t1",
              runId: "run1",
              createdAtMs: 3,
              strategyId: "rlm_lite.v1",
              resultTag: "Ok",
              rlmTraceBlobId: "sha256:trace1",
              rlmTraceEventCount: 12,
            },
            {
              receiptId: "r2",
              signatureId: args.signatureId,
              compiled_id: "sha256:c2",
              threadId: "t2",
              runId: "run2",
              createdAtMs: 2,
              strategyId: "direct.v1",
              resultTag: "Ok",
              rlmTraceBlobId: null,
              rlmTraceEventCount: null,
            },
            {
              receiptId: "r3",
              signatureId: args.signatureId,
              compiled_id: "sha256:c3",
              threadId: "t3",
              runId: "run3",
              createdAtMs: 1,
              strategyId: "rlm_lite.v1",
              resultTag: "Error",
              rlmTraceBlobId: "sha256:trace2",
              rlmTraceEventCount: 2,
            },
          ],
        }
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`)
    })

  const mutation = (_ref: any, _args: any) => Effect.fail(new Error("convex.mutation not used in tests"))
  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in Worker tests"))
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"))

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery })
  return { ...actual, ConvexServiceLive }
})

const { default: worker } = await import("../../src/effuse-host/worker")

describe("apps/web worker DSE receipts list endpoint (Phase 8)", () => {
  it("rejects non-ops-admin sessions", async () => {
    state.authUserId = "admin-1"

    const ORIGIN = "http://example.com"
    const req = new Request(
      `${ORIGIN}/api/dse/receipts/list?signatureId=@openagents/autopilot/blueprint/SelectTool.v1&limit=3`,
      { method: "GET" },
    )

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env as any, ctx)
    await waitOnExecutionContext(ctx)
    expect(res.status).toBe(401)
  })

  it("lists receipts and supports filtering for trace mining", async () => {
    state.authUserId = "user_dse_admin"
    state.queryCalls.length = 0

    const ORIGIN = "http://example.com"
    const req = new Request(
      `${ORIGIN}/api/dse/receipts/list?signatureId=@openagents/autopilot/blueprint/SelectTool.v1&limit=3&requireRlmTrace=1&resultTag=Ok&strategyId=rlm_lite.v1`,
      { method: "GET" },
    )

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env as any, ctx)
    await waitOnExecutionContext(ctx)
    expect(res.status).toBe(200)
    const json = (await res.json()) as any
    expect(json.ok).toBe(true)
    expect(Array.isArray(json.receipts)).toBe(true)
    expect(json.receipts).toHaveLength(1)
    expect(json.receipts[0].receiptId).toBe("r1")
    expect(json.receipts[0].rlmTraceBlobId).toBe("sha256:trace1")

    expect(state.queryCalls.length).toBe(1)
    expect(state.queryCalls[0]?.args?.limit).toBe(3)
  })
})

