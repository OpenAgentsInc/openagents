import { describe, expect, test } from "bun:test"
import {
  ClientGroupId,
  ClientId,
  KHALA_SYNC_PROTOCOL_VERSION,
} from "@openagentsinc/khala-sync"
import { decodeKhalaFleetIntent } from "@openagentsinc/khala-fleet-intents"

import {
  SARAH_FLEET_CURSOR_STATE_SCHEMA,
  MAX_SARAH_FLEET_BOOTSTRAP_PAGES,
  MAX_SARAH_FLEET_LOG_PAGES,
  MAX_SARAH_FLEET_SYNC_PAGE_SIZE,
  SarahFleetSyncClientError,
  buildSarahFleetBootstrapRequest,
  buildSarahFleetLogRequest,
  buildSarahFleetPushRequest,
  makeSarahFleetSyncClient,
  type SarahFleetFetch,
  type SarahFleetSyncCursorState,
} from "./fleet-sync-client.ts"

const scope = "scope.fleet_run.fleet.run.fc3"
const runRef = "fleet.run.fc3"
const identity = {
  clientGroupId: ClientGroupId.make("sarah.web.fc3"),
  clientId: ClientId.make("sarah.web.fc3.tab"),
}

const runControlIntent = decodeKhalaFleetIntent({
  schema: "khala.fleet_intent.v1",
  intentId: "intent.fc3.pause",
  createdAt: "2026-07-09T20:00:00.000Z",
  origin: { surface: "web" },
  idempotencyKey: "idem.fc3.pause",
  runRef,
  kind: "fleet_run_control",
  action: "pause",
})

const approvalIntent = decodeKhalaFleetIntent({
  schema: "khala.fleet_intent.v1",
  intentId: "intent.fc3.approval",
  createdAt: "2026-07-09T20:00:00.000Z",
  origin: { surface: "web" },
  idempotencyKey: "idem.fc3.approval",
  runRef,
  kind: "approval_decision",
  approvalRef: "approval.fc3.claude",
  decision: "allow",
})

const steerBody = "PRIVATE STEER BODY must never enter an error or receipt"
const steerIntent = decodeKhalaFleetIntent({
  schema: "khala.fleet_intent.v1",
  intentId: "intent.fc3.steer",
  createdAt: "2026-07-09T20:00:00.000Z",
  origin: { surface: "web" },
  idempotencyKey: "idem.fc3.steer",
  runRef,
  kind: "steer_message",
  targetRef: "worker.fc3.codex",
  body: steerBody,
})

const cursorState = (cursor: number): SarahFleetSyncCursorState => ({
  schema: SARAH_FLEET_CURSOR_STATE_SCHEMA,
  scope: scope as SarahFleetSyncCursorState["scope"],
  cursor: cursor as SarahFleetSyncCursorState["cursor"],
})

const json = (body: unknown, status = 200): Response =>
  Response.json(body, { status })

