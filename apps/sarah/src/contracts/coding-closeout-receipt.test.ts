import { describe, expect, test } from "bun:test"
import { Schema } from "effect"

import {
  SARAH_CODING_CLOSEOUT_SECTION_ORDER,
  SarahCodingCloseoutEvidence,
  SarahCodingCloseoutReceipt,
  projectSarahCodingCloseoutReceipts,
} from "./coding-closeout-receipt.ts"
import {
  SarahFleetOwnerProjection,
  type SarahFleetOwnerProjection as SarahFleetOwnerProjectionType,
} from "./fleet-owner-projection.ts"

const completedProgress = (harness: "Codex" | "Claude" | "Grok") => ({
  status: "completed" as const,
  phase: "completed" as const,
  observedAt: "2026-07-09T20:00:00.000Z",
  summary: `${harness} worker completed`,
})

const closeout = (assignmentRef: string) => ({
  status: "accepted" as const,
  closeoutRef: assignmentRef,
  closeoutClass: "accepted_work",
  summary: "Closeout accepted",
})

const verification = (assignmentRef: string) => ({
  status: "ready" as const,
  verificationRef: assignmentRef,
  summary: "Verification available",
})

const projection = Schema.decodeUnknownSync(SarahFleetOwnerProjection)({
  schema: "sarah.fleet_owner_projection.v1",
  run: {
    runRef: "fleet.run.receipt.fixture",
    name: "Fleet run",
    status: "completed",
    desiredSlots: 3,
    workerKind: "auto",
    startedAt: "2026-07-09T19:30:00.000Z",
    counters: {
      workUnitsTotal: 3,
      activeAssignments: 0,
      completedAssignments: 3,
      failedAssignments: 0,
      blockedAssignments: 0,
    },
    updatedAt: "2026-07-09T20:00:00.000Z",
    availableControls: [],
    blockers: [],
  },
  workUnits: [
    {
      workUnitRef: "#8637",
      assignmentRef: "assignment.receipt.codex",
      name: "#8637",
      assignmentStatus: "accepted_work",
      workerRef: "worker.receipt.codex",
      progress: completedProgress("Codex"),
      approvalRefs: [],
      verification: verification("assignment.receipt.codex"),
      closeout: closeout("assignment.receipt.codex"),
      summary: "Work unit accepted work",
      updatedAt: "2026-07-09T20:00:00.000Z",
    },
    {
      workUnitRef: "#8633",
      assignmentRef: "assignment.receipt.claude",
      name: "#8633",
      assignmentStatus: "accepted_work",
      workerRef: "worker.receipt.claude",
      progress: completedProgress("Claude"),
      approvalRefs: ["approval.receipt.claude"],
      verification: verification("assignment.receipt.claude"),
      closeout: closeout("assignment.receipt.claude"),
      summary: "Work unit accepted work",
      updatedAt: "2026-07-09T20:00:00.000Z",
    },
    {
      workUnitRef: "#8639",
      assignmentRef: "assignment.receipt.grok",
      name: "#8639",
      assignmentStatus: "accepted_work",
      workerRef: "worker.receipt.grok",
      progress: completedProgress("Grok"),
      approvalRefs: [],
      verification: verification("assignment.receipt.grok"),
      closeout: closeout("assignment.receipt.grok"),
      summary: "Work unit accepted work",
      updatedAt: "2026-07-09T20:00:00.000Z",
    },
  ],
  workers: [
    {
      workerRef: "worker.receipt.codex",
      name: "Codex worker",
      phase: "completed",
      harnessKind: "codex",
      workUnitRef: "#8637",
      accountRefHash: "account.pylon.codex.11111111",
      progress: completedProgress("Codex"),
      approvalRefs: [],
      updatedAt: "2026-07-09T20:00:00.000Z",
    },
    {
      workerRef: "worker.receipt.claude",
      name: "Claude worker",
      phase: "completed",
      harnessKind: "claude",
      workUnitRef: "#8633",
      accountRefHash: "account.pylon.claude.22222222",
      progress: completedProgress("Claude"),
      approvalRefs: ["approval.receipt.claude"],
      updatedAt: "2026-07-09T20:00:00.000Z",
    },
    {
      workerRef: "worker.receipt.grok",
      name: "Grok worker",
      phase: "completed",
      harnessKind: "grok",
      workUnitRef: "#8639",
      accountRefHash: "account.pylon.grok.33333333",
      progress: completedProgress("Grok"),
      approvalRefs: [],
      updatedAt: "2026-07-09T20:00:00.000Z",
    },
  ],
  approvals: [
    {
      approvalRef: "approval.receipt.claude",
      status: "allowed",
      workerRef: "worker.receipt.claude",
      workUnitRef: "#8633",
      toolClass: "write_file",
      openedAt: "2026-07-09T19:55:00.000Z",
      decidedAt: "2026-07-09T19:56:00.000Z",
      availableDecisions: [],
      summary: "Approval allowed",
      updatedAt: "2026-07-09T19:56:00.000Z",
    },
  ],
  projectedAt: "2026-07-09T20:00:00.000Z",
})

