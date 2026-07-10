import { describe, expect, test } from "bun:test"
import {
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetCommandOutcomeEntity,
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  type FleetApprovalEntity,
  type FleetAssignmentEntity,
  type FleetCommandOutcomeEntity,
  type FleetRunEntity,
  type FleetWorkerEntity,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import {
  SarahFleetOwnerProjection,
  projectSarahFleetOwnerRun,
} from "./fleet-owner-projection.ts"

const NOW = Date.parse("2026-07-09T20:00:00.000Z")

const run = decodeFleetRunEntity({
  runId: "fleet.run.sarah.8639",
  status: "running",
  desiredSlots: 3,
  workerKind: "auto",
  startedAt: "2026-07-09T19:50:00.000Z",
  counters: {
    workUnitsTotal: 3,
    activeAssignments: 2,
    completedAssignments: 1,
    failedAssignments: 0,
    blockedAssignments: 1,
  },
  updatedAt: "2026-07-09T19:59:58.000Z",
})

const assignments = [
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.fc3.codex",
    issueRef: "#8637",
    status: "running",
    updatedAt: "2026-07-09T19:59:55.000Z",
  }),
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.fc3.claude",
    issueRef: "#8633",
    status: "accepted",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.fc3.grok",
    issueRef: "#8639",
    status: "accepted_work",
    closeoutClass: "accepted_work",
    updatedAt: "2026-07-09T19:59:50.000Z",
  }),
]

