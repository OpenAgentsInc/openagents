import { describe, expect, test } from "vite-plus/test"

import { fetchFullAutoRunMobileProjection } from "../src/full-auto/full-auto-run-projection-source"

describe("fetchFullAutoRunMobileProjection", () => {
  test("decodes an authenticated active-run response", async () => {
    const result = await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async (input, init) => {
        expect(String(input)).toBe("https://openagents.com/api/full-auto-runs/mine")
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer host-owned")
        return Response.json({
          ok: true,
          fullAutoRun: {
            schema: "full_auto_run.mobile_projection.v1",
            runRef: "full_auto_run.remote.0001",
            threadRef: "thread.full-auto.remote.0001",
            objective: "Ship it.",
            doneCondition: "Mobile shows the thread.",
            lifecycleState: "running",
            workspaceLabel: "openagents",
            startedAt: "2026-07-17T00:00:00.000Z",
            updatedAt: "2026-07-17T00:00:00.000Z",
          },
        })
      },
    })
    expect(result).toMatchObject({
      state: "active",
      projection: { runRef: "full_auto_run.remote.0001", lifecycleState: "running" },
    })
  })

  test("decodes an authenticated no-active-run response", async () => {
    const result = await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async () => Response.json({ ok: true, fullAutoRun: null }),
    })
    expect(result).toEqual({ state: "none" })
  })

  test("fails closed for authentication, non-ok, and malformed payloads", async () => {
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 401 }),
    })).toEqual({ state: "unauthorized" })
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 403 }),
    })).toEqual({ state: "unauthorized" })
    // The endpoint does not exist yet (openagents #8981 not landed): a 404
    // must degrade to "unavailable", which mobile treats like "no active
    // run" — this is what keeps default behavior unregressed until #8981 ships.
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 404 }),
    })).toEqual({ state: "unavailable" })
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => Response.json({ ok: true, fullAutoRun: { schema: "wrong" } }),
    })).toEqual({ state: "unavailable" })
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => {
        throw new Error("network down")
      },
    })).toEqual({ state: "unavailable" })
  })
})
