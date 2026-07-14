import { describe, expect, test } from "vite-plus/test"
import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"

import { PylonFleetRunManager } from "../src/orchestration/fleet-run-manager.js"
import type { FleetRunSupervisorRunner } from "../src/orchestration/fleet-run-supervisor.js"
import { createPylonOrchestrationStore } from "../src/orchestration/store.js"
import { fixtureCandidates, planWorkCandidates } from "../src/orchestration/work-planner.js"

const now = new Date("2026-07-09T15:00:00.000Z")

const lifecycle = (
  event: PylonAssignmentRunLifecycleEvent["event"],
  status: PylonAssignmentRunLifecycleEvent["status"],
): PylonAssignmentRunLifecycleEvent => ({
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
  event,
  observedAt: now.toISOString(),
  status,
})

const managerFixture = () => {
  const store = createPylonOrchestrationStore(new NodeTestDatabase(":memory:"))
  const manager = new PylonFleetRunManager({ now: () => now, store })
  const planner = {
    plan: async () => planWorkCandidates(
      "fixture",
      fixtureCandidates({ kind: "fixture", count: 1 }),
      { claimRegistry: store, now },
    ),
  }
  const capacity = {
    accounts: async () => [{
      accountRef: "codex-owner",
      advertisedCapacity: 1,
      workerKind: "codex" as const,
    }],
  }
  const run = (runRef: string) => ({
    runRef,
    objective: "Run one bounded fixture.",
    workSource: "fixture" as const,
    targetConcurrency: 1,
    workerKind: "codex" as const,
    state: "running" as const,
    dispatchKind: "supervised_dispatch" as const,
    startedAt: now,
    now,
    counters: { workUnitsTotal: 1 },
  })
  return { capacity, manager, planner, run, store }
}

