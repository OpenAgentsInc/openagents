import { describe, expect, test } from "vite-plus/test"

import { fetchFullAutoRunMobileProjection } from "../src/full-auto/full-auto-run-projection-source"

describe("fetchFullAutoRunMobileProjection (translation over the real #8981 client)", () => {
  test("maps an available projection with an active run to state 'active'", async () => {
    const result = await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async (input, init) => {
        expect(String(input)).toBe("https://openagents.com/api/full-auto-runs")
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer host-owned")
        return Response.json({
          ok: true,
          projection: {
            schema: "full_auto_run.mobile_projection.v1",
            privateMaterialExcluded: true,
            generatedAt: "2026-07-17T00:00:00.000Z",
            run: {
              runRef: "run.full-auto.remote-0001",
              threadRef: "thread.full-auto.remote.0001",
              objective: "Ship it.",
              doneCondition: "Mobile shows the thread.",
              lifecycleState: "running",
              workspaceLabel: "openagents",
              startedAt: "2026-07-17T00:00:00.000Z",
              updatedAt: "2026-07-17T00:00:00.000Z",
              lastTransition: { actor: "owner_ui", at: "2026-07-17T00:00:00.000Z" },
            },
          },
        })
      },
    })
    expect(result).toMatchObject({
      state: "active",
      projection: { runRef: "run.full-auto.remote-0001", lifecycleState: "running" },
    })
  })

  test("maps an available projection with run: null to state 'none'", async () => {
    const result = await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async () => Response.json({
        ok: true,
        projection: {
          schema: "full_auto_run.mobile_projection.v1",
          privateMaterialExcluded: true,
          generatedAt: "2026-07-17T00:00:00.000Z",
          run: null,
        },
      }),
    })
    expect(result).toEqual({ state: "none" })
  })

  test("passes through unauthorized and unavailable states from the real client", async () => {
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 401 }),
    })).toEqual({ state: "unauthorized" })
    expect(await fetchFullAutoRunMobileProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 404 }),
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
