import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { DesktopThread } from "./chat-contract.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { openFullAutoRunRegistry, type FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import { openProviderHandoffRegistry, type ProviderHandoffRegistry } from "./full-auto-provider-handoff.ts"
import { makeProviderLaneRegistry } from "./provider-lane-registry.ts"
import { startFullAutoControlServer, type FullAutoControlServer } from "./full-auto-control-server.ts"

const GRANTED_WORKSPACE = "/granted/full-auto-handoff/workspace"

const CODEX_LOCAL_LANE = {
  laneRef: "codex-local",
  provider: "codex",
  profileRef: "codex-local",
  configuration: "configured" as const,
  authentication: "ready" as const,
  admission: "admitted" as const,
  reason: null,
  capabilities: {
    laneRef: "codex-local", provider: "codex", displayName: "Codex", admission: "admitted" as const, reason: null,
    models: [], reasoningEfforts: [], permissionModes: [], approvals: "none" as const,
    questions: true, skills: true, images: true, fullAuto: true,
    interrupt: true, queueFollowup: true, steerTurn: true,
    extensions: [], evidence: "conformant" as const,
  },
}

const FABLE_LOCAL_LANE = {
  ...CODEX_LOCAL_LANE,
  laneRef: "fable-local",
  provider: "fable",
  profileRef: "fable-local",
}

const UNADMITTED_PEER_LANE = {
  laneRef: "acp:cursor-agent",
  provider: "cursor",
  profileRef: "acp:cursor-agent",
  configuration: "unconfigured" as const,
  authentication: "missing" as const,
  admission: "quarantined" as const,
  reason: "Peer profile is not admitted.",
  capabilities: {
    laneRef: "acp:cursor-agent", provider: "cursor", displayName: "Cursor", admission: "quarantined" as const,
    reason: "Peer profile is not admitted.",
    models: [], reasoningEfforts: [], permissionModes: [], approvals: "none" as const,
    questions: false, skills: false, images: false, fullAuto: false,
    interrupt: false, queueFollowup: false, steerTurn: false,
    extensions: [], evidence: "experimental" as const,
  },
}

type Harness = Readonly<{
  root: string
  registry: ReturnType<typeof openFullAutoRegistry>
  runRegistry: FullAutoRunRegistry
  providerHandoffRegistry: ProviderHandoffRegistry
  notes: Array<Readonly<{ threadRef: string; text: string }>>
  liveMap: Map<string, Readonly<{ state: "idle" | "turn_running" | "turn_completed" | "turn_failed" | "cap_reached" | "blocked"; turnRef: string | null }>>
  threads: Map<string, DesktopThread>
  server: FullAutoControlServer
  request: (
    method: "GET" | "POST",
    pathname: string,
    options?: Readonly<{ token?: string | null; body?: unknown }>,
  ) => Promise<Readonly<{ status: number; body: any }>>
  dispose: () => Promise<void>
}>