describe("PylonFleetRunManager", () => {
  test("contract background_agents.fleet.supervisor_scope_and_publication_order.v1: stop aborts and joins the owned dispatch before releasing its slot", async () => {
    const fixture = managerFixture()
    let capacityAvailable = false
    let resolveStarted!: (signal: AbortSignal) => void
    const started = new Promise<AbortSignal>(resolve => {
      resolveStarted = resolve
    })
    let resolveAfterAbort!: () => void
    const afterAbort = new Promise<void>(resolve => {
      resolveAfterAbort = resolve
    })
    let stopSettled = false
    const runner: FleetRunSupervisorRunner = {
      dispatch: async input => {
        if (input.run.runRef === "fleet_run.manager.cancelled") {
          if (input.signal === undefined) throw new Error("missing supervisor signal")
          resolveStarted(input.signal)
          if (!input.signal.aborted) {
            await new Promise<void>(resolve => {
              input.signal?.addEventListener("abort", () => resolve(), { once: true })
            })
          }
          await afterAbort
          return {
            assignmentRef: `assignment.${input.claim.claimRef}`,
            lifecycle: [lifecycle("assignment_run.runtime_failed", "cancelled")],
            status: "failed",
          }
        }
        return {
          assignmentRef: `assignment.${input.claim.claimRef}`,
          lifecycle: [lifecycle("assignment_run.completed", "closed")],
          status: "completed",
        }
      },
    }
    const capacity = {
      accounts: async () => capacityAvailable
        ? [{
            accountRef: "codex-owner",
            advertisedCapacity: 1,
            workerKind: "codex" as const,
          }]
        : [],
    }

    const first = await fixture.manager.start({
      capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager.cancel",
      run: fixture.run("fleet_run.manager.cancelled"),
      runner,
      startImmediately: false,
      tickIntervalMs: 1,
    })
    expect(first.active).toBe(true)
    capacityAvailable = true
    const signal = await started
    const stopping = fixture.manager.control("fleet_run.manager.cancelled", "stop")
      .finally(() => {
        stopSettled = true
      })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(signal.aborted).toBe(true)
    expect(stopSettled).toBe(false)
    let duringJoinSettled = false
    const duringJoin = fixture.manager.start({
      capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager.cancel",
      run: fixture.run("fleet_run.manager.during_cancel_join"),
      runner,
      startImmediately: false,
    }).finally(() => {
      duringJoinSettled = true
    })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(duringJoinSettled).toBe(false)

    resolveAfterAbort()
    const stopped = await stopping
    expect(stopped.active).toBe(false)
    expect(stopped.run.state).toBe("stopped")
    expect(stopped.lifecycle).toContainEqual(expect.objectContaining({
      kind: "dispatch",
      status: "failed",
    }))
    const joinedNext = await duringJoin
    expect(joinedNext.active).toBe(false)
    expect(joinedNext.run.state).toBe("completed")

    const next = await fixture.manager.start({
      capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager.cancel",
      run: fixture.run("fleet_run.manager.after_cancel"),
      runner,
      startImmediately: false,
    })
    expect(next.active).toBe(false)
    expect(next.run.state).toBe("completed")
    await fixture.manager.close()
  })

  test("reports the fire-and-forget completion race once before releasing the supervisor slot", async () => {
    const fixture = managerFixture()
    const runner: FleetRunSupervisorRunner = {
      dispatch: async input => ({
        assignmentRef: `assignment.${input.claim.claimRef}`,
        lifecycle: [lifecycle("assignment_run.completed", "closed")],
        status: "completed",
      }),
    }

    const first = await fixture.manager.start({
      capacity: fixture.capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager",
      run: fixture.run("fleet_run.manager.first"),
      runner,
      startImmediately: false,
    })
    const second = await fixture.manager.start({
      capacity: fixture.capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager",
      run: fixture.run("fleet_run.manager.second"),
      runner,
      startImmediately: false,
    })

    expect(first.active).toBe(false)
    expect(first.run.state).toBe("completed")
    expect(first.lifecycle.some(event => event.kind === "dispatch")).toBe(true)
    expect(first.lifecycle.filter(event => event.kind === "completed")).toHaveLength(1)
    expect(second.active).toBe(false)
    expect(second.run.state).toBe("completed")
    expect(second.lifecycle.filter(event => event.kind === "completed")).toHaveLength(1)
  })

  test("retains a terminal supervisor scope until the final reporting tick succeeds", async () => {
    const fixture = managerFixture()
    let failTerminalReport = true
    let terminalReportAttempts = 0
    const runner: FleetRunSupervisorRunner = {
      dispatch: async input => ({
        assignmentRef: `assignment.${input.claim.claimRef}`,
        lifecycle: [lifecycle("assignment_run.accepted", "accepted")],
        status: "accepted",
      }),
      reconcile: async input => {
        terminalReportAttempts += 1
        if (failTerminalReport) {
          throw new Error("terminal projection temporarily unavailable")
        }
        return input.activeAssignments.map(active => ({
          accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
          assignmentRef: `assignment.${active.claim.claimRef}`,
          closeoutRef: "closeout.public.retry_terminal",
          lifecycle: [lifecycle("assignment_run.completed", "closed")],
          marginalCostClass: "subscription",
          status: "completed" as const,
          taskId: active.taskId,
          verification: {
            evidenceRefs: ["evidence.public.retry_terminal"],
            truth: "passed" as const,
            verifierRef: "verifier.public.retry_terminal",
          },
        }))
      },
    }

    const first = await fixture.manager.start({
      capacity: fixture.capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager.retry-terminal",
      run: fixture.run("fleet_run.manager.retry_terminal"),
      runner,
      startImmediately: false,
    })
    expect(first.active).toBe(true)
    expect(first.run.counters.activeAssignments).toBe(1)
    fixture.store.updateFleetRunState("fleet_run.manager.retry_terminal", "stopped", now, "reconcile")

    const retained = await fixture.manager.status("fleet_run.manager.retry_terminal")
    expect(Array.isArray(retained)).toBe(false)
    if (Array.isArray(retained)) throw new Error("expected one snapshot")
    expect(retained.active).toBe(true)
    expect(retained.run.state).toBe("stopped")
    expect(retained.pylonRef).toBe("pylon.owner.manager.retry-terminal")
    expect(terminalReportAttempts).toBeGreaterThanOrEqual(1)

    failTerminalReport = false
    const released = await fixture.manager.status("fleet_run.manager.retry_terminal")
    expect(Array.isArray(released)).toBe(false)
    if (Array.isArray(released)) throw new Error("expected one snapshot")
    expect(released.active).toBe(false)
    expect(released.pylonRef).toBe("pylon.owner.manager.retry-terminal")
    expect(released.run.counters.activeAssignments).toBe(0)
    expect(released.lifecycle.some(event => event.kind === "terminal")).toBe(true)
    await fixture.manager.close()
  })

  test("records a failed competing start as reconcile-stopped and closes its scope", async () => {
    const fixture = managerFixture()
    const runner: FleetRunSupervisorRunner = {
      dispatch: async input => ({
        assignmentRef: `assignment.${input.claim.claimRef}`,
        lifecycle: [lifecycle("assignment_run.accepted", "accepted")],
        status: "accepted",
      }),
    }

    const active = await fixture.manager.start({
      capacity: fixture.capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager.collision",
      run: fixture.run("fleet_run.manager.active"),
      runner,
      startImmediately: false,
    })
    await expect(fixture.manager.start({
      capacity: fixture.capacity,
      planner: fixture.planner,
      pylonRef: "pylon.owner.manager.collision",
      run: fixture.run("fleet_run.manager.competing"),
      runner,
      startImmediately: false,
    })).rejects.toThrow("fleet run supervisor already active")

    const competing = await fixture.manager.status("fleet_run.manager.competing")
    expect(Array.isArray(competing)).toBe(false)
    expect(active.active).toBe(true)
    if (Array.isArray(competing)) throw new Error("expected one snapshot")
    expect(competing.run.state).toBe("stopped")

    const stopped = await fixture.manager.control("fleet_run.manager.active", "stop")
    expect(stopped.active).toBe(false)
    expect(stopped.run.counters.activeAssignments).toBe(1)
    await fixture.manager.close()
  })
})
