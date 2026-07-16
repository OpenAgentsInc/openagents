import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Schema } from "effect"

import {
  FULL_AUTO_CONTROL_ROUTES,
  FULL_AUTO_CONTROL_SCHEMA,
  FULL_AUTO_CONTROL_TURNS_LIMIT,
  FullAutoControlContinueNowResponseSchema,
  FullAutoControlListResponseSchema,
  FullAutoControlMutationResponseSchema,
  FullAutoControlStatusResponseSchema,
  FullAutoControlTurnsResponseSchema,
  decodeFullAutoControlFile,
} from "./full-auto-control-contract.ts"
import { fullAutoControlOpenApiDocument } from "./full-auto-control-openapi.ts"
import {
  FULL_AUTO_CONTROL_SCOPES,
  isFullAutoControlEnabled,
  mintFullAutoControlCredential,
  startFullAutoControlServer,
  type FullAutoControlServer,
} from "./full-auto-control-server.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { LOCAL_TURN_RECORD_SCHEMA, type LocalTurnRecord } from "./local-turn-journal.ts"
import { readFileSync } from "node:fs"
import {
  readControlConnection,
  verifyControlProcessIdentity,
} from "../scripts/full-auto-control-client.ts"

const GRANTED_WORKSPACE = "/granted/full-auto/workspace"

const makeTurn = (input: Readonly<{
  threadRef: string
  turnRef: string
  updatedAt: string
  phase?: LocalTurnRecord["phase"]
  disposition?: LocalTurnRecord["disposition"]
}>): LocalTurnRecord => ({
  schema: LOCAL_TURN_RECORD_SCHEMA,
  threadRef: input.threadRef,
  turnRef: input.turnRef,
  lane: "codex-local",
  userMessageKey: `${input.turnRef}-user`,
  assistantMessageKey: `${input.turnRef}-assistant`,
  accountRef: "codex",
  providerSessionRef: null,
  model: "gpt-5.6-sol",
  phase: input.phase ?? "completed",
  persistedCursor: 0,
  assistantText: "SECRET transcript text that must never be served",
  assistantSegments: [],
  recoveryGeneration: 0,
  disposition: input.disposition ?? "completed",
  createdAt: input.updatedAt,
  updatedAt: input.updatedAt,
})

type Harness = Readonly<{
  root: string
  registry: ReturnType<typeof openFullAutoRegistry>
  notes: Array<Readonly<{ threadRef: string; text: string }>>
  turns: Array<LocalTurnRecord>
  liveMap: Map<string, Readonly<{ state: "idle" | "turn_running" | "turn_completed" | "turn_failed" | "cap_reached" | "blocked"; turnRef: string | null; detail?: string }>>
  reconcileCalls: () => number
  createdThreads: Array<Readonly<{ threadRef: string; title: string | null }>>
  server: FullAutoControlServer
  request: (
    method: "GET" | "POST",
    pathname: string,
    options?: Readonly<{ token?: string | null; body?: unknown }>,
  ) => Promise<Readonly<{ status: number; body: any }>>
  dispose: () => Promise<void>
}>