describe("Sarah FC-3 Khala Sync request builders", () => {
  test("builds only same-origin credentialed bootstrap and exact-cursor log requests", () => {
    const bootstrap = buildSarahFleetBootstrapRequest(identity, {
      scope,
      pageSize: 50,
      pageToken: "opaque-page-token",
    })
    expect(bootstrap.path).toBe("/api/sync/bootstrap")
    expect(bootstrap.init).toMatchObject({
      method: "POST",
      credentials: "same-origin",
    })
    expect(JSON.parse(String(bootstrap.init.body))).toMatchObject({
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      scope,
      clientGroupId: "sarah.web.fc3",
      pageSize: 50,
      pageToken: "opaque-page-token",
    })

    const log = buildSarahFleetLogRequest({ scope, cursor: 41, limit: 100 })
    expect(log.init).toMatchObject({
      method: "GET",
      credentials: "same-origin",
    })
    const logUrl = new URL(log.path, "https://openagents.com")
    expect(logUrl.pathname).toBe("/api/sync/log")
    expect(logUrl.searchParams.get("scope")).toBe(scope)
    expect(logUrl.searchParams.get("cursor")).toBe("41")
    expect(log.path).not.toContain("latest")
  })

  test("rejects non-fleet, malformed, and ambiguous fleet scopes", () => {
    for (const invalidScope of [
      "scope.team.team-a",
      "scope.fleet_run.",
      "scope.fleet_run.fleet/run",
      "scope.fleet_run.fleet.run trailing",
      `${scope}.`,
    ]) {
      expect(() =>
        buildSarahFleetLogRequest({
          scope: invalidScope,
          cursor: 0,
          limit: 100,
        }),
      ).toThrow(SarahFleetSyncClientError)
    }
  })

  test("hard-caps request and loop bounds even when callers override them", () => {
    expect(() =>
      buildSarahFleetBootstrapRequest(identity, {
        scope,
        pageSize: MAX_SARAH_FLEET_SYNC_PAGE_SIZE + 1,
      }),
    ).toThrow(SarahFleetSyncClientError)
    expect(() =>
      buildSarahFleetLogRequest({
        scope,
        cursor: 0,
        limit: MAX_SARAH_FLEET_SYNC_PAGE_SIZE + 1,
      }),
    ).toThrow(SarahFleetSyncClientError)

    for (const overrides of [
      { pageSize: MAX_SARAH_FLEET_SYNC_PAGE_SIZE + 1 },
      { maxBootstrapPages: MAX_SARAH_FLEET_BOOTSTRAP_PAGES + 1 },
      { maxLogPages: MAX_SARAH_FLEET_LOG_PAGES + 1 },
    ]) {
      expect(() =>
        makeSarahFleetSyncClient({
          fetch: async () => json({}),
          clientGroupId: "sarah.web.fc3",
          clientId: "sarah.web.fc3.tab",
          ...overrides,
        }),
      ).toThrow(SarahFleetSyncClientError)
    }
  })

  test("maps all three existing intent kinds to their existing push mutators", () => {
    const expected = [
      [runControlIntent, "fleet.dispatchRunControl"],
      [approvalIntent, "fleet.dispatchApprovalDecision"],
      [steerIntent, "fleet.dispatchSteerMessage"],
    ] as const

    for (const [intent, mutator] of expected) {
      const request = buildSarahFleetPushRequest(identity, {
        scope,
        mutationId: 7,
        intent,
      })
      expect(request.path).toBe("/api/sync/push")
      expect(request.init.credentials).toBe("same-origin")
      const body = JSON.parse(String(request.init.body)) as {
        mutations: Array<{ name: string; argsJson: string }>
      }
      expect(body.mutations[0]?.name).toBe(mutator)
      expect(JSON.parse(body.mutations[0]?.argsJson ?? "{}")).toEqual(intent)
    }
  })
})

