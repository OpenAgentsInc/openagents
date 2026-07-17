// Oracle for FA-RUN-03 (#8971): the control-API surface for the liveness/
// stall projection (stallCause, nextRetryAt, recoveryAction) and the
// owner-actionable "retry now" recovery route (AC-48). A separate file from
// full-auto-run-control-server.test.ts (FA-RUN-01 #8969's pause/resume/stop
// surface) to keep the two lanes' harnesses independently evolvable.
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { FULL_AUTO_LIVENESS_DISPATCH_SLO_MS } from "./full-auto-liveness.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { openFullAutoRunRegistry, type FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import { startFullAutoControlServer, type FullAutoControlServer } from "./full-auto-control-server.ts"

const GRANTED_WORKSPACE = "/granted/full-auto-liveness/workspace"

type Harness = Readonly<{
  root: string
  registry: ReturnType<typeof openFullAutoRegistry>
  runRegistry: FullAutoRunRegistry
  liveMap: Map<string, Readonly<{ state: "idle" | "turn_running" | "turn_completed" | "turn_failed" | "cap_reached" | "blocked"; turnRef: string | null }>>
  reconcileCalls: () => number
  advance: (deltaMs: number) => void
  server: FullAutoControlServer
  request: (
    method: "GET" | "POST",
    pathname: string,
    options?: Readonly<{ token?: string | null; body?: unknown }>,
  ) => Promise<Readonly<{ status: number; body: any }>>
  dispose: () => Promise<void>
}>

const startHarness = async (): Promise<Harness> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-liveness-control-"))
  let clockMs = Date.parse("2026-07-17T00:00:00.000Z")
  const now = () => new Date(clockMs)
  const registry = openFullAutoRegistry(path.join(root, "registry.json"), now)
  const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), now)
  const liveMap: Harness["liveMap"] = new Map()
  let reconcileCallCount = 0
  let mintedThreadCount = 0
  const server = await startFullAutoControlServer({
    capabilities: {
      registry,
      runRegistry,
      resolveWorkspaceRef: () => GRANTED_WORKSPACE,
      triggerReconciliation: async () => {
        reconcileCallCount += 1
      },
      liveState: threadRef => liveMap.get(threadRef) ?? null,
      listTurns: () => [],
      appendSystemNote: () => {},
      createThread: () => {
        mintedThreadCount += 1
        return `thread.liveness-control.${mintedThreadCount}`
      },
      isLaneEligible: laneRef => laneRef === "codex-local",
      interruptLiveTurn: () => true,
    },
    controlFilePath: path.join(root, "full-auto", "control.json"),
    now,
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
    runRegistry,
    liveMap,
    reconcileCalls: () => reconcileCallCount,
    advance: deltaMs => {
      clockMs += deltaMs
    },
    server,
    request,
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
}

describe("FullAutoRun liveness projection on the control API (FA-RUN-03 #8971)", () => {
  test("a freshly started run reports stallCause: null, nextRetryAt: null, recoveryAction: none", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      expect(started.status).toBe(200)
      expect(started.body.run.stallCause).toBeNull()
      expect(started.body.run.nextRetryAt).toBeNull()
      expect(started.body.run.recoveryAction).toBe("none")
      expect(started.body.run.state).toBe("running")
    } finally {
      await harness.dispose()
    }
  })

  test(
    "AC: a successful terminal turn followed by no accepted continuation beyond the SLO window becomes Stalled with a cause -- GET and the list route agree",
    async () => {
      const harness = await startHarness()
      try {
        const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
        const runRef = started.body.run.runRef
        harness.advance(FULL_AUTO_LIVENESS_DISPATCH_SLO_MS + 1_000)

        const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
        expect(status.status).toBe(200)
        expect(status.body.run.state).toBe("stalled")
        expect(status.body.run.stallCause).toBe("dispatch_overdue")
        expect(status.body.run.recoveryAction).toBe("retry_now")

        const list = await harness.request("GET", "/v1/full-auto/runs")
        expect(list.body.runs[0].state).toBe("stalled")
        expect(list.body.runs[0].stallCause).toBe("dispatch_overdue")
        // Sidebar/run view and control API AC: identical typed state and cause.
        expect(list.body.runs[0]).toEqual(status.body.run)
      } finally {
        await harness.dispose()
      }
    },
  )

  test("AC: a run whose bound thread record was never (or no longer) registered is Stalled with host_thread_missing and recoveryAction stop_only -- never a silent reattachment", async () => {
    const harness = await startHarness()
    try {
      // Mint a run directly against the durable run registry with a
      // threadRef that has no corresponding `full-auto-registry.ts` record
      // at all -- the exact FA-AC-42 orphan shape.
      const orphan = harness.runRegistry.startNew({
        title: "Orphaned mission",
        objective: "This run is bound to a threadRef the thread-level registry never saw.",
        doneCondition: "n/a",
        objectiveSource: "control_caller",
        workspaceRef: GRANTED_WORKSPACE,
        threadRef: "thread.never-registered",
        actor: "control_api",
        reason: "test setup",
      })
      expect(orphan.ok).toBe(true)
      if (!orphan.ok) return

      const status = await harness.request("GET", `/v1/full-auto/runs/${orphan.run.runRef}`)
      expect(status.status).toBe(200)
      expect(status.body.run.state).toBe("stalled")
      expect(status.body.run.stallCause).toBe("host_thread_missing")
      expect(status.body.run.recoveryAction).toBe("stop_only")

      // AC-48: a nonrecoverable cause refuses retry-now, naming Stop instead.
      const retried = await harness.request("POST", `/v1/full-auto/runs/${orphan.run.runRef}/retry-now`)
      expect(retried.status).toBe(409)
      expect(retried.body.error).toBe("not_recoverable")
      expect(retried.body.stallCause).toBe("host_thread_missing")
    } finally {
      await harness.dispose()
    }
  })
})

describe("retry-now control route (AC-48 owner-actionable recovery affordance)", () => {
  test("refuses with 409 illegal_transition when the run is not Stalled", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const retried = await harness.request("POST", `/v1/full-auto/runs/${runRef}/retry-now`)
      expect(retried.status).toBe(409)
      expect(retried.body.error).toBe("illegal_transition")
    } finally {
      await harness.dispose()
    }
  })

  test("a Stalled run with a recoverable cause (dispatch_overdue) transitions to Retrying and schedules the shared reconcile pass", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      harness.advance(FULL_AUTO_LIVENESS_DISPATCH_SLO_MS + 1_000)
      const stalled = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(stalled.body.run.state).toBe("stalled")
      const reconcilesBefore = harness.reconcileCalls()

      const retried = await harness.request("POST", `/v1/full-auto/runs/${runRef}/retry-now`)
      expect(retried.status).toBe(200)
      expect(retried.body.ok).toBe(true)
      expect(retried.body.run.state).toBe("retrying")
      expect(harness.reconcileCalls()).toBe(reconcilesBefore + 1)
    } finally {
      await harness.dispose()
    }
  })

  test("retry-now requires the bearer credential", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const response = await harness.request("POST", `/v1/full-auto/runs/${runRef}/retry-now`, { token: null })
      expect(response.status).toBe(401)
    } finally {
      await harness.dispose()
    }
  })

  test("retry-now 404s for an unknown runRef", async () => {
    const harness = await startHarness()
    try {
      const response = await harness.request("POST", "/v1/full-auto/runs/run.does-not-exist/retry-now")
      expect(response.status).toBe(404)
      expect(response.body.error).toBe("not_found")
    } finally {
      await harness.dispose()
    }
  })
})