const startHarness = async (): Promise<Harness> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-control-"))
  const registry = openFullAutoRegistry(path.join(root, "registry.json"))
  const notes: Array<Readonly<{ threadRef: string; text: string }>> = []
  const turns: Array<LocalTurnRecord> = []
  const createdThreads: Array<Readonly<{ threadRef: string; title: string | null }>> = []
  const liveMap: Harness["liveMap"] = new Map()
  let reconcileCallCount = 0
  // The continue-now spy IS the injected trigger -- the server must invoke
  // this exact function (main passes runFullAutoReconciliation the same way).
  const triggerReconciliation = async (): Promise<void> => {
    reconcileCallCount += 1
  }
  const server = await startFullAutoControlServer({
    capabilities: {
      registry,
      resolveWorkspaceRef: () => GRANTED_WORKSPACE,
      triggerReconciliation,
      liveState: threadRef => liveMap.get(threadRef) ?? null,
      listTurns: threadRef => turns.filter(record => record.threadRef === threadRef),
      appendSystemNote: (threadRef, text) => notes.push({ threadRef, text }),
      createThread: title => {
        const threadRef = `thread.started.${createdThreads.length + 1}`
        createdThreads.push({ threadRef, title })
        return threadRef
      },
      listLanes: async () => [{
        laneRef: "peer.cursor",
        provider: "cursor",
        profileRef: "acp:cursor",
        configuration: "unconfigured",
        authentication: "missing",
        admission: "quarantined",
        reason: "Peer profile is not admitted.",
        capabilities: {
          laneRef: "peer.cursor",
          provider: "cursor",
          displayName: "Cursor",
          admission: "quarantined",
          reason: "Peer profile is not admitted.",
          models: [], reasoningEfforts: [], permissionModes: [], approvals: "none",
          questions: false, skills: false, images: false, fullAuto: false,
          interrupt: false, queueFollowup: false, steerTurn: false,
          extensions: [], evidence: "experimental",
        },
      }],
      isLaneEligible: laneRef => laneRef === "codex-local" || laneRef === "fable-local",
    },
    controlFilePath: path.join(root, "full-auto", "control.json"),
  })
  const request: Harness["request"] = async (method, pathname, options) => {
    const token = options?.token === undefined ? server.credential.token : options.token
    const response = await fetch(`${server.url}${pathname}`, {
      method,
      headers: {
        ...(token === null ? {} : { authorization: `Bearer ${token}` }),
        ...(options?.body === undefined ? {} : { "content-type": "application/json" }),
      },
      ...(options?.body === undefined ? {} : { body: JSON.stringify(options.body) }),
    })
    return { status: response.status, body: await response.json() }
  }
  return {
    root,
    registry,
    notes,
    turns,
    liveMap,
    reconcileCalls: () => reconcileCallCount,
    createdThreads,
    server,
    request,
    dispose: async () => {
      await server.stop()
      rmSync(root, { recursive: true, force: true })
    },
  }
}

