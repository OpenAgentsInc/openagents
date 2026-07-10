import { describe, expect, test } from "bun:test"
import {
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetAttemptEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  decodeFleetWorkUnitEntity,
  type FleetAssignmentEntity,
  type FleetAttemptEntity,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import {
  SarahFleetOwnerProjection,
  projectSarahFleetOwnerRun,
} from "./fleet-owner-projection.ts"

const NOW = Date.parse("2026-07-09T20:00:00.000Z")
const CLAIM_REF = `claim.sarah_fleet_run.${"a".repeat(24)}`

const exactUsage = (
  assignmentRef: string,
  evidenceRef: string,
  totalTokens: number,
) => ({
  schema: "openagents.pylon.fleet_run_usage_evidence.v1" as const,
  truth: "exact" as const,
  harnessKind: "codex" as const,
  evidenceRef,
  assignmentRef,
  pylonRef: "pylon-owner-1",
  provider: "pylon-codex-own-capacity" as const,
  model: "openagents/pylon-codex" as const,
  demandKind: "own_capacity" as const,
  demandSource: "khala_coding_delegation" as const,
  inputTokens: totalTokens - 4,
  outputTokens: 4,
  reasoningTokens: 2,
  cacheReadTokens: 1,
  totalTokens,
  tokenRows: 1,
  tokenUsageRefs: [`usage.${evidenceRef}`],
  proofRefs: [`proof.usage.${evidenceRef}`],
  closeoutChecklistRefs: [`check.closeout.${evidenceRef}`],
  proofChecklistRefs: [`check.proof.${evidenceRef}`],
})

const retryFailed = decodeFleetAttemptEntity({
  attemptRef: "work_claim.retry.1",
  workUnitRef: "unit.retry",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "codex",
  state: "failed",
  progressClass: "terminal",
  assignmentRef: "assignment.retry.1",
  accountRefHash: `account.pylon.codex.${"1".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "subscription",
  verification: {
    truth: "failed",
    verifierRef: "verifier.retry.1",
    evidenceRefs: ["test.retry.1"],
  },
  artifactRefs: ["artifact.retry.1"],
  proofRefs: ["proof.retry.1"],
  authorityReceiptRefs: ["authority.retry.1"],
  closeoutRef: "closeout.retry.1",
  usageEvidence: exactUsage(
    "assignment.retry.1",
    "evidence.retry.1",
    10,
  ),
  blockerRefs: ["blocker.retry.test_failed"],
  lastEventRef: `event.pylon.fleet_run.${"1".repeat(24)}`,
  startedAt: "2026-07-09T19:50:00.000Z",
  lastObservedAt: "2026-07-09T19:52:00.000Z",
  remoteObservedAt: "2026-07-09T19:51:59.000Z",
  terminalAt: "2026-07-09T19:52:00.000Z",
  updatedAt: "2026-07-09T19:52:00.000Z",
})

const retrySucceeded = decodeFleetAttemptEntity({
  attemptRef: "work_claim.retry.2",
  workUnitRef: "unit.retry",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "codex",
  state: "succeeded",
  progressClass: "terminal",
  assignmentRef: "assignment.retry.2",
  accountRefHash: `account.pylon.codex.${"2".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "subscription",
  verification: {
    truth: "passed",
    verifierRef: "verifier.retry.2",
    evidenceRefs: ["test.retry.2"],
  },
  artifactRefs: ["artifact.retry.2"],
  proofRefs: ["proof.retry.2"],
  authorityReceiptRefs: ["authority.retry.2"],
  closeoutRef: "closeout.retry.2",
  usageEvidence: exactUsage(
    "assignment.retry.2",
    "evidence.retry.2",
    14,
  ),
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"2".repeat(24)}`,
  startedAt: "2026-07-09T19:53:00.000Z",
  lastObservedAt: "2026-07-09T19:56:00.000Z",
  remoteObservedAt: "2026-07-09T19:55:58.000Z",
  terminalAt: "2026-07-09T19:56:00.000Z",
  updatedAt: "2026-07-09T19:56:00.000Z",
})

const runningAttempt = decodeFleetAttemptEntity({
  attemptRef: "work_claim.running.1",
  workUnitRef: "unit.running",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "codex",
  state: "running",
  progressClass: "active",
  assignmentRef: "assignment.running.1",
  accountRefHash: `account.pylon.codex.${"3".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "not_measured",
  verification: { truth: "pending" },
  artifactRefs: [],
  proofRefs: [],
  authorityReceiptRefs: [],
  closeoutRef: null,
  usageEvidence: { truth: "pending" },
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"3".repeat(24)}`,
  startedAt: "2026-07-09T19:58:00.000Z",
  lastObservedAt: "2026-07-09T19:59:30.000Z",
  // Deliberately newer than the server receipt clock. Audit only.
  remoteObservedAt: "2026-07-09T20:59:59.000Z",
  terminalAt: null,
  updatedAt: "2026-07-09T19:59:30.000Z",
})

const grokSucceeded = decodeFleetAttemptEntity({
  attemptRef: "work_claim.grok.1",
  workUnitRef: "unit.grok",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "grok",
  state: "succeeded",
  progressClass: "terminal",
  assignmentRef: null,
  accountRefHash: `account.pylon.grok.${"4".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "api_metered",
  verification: {
    truth: "passed",
    verifierRef: "verifier.grok.1",
    evidenceRefs: ["test.grok.1"],
  },
  artifactRefs: ["artifact.grok.1"],
  proofRefs: ["proof.grok.1"],
  authorityReceiptRefs: ["authority.grok.1"],
  closeoutRef: "closeout.grok.1",
  usageEvidence: {
    schema: "openagents.pylon.fleet_run_usage_evidence.v1",
    truth: "not_measured",
    harnessKind: "grok",
    evidenceRef: "evidence.grok.1",
    assignmentRef: "assignment.grok.receipt.1",
    receiptRef: "receipt.grok.1",
    tokenUsageRefs: [],
    caveatRefs: ["caveat.grok.not_measured"],
  },
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"4".repeat(24)}`,
  startedAt: "2026-07-09T19:54:00.000Z",
  lastObservedAt: "2026-07-09T19:58:00.000Z",
  remoteObservedAt: "2026-07-09T19:57:59.000Z",
  terminalAt: "2026-07-09T19:58:00.000Z",
  updatedAt: "2026-07-09T19:58:00.000Z",
})

const workUnits = [
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.planned",
    issueRef: "#8640",
    dependsOnRefs: [],
    state: "planned",
    latestAttemptRef: null,
    acceptedAttemptRef: null,
    updatedAt: "2026-07-09T19:49:00.000Z",
  }),
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.retry",
    issueRef: "#8639",
    dependsOnRefs: ["unit.planned"],
    state: "succeeded",
    latestAttemptRef: retrySucceeded.attemptRef,
    acceptedAttemptRef: retrySucceeded.attemptRef,
    updatedAt: "2026-07-09T19:56:00.000Z",
  }),
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.running",
    issueRef: null,
    dependsOnRefs: [],
    state: "running",
    latestAttemptRef: runningAttempt.attemptRef,
    acceptedAttemptRef: null,
    updatedAt: "2026-07-09T19:59:30.000Z",
  }),
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.grok",
    issueRef: "#8650",
    dependsOnRefs: [],
    state: "succeeded",
    latestAttemptRef: grokSucceeded.attemptRef,
    acceptedAttemptRef: grokSucceeded.attemptRef,
    updatedAt: "2026-07-09T19:58:00.000Z",
  }),
]

const attempts = [retryFailed, retrySucceeded, runningAttempt, grokSucceeded]

const assignments = [
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.retry.1",
    issueRef: "#8639",
    status: "rejected",
    closeoutClass: "rejected",
    updatedAt: "2026-07-09T19:52:00.000Z",
  }),
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.retry.2",
    issueRef: "#8639",
    status: "accepted_work",
    closeoutClass: "accepted_work",
    updatedAt: "2026-07-09T19:56:00.000Z",
  }),
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.running.1",
    status: "running",
    updatedAt: "2026-07-09T19:59:30.000Z",
  }),
]

const workers = [
  decodeFleetWorkerEntity({
    workerId: "worker.retry.2",
    phase: "completed",
    harnessKind: "codex",
    assignmentRef: "assignment.retry.2",
    accountRefHash: `account.pylon.codex.${"2".repeat(24)}`,
    lastProgressAt: "2026-07-09T19:56:00.000Z",
    updatedAt: "2026-07-09T19:56:00.000Z",
  }),
  decodeFleetWorkerEntity({
    workerId: "worker.running.1",
    phase: "dispatched",
    harnessKind: "codex",
    assignmentRef: "assignment.running.1",
    accountRefHash: `account.pylon.codex.${"3".repeat(24)}`,
    lastProgressAt: "2026-07-09T20:59:59.000Z",
    updatedAt: "2026-07-09T20:59:59.000Z",
  }),
]

const approvals = [
  decodeFleetApprovalEntity({
    approvalRef: "approval.running.1",
    status: "pending",
    workerId: "worker.running.1",
    toolClass: "write_file",
    openedAt: "2026-07-09T19:59:20.000Z",
    updatedAt: "2026-07-09T19:59:20.000Z",
  }),
]

const run = decodeFleetRunEntity({
  runId: "fleet.run.sarah.8639",
  status: "running",
  desiredSlots: 3,
  workerKind: "auto",
  startedAt: "2026-07-09T19:49:00.000Z",
  counters: {
    workUnitsTotal: 4,
    activeAssignments: 1,
    completedAssignments: 2,
    failedAssignments: 1,
    blockedAssignments: 0,
  },
  updatedAt: "2026-07-09T19:59:30.000Z",
})

const project = () =>
  projectSarahFleetOwnerRun(
    {
      run,
      workUnits,
      attempts,
      assignments,
      workers,
      approvals,
      inboxFlags: [],
    },
    NOW,
  )

describe("FC-3 direct owner-safe fleet projection", () => {
  test("uses stable work-unit identity and preserves every retry attempt", () => {
    const projection = project()
    expect(Schema.decodeUnknownSync(SarahFleetOwnerProjection)(projection)).toEqual(
      projection,
    )
    expect(projection.workUnits.map((unit) => unit.workUnitRef)).toEqual([
      "unit.grok",
      "unit.planned",
      "unit.retry",
      "unit.running",
    ])

    const retry = projection.workUnits.find(
      (unit) => unit.workUnitRef === "unit.retry",
    )!
    expect(retry.assignmentRef).toBe("assignment.retry.2")
    expect(retry.latestAttemptRef).toBe("work_claim.retry.2")
    expect(retry.acceptedAttemptRef).toBe("work_claim.retry.2")
    expect(retry.attempts.map((attempt) => attempt.attemptRef)).toEqual([
      "work_claim.retry.1",
      "work_claim.retry.2",
    ])
    expect(retry.attempts.map((attempt) => attempt.state)).toEqual([
      "failed",
      "succeeded",
    ])
    expect(retry.verification).toMatchObject({
      status: "ready",
      verificationRef: "verifier.retry.2",
      evidenceRefs: ["test.retry.2"],
    })
    expect(retry.closeout.closeoutRef).toBe("closeout.retry.2")
    expect(retry.artifactRefs).toEqual(["artifact.retry.2"])
    expect(retry.proofRefs).toEqual(["proof.retry.2"])
    expect(retry.authorityReceiptRefs).toEqual(["authority.retry.2"])
    expect(retry.usageEvidence).toMatchObject({
      truth: "exact",
      totalTokens: 14,
    })
    expect(retry.approvalRefs).toEqual([])
    expect(retry.attempts.every((attempt) => attempt.approvalRefs.length === 0)).toBe(
      true,
    )
    expect(
      projection.workers.find((worker) => worker.workerRef === "worker.retry.2")
        ?.workUnitRef,
    ).toBe("unit.retry")
    expect(projection.approvals[0]).toMatchObject({
      approvalRef: "approval.running.1",
      workUnitRef: null,
      availableDecisions: [],
    })
    expect(projection.workUnits.find((unit) => unit.issueRef === "#8639")).toMatchObject({
      workUnitRef: "unit.retry",
      name: "#8639",
    })
  })

  test("represents planned work with no attempt or assignment", () => {
    const planned = project().workUnits.find(
      (unit) => unit.workUnitRef === "unit.planned",
    )!
    expect(planned).toMatchObject({
      state: "planned",
      latestAttemptRef: null,
      acceptedAttemptRef: null,
      assignmentRef: null,
      workerRef: null,
      attempts: [],
      progress: { status: "not_assigned", summary: "Work unit planned" },
    })
    expect(planned.verification.verificationRef).toBeNull()
    expect(planned.closeout.closeoutRef).toBeNull()
  })

  test("keeps unreported failed and stale closeouts open without changing attempt outcome", () => {
    for (const state of ["failed", "stale"] as const) {
      const attempt = decodeFleetAttemptEntity({
        ...retryFailed,
        attemptRef: `work_claim.no-closeout.${state}`,
        workUnitRef: `unit.no-closeout.${state}`,
        state,
        verification: { truth: "not_reported" },
        artifactRefs: [],
        proofRefs: [],
        authorityReceiptRefs: [],
        closeoutRef: null,
        usageEvidence: { truth: "pending" },
        lastEventRef: `event.pylon.fleet_run.${(
          state === "failed" ? "5" : "6"
        ).repeat(24)}`,
      })
      const workUnit = decodeFleetWorkUnitEntity({
        workUnitRef: attempt.workUnitRef,
        issueRef: null,
        dependsOnRefs: [],
        state,
        latestAttemptRef: attempt.attemptRef,
        acceptedAttemptRef: null,
        updatedAt: attempt.updatedAt,
      })
      const projection = projectSarahFleetOwnerRun(
        {
          run,
          workUnits: [workUnit],
          attempts: [attempt],
          assignments: [assignments[0]!],
          workers: [],
          approvals: [],
          inboxFlags: [],
        },
        NOW,
      )
      expect(projection.workUnits[0]?.attempts[0]).toMatchObject({
        state,
        closeout: {
          status: "open",
          closeoutRef: null,
          closeoutClass: null,
          summary: "Closeout not reported",
        },
      })
      expect(projection.workUnits[0]?.closeout.status).toBe("open")
    }
  })

  test("uses server receipt time for freshness and labels remote time audit-only", () => {
    const running = project().workUnits.find(
      (unit) => unit.workUnitRef === "unit.running",
    )!
    expect(running.progress).toMatchObject({
      status: "stalled",
      heartbeatAt: "2026-07-09T19:59:30.000Z",
      ageMs: 30_000,
      reconnect: true,
    })
    expect(running.attempts[0]).toMatchObject({
      lastObservedAt: "2026-07-09T19:59:30.000Z",
      remoteObservedAtAudit: "2026-07-09T20:59:59.000Z",
    })
    expect(projectionWithoutPrivateFields(running)).not.toContain(
      "remoteObservedAt\"",
    )
  })

  test("assignments alone never fabricate work units or evidence", () => {
    const projection = projectSarahFleetOwnerRun(
      {
        run,
        workUnits: [],
        attempts: [],
        assignments,
        workers,
        approvals,
        inboxFlags: [],
      },
      NOW,
    )
    expect(projection.workUnits).toEqual([])
  })

  test("preserves retry history when attempts reuse one assignment edge", () => {
    const reusedAssignmentAttempt = decodeFleetAttemptEntity({
      ...retryFailed,
      assignmentRef: retrySucceeded.assignmentRef,
      usageEvidence: exactUsage(
        retrySucceeded.assignmentRef!,
        "evidence.retry.reused",
        10,
      ),
    })
    const projection = projectSarahFleetOwnerRun(
      {
        run,
        workUnits,
        attempts: [
          reusedAssignmentAttempt,
          retrySucceeded,
          runningAttempt,
          grokSucceeded,
        ],
        assignments: [assignments[1]!, assignments[2]!],
        workers,
        approvals,
        inboxFlags: [],
      },
      NOW,
    )
    const retry = projection.workUnits.find(
      (unit) => unit.workUnitRef === "unit.retry",
    )!
    expect(retry.attempts.map((attempt) => attempt.attemptRef)).toEqual([
      "work_claim.retry.1",
      "work_claim.retry.2",
    ])
    expect(retry.assignmentRef).toBe("assignment.retry.2")
  })

  test("fails closed on duplicate, orphan, and cross-unit attempt pointers", () => {
    expect(() =>
      projectSarahFleetOwnerRun(
        {
          run,
          workUnits: [...workUnits, workUnits[0]!],
          attempts,
          assignments,
          workers,
          approvals,
          inboxFlags: [],
        },
        NOW,
      ),
    ).toThrow("duplicate work-unit ref")
    expect(() =>
      projectSarahFleetOwnerRun(
        {
          run,
          workUnits,
          attempts: [...attempts, attempts[0]!],
          assignments,
          workers,
          approvals,
          inboxFlags: [],
        },
        NOW,
      ),
    ).toThrow("duplicate attempt ref")

    const orphan = {
      ...runningAttempt,
      attemptRef: "work_claim.orphan.1",
      workUnitRef: "unit.unknown",
      lastEventRef: `event.pylon.fleet_run.${"5".repeat(24)}`,
    } as FleetAttemptEntity
    expect(() =>
      projectSarahFleetOwnerRun(
        {
          run,
          workUnits,
          attempts: [...attempts, orphan],
          assignments,
          workers,
          approvals,
          inboxFlags: [],
        },
        NOW,
      ),
    ).toThrow("attempt names an unknown work unit")

    const crossUnit = decodeFleetWorkUnitEntity({
      workUnitRef: "unit.running",
      issueRef: null,
      dependsOnRefs: [],
      state: "running",
      latestAttemptRef: retrySucceeded.attemptRef,
      acceptedAttemptRef: null,
      updatedAt: "2026-07-09T19:59:30.000Z",
    })
    expect(() =>
      projectSarahFleetOwnerRun(
        {
          run,
          workUnits: workUnits.map((unit) =>
            unit.workUnitRef === crossUnit.workUnitRef ? crossUnit : unit,
          ),
          attempts,
          assignments,
          workers,
          approvals,
          inboxFlags: [],
        },
        NOW,
      ),
    ).toThrow("work-unit attempt pointer is unresolved or cross-unit")

    const incoherentUnit = decodeFleetWorkUnitEntity({
      workUnitRef: "unit.retry",
      issueRef: "#8639",
      dependsOnRefs: [],
      state: "succeeded",
      latestAttemptRef: retryFailed.attemptRef,
      acceptedAttemptRef: retryFailed.attemptRef,
      updatedAt: retryFailed.updatedAt,
    })
    expect(() =>
      projectSarahFleetOwnerRun(
        {
          run,
          workUnits: workUnits.map((unit) =>
            unit.workUnitRef === incoherentUnit.workUnitRef
              ? incoherentUnit
              : unit,
          ),
          attempts,
          assignments,
          workers,
          approvals,
          inboxFlags: [],
        },
        NOW,
      ),
    ).toThrow("work-unit state disagrees with its latest attempt")
  })

  test("strips hostile assignment and attempt material at the projection boundary", () => {
    const unsafeAttempt = {
      ...retrySucceeded,
      rawPrompt: "PRIVATE PROMPT SENTINEL",
      rawDiff: "PRIVATE DIFF SENTINEL",
      workspacePath: "/Users/operator/private/repo",
    } as unknown as FleetAttemptEntity
    const unsafeAssignment = {
      ...assignments[1]!,
      commandOutput: "PRIVATE OUTPUT SENTINEL",
      credential: "bearer-secret-sentinel",
    } as unknown as FleetAssignmentEntity
    const json = JSON.stringify(
      projectSarahFleetOwnerRun(
        {
          run,
          workUnits,
          attempts: [retryFailed, unsafeAttempt, runningAttempt, grokSucceeded],
          assignments: [assignments[0]!, unsafeAssignment, assignments[2]!],
          workers,
          approvals,
          inboxFlags: [],
        },
        NOW,
      ),
    )
    expect(json).not.toMatch(
      /rawPrompt|rawDiff|workspacePath|commandOutput|credential|PRIVATE|bearer-secret|\/Users\//,
    )
  })
})

const projectionWithoutPrivateFields = (value: unknown): string =>
  JSON.stringify(value)
