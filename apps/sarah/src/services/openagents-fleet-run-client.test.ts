import { describe, expect, test } from "bun:test"

import {
  OPENAGENTS_SARAH_FLEET_RUNS_PATH,
  startSarahCodingFleetRunThroughAuthority,
} from "./openagents-fleet-run-client.ts"

const validStartEnvelope = () => ({
  ok: true,
  duplicate: false,
  policy: {
    source: "openagents_server_policy",
    relationshipMode: "operator",
    codingFleetStartAllowed: true,
    fleetObservationAllowed: true,
    retrievalScope: "owner_fleet_runs",
    responsePosture: "state_oriented",
    uiDensity: "dense",
    administratorToolsAllowed: false,
  },
  routeRef: "route.sarah.fleet_runs.authority.v1",
  run: {
    runRef: "fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa",
    scope: "scope.fleet_run.fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa",
    status: "pending_executor",
    objective: "Implement one bounded public issue.",
    repository: {
      owner: "OpenAgentsInc",
      name: "openagents",
      branch: "main",
      commit: "6af4e38282e4e71882fc5fdd86ae8adadab6df50",
    },
    verifier: { kind: "command", command: "bun test" },
    workSource: { kind: "issue_list", issueRefs: ["#8637"] },
    workerPolicy: { workerKind: "auto", targetPreference: "owner_local" },
    targetConcurrency: 2,
    createdAt: "2026-07-09T12:00:00.000Z",
    updatedAt: "2026-07-09T12:00:00.000Z",
    privateMaterialExcluded: true,
  },
})

describe("Sarah FleetRun authority HTTP client", () => {
  test("forwards only existing auth context and raw tool args to the fixed authority path", async () => {
    const calls: Array<Request> = []
    const args = {
      objective: "Implement one bounded public issue.",
      idempotencyKey: "fleet-client-1",
    }
    const result = await startSarahCodingFleetRunThroughAuthority(
      new Request("http://sarah-cloud-run.internal/sarah/api/eve/tool-call", {
        headers: {
          authorization: "Bearer fixture-user-session",
          cookie: "oa_access=fixture-access",
          "x-owner-user-id": "must-not-forward",
          "x-relationship-mode": "must-not-forward",
        },
      }),
      args,
      request => {
        calls.push(request)
        const headers = new Headers({
          "content-type": "application/json",
        })
        headers.append(
          "set-cookie",
          "oa_access=fixture-rotated; Path=/; HttpOnly; SameSite=Lax",
        )
        return Promise.resolve(
          new Response(
            JSON.stringify(validStartEnvelope()),
            { headers },
          ),
        )
      },
    )

    expect(calls).toHaveLength(1)
    expect(new URL(calls[0]!.url).pathname).toBe(
      OPENAGENTS_SARAH_FLEET_RUNS_PATH,
    )
    expect(new URL(calls[0]!.url).origin).toBe("https://openagents.com")
    expect(calls[0]!.method).toBe("POST")
    expect(calls[0]!.headers.has("cookie")).toBe(true)
    expect(calls[0]!.headers.has("authorization")).toBe(true)
    expect(calls[0]!.headers.has("x-owner-user-id")).toBe(false)
    expect(calls[0]!.headers.has("x-relationship-mode")).toBe(false)
    expect(await calls[0]!.json()).toEqual(args)
    expect(result.ok).toBe(true)
    expect(result.refreshedSessionCookies).toHaveLength(1)
    expect(JSON.stringify(result.output)).not.toContain("oa_access")
    expect(JSON.stringify(result.output)).not.toContain("fixture-rotated")
  })

  test("preserves typed authority refusal while keeping refreshed cookies out of output", async () => {
    const result = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call", {
        headers: { cookie: "oa_access=fixture-access" },
      }),
      { idempotencyKey: "fleet-client-conflict" },
      () =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              ok: false,
              error: { code: "idempotency_conflict", retryable: false },
              routeRef: "route.sarah.fleet_runs.authority.v1",
            }),
            {
              status: 409,
              headers: {
                "content-type": "application/json",
                "set-cookie":
                  "oa_access=fixture-rotated; Path=/; HttpOnly; SameSite=Lax",
              },
            },
          ),
        ),
    )
    expect(result.ok).toBe(false)
    expect(result.output).toMatchObject({
      error: { code: "idempotency_conflict", retryable: false },
      ok: false,
    })
    expect(result.refreshedSessionCookies).toHaveLength(1)
    expect(JSON.stringify(result.output)).not.toContain("oa_access")
  })

  test("collapses transport and malformed-response failures to fixed diagnostics", async () => {
    const transport = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call"),
      {},
      () => Promise.reject(new Error("private transport detail")),
    )
    expect(transport).toMatchObject({
      ok: false,
      output: { error: { code: "store_unavailable", retryable: true } },
      refreshedSessionCookies: [],
    })

    const malformed = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call"),
      {},
      () => Promise.resolve(new Response(JSON.stringify({ unexpected: true }))),
    )
    expect(malformed).toMatchObject({
      ok: false,
      output: { error: { code: "invalid_response", retryable: false } },
      refreshedSessionCookies: [],
    })
  })

  test("rejects hostile success and failure envelopes without echoing private material", async () => {
    const privateToken = "OPENAGENTS_AGENT_TOKEN=fixture-do-not-echo"
    const hostileSuccess = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call"),
      {},
      () =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              ...validStartEnvelope(),
              rawPrompt: privateToken,
            }),
          ),
        ),
    )
    expect(hostileSuccess).toMatchObject({
      ok: false,
      output: { error: { code: "invalid_response", retryable: false } },
      refreshedSessionCookies: [],
    })
    expect(JSON.stringify(hostileSuccess)).not.toContain(privateToken)

    const hostileFailure = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call"),
      {},
      () =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              ok: false,
              error: { code: "storage_unavailable", retryable: true },
              routeRef: "route.sarah.fleet_runs.authority.v1",
              detail: privateToken,
            }),
            { status: 503 },
          ),
        ),
    )
    expect(hostileFailure).toMatchObject({
      ok: false,
      output: { error: { code: "invalid_response", retryable: false } },
      refreshedSessionCookies: [],
    })
    expect(JSON.stringify(hostileFailure)).not.toContain(privateToken)
  })

  test("rejects private material inside an otherwise exact public run", async () => {
    const privatePath = "/Users/owner/work/private-repository"
    const envelope = validStartEnvelope()
    const result = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call"),
      {},
      () =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              ...envelope,
              run: { ...envelope.run, objective: `Read ${privatePath}` },
            }),
          ),
        ),
    )
    expect(result).toMatchObject({
      ok: false,
      output: { error: { code: "invalid_response", retryable: false } },
      refreshedSessionCookies: [],
    })
    expect(JSON.stringify(result)).not.toContain(privatePath)
  })

  test("rejects a known authority error with false retryability semantics", async () => {
    const result = await startSarahCodingFleetRunThroughAuthority(
      new Request("https://openagents.com/sarah/api/eve/tool-call"),
      {},
      () =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              ok: false,
              error: { code: "storage_unavailable", retryable: false },
              routeRef: "route.sarah.fleet_runs.authority.v1",
            }),
            { status: 503 },
          ),
        ),
    )
    expect(result).toMatchObject({
      ok: false,
      output: { error: { code: "invalid_response", retryable: false } },
      refreshedSessionCookies: [],
    })
  })
})
