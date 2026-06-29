import { describe, expect, test } from "bun:test"
import { healthFixture } from "@openagentsinc/autopilot-control-protocol/fixtures"
import {
  cancelSession,
  fetchAppleFmReadiness,
  fetchNodeState,
  resolveApproval,
  setCoordinatorPaused,
  startAppleFmSession,
  spawnSession,
  submitIntent,
} from "../src/bun/pylon-control"

// A tiny fetch stub that routes by the POSTed command `type` (and GET /health).
function stubFetch(routes: Record<string, unknown>): typeof fetch {
  return (async (url: string, init?: RequestInit) => {
    if (String(url).endsWith("/health")) {
      return new Response(JSON.stringify(routes["__health"] ?? healthFixture), {
        status: 200,
      })
    }
    const body = init?.body ? JSON.parse(String(init.body)) : {}
    const payload = routes[body.type as string]
    if (payload === undefined) return new Response(JSON.stringify({ ok: true, result: [] }), { status: 200 })
    return new Response(JSON.stringify(payload), { status: 200 })
  }) as unknown as typeof fetch
}

const base = { baseUrl: "http://127.0.0.1:4716", token: "tok" }

describe("CL-46 control verbs", () => {
  test("submitIntent returns the ship status", async () => {
    const r = await submitIntent({
      ...base,
      title: "ship it",
      body: "",
      fetchFn: stubFetch({ "intent.submit": { ok: true, result: { status: "received" } } }),
    })
    expect(r).toEqual({ ok: true, status: "received" })
  })

  test("submitIntent surfaces a node failure honestly", async () => {
    const r = await submitIntent({
      ...base,
      title: "x",
      body: "",
      fetchFn: stubFetch({ "intent.submit": { ok: false } }),
    })
    expect(r.ok).toBe(false)
  })

  test("resolveApproval reports applied + duplicate", async () => {
    const r = await resolveApproval({
      ...base,
      approvalRef: "a1",
      decision: "approve",
      fetchFn: stubFetch({ "approvals.resolve": { ok: true, result: { applied: true, duplicate: false, decision: "approve" } } }),
    })
    expect(r).toEqual({ applied: true, duplicate: false, decision: "approve" })
  })

  test("setCoordinatorPaused echoes the resulting flag", async () => {
    const r = await setCoordinatorPaused({
      ...base,
      paused: true,
      fetchFn: stubFetch({ "coordinator.pause": { ok: true, result: { paused: true } } }),
    })
    expect(r.paused).toBe(true)
  })

  test("cancelSession returns the new state", async () => {
    const r = await cancelSession({
      ...base,
      sessionRef: "s1",
      fetchFn: stubFetch({ "session.cancel": { ok: true, result: { state: "cancelled" } } }),
    })
    expect(r).toEqual({ ok: true, state: "cancelled" })
  })

  test("spawnSession returns the new session ref", async () => {
    const r = await spawnSession({
      ...base,
      adapter: "codex",
      objective: "do the thing",
      fetchFn: stubFetch({ "session.spawn": { ok: true, result: { sessionRef: "sess-123" } } }),
    })
    expect(r).toEqual({ ok: true, sessionRef: "sess-123" })
  })

  // #4998: the lane selector must round-trip onto the session.spawn command body.
  test("spawnSession forwards the requested cloud-gcp lane", async () => {
    let captured: Record<string, unknown> | null = null
    const captureFetch = (async (url: string, init?: RequestInit) => {
      if (String(url).endsWith("/health")) {
        return new Response(JSON.stringify(healthFixture), { status: 200 })
      }
      captured = init?.body ? JSON.parse(String(init.body)) : null
      return new Response(JSON.stringify({ ok: true, result: { sessionRef: "sess-gce" } }), { status: 200 })
    }) as unknown as typeof fetch
    const r = await spawnSession({
      ...base,
      adapter: "codex",
      objective: "deploy to gce",
      lane: "cloud-gcp",
      timeoutSeconds: 600,
      worktreePath: "/tmp/openagents-builtin-agent",
      fetchFn: captureFetch,
    })
    expect(r).toEqual({ ok: true, sessionRef: "sess-gce" })
    expect(captured).not.toBeNull()
    expect(captured).toMatchObject({
      type: "session.spawn",
      lane: "cloud-gcp",
      timeoutSeconds: 600,
      worktreePath: "/tmp/openagents-builtin-agent",
    })
  })

  // #4998: omitting the lane must not put a lane key on the wire (node defaults).
  test("spawnSession omits lane when none is requested and supplies no-op verify", async () => {
    let captured: Record<string, unknown> | null = null
    const captureFetch = (async (url: string, init?: RequestInit) => {
      if (String(url).endsWith("/health")) {
        return new Response(JSON.stringify(healthFixture), { status: 200 })
      }
      captured = init?.body ? JSON.parse(String(init.body)) : null
      return new Response(JSON.stringify({ ok: true, result: { sessionRef: "sess-x" } }), { status: 200 })
    }) as unknown as typeof fetch
    await spawnSession({ ...base, adapter: "codex", objective: "x", fetchFn: captureFetch })
    expect(captured).not.toBeNull()
    expect(Object.prototype.hasOwnProperty.call(captured ?? {}, "lane")).toBe(false)
    expect(captured).toMatchObject({ verify: ["true"] })
  })

  // #5453: when the node rejects a command (e.g. HTTP 400 typed validation),
  // spawnSession surfaces the node's typed error message rather than an opaque
  // `control <status>`. This is the path the desktop composer once showed as a
  // raw `control 500`; it now carries an honest reason.
  test("spawnSession surfaces the node's typed error body over a bare control status", async () => {
    const typedErrorFetch = (async (url: string) => {
      if (String(url).endsWith("/health")) {
        return new Response(JSON.stringify(healthFixture), { status: 200 })
      }
      return new Response(
        JSON.stringify({
          ok: false,
          error: "session.spawn must use only one workspace selector",
          reason: "workspace_selector_conflict",
        }),
        { status: 400 },
      )
    }) as unknown as typeof fetch
    const r = await spawnSession({ ...base, adapter: "codex", objective: "x", fetchFn: typedErrorFetch })
    expect(r.ok).toBe(false)
    expect(r.error).toBe("session.spawn must use only one workspace selector")
  })

  // #5453: if the node returns a non-ok status with no parseable body, fall
  // back to the legacy `control <status>` string so the composer still shows
  // something honest.
  test("spawnSession falls back to control <status> when the error body is unparseable", async () => {
    const opaqueFetch = (async (url: string) => {
      if (String(url).endsWith("/health")) {
        return new Response(JSON.stringify(healthFixture), { status: 200 })
      }
      return new Response("not json", { status: 503 })
    }) as unknown as typeof fetch
    const r = await spawnSession({ ...base, adapter: "codex", objective: "x", fetchFn: opaqueFetch })
    expect(r).toEqual({ ok: false, sessionRef: "", error: "control 503" })
  })

  test("fetchNodeState aggregates the parity surfaces", async () => {
    const state = await fetchNodeState({
      ...base,
      fetchFn: stubFetch({
        "session.list": { ok: true, result: [] },
        "intent.list": { ok: true, result: { intents: [{ intentId: "i1", title: "t", status: "shipping" }] } },
        "approvals.list": { ok: true, result: { approvals: [{ approvalRef: "ap1", kind: "exec", prompt: "ok?" }] } },
        "wallet.status": { ok: true, result: { configured: true, daemonOnline: true, balanceSats: 1234, readiness: "ready" } },
        "assignments.poll": { ok: true, result: [{ assignmentRef: "as1", leaseRef: "l1", goal: "g", paymentMode: "credits" }] },
        "coordinator.status": { ok: true, result: { paused: true } },
        "deploy.status": { ok: true, result: { state: "deployed", message: "live" } },
        "accounts.list": { ok: true, result: { accounts: [] } },
      }),
    })
    expect(state.intents[0]?.status).toBe("shipping")
    expect(state.approvals[0]?.approvalRef).toBe("ap1")
    expect(state.wallet?.balanceSats).toBe(1234)
    expect(state.assignments[0]?.paymentMode).toBe("credits")
    expect(state.coordinatorPaused).toBe(true)
  })

  test("fetchNodeState fetches proof external-session events for redacted control sessions", async () => {
    const sessionRef = "session.pylon.control.5ba05b978a0a8a8b5cc91551"
    const externalSessionRef = "session.pylon.codex_composer.be4d2b8c1eb3512e70bf59be"
    const fetchFn = (async (url: string, init?: RequestInit) => {
      if (String(url).endsWith("/health")) {
        return new Response(JSON.stringify(healthFixture), { status: 200 })
      }
      const body = init?.body ? JSON.parse(String(init.body)) : {}
      if (body.type === "session.list") {
        return new Response(JSON.stringify({
          ok: true,
          result: [{
            sessionRef,
            adapter: "codex",
            state: "completed",
            accountRefHash: null,
            updatedAt: "2026-06-20T02:47:10.326Z",
          }],
        }), { status: 200 })
      }
      if (body.type === "session.events" && body.sessionRef === sessionRef) {
        return new Response(JSON.stringify({
          ok: true,
          result: {
            recentEvents: [
              { eventIndex: 4, phase: "redaction_blocked", state: "running", observedAt: "t" },
            ],
          },
        }), { status: 200 })
      }
      if (body.type === "session.artifact") {
        return new Response(JSON.stringify({
          ok: true,
          result: {
            kind: "proof",
            artifact: {
              schema: "openagents.pylon.control_session_artifact.v0.1",
              executor: {
                outcome: "completed",
                editedFileCount: 0,
                commandCount: 0,
                totalTokens: 12,
                externalSessionRef,
              },
              task: {},
              devCheck: { state: "passed" },
              redactionScan: { state: "clean" },
              deviations: [],
            },
          },
        }), { status: 200 })
      }
      if (body.type === "session.events" && body.sessionRef === externalSessionRef) {
        return new Response(JSON.stringify({
          ok: true,
          result: {
            recentEvents: [
              {
                eventIndex: 3,
                phase: "agent_message",
                state: "completed",
                observedAt: "t",
                messageText: "agent: I am Codex.",
              },
            ],
          },
        }), { status: 200 })
      }
      return new Response(JSON.stringify({ ok: true, result: [] }), { status: 200 })
    }) as unknown as typeof fetch

    const state = await fetchNodeState({ ...base, fetchFn })

    expect(state.artifacts[sessionRef]?.detail?.externalSessionRef).toBe(externalSessionRef)
    expect(state.events[externalSessionRef]?.[0]?.detail).toBe("agent: I am Codex.")
  })

  test("fetchNodeState follows live external-session refs from running control events", async () => {
    const sessionRef = "session.pylon.control.live"
    const externalSessionRef = "session.pylon.codex_composer.live"
    const fetchFn = (async (url: string, init?: RequestInit) => {
      if (String(url).endsWith("/health")) {
        return new Response(JSON.stringify(healthFixture), { status: 200 })
      }
      const body = init?.body ? JSON.parse(String(init.body)) : {}
      if (body.type === "session.list") {
        return new Response(JSON.stringify({
          ok: true,
          result: [{
            sessionRef,
            adapter: "codex",
            state: "running",
            accountRefHash: null,
            updatedAt: "2026-06-20T02:47:10.326Z",
          }],
        }), { status: 200 })
      }
      if (body.type === "session.events" && body.sessionRef === sessionRef) {
        return new Response(JSON.stringify({
          ok: true,
          result: {
            recentEvents: [
              {
                eventIndex: 2,
                phase: "composer_event",
                state: "running",
                observedAt: "t",
                messageText: `external session: ${externalSessionRef}`,
              },
            ],
          },
        }), { status: 200 })
      }
      if (body.type === "session.events" && body.sessionRef === externalSessionRef) {
        return new Response(JSON.stringify({
          ok: true,
          result: {
            recentEvents: [
              {
                eventIndex: 3,
                phase: "reasoning",
                state: "running",
                observedAt: "t",
                messageText: "thinking tokens: 5; output tokens: 12",
              },
              {
                eventIndex: 4,
                phase: "agent_message",
                state: "running",
                observedAt: "t",
                messageText: "agent: streaming now",
              },
            ],
          },
        }), { status: 200 })
      }
      return new Response(JSON.stringify({ ok: true, result: [] }), { status: 200 })
    }) as unknown as typeof fetch

    const state = await fetchNodeState({ ...base, fetchFn })

    expect(state.events[externalSessionRef]?.map(event => event.detail)).toEqual([
      "thinking tokens: 5; output tokens: 12",
      "agent: streaming now",
    ])
  })

  test("fetchAppleFmReadiness normalizes ready Pylon status", async () => {
    let captured: Record<string, unknown> | null = null
    const readiness = await fetchAppleFmReadiness({
      ...base,
      fetchFn: (async (url: string, init?: RequestInit) => {
        captured = init?.body ? JSON.parse(String(init.body)) : null
        return new Response(JSON.stringify({
          ok: true,
          result: {
            backendKind: "apple_fm_bridge",
            profileId: "apple-fm-local",
            model: "apple-foundation-model",
            capability: "probe.backend.apple_fm_bridge",
            advertisedCapabilities: ["probe.backend.apple_fm_bridge"],
            available: true,
            status: "ready",
            baseUrl: "http://127.0.0.1:11435",
            platform: "darwin-arm64",
            version: "fake-bridge",
            blockerRefs: [],
          },
        }), { status: 200 })
      }) as unknown as typeof fetch,
    })

    expect(captured).toMatchObject({ type: "apple_fm.status" })
    expect(readiness.ok).toBe(true)
    expect(readiness.available).toBe(true)
    expect(readiness.advertisedCapabilities).toContain("probe.backend.apple_fm_bridge")
    expect(readiness.blockerRefs).toEqual([])
  })

  test("fetchAppleFmReadiness preserves unsupported blocker refs", async () => {
    const readiness = await fetchAppleFmReadiness({
      ...base,
      fetchFn: stubFetch({
        "apple_fm.status": {
          ok: true,
          result: {
            available: false,
            status: "unsupported",
            unavailableReason: "unsupported_hardware",
            message: "device not eligible",
            blockerRefs: [
              "blocker.pylon.apple_fm.unsupported_hardware",
              "blocker.pylon.apple_fm.live_health_not_ready",
            ],
          },
        },
      }),
    })

    expect(readiness.ok).toBe(false)
    expect(readiness.status).toBe("unsupported")
    expect(readiness.unavailableReason).toBe("unsupported_hardware")
    expect(readiness.blockerRefs).toContain("blocker.pylon.apple_fm.unsupported_hardware")
  })

  test("startAppleFmSession posts the bounded local Apple FM command", async () => {
    let captured: Record<string, unknown> | null = null
    const result = await startAppleFmSession({
      ...base,
      prompt: "local prompt owned by Bun",
      worktreePath: "/tmp/openagents-builtin-agent",
      timeoutSeconds: 300,
      fetchFn: (async (_url: string, init?: RequestInit) => {
        captured = init?.body ? JSON.parse(String(init.body)) : null
        return new Response(JSON.stringify({
          ok: true,
          result: {
            ok: true,
            sessionRef: "session.pylon.apple_fm.local",
            blockerRefs: [],
          },
        }))
      }) as unknown as typeof fetch,
    })
    expect(result).toEqual({
      ok: true,
      sessionRef: "session.pylon.apple_fm.local",
      blockerRefs: [],
    })
    expect(captured).toEqual({
      type: "apple_fm.session.start",
      prompt: "local prompt owned by Bun",
      worktreePath: "/tmp/openagents-builtin-agent",
      timeoutSeconds: 300,
    })
  })
})
