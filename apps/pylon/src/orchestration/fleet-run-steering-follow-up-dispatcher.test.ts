import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  tickPylonFleetRunSteeringFollowUpDispatcher,
  type PylonFleetRunAttemptControl,
} from "./fleet-run-steering-follow-up-dispatcher.js"
import { openPylonFleetRunRuntime } from "./fleet-run-runtime.js"
import type {
  FleetRunSteeringApplication,
  PylonOrchestrationStore,
} from "./store.js"

const initialNow = new Date("2026-07-10T03:00:00.000Z")
const runRef = "fleet_run.sarah.11111111111111111111"
const intakeClaimRef = "claim.sarah_fleet_run.111111111111111111111111"
const pylonRef = "pylon.public.fc3.follow_up"
const workUnitRef = "fixture:fc3.follow_up.unit"
const workClaimRef = "claim.public.fc3.follow_up.unit"
const assignmentRef = "assignment.public.fc3.follow_up.unit"

const seed = async (home: string) => {
  const runtime = await openPylonFleetRunRuntime({
    env: { PYLON_HOME: home },
    now: () => initialNow,
  })
  if (runtime.store.getFleetRun(runRef) === null) {
    runtime.store.createFleetRun({
      runRef,
      objective: "Exercise durable exact steering follow-ups.",
      workSource: "fixture",
      authorityBinding: {
        schema: "openagents.pylon.fleet_run_authority_binding.v1",
        source: "sarah_authority",
        authorityFingerprint: "b".repeat(64),
        claimRef: intakeClaimRef,
        pylonRef,
        targetPreference: "owner_local",
        phase: "accepted",
      },
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: initialNow,
    })
    runtime.store.tryClaimWorkUnit({
      claimRef: workClaimRef,
      workUnitRef,
      runRef,
      assignmentRef,
      workerAccountRef: "codex-fc3-follow-up",
      ttl: 120_000,
      now: initialNow,
    })
    runtime.store.updateWorkClaimState(workClaimRef, "in_progress", initialNow)
  }
  return runtime
}

const queue = (
  store: PylonOrchestrationStore,
  input: {
    readonly seq: number
    readonly intentId: string
    readonly intentKind: string
    readonly application: FleetRunSteeringApplication
    readonly now?: Date
  },
) => store.applyFleetRunSteeringIntent({
  pylonRef,
  runRef,
  claimRef: intakeClaimRef,
  seq: input.seq,
  intentId: input.intentId,
  intentKind: input.intentKind,
  intentDigest: `${input.seq}`.padStart(64, "0"),
  observedAt: input.now ?? initialNow,
  outcomeRefFor: outcome => `outcome.pylon.fleet_steering.${outcome}.${input.seq}`,
}, () => input.application)

const exactSteer = (body = "private direction"): FleetRunSteeringApplication => ({
  outcome: "queued_follow_up",
  queuedFollowUp: {
    workUnitRef,
    workClaimRef,
    assignmentRef,
    targetRef: assignmentRef,
    intentKind: "steer_message",
    approvalRef: null,
    decision: null,
    residualRefs: [],
    body,
    bodyRef: null,
  },
})

const retryControl: PylonFleetRunAttemptControl = {
  applyApproval: () => Promise.resolve({
    state: "retry",
    failureRef: "blocker.fixture.approval_retry",
  }),
  applySteer: () => Promise.resolve({
    state: "retry",
    failureRef: "blocker.fixture.steer_retry",
  }),
  observeStop: () => Promise.resolve({
    state: "retry",
    failureRef: "blocker.fixture.stop_retry",
  }),
}

