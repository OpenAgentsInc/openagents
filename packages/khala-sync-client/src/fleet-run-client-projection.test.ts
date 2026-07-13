import { describe, expect, test } from "bun:test"
import { fetchFleetRunClientProjection } from "./fleet-run-client-projection.js"

describe("fetchFleetRunClientProjection", () => {
  test("decodes an authenticated refs-only page", async () => {
    const result = await fetchFleetRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async (_input, init) => {
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer host-owned")
        return Response.json({
          ok: true,
          fleet: {
            schema: "openagents.fleet_run_client_projection.v1",
            privateMaterialExcluded: true,
            generatedAt: "2026-07-13T10:55:20.179Z",
            runs: [],
          },
        })
      },
    })
    expect(result).toMatchObject({ state: "available", projection: { runs: [] } })
  })

  test("fails closed for authentication and malformed payloads", async () => {
    expect(await fetchFleetRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 401 }),
    })).toEqual({ state: "unauthorized" })
    expect(await fetchFleetRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => Response.json({ ok: true, fleet: {} }),
    })).toEqual({ state: "unavailable" })
  })
})
