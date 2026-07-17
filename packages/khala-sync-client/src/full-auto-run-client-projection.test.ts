import { describe, expect, test } from "vite-plus/test"

import {
  fetchFullAutoRunClientProjection,
  publishFullAutoRunClientProjection,
} from "./full-auto-run-client-projection.js"

const timestamp = "2026-07-17T21:00:00.000Z"
const run = {
  runRef: "run.full-auto.abc123.def456",
  threadRef: "thread.abc123",
  objective: "Ship the mobile live-run projection.",
  doneCondition: "The new endpoint round-trips a projection end to end.",
  lifecycleState: "running" as const,
  workspaceLabel: "openagents",
  startedAt: timestamp,
  updatedAt: timestamp,
  lastTransition: { actor: "control_api" as const, at: timestamp },
}

describe("fetchFullAutoRunClientProjection", () => {
  test("decodes an authenticated projection with an active run", async () => {
    const result = await fetchFullAutoRunClientProjection({
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
            generatedAt: timestamp,
            run,
          },
        })
      },
    })
    expect(result).toMatchObject({ state: "available", projection: { run: { lifecycleState: "running" } } })
  })

  test("decodes a projection with no active run", async () => {
    const result = await fetchFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      fetchImpl: async () =>
        Response.json({
          ok: true,
          projection: {
            schema: "full_auto_run.mobile_projection.v1",
            privateMaterialExcluded: true,
            generatedAt: timestamp,
            run: null,
          },
        }),
    })
    expect(result).toEqual({
      state: "available",
      projection: {
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: null,
      },
    })
  })

  test("fails closed for authentication and malformed payloads", async () => {
    expect(await fetchFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => new Response(null, { status: 401 }),
    })).toEqual({ state: "unauthorized" })
    expect(await fetchFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      fetchImpl: async () => Response.json({ ok: true, projection: {} }),
    })).toEqual({ state: "unavailable" })
  })
})

describe("publishFullAutoRunClientProjection", () => {
  test("POSTs the run projection with a bearer token", async () => {
    const result = await publishFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      run,
      fetchImpl: async (input, init) => {
        expect(String(input)).toBe("https://openagents.com/api/full-auto-runs")
        expect(init?.method).toBe("POST")
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer host-owned")
        const body = JSON.parse(String(init?.body))
        expect(body.run.runRef).toBe(run.runRef)
        return Response.json({
          ok: true,
          projection: {
            schema: "full_auto_run.mobile_projection.v1",
            privateMaterialExcluded: true,
            generatedAt: timestamp,
            run,
          },
        })
      },
    })
    expect(result).toMatchObject({ state: "published", projection: { run: { runRef: run.runRef } } })
  })

  test("publishing null clears the projection", async () => {
    const result = await publishFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "host-owned",
      run: null,
      fetchImpl: async (_input, init) => {
        const body = JSON.parse(String(init?.body))
        expect(body.run).toBeNull()
        return Response.json({
          ok: true,
          projection: {
            schema: "full_auto_run.mobile_projection.v1",
            privateMaterialExcluded: true,
            generatedAt: timestamp,
            run: null,
          },
        })
      },
    })
    expect(result).toMatchObject({ state: "published", projection: { run: null } })
  })

  test("fails closed for authentication and unavailable storage", async () => {
    expect(await publishFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      run,
      fetchImpl: async () => new Response(null, { status: 401 }),
    })).toEqual({ state: "unauthorized" })
    expect(await publishFullAutoRunClientProjection({
      baseUrl: "https://openagents.com",
      accessToken: "x",
      run,
      fetchImpl: async () => new Response(null, { status: 503 }),
    })).toEqual({ state: "unavailable" })
  })
})