describe("Pylon FleetRun steering follow-up dispatcher", () => {
  test("recovers an expired dispatch lease, applies sequentially, and emits a body-free completion", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-follow-up-restart-"))
    const home = join(root, "home")
    try {
      const first = await seed(home)
      queue(first.store, {
        seq: 1,
        intentId: "intent.fc3.follow_up.restart",
        intentKind: "steer_message",
        application: exactSteer("owner-private restart direction"),
      })
      const leased = first.store.acquireFleetRunSteeringFollowUp({
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        now: initialNow,
        leaseMs: 1_000,
      })
      expect(leased?.state).toBe("dispatching")
      await first.close()

      const reopened = await seed(home)
      const completions: unknown[] = []
      const result = await tickPylonFleetRunSteeringFollowUpDispatcher({
        store: reopened.store,
        control: {
          ...retryControl,
          applySteer: input => {
            expect(input).toMatchObject({ workClaimRef, assignmentRef })
            expect(input.body).toBe("owner-private restart direction")
            return Promise.resolve({ state: "applied" })
          },
        },
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        now: () => new Date(initialNow.getTime() + 2_000),
        onCompletion: completion => {
          completions.push(completion)
          return Promise.resolve()
        },
      })
      expect(result).toMatchObject({ ok: true, dispatched: 1, completionsDelivered: 1, pending: 0 })
      const bytes = JSON.stringify(completions)
      expect(bytes).not.toContain("owner-private restart direction")
      expect(completions).toEqual([expect.objectContaining({
        state: "applied",
        workClaimRef,
        assignmentRef,
        deliveredAt: null,
      })])
      expect(reopened.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        includeTerminal: true,
      })[0]).toMatchObject({ state: "applied", attemptCount: 2 })
      expect(reopened.store.listFleetRunSteeringFollowUpCompletionOutbox({
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
      })).toEqual([])
      await reopened.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("keeps a stop queued until every exact residual claim is terminal", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-follow-up-stop-"))
    try {
      const runtime = await seed(join(root, "home"))
      runtime.store.updateFleetRunState(runRef, "stopped", initialNow, "operator")
      queue(runtime.store, {
        seq: 2,
        intentId: "intent.fc3.follow_up.stop",
        intentKind: "fleet_run_control",
        application: {
          outcome: "queued_follow_up",
          queuedFollowUp: {
            workUnitRef: null,
            workClaimRef: null,
            assignmentRef: null,
            targetRef: null,
            intentKind: "fleet_run_control",
            approvalRef: null,
            decision: null,
            residualRefs: [workClaimRef, assignmentRef],
            body: null,
            bodyRef: null,
          },
        },
      })
      let clock = new Date(initialNow.getTime() + 1_000)
      const first = await tickPylonFleetRunSteeringFollowUpDispatcher({
        store: runtime.store,
        control: retryControl,
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        now: () => clock,
      })
      expect(first).toMatchObject({ ok: false, dispatched: 1, pending: 1, failure: "control_failed" })
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
      })[0]).toMatchObject({ state: "queued", attemptCount: 1 })

      runtime.store.releaseWorkClaim(workClaimRef, clock)
      clock = new Date(clock.getTime() + 2_000)
      const second = await tickPylonFleetRunSteeringFollowUpDispatcher({
        store: runtime.store,
        control: retryControl,
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        now: () => clock,
      })
      expect(second).toMatchObject({ ok: true, dispatched: 1, pending: 0 })
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        includeTerminal: true,
      })[0]).toMatchObject({ state: "applied", attemptCount: 2 })
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("binds approvals exactly, resolves once, and rejects rebinding", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-follow-up-approval-"))
    try {
      const runtime = await seed(join(root, "home"))
      const approvalRef = "approval.public.fc3.follow_up.unit"
      const bound = runtime.store.bindFleetRunSteeringApproval({
        approvalRef,
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        workUnitRef,
        workClaimRef,
        assignmentRef,
        now: initialNow,
      })
      expect(bound).toMatchObject({ created: true, binding: { state: "pending" } })
      expect(runtime.store.bindFleetRunSteeringApproval({
        approvalRef,
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        workUnitRef,
        workClaimRef,
        assignmentRef,
        now: initialNow,
      }).created).toBe(false)
      expect(() => runtime.store.bindFleetRunSteeringApproval({
        approvalRef,
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        workUnitRef,
        workClaimRef,
        assignmentRef: "assignment.public.fc3.follow_up.other",
        now: initialNow,
      })).toThrow("rebound")

      queue(runtime.store, {
        seq: 3,
        intentId: "intent.fc3.follow_up.approval",
        intentKind: "approval_decision",
        application: {
          outcome: "queued_follow_up",
          queuedFollowUp: {
            workUnitRef,
            workClaimRef,
            assignmentRef,
            targetRef: approvalRef,
            intentKind: "approval_decision",
            approvalRef,
            decision: "allow",
            residualRefs: [],
            body: null,
            bodyRef: null,
          },
        },
      })
      const result = await tickPylonFleetRunSteeringFollowUpDispatcher({
        store: runtime.store,
        control: {
          ...retryControl,
          applyApproval: () => Promise.resolve({ state: "applied" }),
        },
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        now: () => new Date(initialNow.getTime() + 1_000),
      })
      expect(result.ok).toBe(true)
      expect(runtime.store.getFleetRunSteeringApprovalBinding(approvalRef)).toMatchObject({
        state: "resolved",
        decision: "allow",
        completionRef: expect.stringMatching(/^completion\.pylon\.fleet_steering\./u),
      })
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("retains terminal completions when the callback is unavailable", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-follow-up-outbox-"))
    try {
      const runtime = await seed(join(root, "home"))
      queue(runtime.store, {
        seq: 4,
        intentId: "intent.fc3.follow_up.outbox",
        intentKind: "steer_message",
        application: exactSteer(),
      })
      const result = await tickPylonFleetRunSteeringFollowUpDispatcher({
        store: runtime.store,
        control: { ...retryControl, applySteer: () => Promise.resolve({ state: "applied" }) },
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
        now: () => new Date(initialNow.getTime() + 1_000),
        onCompletion: () => Promise.reject(new Error("offline")),
      })
      expect(result).toMatchObject({ ok: false, dispatched: 1, failure: "completion_delivery_failed" })
      expect(runtime.store.listFleetRunSteeringFollowUpCompletionOutbox({
        pylonRef,
        runRef,
        claimRef: intakeClaimRef,
      })).toHaveLength(1)
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
