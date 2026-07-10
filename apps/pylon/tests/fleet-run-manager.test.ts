import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
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
  const store = createPylonOrchestrationStore(new Database(":memory:"))
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
    expect(stopped.active).toBe(true)
    expect(stopped.run.counters.activeAssignments).toBe(1)
    await fixture.manager.close()
  })
})