const workers = [
  decodeFleetWorkerEntity({
    workerId: "worker.fc3.codex",
    phase: "dispatched",
    harnessKind: "codex",
    assignmentRef: "assignment.fc3.codex",
    accountRefHash: "account.pylon.codex.11111111",
    lastProgressAt: "2026-07-09T19:59:55.000Z",
    updatedAt: "2026-07-09T19:59:55.000Z",
  }),
  decodeFleetWorkerEntity({
    workerId: "worker.fc3.claude",
    phase: "blocked",
    harnessKind: "claude",
    assignmentRef: "assignment.fc3.claude",
    accountRefHash: "account.pylon.claude.22222222",
    lastProgressAt: "2026-07-09T19:59:10.000Z",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
  decodeFleetWorkerEntity({
    workerId: "worker.fc3.grok",
    phase: "completed",
    harnessKind: "grok",
    assignmentRef: "assignment.fc3.grok",
    accountRefHash: "account.pylon.grok.33333333",
    lastProgressAt: "2026-07-09T19:59:50.000Z",
    updatedAt: "2026-07-09T19:59:50.000Z",
  }),
]

const approvals = [
  decodeFleetApprovalEntity({
    approvalRef: "approval.fc3.claude",
    status: "pending",
    workerId: "worker.fc3.claude",
    toolClass: "write_file",
    openedAt: "2026-07-09T19:59:45.000Z",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
]

const inboxFlags = [
  decodeFleetInboxFlagEntity({
    flagRef: "flag.fc3.approval",
    kind: "approval_required",
    status: "open",
    openedAt: "2026-07-09T19:59:45.000Z",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
]

const commandOutcomes = [
  decodeFleetCommandOutcomeEntity({
    intentId: "intent.fc3.pause",
    seq: 41,
    kind: "fleet_run_control",
    targetRef: run.runId,
    deliveryOutcome: "applied",
    completionOutcome: "applied",
    effectiveOutcome: "paused",
    completionRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
    completedAt: "2026-07-09T19:59:57.000Z",
    outcomeRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
    observedAt: "2026-07-09T19:59:56.000Z",
    recordedAt: "2026-07-09T19:59:57.000Z",
    updatedAt: "2026-07-09T19:59:57.000Z",
  }),
  decodeFleetCommandOutcomeEntity({
    intentId: "intent.fc3.steer",
    seq: 42,
    kind: "steer_message",
    targetRef: "assignment.fc3.codex",
    deliveryOutcome: "queued_follow_up",
    completionOutcome: null,
    effectiveOutcome: null,
    completionRef: null,
    completedAt: null,
    outcomeRef: "outcome.pylon.fleet_steering.0123456789abcdef01234567",
    observedAt: "2026-07-09T19:59:58.000Z",
    recordedAt: "2026-07-09T19:59:59.000Z",
    updatedAt: "2026-07-09T19:59:59.000Z",
  }),
]

const project = () =>
  projectSarahFleetOwnerRun(
    { run, assignments, workers, approvals, inboxFlags, commandOutcomes },
    NOW,
  )

describe("FC-3 owner-safe fleet projection", () => {
  test("maps a named Codex, Claude, and Grok three-stream run", () => {
    const projection = project()

    expect(Schema.decodeUnknownSync(SarahFleetOwnerProjection)(projection)).toEqual(
      projection,
    )
    expect(projection.run).toMatchObject({
      runRef: "fleet.run.sarah.8639",
      status: "running",
      workerKind: "auto",
      availableControls: ["pause", "drain", "stop"],
    })
    expect(projection.workers.map((worker) => worker.harnessKind)).toEqual([
      "claude",
      "codex",
      "grok",
    ])
    expect(projection.workUnits.map((workUnit) => workUnit.name)).toEqual([
      "#8633",
      "#8637",
      "#8639",
    ])

    const codex = projection.workers.find(
      (worker) => worker.workerRef === "worker.fc3.codex",
    )
    const claude = projection.workers.find(
      (worker) => worker.workerRef === "worker.fc3.claude",
    )
    const grok = projection.workers.find(
      (worker) => worker.workerRef === "worker.fc3.grok",
    )
    expect(codex?.progress.status).toBe("fresh")
    expect(claude?.progress).toMatchObject({
      status: "blocked",
      blockerRef: "approval.fc3.claude",
      blockerClass: "approval_pending",
    })
    expect(grok?.progress.status).toBe("completed")

    const grokWork = projection.workUnits.find(
      (workUnit) => workUnit.assignmentRef === "assignment.fc3.grok",
    )
    expect(grokWork?.verification).toEqual({
      status: "ready",
      verificationRef: "assignment.fc3.grok",
      summary: "Verification available",
    })
    expect(grokWork?.closeout).toEqual({
      status: "accepted",
      closeoutRef: "assignment.fc3.grok",
      closeoutClass: "accepted_work",
      summary: "Closeout accepted",
    })
    expect(projection.approvals[0]).toMatchObject({
      approvalRef: "approval.fc3.claude",
      workUnitRef: "#8633",
      availableDecisions: ["allow", "deny"],
    })
    expect(projection.commandOutcomes).toEqual(commandOutcomes)
  })

  test("keeps canonical run, work-unit, and worker identity stable across reconnect", () => {
    const first = project()
    const reconnect = projectSarahFleetOwnerRun(
      {
        run,
        assignments: [...assignments].reverse(),
        workers: [...workers].reverse(),
        approvals: [...approvals].reverse(),
        inboxFlags: [...inboxFlags].reverse(),
        commandOutcomes: [...commandOutcomes].reverse(),
      },
      NOW + 1_000,
    )

    expect(reconnect.run.runRef).toBe(run.runId)
    expect(reconnect.run.runRef).toBe(first.run.runRef)
    expect(reconnect.workUnits.map((workUnit) => workUnit.workUnitRef)).toEqual([
      "#8633",
      "#8637",
      "#8639",
    ])
    expect(reconnect.workUnits.map((workUnit) => workUnit.assignmentRef)).toEqual(
      first.workUnits.map((workUnit) => workUnit.assignmentRef),
    )
    expect(reconnect.workers.map((worker) => worker.workerRef)).toEqual(
      first.workers.map((worker) => worker.workerRef),
    )
    expect(reconnect.commandOutcomes).toEqual(first.commandOutcomes)
  })

  test("projects a dispatched worker as stalled at exactly 30 seconds", () => {
    const staleWorker = decodeFleetWorkerEntity({
      workerId: "worker.fc3.codex",
      phase: "dispatched",
      harnessKind: "codex",
      assignmentRef: "assignment.fc3.codex",
      accountRefHash: "account.pylon.codex.11111111",
      lastProgressAt: "2026-07-09T19:59:30.000Z",
      updatedAt: "2026-07-09T19:59:30.000Z",
    })
    const projection = projectSarahFleetOwnerRun(
      {
        run,
        assignments,
        workers: [staleWorker, workers[1]!, workers[2]!],
        approvals,
        inboxFlags,
      },
      NOW,
    )

    expect(
      projection.workers.find(
        (worker) => worker.workerRef === staleWorker.workerId,
      )?.progress,
    ).toMatchObject({
      status: "stalled",
      ageMs: 30_000,
      reconnect: true,
    })
  })

  test("allowlist mapping structurally excludes raw and private material", () => {
    const unsafeRun = {
      ...run,
      rawPrompt: "PRIVATE PROMPT SENTINEL",
      credential: "bearer-secret-sentinel",
    } as unknown as FleetRunEntity
    const unsafeWorker = {
      ...workers[0]!,
      steerBody: "PRIVATE STEER SENTINEL",
      workspacePath: "/Users/alice/private/repo",
      privateEvents: [{ output: "PRIVATE EVENT SENTINEL" }],
    } as unknown as FleetWorkerEntity
    const unsafeAssignment = {
      ...assignments[0]!,
      output: "PRIVATE OUTPUT SENTINEL",
      command: "rm private-file",
    } as unknown as FleetAssignmentEntity
    const unsafeApproval = {
      ...approvals[0]!,
      toolArgs: { path: "/Users/alice/private/repo" },
    } as unknown as FleetApprovalEntity
    const unsafeCommandOutcome = {
      ...commandOutcomes[1]!,
      body: "PRIVATE STEER BODY SENTINEL",
      localPath: "/Users/alice/private/repo",
    } as unknown as FleetCommandOutcomeEntity

    const json = JSON.stringify(
      projectSarahFleetOwnerRun(
        {
          run: unsafeRun,
          assignments: [unsafeAssignment, assignments[1]!, assignments[2]!],
          workers: [unsafeWorker, workers[1]!, workers[2]!],
          approvals: [unsafeApproval],
          inboxFlags,
          commandOutcomes: [commandOutcomes[0]!, unsafeCommandOutcome],
        },
        NOW,
      ),
    )

    expect(json).not.toMatch(
      /rawPrompt|steerBody|output|credential|workspacePath|privateEvents|toolArgs|localPath|"command":/,
    )
    expect(json).not.toMatch(
      /PRIVATE PROMPT SENTINEL|PRIVATE STEER SENTINEL|PRIVATE STEER BODY SENTINEL|PRIVATE OUTPUT SENTINEL|PRIVATE EVENT SENTINEL|bearer-secret-sentinel|\/Users\/alice/,
    )
  })
})