const completeEvidence = [
  {
    assignmentRef: "assignment.receipt.codex",
    verification: {
      status: "passed" as const,
      verificationRef: "verification.receipt.codex",
    },
    changes: {
      changeClass: "source_and_tests",
      artifactRef: "artifact.public.receipt.codex",
    },
    capacity: {
      capacityClass: "owner_local",
      marginalCostClass: "subscription" as const,
    },
    authority: {
      authorityClass: "coding_session_control",
      authorityRef: "authority.owner.receipt.codex",
    },
  },
  {
    assignmentRef: "assignment.receipt.claude",
    verification: {
      status: "passed" as const,
      verificationRef: "verification.receipt.claude",
    },
    changes: {
      changeClass: "source_and_tests",
      artifactRef: "artifact.public.receipt.claude",
    },
    capacity: {
      capacityClass: "owner_local",
      marginalCostClass: "subscription" as const,
    },
    authority: {
      authorityClass: "approval_resolution",
      authorityRef: "authority.owner.receipt.claude",
    },
  },
  {
    assignmentRef: "assignment.receipt.grok",
    verification: {
      status: "passed" as const,
      verificationRef: "verification.receipt.grok",
    },
    changes: {
      changeClass: "source_and_tests",
      artifactRef: "artifact.public.receipt.grok",
    },
    capacity: {
      capacityClass: "owner_local",
      marginalCostClass: "free" as const,
    },
    authority: {
      authorityClass: "coding_session_control",
      authorityRef: "authority.owner.receipt.grok",
    },
  },
]