describe("Sarah FC-3 bounded cursor client", () => {
  test("propagates AbortSignal and rejects an abort during or after JSON decode", async () => {
    for (const abortDuringDecode of [true, false]) {
      const aborter = new AbortController()
      let observedSignal: AbortSignal | null | undefined
      const client = makeSarahFleetSyncClient({
        fetch: async (_path, init) => {
          observedSignal = init.signal
          return {
            ok: true,
            json: async () => {
              if (abortDuringDecode) {
                await new Promise<void>((_resolve, reject) => {
                  aborter.signal.addEventListener(
                    "abort",
                    () => reject(new DOMException("aborted", "AbortError")),
                    { once: true },
                  )
                  aborter.abort()
                })
              }
              aborter.abort()
              return {
                protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
                scope,
                entities: [],
                cursor: 0,
              }
            },
          } as Response
        },
        clientGroupId: "sarah.web.fc3",
        clientId: "sarah.web.fc3.tab",
      })

      await expect(
        client.bootstrap(scope, { signal: aborter.signal }),
      ).rejects.toMatchObject({ reason: "request_aborted" })
      expect(observedSignal).toBe(aborter.signal)
    }
  })

  test("drains bounded bootstrap pages and saves only the final exact cursor", async () => {
    const requests: Array<{ path: string; init: RequestInit }> = []
    const responses = [
      {
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope,
        entities: [],
        nextPageToken: "page-two",
      },
      {
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope,
        entities: [],
        cursor: 12,
      },
    ]
    const fetch: SarahFleetFetch = async (path, init) => {
      requests.push({ path, init })
      return json(responses.shift())
    }
    const client = makeSarahFleetSyncClient({
      fetch,
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })

    const result = await client.bootstrap(scope)
    expect(result.pages).toHaveLength(2)
    expect(result.state).toEqual(cursorState(12))
    expect(requests).toHaveLength(2)
    expect(requests.every((request) => request.init.credentials === "same-origin")).toBe(
      true,
    )
    expect(JSON.parse(String(requests[1]?.init.body))).toMatchObject({
      scope,
      pageToken: "page-two",
    })
  })

  test("fails closed on bootstrap token cycles and raw wrapper fields", async () => {
    const cyclic = makeSarahFleetSyncClient({
      fetch: async () =>
        json({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope,
          entities: [],
          nextPageToken: "same-token",
        }),
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
      maxBootstrapPages: 4,
    })
    await expect(cyclic.bootstrap(scope)).rejects.toMatchObject({
      reason: "pagination_cycle",
    })

    let page = 0
    const bounded = makeSarahFleetSyncClient({
      fetch: async () => {
        page += 1
        return json({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope,
          entities: [],
          nextPageToken: `page-${page}`,
        })
      },
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
      maxBootstrapPages: 2,
    })
    await expect(bounded.bootstrap(scope)).rejects.toMatchObject({
      reason: "pagination_limit",
    })

    const privateRaw = "PRIVATE RAW RESPONSE"
    const malformed = makeSarahFleetSyncClient({
      fetch: async () =>
        json({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope,
          entities: [],
          cursor: 1,
          rawPrompt: privateRaw,
        }),
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })
    const error = await malformed.bootstrap(scope).catch((failure) => failure)
    expect(error).toMatchObject({ reason: "malformed_response" })
    expect(JSON.stringify(error)).not.toContain(privateRaw)
  })

  test("resumes from the exact saved cursor without bootstrapping or latest ambiguity", async () => {
    const paths: string[] = []
    const responses = [
      {
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope,
        entries: [
          {
            scope,
            version: 13,
            entityType: "fleet_worker",
            entityId: "worker.fc3.codex",
            op: "upsert",
            postImageJson: "{}",
            committedAt: "2026-07-09T20:00:00.000Z",
          },
        ],
        nextCursor: 13,
        upToDate: false,
      },
      {
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope,
        entries: [],
        nextCursor: 13,
        upToDate: true,
      },
    ]
    const client = makeSarahFleetSyncClient({
      fetch: async (path) => {
        paths.push(path)
        return json(responses.shift())
      },
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })

    const result = await client.resume(cursorState(12))
    expect(result.state).toEqual(cursorState(13))
    expect(paths).toHaveLength(2)
    expect(paths.every((path) => path.startsWith("/api/sync/log?"))).toBe(true)
    expect(new URL(paths[0]!, "https://openagents.com").searchParams.get("cursor")).toBe(
      "12",
    )
    expect(new URL(paths[1]!, "https://openagents.com").searchParams.get("cursor")).toBe(
      "13",
    )
    expect(paths.join(" ")).not.toContain("latest")
    expect(paths.join(" ")).not.toContain("bootstrap")
  })

  test("rejects foreign scopes and non-progressing cursor pages", async () => {
    const foreign = makeSarahFleetSyncClient({
      fetch: async () =>
        json({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: "scope.fleet_run.foreign.run",
          entries: [],
          nextCursor: 12,
          upToDate: true,
        }),
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })
    await expect(foreign.resume(cursorState(12))).rejects.toMatchObject({
      reason: "foreign_scope",
    })

    const stalled = makeSarahFleetSyncClient({
      fetch: async () =>
        json({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope,
          entries: [],
          nextCursor: 12,
          upToDate: false,
        }),
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })
    await expect(stalled.resume(cursorState(12))).rejects.toMatchObject({
      reason: "cursor_no_progress",
    })
  })

  test("submits a typed intent without returning or echoing a steer body", async () => {
    const bodies: string[] = []
    const client = makeSarahFleetSyncClient({
      fetch: async (_path, init) => {
        bodies.push(String(init.body))
        return json({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          results: [{ mutationId: 3, status: "applied" }],
          lastMutationId: 3,
        })
      },
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })
    const receipt = await client.submitIntent({
      scope,
      mutationId: 3,
      intent: steerIntent,
    })

    expect(receipt).toEqual({
      intentId: "intent.fc3.steer",
      mutationId: 3,
      status: "applied",
      lastMutationId: 3,
    })
    expect(JSON.stringify(receipt)).not.toContain(steerBody)
    expect(bodies[0]).toContain(steerBody)

    const networkFailure = makeSarahFleetSyncClient({
      fetch: async () => {
        throw new Error(steerBody)
      },
      clientGroupId: "sarah.web.fc3",
      clientId: "sarah.web.fc3.tab",
    })
    const error = await networkFailure
      .submitIntent({ scope, mutationId: 4, intent: steerIntent })
      .catch((failure) => failure)
    expect(error).toMatchObject({
      reason: "network_unavailable",
      message: "Fleet Sync network request failed.",
    })
    expect(JSON.stringify(error)).not.toContain(steerBody)
  })
})
