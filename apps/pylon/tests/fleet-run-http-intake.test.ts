import { describe, expect, test } from "vite-plus/test"

import { makePylonFleetRunHttpIntake } from "../src/orchestration/fleet-run-http-intake.js"
import { PylonFleetRunRemotePortError } from "../src/orchestration/fleet-run-remote-intake.js"

const pylonRef = "pylon.public.http_intake"
const runRef = "fleet_run.sarah.0123456789abcdef0123"
const claimRef = "claim.sarah_fleet_run.0123456789abcdef01234567"

const json = (value: unknown, status = 200): Response =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
    status,
  })

const success = (operation: "accept" | "claim", result: unknown): Response =>
  json({
    schema: "openagents.pylon.fleet_run_transport.v1",
    operation,
    result,
  })

describe("Pylon FleetRun HTTP intake adapter", () => {
  test("refuses bearer transport over non-loopback HTTP", () => {
    expect(() =>
      makePylonFleetRunHttpIntake({
        agentToken: "oa_agent_private_fixture",
        baseUrl: "http://openagents.example",
      }),
    ).toThrow()

    for (const baseUrl of [
      "http://localhost:4716",
      "http://127.0.0.1:4716",
      "http://[::1]:4716",
      "https://openagents.com",
    ]) {
      expect(() =>
        makePylonFleetRunHttpIntake({
          agentToken: "oa_agent_private_fixture",
          baseUrl,
        }),
      ).not.toThrow()
    }
  })

  test("sends only the private bearer request contract and unwraps a strict claim", async () => {
    const requests: Request[] = []
    const adapter = makePylonFleetRunHttpIntake({
      agentToken: "oa_agent_private_fixture",
      baseUrl: "https://openagents.com",
      makeId: () => "request-one",
      fetchImpl: async (input, init) => {
        requests.push(new Request(input, init))
        return success("claim", { run: { runRef } })
      },
    })

    await expect(adapter.claimNext({ pylonRef })).resolves.toEqual({
      run: { runRef },
    })
    expect(requests).toHaveLength(1)
    const request = requests[0]!
    expect(request.url).toBe(
      `https://openagents.com/api/pylons/${pylonRef}/fleet-runs/claim`,
    )
    expect(request.method).toBe("POST")
    expect(request.headers.get("authorization")).toBe(
      "Bearer oa_agent_private_fixture",
    )
    expect(request.headers.get("idempotency-key")).toBe(
      "pylon.fleet-run.claim.request-one",
    )
    expect(await request.json()).toEqual({
      schema: "openagents.pylon.fleet_run_claim.request.v1",
    })
  })

  test("uses an exact replacement body and deterministic accept replay key", async () => {
    const requests: Request[] = []
    const adapter = makePylonFleetRunHttpIntake({
      agentToken: "oa_agent_private_fixture",
      baseUrl: "https://openagents.com",
      makeId: () => "replacement-two",
      fetchImpl: async (input, init) => {
        const request = new Request(input, init)
        requests.push(request)
        return request.url.endsWith("/claim")
          ? success("claim", { exact: true })
          : success("accept", { accepted: true })
      },
    })
    await adapter.claimNext({ pylonRef, runRef })
    await adapter.acceptClaim({ pylonRef, runRef, claimRef })

    expect(await requests[0]!.json()).toEqual({
      schema: "openagents.pylon.fleet_run_claim.request.v1",
      runRef,
    })
    expect(await requests[1]!.json()).toEqual({
      schema: "openagents.pylon.fleet_run_accept.request.v1",
      runRef,
      claimRef,
    })
    expect(requests[1]!.headers.get("idempotency-key")).toBe(
      `pylon.fleet-run.accept.${claimRef}`,
    )
  })

  test("maps an empty owner queue to null", async () => {
    const adapter = makePylonFleetRunHttpIntake({
      agentToken: "oa_agent_private_fixture",
      baseUrl: "https://openagents.com",
      fetchImpl: async () => new Response(null, { status: 204 }),
    })
    await expect(adapter.claimNext({ pylonRef })).resolves.toBeNull()
  })

  test("reuses one claim key after a dropped response and after local import failure", async () => {
    const keys: string[] = []
    let ids = 0
    let calls = 0
    const adapter = makePylonFleetRunHttpIntake({
      agentToken: "oa_agent_private_fixture",
      baseUrl: "https://openagents.com",
      makeId: () => `request-${++ids}`,
      fetchImpl: async (input, init) => {
        const request = new Request(input, init)
        if (request.url.endsWith("/accept")) {
          return success("accept", { accepted: true })
        }
        keys.push(request.headers.get("idempotency-key")!)
        calls += 1
        if (calls === 1) {
          // The authority committed, but the response never reached Pylon.
          throw new Error("connection reset after commit")
        }
        return success("claim", {
          duplicate: true,
          claim: { claimRef, runRef },
        })
      },
    })

    await expect(adapter.claimNext({ pylonRef })).rejects.toMatchObject({
      kind: "unavailable",
    })
    await expect(adapter.claimNext({ pylonRef })).resolves.toMatchObject({
      duplicate: true,
    })
    // Simulate canonical local import failing after the decoded replay. The
    // orchestration service asks again because no binding was persisted.
    await expect(adapter.claimNext({ pylonRef })).resolves.toMatchObject({
      duplicate: true,
    })
    expect(keys).toEqual([
      "pylon.fleet-run.claim.request-1",
      "pylon.fleet-run.claim.request-1",
      "pylon.fleet-run.claim.request-1",
    ])
    expect(ids).toBe(1)

    await adapter.acceptClaim({ claimRef, pylonRef, runRef })
    await adapter.claimNext({ pylonRef })
    expect(keys.at(-1)).toBe("pylon.fleet-run.claim.request-2")
  })

  test.each([
    [401, "not_authorized", "not_authorized"],
    [403, "not_authorized", "not_authorized"],
    [409, "claim_conflict", "claim_conflict"],
    [409, "claim_expired", "claim_expired"],
    [503, "unavailable", "unavailable"],
  ] as const)(
    "maps status %s code %s to %s without returning server text",
    async (status, code, kind) => {
      const adapter = makePylonFleetRunHttpIntake({
        agentToken: "oa_agent_private_fixture",
        baseUrl: "https://openagents.com",
        fetchImpl: async () =>
          json(
            {
              schema: "openagents.pylon.fleet_run_transport.v1",
              error: { code, retryable: code === "unavailable" },
            },
            status,
          ),
      })
      const error = await adapter.claimNext({ pylonRef }).catch(value => value)
      expect(error).toBeInstanceOf(PylonFleetRunRemotePortError)
      expect(error).toMatchObject({ kind })
      expect(JSON.stringify(error)).not.toContain("oa_agent_private_fixture")
    },
  )

  test("fails closed on an excess or oversized response", async () => {
    const responses = [
      json({
        schema: "openagents.pylon.fleet_run_transport.v1",
        operation: "claim",
        result: {},
        privatePath: "/Users/private/repo",
      }),
      new Response("x".repeat(256 * 1_024 + 1), { status: 200 }),
    ]
    const adapter = makePylonFleetRunHttpIntake({
      agentToken: "oa_agent_private_fixture",
      baseUrl: "https://openagents.com",
      fetchImpl: async () => responses.shift()!,
    })
    for (let index = 0; index < 2; index += 1) {
      await expect(adapter.claimNext({ pylonRef })).rejects.toMatchObject({
        kind: "unavailable",
      })
    }
  })
})