describe("FC-3 one-minute coding closeout receipt", () => {
  test("the tuple contract fixes the comprehension order", () => {
    const [receipt] = projectSarahCodingCloseoutReceipts({
      projection,
      evidence: completeEvidence,
    })
    expect(receipt?.sections.map((section) => section.kind)).toEqual(
      [...SARAH_CODING_CLOSEOUT_SECTION_ORDER],
    )
    expect(Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)(receipt)).toEqual(
      receipt,
    )
  })

  test("maps complete Codex, Claude, and Grok closeout cards", () => {
    const receipts = projectSarahCodingCloseoutReceipts({
      projection,
      evidence: completeEvidence,
    })

    expect(receipts).toHaveLength(3)
    expect(
      receipts.map((receipt) => receipt.sections[3].harnessKind),
    ).toEqual(["claude", "codex", "grok"])
    expect(
      receipts.map((receipt) => receipt.sections[3].marginalCostClass),
    ).toEqual(["subscription", "subscription", "free"])
    for (const receipt of receipts) {
      expect(receipt.sections[0].status).toBe("succeeded")
      expect(receipt.sections[1].status).toBe("passed")
      expect(receipt.sections[1].verificationRef).not.toBeNull()
      expect(receipt.sections[2].status).toBe("reported")
      expect(receipt.sections[2].artifactRef).not.toBeNull()
      expect(receipt.sections[3].status).toBe("reported")
      expect(receipt.sections[4].authorityStatus).toBe("reported")
      expect(receipt.sections[5].next.action).toBe("open_artifact")
    }
    expect(receipts[0]?.sections[4].approvalStatus).toBe("allowed")
    expect(receipts[1]?.sections[4].approvalStatus).toBe("not_required")
  })

  test("missing evidence remains not reported and cost remains not measured", () => {
    const singleWorkUnitProjection = {
      ...projection,
      workUnits: [projection.workUnits[0]!],
      workers: [projection.workers[0]!],
      approvals: [],
    } as SarahFleetOwnerProjectionType
    const [receipt] = projectSarahCodingCloseoutReceipts({
      projection: singleWorkUnitProjection,
      evidence: [{ assignmentRef: "assignment.receipt.codex" }],
    })

    expect(receipt?.sections[1]).toMatchObject({
      status: "not_reported",
      verificationRef: "assignment.receipt.codex",
    })
    expect(receipt?.sections[2]).toEqual({
      kind: "changes",
      status: "not_reported",
      changeClass: null,
      artifactRef: null,
      summary: "Changes not reported",
    })
    expect(receipt?.sections[3]).toMatchObject({
      status: "not_reported",
      capacityClass: null,
      marginalCostClass: "not_measured",
    })
    expect(receipt?.sections[4]).toMatchObject({
      approvalStatus: "not_required",
      authorityStatus: "not_reported",
      authorityRef: null,
    })

    const [capacityWithoutCost] = projectSarahCodingCloseoutReceipts({
      projection: singleWorkUnitProjection,
      evidence: [
        {
          assignmentRef: "assignment.receipt.codex",
          capacity: { capacityClass: "owner_local" },
        },
      ],
    })
    expect(capacityWithoutCost?.sections[3]).toMatchObject({
      status: "reported",
      capacityClass: "owner_local",
      marginalCostClass: "not_measured",
    })
  })

  test("unsafe raw coding fields cannot cross evidence decoding", () => {
    const unsafeEvidence = {
      ...completeEvidence[0]!,
      rawPrompt: "PRIVATE PROMPT SENTINEL",
      rawDiff: "PRIVATE DIFF SENTINEL",
      command: "PRIVATE COMMAND SENTINEL",
      path: "/Users/alice/private/repo",
      output: "PRIVATE OUTPUT SENTINEL",
      changes: {
        ...completeEvidence[0]!.changes,
        rawDiff: "PRIVATE NESTED DIFF SENTINEL",
        path: "/Users/alice/private/repo",
      },
    }
    const [receipt] = projectSarahCodingCloseoutReceipts({
      projection: {
        ...projection,
        workUnits: [projection.workUnits[0]!],
        workers: [projection.workers[0]!],
        approvals: [],
      } as SarahFleetOwnerProjectionType,
      evidence: [unsafeEvidence],
    })
    const json = JSON.stringify(receipt)

    expect(json).not.toMatch(/rawPrompt|rawDiff|command|path|output/)
    expect(json).not.toMatch(
      /PRIVATE PROMPT SENTINEL|PRIVATE DIFF SENTINEL|PRIVATE COMMAND SENTINEL|PRIVATE OUTPUT SENTINEL|\/Users\/alice/,
    )
    expect(() =>
      Schema.decodeUnknownSync(SarahCodingCloseoutEvidence)({
        assignmentRef: "assignment.receipt.bad",
        changes: {
          changeClass: "source_and_tests",
          artifactRef: "/Users/alice/private/repo",
        },
      }),
    ).toThrow()
    expect(() =>
      Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
        ...receipt,
        workUnitRef: "/Users/alice/private/repo",
      }),
    ).toThrow()
  })
})