describe("Full Auto control surface (FA-H13 #8886)", () => {
  test("lists unavailable and unadmitted lanes honestly through the bearer-gated route", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("GET", "/v1/lanes")
      expect(result.status).toBe(200)
      expect(result.body.lanes).toEqual([expect.objectContaining({
        laneRef: "peer.cursor",
        authentication: "missing",
        admission: "quarantined",
        reason: "Peer profile is not admitted.",
      })])
    } finally { await harness.dispose() }
  })
  test("off by default: main's guard requires OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 exactly", () => {
    expect(isFullAutoControlEnabled({})).toBe(false)
    expect(isFullAutoControlEnabled({ OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL: undefined })).toBe(false)
    expect(isFullAutoControlEnabled({ OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL: "0" })).toBe(false)
    expect(isFullAutoControlEnabled({ OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL: "true" })).toBe(false)
    expect(isFullAutoControlEnabled({ OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL: "1" })).toBe(true)
  })

  test("credential mint uses the environment-auth narrowing-only exchange: scopes are the declared set and widening rejects", () => {
    const credential = mintFullAutoControlCredential()
    expect([...credential.scopes].sort()).toEqual([...FULL_AUTO_CONTROL_SCOPES].sort())
    expect(credential.token.startsWith("oafa_")).toBe(true)
    expect(() => mintFullAutoControlCredential({ requestedScopes: ["admin"] })).toThrow(/rejected/)
  })

  test("auth: no bearer and a wrong bearer are 401 on every route; the minted bearer is accepted", async () => {
    const harness = await startHarness()
    try {
      for (const pathname of ["/v1/openapi.json", "/v1/full-auto", "/v1/full-auto/thread.x"]) {
        const missing = await harness.request("GET", pathname, { token: null })
        expect(missing.status).toBe(401)
        expect(missing.body.error).toBe("unauthorized")
        const wrong = await harness.request("GET", pathname, { token: "oafa_not-the-token" })
        expect(wrong.status).toBe(401)
      }
      const ok = await harness.request("GET", "/v1/full-auto")
      expect(ok.status).toBe(200)
    } finally {
      await harness.dispose()
    }
  })

  test("the connection file is written mode 0600 with the url and token a local agent needs", async () => {
    const harness = await startHarness()
    try {
      const filePath = path.join(harness.root, "full-auto", "control.json")
      const decoded = decodeFullAutoControlFile(JSON.parse(readFileSync(filePath, "utf8")))
      expect(decoded).not.toBeNull()
      expect(decoded!.url).toBe(harness.server.url)
      expect(decoded!.token).toBe(harness.server.credential.token)
      expect(decoded!.pid).toBe(process.pid)
      expect(decoded!.serverInstanceId).toBe(harness.server.instanceId)
      const list = Schema.decodeUnknownSync(FullAutoControlListResponseSchema)(
        (await harness.request("GET", "/v1/full-auto")).body,
      )
      expect(list.serverInstanceId).toBe(decoded!.serverInstanceId)
      const verified = await verifyControlProcessIdentity(readControlConnection(harness.root))
      expect(verified).toEqual({
        pid: process.pid,
        serverInstanceId: harness.server.instanceId,
      })
      if (process.platform !== "win32") {
        expect(statSync(filePath).mode & 0o777).toBe(0o600)
      }
    } finally {
      await harness.dispose()
    }
  })

  test("legacy v1 connection files without ownership fields still decode, but expose no signal authority", () => {
    const decoded = decodeFullAutoControlFile({
      schema: FULL_AUTO_CONTROL_SCHEMA,
      url: "http://127.0.0.1:49999",
      token: "oafa_legacy-token-long-enough",
      scopes: ["operator_read"],
      issuedAtIso: "2026-07-16T15:00:00.000Z",
    })
    expect(decoded).not.toBeNull()
    expect(decoded?.pid).toBeUndefined()
    expect(decoded?.serverInstanceId).toBeUndefined()
  })

  test("process-identity guard fails closed for legacy and mismatched instance evidence", async () => {
    await expect(verifyControlProcessIdentity({
      url: "http://127.0.0.1:49999",
      token: "oafa_legacy-token-long-enough",
    })).rejects.toThrow("do not signal a process")

    const harness = await startHarness()
    try {
      await expect(verifyControlProcessIdentity({
        url: harness.server.url,
        token: harness.server.credential.token,
        pid: process.pid,
        serverInstanceId: "oafa_instance_intentionally_wrong",
      })).rejects.toThrow("did not echo")
    } finally {
      await harness.dispose()
    }
  })

  test("enable with a mismatched workspaceRef is a 409 typed refusal and the registry is untouched", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("POST", "/v1/full-auto/thread.a/enable", {
        body: { workspaceRef: "/somewhere/else" },
      })
      expect(result.status).toBe(409)
      expect(result.body.error).toBe("workspace_mismatch")
      expect(result.body.expectedWorkspaceRef).toBe("/somewhere/else")
      expect(result.body.resolvedWorkspaceRef).toBe(GRANTED_WORKSPACE)
      // Refusal, not redirect: nothing was written, nothing was enabled.
      expect(harness.registry.record("thread.a")).toBeNull()
      expect(harness.notes.filter(note => note.text.includes("enabled"))).toHaveLength(0)
    } finally {
      await harness.dispose()
    }
  })

  test("enable with the matching workspaceRef enables + binds the record and appends the distinctly-attributed note", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("POST", "/v1/full-auto/thread.a/enable", {
        body: { workspaceRef: GRANTED_WORKSPACE },
      })
      expect(result.status).toBe(200)
      const decoded = Schema.decodeUnknownSync(FullAutoControlMutationResponseSchema)(result.body)
      expect(decoded.record.enabled).toBe(true)
      expect(decoded.record.workspaceRef).toBe(GRANTED_WORKSPACE)
      const record = harness.registry.record("thread.a")
      expect(record?.enabled).toBe(true)
      expect(record?.workspaceRef).toBe(GRANTED_WORKSPACE)
      expect(harness.notes).toHaveLength(1)
      expect(harness.notes[0]!.threadRef).toBe("thread.a")
      expect(harness.notes[0]!.text).toContain("enabled programmatically")
      expect(harness.notes[0]!.text).toContain("control-api")
    } finally {
      await harness.dispose()
    }
  })

  test("enable accepts an admitted lane selector, persists it, and rejects an ineligible lane without mutation", async () => {
    const harness = await startHarness()
    try {
      const enabled = await harness.request("POST", "/v1/full-auto/thread.claude/enable", {
        body: { workspaceRef: GRANTED_WORKSPACE, lane: "fable-local" },
      })
      expect(enabled.status).toBe(200)
      expect(enabled.body.record.lane).toBe("fable-local")
      expect(harness.registry.record("thread.claude")?.profile?.lane).toBe("fable-local")

      const refused = await harness.request("POST", "/v1/full-auto/thread.peer/enable", {
        body: { workspaceRef: GRANTED_WORKSPACE, lane: "acp-unadmitted" },
      })
      expect(refused.status).toBe(409)
      expect(refused.body.error).toBe("lane_not_eligible")
      expect(harness.registry.record("thread.peer")).toBeNull()
    } finally {
      await harness.dispose()
    }
  })

  test("start with the matching workspaceRef mints a thread, enables + binds it, notes it, and schedules reconcile", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("POST", "/v1/full-auto/start", {
        body: { workspaceRef: GRANTED_WORKSPACE, title: "Dogfood lane" },
      })
      expect(result.status).toBe(200)
      const decoded = Schema.decodeUnknownSync(FullAutoControlMutationResponseSchema)(result.body)
      expect(decoded.record.enabled).toBe(true)
      expect(decoded.record.workspaceRef).toBe(GRANTED_WORKSPACE)
      // Main minted the ref -- the response carries it back to the caller.
      expect(harness.createdThreads).toHaveLength(1)
      expect(harness.createdThreads[0]!.title).toBe("Dogfood lane")
      expect(decoded.record.threadRef).toBe(harness.createdThreads[0]!.threadRef)
      const record = harness.registry.record(decoded.record.threadRef)
      expect(record?.enabled).toBe(true)
      expect(record?.workspaceRef).toBe(GRANTED_WORKSPACE)
      expect(harness.notes.at(-1)!.threadRef).toBe(decoded.record.threadRef)
      expect(harness.notes.at(-1)!.text).toContain("started programmatically")
      expect(harness.notes.at(-1)!.text).toContain("control-api")
      // Bootstrap schedules the shared serialized reconcile pass itself.
      expect(harness.reconcileCalls()).toBe(1)
    } finally {
      await harness.dispose()
    }
  })

  test("start with a mismatched workspaceRef is a 409 typed refusal: NO thread minted, registry untouched", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("POST", "/v1/full-auto/start", {
        body: { workspaceRef: "/somewhere/else" },
      })
      expect(result.status).toBe(409)
      expect(result.body.error).toBe("workspace_mismatch")
      expect(result.body.expectedWorkspaceRef).toBe("/somewhere/else")
      expect(result.body.resolvedWorkspaceRef).toBe(GRANTED_WORKSPACE)
      expect(harness.createdThreads).toHaveLength(0)
      expect(harness.registry.list()).toHaveLength(0)
      expect(harness.notes).toHaveLength(0)
      expect(harness.reconcileCalls()).toBe(0)
    } finally {
      await harness.dispose()
    }
  })

  test("start discipline: bodyless start is 400 with nothing minted, and GET /v1/full-auto/start is 405", async () => {
    const harness = await startHarness()
    try {
      const invalid = await harness.request("POST", "/v1/full-auto/start")
      expect(invalid.status).toBe(400)
      expect(invalid.body.error).toBe("invalid_request")
      const wrongVerb = await harness.request("GET", "/v1/full-auto/start")
      expect(wrongVerb.status).toBe(405)
      expect(harness.createdThreads).toHaveLength(0)
      expect(harness.registry.list()).toHaveLength(0)
    } finally {
      await harness.dispose()
    }
  })

  test("disable durably disables and appends the distinctly-attributed note", async () => {
    const harness = await startHarness()
    try {
      harness.registry.set("thread.a", true, { workspaceRef: GRANTED_WORKSPACE })
      const result = await harness.request("POST", "/v1/full-auto/thread.a/disable")
      expect(result.status).toBe(200)
      const decoded = Schema.decodeUnknownSync(FullAutoControlMutationResponseSchema)(result.body)
      expect(decoded.record.enabled).toBe(false)
      expect(decoded.record.disabledBy).toBe("control_api")
      expect(decoded.record.disabledAt).not.toBeNull()
      expect(harness.registry.record("thread.a")?.enabled).toBe(false)
      expect(harness.notes.at(-1)!.text).toContain("disabled programmatically")
      expect(harness.notes.at(-1)!.text).toContain("control-api")
    } finally {
      await harness.dispose()
    }
  })

  test("authenticated status echoes this exact server instance identity", async () => {
    const harness = await startHarness()
    try {
      harness.registry.set("thread.identity", true, { workspaceRef: GRANTED_WORKSPACE })
      const result = await harness.request("GET", "/v1/full-auto/thread.identity")
      expect(result.status).toBe(200)
      const decoded = Schema.decodeUnknownSync(FullAutoControlStatusResponseSchema)(result.body)
      expect(decoded.serverInstanceId).toBe(harness.server.instanceId)
    } finally {
      await harness.dispose()
    }
  })

  test("continue-now invokes the injected reconcile trigger exactly once and returns { scheduled: true }", async () => {
    const harness = await startHarness()
    try {
      harness.registry.set("thread.a", true, { workspaceRef: GRANTED_WORKSPACE })
      const result = await harness.request("POST", "/v1/full-auto/thread.a/continue-now")
      expect(result.status).toBe(200)
      const decoded = Schema.decodeUnknownSync(FullAutoControlContinueNowResponseSchema)(result.body)
      expect(decoded.scheduled).toBe(true)
      // Fire-and-forget lands on the microtask queue; settle it before asserting.
      await new Promise(resolve => setTimeout(resolve, 0))
      expect(harness.reconcileCalls()).toBe(1)
      expect(harness.notes.at(-1)!.text).toContain("continuation requested programmatically")
      expect(harness.notes.at(-1)!.text).toContain("control-api")
    } finally {
      await harness.dispose()
    }
  })

  test("continue-now on an unknown threadRef is a 404 and never touches the trigger", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("POST", "/v1/full-auto/thread.unknown/continue-now")
      expect(result.status).toBe(404)
      expect(result.body.error).toBe("not_found")
      await new Promise(resolve => setTimeout(resolve, 0))
      expect(harness.reconcileCalls()).toBe(0)
      expect(harness.notes).toHaveLength(0)
    } finally {
      await harness.dispose()
    }
  })

  test("list and status match the contract schemas, carry live state, and expose no profile material beyond accountRef", async () => {
    const harness = await startHarness()
    try {
      harness.registry.set("thread.a", true, {
        workspaceRef: GRANTED_WORKSPACE,
        profile: { accountRef: "codex", model: "gpt-5.6-sol", reasoningEffort: "high" },
      })
      harness.liveMap.set("thread.a", { state: "turn_running", turnRef: "turn.full-auto.live-1" })
      const list = await harness.request("GET", "/v1/full-auto")
      expect(list.status).toBe(200)
      const decodedList = Schema.decodeUnknownSync(FullAutoControlListResponseSchema)(list.body)
      expect(decodedList.schema).toBe(FULL_AUTO_CONTROL_SCHEMA)
      expect(decodedList.records).toHaveLength(1)
      expect(decodedList.records[0]!.accountRef).toBe("codex")
      // Never raw profile material beyond accountRef.
      expect(JSON.stringify(list.body)).not.toContain("gpt-5.6-sol")
      expect(JSON.stringify(list.body)).not.toContain("reasoningEffort")
      const status = await harness.request("GET", "/v1/full-auto/thread.a")
      expect(status.status).toBe(200)
      const decodedStatus = Schema.decodeUnknownSync(FullAutoControlStatusResponseSchema)(status.body)
      expect(decodedStatus.record.live.state).toBe("turn_running")
      expect(decodedStatus.record.live.turnRef).toBe("turn.full-auto.live-1")
      const missing = await harness.request("GET", "/v1/full-auto/thread.unknown")
      expect(missing.status).toBe(404)
    } finally {
      await harness.dispose()
    }
  })

  test("turns returns a bounded, most-recent-first Full Auto projection with no transcript text", async () => {
    const harness = await startHarness()
    try {
      for (let index = 0; index < FULL_AUTO_CONTROL_TURNS_LIMIT + 5; index++) {
        harness.turns.push(makeTurn({
          threadRef: "thread.a",
          turnRef: `turn.full-auto.${index}`,
          updatedAt: new Date(Date.UTC(2026, 6, 16, 0, 0, index)).toISOString(),
        }))
      }
      // A manual (non-Full-Auto) turn on the same thread stays out of the history.
      harness.turns.push(makeTurn({
        threadRef: "thread.a",
        turnRef: "turn.manual-send",
        updatedAt: new Date(Date.UTC(2026, 6, 16, 1, 0, 0)).toISOString(),
      }))
      const result = await harness.request("GET", "/v1/full-auto/thread.a/turns")
      expect(result.status).toBe(200)
      const decoded = Schema.decodeUnknownSync(FullAutoControlTurnsResponseSchema)(result.body)
      expect(decoded.turns).toHaveLength(FULL_AUTO_CONTROL_TURNS_LIMIT)
      expect(decoded.turns[0]!.turnRef).toBe(`turn.full-auto.${FULL_AUTO_CONTROL_TURNS_LIMIT + 4}`)
      expect(decoded.turns.every(turn => turn.turnRef.startsWith("turn.full-auto."))).toBe(true)
      expect(JSON.stringify(result.body)).not.toContain("SECRET transcript")
      expect(JSON.stringify(result.body)).not.toContain("assistantText")
    } finally {
      await harness.dispose()
    }
  })

  test("schema validation: a bodyless/invalid enable is 400 and an over-long threadRef is 400", async () => {
    const harness = await startHarness()
    try {
      const noBody = await harness.request("POST", "/v1/full-auto/thread.a/enable")
      expect(noBody.status).toBe(400)
      expect(noBody.body.error).toBe("invalid_request")
      const badBody = await harness.request("POST", "/v1/full-auto/thread.a/enable", {
        body: { workspaceRef: 42 },
      })
      expect(badBody.status).toBe(400)
      const longRef = await harness.request("GET", `/v1/full-auto/${"x".repeat(121)}`)
      expect(longRef.status).toBe(400)
      expect(harness.registry.list()).toHaveLength(0)
    } finally {
      await harness.dispose()
    }
  })

  test("method discipline: wrong verbs are 405 and unknown routes are 404", async () => {
    const harness = await startHarness()
    try {
      expect((await harness.request("POST", "/v1/full-auto")).status).toBe(405)
      expect((await harness.request("POST", "/v1/openapi.json")).status).toBe(405)
      expect((await harness.request("GET", "/v1/full-auto/thread.a/enable")).status).toBe(405)
      expect((await harness.request("GET", "/v1/nope")).status).toBe(404)
    } finally {
      await harness.dispose()
    }
  })

  test("GET /v1/openapi.json serves the document, and the document <-> served routes agree in both directions", async () => {
    const harness = await startHarness()
    try {
      const result = await harness.request("GET", "/v1/openapi.json")
      expect(result.status).toBe(200)
      expect(result.body.openapi).toBe("3.1.0")
      const doc = result.body as typeof fullAutoControlOpenApiDocument
      // Every route in the shared table is described by the served document.
      for (const route of FULL_AUTO_CONTROL_ROUTES) {
        const pathItem = (doc.paths as Record<string, Record<string, { operationId?: string }>>)[route.path]
        expect(pathItem, `OpenAPI document is missing path ${route.path}`).toBeDefined()
        const operation = pathItem![route.method]
        expect(operation, `OpenAPI document is missing ${route.method.toUpperCase()} ${route.path}`).toBeDefined()
        expect(operation!.operationId).toBe(route.operationId)
      }
      // Every operation in the document is a route the server actually serves.
      const tableKeys = new Set(FULL_AUTO_CONTROL_ROUTES.map(route => `${route.method} ${route.path}`))
      for (const [docPath, operations] of Object.entries(doc.paths as Record<string, Record<string, unknown>>)) {
        for (const method of Object.keys(operations)) {
          expect(tableKeys.has(`${method} ${docPath}`), `document describes unserved ${method} ${docPath}`).toBe(true)
        }
      }
      // The document also matches the module const byte-for-byte semantics.
      expect(result.body).toEqual(JSON.parse(JSON.stringify(fullAutoControlOpenApiDocument)))
    } finally {
      await harness.dispose()
    }
  })
})
