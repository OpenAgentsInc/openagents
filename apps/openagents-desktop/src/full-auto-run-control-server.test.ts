import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openFullAutoRegistry } from "./full-auto-registry.ts"
import {
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  isFullAutoRunAutonomyEnabled,
  openFullAutoRunRegistry,
  type FullAutoRunRegistry,
} from "./full-auto-run-registry.ts"
import { openFullAutoRunReportStore } from "./full-auto-run-report.ts"
import { startFullAutoControlServer, type FullAutoControlServer } from "./full-auto-control-server.ts"

const GRANTED_WORKSPACE = "/granted/full-auto-run/workspace"

type Harness = Readonly<{
  root: string
  registry: ReturnType<typeof openFullAutoRegistry>
  runRegistry: FullAutoRunRegistry
  liveMap: Map<string, Readonly<{ state: "idle" | "turn_running" | "turn_completed" | "turn_failed" | "cap_reached" | "blocked"; turnRef: string | null }>>
  interruptCalls: Array<string>
  reconcileCalls: () => number
  setResolvedWorkspace: (workspaceRef: string) => void
  setModelAdmissionEnabled: (enabled: boolean) => void
  server: FullAutoControlServer
  request: (
    method: "GET" | "POST",
    pathname: string,
    options?: Readonly<{ token?: string | null; body?: unknown }>,
  ) => Promise<Readonly<{ status: number; body: any }>>
  dispose: () => Promise<void>
}>