const startHarness = async (): Promise<Harness> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-handoff-control-"))
  const registry = openFullAutoRegistry(path.join(root, "registry.json"))
  const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
  const providerHandoffRegistry = openProviderHandoffRegistry(path.join(root, "full-auto", "provider-handoffs.json"))
  const providerLaneRegistry = makeProviderLaneRegistry({ file: path.join(root, "provider-lanes.json") })
  const notes: Array<Readonly<{ threadRef: string; text: string }>> = []
  const liveMap: Harness["liveMap"] = new Map()
  const threads = new Map<string, DesktopThread>()
  let mintedThreadCount = 0

  const server = await startFullAutoControlServer({
    capabilities: {
      registry,
      runRegistry,
      resolveWorkspaceRef: () => GRANTED_WORKSPACE,
      triggerReconciliation: async () => {},
      liveState: threadRef => liveMap.get(threadRef) ?? null,
      listTurns: () => [],
      appendSystemNote: (threadRef, text) => notes.push({ threadRef, text }),
      createThread: () => {
        mintedThreadCount += 1
        const threadRef = `thread.handoff-control.${mintedThreadCount}`
        threads.set(threadRef, { id: threadRef, title: "Handoff run", updatedAt: new Date().toISOString(), notes: [] })
        return threadRef
      },
      isLaneEligible: laneRef => laneRef === "codex-local" || laneRef === "fable-local",
      listLanes: async () => [CODEX_LOCAL_LANE, FABLE_LOCAL_LANE, UNADMITTED_PEER_LANE],
      providerLaneRegistry: { switchThread: providerLaneRegistry.switchThread },
      getThread: threadRef => threads.get(threadRef) ?? null,
      providerHandoffRegistry,
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
    root, registry, runRegistry, providerHandoffRegistry, notes, liveMap, threads, server, request,
    dispose: async () => {
      await server.stop()
      rmSync(root, { recursive: true, force: true })
    },
  }
}

const START_BODY = {
  workspaceRef: GRANTED_WORKSPACE,
  title: "Fix the flaky test",
  objective: "Make tests/flaky.test.ts stop flaking.",
  doneCondition: "The test passes 20 consecutive local runs.",
  lane: "codex-local",
}

describe("Provider handoff control route (FA-HO-01 #8975)", () => {
  test("handoff is refused while the run is running: the run's lane is untouched (rollback, never a partial state change)", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef

      const handoff = await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`, {
        body: { targetLaneRef: "fable-local" },
      })
      expect(handoff.status).toBe(409)
      expect(handoff.body.error).toBe("illegal_transition")

      const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(status.body.run.lane).toBe("codex-local")
      expect(harness.providerHandoffRegistry.list({ runRef })).toEqual([])
    } finally {
      await harness.dispose()
    }
  })

  test("target admission refusal (unadmitted peer) leaves the run's lane/profile unchanged and records no receipt", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)

      const handoff = await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`, {
        body: { targetLaneRef: "acp:cursor-agent" },
      })
      expect(handoff.status).toBe(409)
      expect(handoff.body.error).toBe("handoff_refused")
      expect(handoff.body.handoffRefusalReason).toBe("unadmitted_peer")

      const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(status.body.run.state).toBe("paused")
      expect(status.body.run.lane).toBe("codex-local")
      expect(harness.providerHandoffRegistry.list({ runRef })).toEqual([])
    } finally {
      await harness.dispose()
    }
  })

  test("handoff to an unknown lane is refused typed unknown_lane, run untouched", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)

      const handoff = await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`, {
        body: { targetLaneRef: "does-not-exist" },
      })
      expect(handoff.status).toBe(409)
      expect(handoff.body.handoffRefusalReason).toBe("unknown_lane")
      const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(status.body.run.lane).toBe("codex-local")
    } finally {
      await harness.dispose()
    }
  })

  test("successful handoff while paused: rebinds the profile lane, records a durable receipt, and appends a visible transition note", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)

      const handoff = await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`, {
        body: { targetLaneRef: "fable-local", reason: "Owner wants a second opinion from Claude." },
      })
      expect(handoff.status).toBe(200)
      expect(handoff.body.run.lane).toBe("fable-local")
      expect(handoff.body.run.state).toBe("paused")
      expect(handoff.body.transition.from).toBe("codex-local")
      expect(handoff.body.transition.to).toBe("fable-local")
      expect(handoff.body.transition.actor).toBe("control_api")
      expect(handoff.body.transition.disposition).toBe("complete_within_bounds")
      expect(handoff.body.transition.reason).toBe("Owner wants a second opinion from Claude.")
      expect(handoff.body.transition.handoffRef.length).toBeGreaterThan(0)

      // Durable: reflected in the run registry itself.
      expect(harness.runRegistry.get(runRef)?.profile?.lane).toBe("fable-local")
      // Durable: reflected in the independent receipt store, restart-safe.
      const receipts = harness.providerHandoffRegistry.list({ runRef })
      expect(receipts).toHaveLength(1)
      expect(receipts[0]!.from).toBe("codex-local")
      expect(receipts[0]!.to).toBe("fable-local")
      expect(receipts[0]!.threadRef).toBe(threadRef)
      // Visible transcript note.
      expect(harness.notes.some(note => note.threadRef === threadRef && note.text.includes("codex-local") && note.text.includes("fable-local"))).toBe(true)

      // Resume dispatches on the NEW lane -- the thread-level record was rebound.
      const resumed = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumed.status).toBe(200)
      expect(resumed.body.run.lane).toBe("fable-local")
      expect(harness.registry.record(threadRef)?.profile?.lane).toBe("fable-local")
    } finally {
      await harness.dispose()
    }
  })

  test("a handoff receipt survives independently of the FullAutoRun's own transitions array (distinct event kinds)", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`, { body: { targetLaneRef: "fable-local" } })

      const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      // The run's own lifecycle transitions (draft->running, running->paused)
      // never gain a synthetic provider-pair entry.
      for (const transition of status.body.run.transitions) {
        expect(["draft", "running", "pausing", "paused", "retrying", "stalled", "completed", "failed", "stopped", "cap_reached"]).toContain(transition.from)
        expect(["draft", "running", "pausing", "paused", "retrying", "stalled", "completed", "failed", "stopped", "cap_reached"]).toContain(transition.to)
      }
      expect(harness.providerHandoffRegistry.list({ runRef })).toHaveLength(1)
    } finally {
      await harness.dispose()
    }
  })

  test("every handoff route requires the bearer credential", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const response = await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`, {
        token: null,
        body: { targetLaneRef: "fable-local" },
      })
      expect(response.status).toBe(401)
    } finally {
      await harness.dispose()
    }
  })

  test("a bodyless handoff is a 400 invalid_request and mutates nothing", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      const response = await harness.request("POST", `/v1/full-auto/runs/${runRef}/handoff`)
      expect(response.status).toBe(400)
      expect(response.body.error).toBe("invalid_request")
      expect(harness.providerHandoffRegistry.list({ runRef })).toEqual([])
    } finally {
      await harness.dispose()
    }
  })

  test("handoff on an unknown runRef is a 404", async () => {
    const harness = await startHarness()
    try {
      const response = await harness.request("POST", "/v1/full-auto/runs/run.does-not-exist/handoff", {
        body: { targetLaneRef: "fable-local" },
      })
      expect(response.status).toBe(404)
      expect(response.body.error).toBe("not_found")
    } finally {
      await harness.dispose()
    }
  })
})
