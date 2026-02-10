import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test"
import { describe, expect, it, vi } from "vitest"

import type { WorkerEnv } from "../../src/effuse-host/env"

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  putExampleCalls: [] as any[],
}))

vi.mock("../../src/effuse-host/dseAdminSecret", async (importOriginal) => {
  const actual = await importOriginal<any>()
  return {
    ...actual,
    isDseAdminSecretAuthorized: (request: Request, env: any) => {
      const secret = env?.OA_DSE_ADMIN_SECRET
      const h = request.headers.get("authorization") ?? ""
      return Boolean(secret) && h.trim() === `Bearer ${secret}`
    },
    withDseAdminSecretServices: (_env: any, _convexUrl: string, program: any) => program,
  }
})

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<any>()
  const { Effect, Layer, Stream } = await import("effect")

  const isRecord = (u: unknown): u is Record<string, any> => Boolean(u) && typeof u === "object"

  const query = (_ref: any, _args: any) => Effect.fail(new Error("convex.query not used in tests"))

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      // We only expect calls to dse.examples.putExample.
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.exampleId === "string") {
        state.putExampleCalls.push({ ref, args })
        // Simulate "upsert": first insert, then update.
        const existed = state.putExampleCalls.length > 1
        return { ok: true, existed }
      }
      throw new Error(`Unexpected Convex mutation in tests: ${String(ref?.name ?? ref)}`)
    })

  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in tests"))
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"))

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery })
  return { ...actual, ConvexServiceLive }
})

const { default: worker } = await import("../../src/effuse-host/worker")

describe("apps/web worker DSE examples import endpoint (Phase 3)", () => {
  it("rejects missing admin secret", async () => {
    const ORIGIN = "http://example.com"
    const req = new Request(`${ORIGIN}/api/dse/examples/import`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
        examples: [],
      }),
    })

    const env0 = Object.assign(Object.create(env as any), {
      OA_DSE_ADMIN_SECRET: "secret",
    }) as any

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env0, ctx)
    await waitOnExecutionContext(ctx)
    expect(res.status).toBe(401)
  })

  it("imports JSONL rows by upserting examples", async () => {
    state.putExampleCalls.length = 0

    const ORIGIN = "http://example.com"
    const jsonl = [
      JSON.stringify({
        exampleId: "ex1",
        inputJson: { message: "Call me Ada.", blueprintHint: { userHandle: "Unknown", agentName: "Autopilot" } },
        expectedJson: { action: "tool", toolName: "user_update" },
        split: "train",
        tags: ["dataset:dse-selecttool.dataset.v1"],
      }),
      JSON.stringify({
        exampleId: "ex2",
        inputJson: { message: "What can you do?", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
        expectedJson: { action: "none" },
        split: "holdout",
        tags: ["dataset:dse-selecttool.dataset.v1"],
      }),
    ].join("\n")

    const req = new Request(`${ORIGIN}/api/dse/examples/import`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: "Bearer secret",
      },
      body: JSON.stringify({
        signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
        jsonl,
        source: "fixture:test",
        tagsAppend: ["import:test"],
      }),
    })

    const env0 = Object.assign(Object.create(env as any), {
      OA_DSE_ADMIN_SECRET: "secret",
    }) as any

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env0, ctx)
    await waitOnExecutionContext(ctx)

    expect(res.status).toBe(200)
    const json = (await res.json()) as any
    expect(json.ok).toBe(true)
    expect(json.total).toBe(2)
    expect(json.inserted).toBe(1)
    expect(json.updated).toBe(1)

    expect(state.putExampleCalls).toHaveLength(2)
    for (const c of state.putExampleCalls) {
      expect(c.args.signatureId).toBe("@openagents/autopilot/blueprint/SelectTool.v1")
      expect(c.args.source).toBe("fixture:test")
      expect(Array.isArray(c.args.tags)).toBe(true)
      expect(c.args.tags).toContain("import:test")
    }
  })
})