const startHarness = async (): Promise<Harness> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-run-control-"))
  const registry = openFullAutoRegistry(path.join(root, "registry.json"))
  const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
  const reportStore = openFullAutoRunReportStore(path.join(root, "reports.json"))
  const liveMap: Harness["liveMap"] = new Map()
  const interruptCalls: Array<string> = []
  let reconcileCallCount = 0
  let mintedThreadCount = 0
  let resolvedWorkspace = GRANTED_WORKSPACE
  let modelAdmissionEnabled = true
  const server = await startFullAutoControlServer({
    capabilities: {
      registry,
      runRegistry,
      reportStore,
      resolveWorkspaceRef: () => resolvedWorkspace,
      triggerReconciliation: async () => {
        reconcileCallCount += 1
      },
      liveState: threadRef => liveMap.get(threadRef) ?? null,
      listTurns: () => [],
      appendSystemNote: () => {},
      createThread: () => {
        mintedThreadCount += 1
        return `thread.run-control.${mintedThreadCount}`
      },
      isLaneEligible: laneRef => laneRef === "codex-local",
      isModelEligible: (laneRef, model) =>
        modelAdmissionEnabled && laneRef === "codex-local" && model === "gpt-5.6-sol",
      interruptLiveTurn: threadRef => {
        interruptCalls.push(threadRef)
        return true
      },
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
    runRegistry,
    liveMap,
    interruptCalls,
    reconcileCalls: () => reconcileCallCount,
    setResolvedWorkspace: workspaceRef => {
      resolvedWorkspace = workspaceRef
    },
    setModelAdmissionEnabled: enabled => {
      modelAdmissionEnabled = enabled
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

describe("FullAutoRun control routes (FA-RUN-01 #8969)", () => {
  test("runs/start creates a run with title/objective/doneCondition/workspace/state and schedules the shared reconcile pass", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      expect(started.status).toBe(200)
      expect(started.body.ok).toBe(true)
      expect(started.body.run.title).toBe(START_BODY.title)
      expect(started.body.run.objective).toBe(START_BODY.objective)
      expect(started.body.run.doneCondition).toBe(START_BODY.doneCondition)
      expect(started.body.run.workspaceRef).toBe(GRANTED_WORKSPACE)
      expect(started.body.run.state).toBe("running")
      expect(started.body.run.threadRef).toMatch(/^thread\.run-control\./)
      expect(harness.reconcileCalls()).toBe(1)
      // The underlying thread-level record is enabled through the unchanged
      // exactly-once dispatch path.
      expect(harness.registry.get(started.body.run.threadRef)).toBe(true)
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start refuses a workspace mismatch without creating a thread or a run", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, workspaceRef: "/wrong/workspace" },
      })
      expect(started.status).toBe(409)
      expect(started.body.error).toBe("workspace_mismatch")
      expect(harness.runRegistry.list()).toEqual([])
      expect(harness.registry.list()).toEqual([])
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start without --autonomy leaves the run on the passive base loop (autonomy off, unchanged)", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      expect(started.status).toBe(200)
      // The projection surfaces the gate as a first-class field, defaulting off.
      expect(started.body.run.autonomyEnabled).toBe(false)
      // The durable run carries no enabled autonomy block.
      const run = harness.runRegistry.get(started.body.run.runRef)
      expect(run).not.toBeNull()
      expect(isFullAutoRunAutonomyEnabled(run!)).toBe(false)
      expect(run!.autonomy).toBeUndefined()
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start --autonomy creates a run with the autonomy core enabled before the first reconcile", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, autonomy: true },
      })
      expect(started.status).toBe(200)
      expect(started.body.ok).toBe(true)
      // The gate is visible in the run projection...
      expect(started.body.run.autonomyEnabled).toBe(true)
      // ...and flipped durably on the created run (which activates objective
      // selection #9172, plan brief #9174, host verification #9173, churn #9175,
      // and initiative #9184 -- all gated on isFullAutoRunAutonomyEnabled).
      const run = harness.runRegistry.get(started.body.run.runRef)
      expect(run).not.toBeNull()
      expect(isFullAutoRunAutonomyEnabled(run!)).toBe(true)
      expect(run!.autonomy?.enabled).toBe(true)
      // The gate was flipped BEFORE reconciliation ran, and the shared pass
      // still fired exactly once.
      expect(harness.reconcileCalls()).toBe(1)
      // A subsequent status read also reflects the enabled gate.
      const status = await harness.request("GET", `/v1/full-auto/runs/${started.body.run.runRef}`)
      expect(status.status).toBe(200)
      expect(status.body.run.autonomyEnabled).toBe(true)
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start admits and durably binds an exact lane model before dispatch", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, lane: "codex-local", model: "gpt-5.6-sol" },
      })
      expect(started.status).toBe(200)
      const threadRef = started.body.run.threadRef as string
      expect(harness.registry.record(threadRef)?.profile).toEqual({
        lane: "codex-local",
        model: "gpt-5.6-sol",
      })
      expect(harness.runRegistry.get(started.body.run.runRef)?.profile).toEqual({
        lane: "codex-local",
        model: "gpt-5.6-sol",
      })
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start refuses an unadmitted lane/model pair before minting anything", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, lane: "codex-local", model: "claude-sonnet-5" },
      })
      expect(started.status).toBe(409)
      expect(started.body.error).toBe("model_not_eligible")
      expect(harness.runRegistry.list()).toEqual([])
      expect(harness.registry.list()).toEqual([])
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start admits multiple independent active runs and lists both", async () => {
    const harness = await startHarness()
    try {
      const first = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      expect(first.status).toBe(200)
      const second = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, title: "Second mission" },
      })
      expect(second.status).toBe(200)
      expect(second.body.run.runRef).not.toBe(first.body.run.runRef)
      expect(second.body.run.threadRef).not.toBe(first.body.run.threadRef)
      expect(harness.registry.list()).toHaveLength(2)
      expect(harness.runRegistry.activeRuns()).toHaveLength(2)
    } finally {
      await harness.dispose()
    }
  })

  test("runs/start refuses only when bounded concurrent capacity is full and does not mint an orphan thread", async () => {
    const harness = await startHarness()
    try {
      for (let index = 0; index < FULL_AUTO_RUN_ACTIVE_LIMIT; index += 1) {
        const started = await harness.request("POST", "/v1/full-auto/runs/start", {
          body: { ...START_BODY, title: `Mission ${index}` },
        })
        expect(started.status).toBe(200)
      }
      const beforeThreadCount = harness.registry.list().length
      const refused = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, title: "Mission over limit" },
      })
      expect(refused.status).toBe(409)
      expect(refused.body).toMatchObject({
        error: "active_run_limit_reached",
        activeRunCount: FULL_AUTO_RUN_ACTIVE_LIMIT,
        activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
      })
      expect(harness.registry.list()).toHaveLength(beforeThreadCount)
      expect(harness.runRegistry.list()).toHaveLength(FULL_AUTO_RUN_ACTIVE_LIMIT)
    } finally {
      await harness.dispose()
    }
  })

  test("GET runs/{runRef} 404s for an unknown runRef; GET runs lists every run", async () => {
    const harness = await startHarness()
    try {
      const missing = await harness.request("GET", "/v1/full-auto/runs/run.does-not-exist")
      expect(missing.status).toBe(404)
      expect(missing.body.error).toBe("not_found")

      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const list = await harness.request("GET", "/v1/full-auto/runs")
      expect(list.status).toBe(200)
      expect(list.body.runs).toHaveLength(1)
      expect(list.body.runs[0].runRef).toBe(started.body.run.runRef)

      const status = await harness.request("GET", `/v1/full-auto/runs/${started.body.run.runRef}`)
      expect(status.status).toBe(200)
      expect(status.body.run.runRef).toBe(started.body.run.runRef)
    } finally {
      await harness.dispose()
    }
  })

  test("Pause with no turn in flight goes directly to Paused and disables the thread-level dispatch gate immediately", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef

      const paused = await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      expect(paused.status).toBe(200)
      expect(paused.body.run.state).toBe("paused")
      expect(harness.registry.get(threadRef)).toBe(false)
      expect(harness.interruptCalls).toEqual([])
    } finally {
      await harness.dispose()
    }
  })

  test("Pause drains an active turn while preventing a new dispatch, then settles to Paused", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef
      harness.liveMap.set(threadRef, { state: "turn_running", turnRef: "turn.x" })

      const pausing = await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      expect(pausing.status).toBe(200)
      expect(pausing.body.run.state).toBe("pausing")
      // New dispatch is prevented immediately, even while the turn is still resolving.
      expect(harness.registry.get(threadRef)).toBe(false)
      // Pause is a drain boundary; Stop is the explicit interrupt action.
      expect(harness.interruptCalls).toEqual([])

      // Still running: GET observes Pausing, unresolved.
      const stillPausing = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(stillPausing.body.run.state).toBe("pausing")

      // Turn resolves.
      harness.liveMap.set(threadRef, { state: "turn_completed", turnRef: null })
      const settled = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(settled.body.run.state).toBe("paused")
    } finally {
      await harness.dispose()
    }
  })

  test("Resume is legal only from Paused: illegal from Running is refused with a typed 409, never silently coerced", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const resumed = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumed.status).toBe(409)
      expect(resumed.body.error).toBe("illegal_transition")
      expect(resumed.body.fromState).toBe("running")
      expect(resumed.body.toState).toBe("running")
      const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(status.body.run.state).toBe("running")
    } finally {
      await harness.dispose()
    }
  })

  test("Resume from Paused re-enables the thread-level dispatch gate exactly-once and schedules the shared reconcile pass", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      const reconcilesBeforeResume = harness.reconcileCalls()

      const resumed = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumed.status).toBe(200)
      expect(resumed.body.run.state).toBe("running")
      expect(harness.registry.get(threadRef)).toBe(true)
      expect(harness.reconcileCalls()).toBe(reconcilesBeforeResume + 1)
    } finally {
      await harness.dispose()
    }
  })

  test("Resume revalidates workspace admission: a mismatch is a 409 refusal that leaves the run exactly Paused, never a redirect or a silent Failed coercion", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      harness.setResolvedWorkspace("/a/different/workspace")

      const resumed = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumed.status).toBe(409)
      expect(resumed.body.error).toBe("workspace_mismatch")
      expect(harness.registry.get(threadRef)).toBe(false)

      const status = await harness.request("GET", `/v1/full-auto/runs/${runRef}`)
      expect(status.body.run.state).toBe("paused")

      // Once the expected workspace is available again, Resume succeeds.
      harness.setResolvedWorkspace(GRANTED_WORKSPACE)
      const resumedAgain = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumedAgain.status).toBe(200)
      expect(resumedAgain.body.run.state).toBe("running")
    } finally {
      await harness.dispose()
    }
  })

  test("Resume revalidates the durable exact model and remains Paused if that lane/model admission was withdrawn", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: { ...START_BODY, lane: "codex-local", model: "gpt-5.6-sol" },
      })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef
      await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      harness.setModelAdmissionEnabled(false)

      const resumed = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumed.status).toBe(409)
      expect(resumed.body.error).toBe("model_not_eligible")
      expect(harness.registry.get(threadRef)).toBe(false)
      expect(harness.runRegistry.get(runRef)?.state).toBe("paused")
    } finally {
      await harness.dispose()
    }
  })

  test("Stop is terminal and distinct from Pause; a stopped run refuses a second Stop and refuses Resume", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef

      const stopped = await harness.request("POST", `/v1/full-auto/runs/${runRef}/stop`)
      expect(stopped.status).toBe(200)
      expect(stopped.body.run.state).toBe("stopped")
      expect(harness.registry.get(threadRef)).toBe(false)

      const secondStop = await harness.request("POST", `/v1/full-auto/runs/${runRef}/stop`)
      expect(secondStop.status).toBe(409)
      expect(secondStop.body.error).toBe("illegal_transition")

      const resumeAfterStop = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumeAfterStop.status).toBe(409)
      expect(resumeAfterStop.body.error).toBe("illegal_transition")

      // A terminal run remains immutable while a distinct new run starts.
      const rerunStart = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      expect(rerunStart.status).toBe(200)
      expect(rerunStart.body.run.runRef).not.toBe(runRef)
    } finally {
      await harness.dispose()
    }
  })

  test("every run-level route requires the bearer credential", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", { body: START_BODY })
      const runRef = started.body.run.runRef
      for (const [method, pathname] of [
        ["GET", "/v1/full-auto/runs"],
        ["POST", "/v1/full-auto/runs/start"],
        ["GET", `/v1/full-auto/runs/${runRef}`],
        ["POST", `/v1/full-auto/runs/${runRef}/pause`],
        ["POST", `/v1/full-auto/runs/${runRef}/resume`],
        ["POST", `/v1/full-auto/runs/${runRef}/stop`],
      ] as const) {
        const response = await harness.request(method, pathname, { token: null })
        expect(response.status, `${method} ${pathname}`).toBe(401)
      }
    } finally {
      await harness.dispose()
    }
  })
})
