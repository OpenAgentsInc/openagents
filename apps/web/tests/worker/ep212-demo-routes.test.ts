import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test"
import { describe, expect, it } from "vitest"

import type { WorkerEnv } from "../../src/effuse-host/env"

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const { default: worker } = await import("../../src/effuse-host/worker")

describe("apps/web worker EP212 demo upstream routes", () => {
  it("serves deterministic premium payload for /ep212/premium-signal", async () => {
    const req = new Request("http://example.com/ep212/premium-signal", {
      method: "GET",
      headers: { "x-oa-request-id": "req-ep212-premium-1" },
    })

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env, ctx)
    await waitOnExecutionContext(ctx)

    expect(res.status).toBe(200)
    expect(res.headers.get("content-type") ?? "").toContain("application/json")
    expect(res.headers.get("x-oa-request-id")).toBe("req-ep212-premium-1")
    const body = (await res.json()) as {
      ok: boolean
      route: string
      tier: string
      requestId: string
      signals: ReadonlyArray<{ symbol: string }>
    }
    expect(body.ok).toBe(true)
    expect(body.route).toBe("/ep212/premium-signal")
    expect(body.tier).toBe("under-cap")
    expect(body.requestId).toBe("req-ep212-premium-1")
    expect(body.signals[0]?.symbol).toBe("BTC")
  })

  it("serves deterministic over-cap payload for /ep212/expensive-signal", async () => {
    const req = new Request("http://example.com/ep212/expensive-signal", {
      method: "GET",
      headers: { "x-oa-request-id": "req-ep212-expensive-1" },
    })

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env, ctx)
    await waitOnExecutionContext(ctx)

    expect(res.status).toBe(200)
    const body = (await res.json()) as { route: string; tier: string; ok: boolean }
    expect(body.ok).toBe(true)
    expect(body.route).toBe("/ep212/expensive-signal")
    expect(body.tier).toBe("over-cap")
  })

  it("returns method_not_allowed for non-GET requests", async () => {
    const req = new Request("http://example.com/ep212/premium-signal", {
      method: "POST",
    })

    const ctx = createExecutionContext()
    const res = await worker.fetch(req, env, ctx)
    await waitOnExecutionContext(ctx)

    expect(res.status).toBe(405)
    const body = (await res.json()) as { ok: boolean; error: string }
    expect(body.ok).toBe(false)
    expect(body.error).toBe("method_not_allowed")
  })
})
