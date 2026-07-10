import { describe, expect, test } from "bun:test"
import {
  decodeFleetApprovalEntity,
  decodeFleetAttemptEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  decodeFleetWorkUnitEntity,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import {
  SARAH_CODING_CLOSEOUT_SECTION_ORDER,
  SarahCodingCloseoutReceipt,
  projectSarahCodingCloseoutReceipts,
} from "./coding-closeout-receipt.ts"
import { projectSarahFleetOwnerRun } from "./fleet-owner-projection.ts"

const CLAIM_REF = `claim.sarah_fleet_run.${"a".repeat(24)}`
const NOW = Date.parse("2026-07-09T20:00:00.000Z")

const exactUsage = {
  schema: "openagents.pylon.fleet_run_usage_evidence.v1" as const,
  truth: "exact" as const,
  harnessKind: "codex" as const,
  evidenceRef: "evidence.receipt.codex",
  assignmentRef: "assignment.receipt.codex",
  pylonRef: "pylon-owner-1",
  provider: "pylon-codex-own-capacity" as const,
  model: "openagents/pylon-codex" as const,
  demandKind: "own_capacity" as const,
  demandSource: "khala_coding_delegation" as const,
  inputTokens: 8,
  outputTokens: 5,
  reasoningTokens: 2,
  cacheReadTokens: 3,
  totalTokens: 13,
  tokenRows: 1,
  tokenUsageRefs: ["usage.receipt.codex"],
  proofRefs: ["proof.usage.receipt.codex"],
  closeoutChecklistRefs: ["check.closeout.receipt.codex"],
  proofChecklistRefs: ["check.proof.receipt.codex"],
}

const succeeded = decodeFleetAttemptEntity({
  attemptRef: "attempt.receipt.codex",
  workUnitRef: "unit.receipt.codex",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "codex",
  state: "succeeded",
  progressClass: "terminal",
  assignmentRef: "assignment.receipt.codex",
  accountRefHash: `account.pylon.codex.${"1".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "subscription",
  verification: {
    truth: "passed",
    verifierRef: "verifier.receipt.codex",
    evidenceRefs: ["test.receipt.codex"],
  },
  artifactRefs: ["artifact.receipt.codex"],
  proofRefs: ["proof.receipt.codex"],
  authorityReceiptRefs: ["authority.receipt.codex"],
  closeoutRef: "closeout.receipt.codex",
  usageEvidence: exactUsage,
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"1".repeat(24)}`,
  startedAt: "2026-07-09T19:50:00.000Z",
  lastObservedAt: "2026-07-09T19:55:00.000Z",
  remoteObservedAt: "2026-07-09T19:54:59.000Z",
  terminalAt: "2026-07-09T19:55:00.000Z",
  updatedAt: "2026-07-09T19:55:00.000Z",
})

const failedWithoutVerification = decodeFleetAttemptEntity({
  attemptRef: "attempt.receipt.failed",
  workUnitRef: "unit.receipt.failed",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "codex",
  state: "failed",
  progressClass: "terminal",
  assignmentRef: "assignment.receipt.failed",
  accountRefHash: `account.pylon.codex.${"2".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "not_measured",
  verification: { truth: "not_reported" },
  artifactRefs: [],
  proofRefs: ["proof.receipt.failed"],
  authorityReceiptRefs: [],
  closeoutRef: null,
  usageEvidence: { truth: "pending" },
  blockerRefs: ["blocker.receipt.failed"],
  lastEventRef: `event.pylon.fleet_run.${"2".repeat(24)}`,
  startedAt: "2026-07-09T19:51:00.000Z",
  lastObservedAt: "2026-07-09T19:56:00.000Z",
  remoteObservedAt: "2026-07-09T19:55:59.000Z",
  terminalAt: "2026-07-09T19:56:00.000Z",
  updatedAt: "2026-07-09T19:56:00.000Z",
})

const evidencePending = decodeFleetAttemptEntity({
  attemptRef: "attempt.receipt.evidence_pending",
  workUnitRef: "unit.receipt.evidence_pending",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "claude",
  state: "evidence_pending",
  progressClass: "terminal",
  assignmentRef: "assignment.receipt.evidence_pending",
  accountRefHash: `account.pylon.claude_agent.${"3".repeat(24)}`,
  capacityClass: "owner_local",
  marginalCostClass: "subscription",
  verification: { truth: "not_reported" },
  artifactRefs: [],
  proofRefs: [],
  authorityReceiptRefs: [],
  closeoutRef: "closeout.receipt.evidence_pending",
  usageEvidence: { truth: "pending" },
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"3".repeat(24)}`,
  startedAt: "2026-07-09T19:52:00.000Z",
  lastObservedAt: "2026-07-09T19:57:00.000Z",
  remoteObservedAt: "2026-07-09T19:56:59.000Z",
  terminalAt: "2026-07-09T19:57:00.000Z",
  updatedAt: "2026-07-09T19:57:00.000Z",
})

const grok = decodeFleetAttemptEntity({
  attemptRef: "attempt.receipt.grok",
  workUnitRef: "unit.receipt.grok",
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
    verifierRef: "verifier.receipt.grok",
    evidenceRefs: ["test.receipt.grok"],
  },
  artifactRefs: ["artifact.receipt.grok"],
  proofRefs: ["proof.receipt.grok"],
  authorityReceiptRefs: ["authority.receipt.grok"],
  closeoutRef: "closeout.receipt.grok",
  usageEvidence: {
    schema: "openagents.pylon.fleet_run_usage_evidence.v1",
    truth: "not_measured",
    harnessKind: "grok",
    evidenceRef: "evidence.receipt.grok",
    assignmentRef: "assignment.receipt.grok.usage",
    receiptRef: "receipt.receipt.grok",
    tokenUsageRefs: [],
    caveatRefs: ["caveat.receipt.grok.not_measured"],
  },
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"4".repeat(24)}`,
  startedAt: "2026-07-09T19:53:00.000Z",
  lastObservedAt: "2026-07-09T19:58:00.000Z",
  remoteObservedAt: "2026-07-09T19:57:59.000Z",
  terminalAt: "2026-07-09T19:58:00.000Z",
  updatedAt: "2026-07-09T19:58:00.000Z",
})

const running = decodeFleetAttemptEntity({
  attemptRef: "attempt.receipt.running",
  workUnitRef: "unit.receipt.running",
  intakeClaimRef: CLAIM_REF,
  pylonRef: "pylon-owner-1",
  workerKind: "codex",
  state: "running",
  progressClass: "active",
  assignmentRef: null,
  accountRefHash: null,
  capacityClass: "owner_local",
  marginalCostClass: "not_measured",
  verification: { truth: "pending" },
  artifactRefs: [],
  proofRefs: [],
  authorityReceiptRefs: [],
  closeoutRef: null,
  usageEvidence: { truth: "pending" },
  blockerRefs: [],
  lastEventRef: `event.pylon.fleet_run.${"5".repeat(24)}`,
  startedAt: "2026-07-09T19:59:00.000Z",
  lastObservedAt: "2026-07-09T19:59:59.000Z",
  remoteObservedAt: "2026-07-09T19:59:58.000Z",
  terminalAt: null,
  updatedAt: "2026-07-09T19:59:59.000Z",
})

const attemptFixtures = [
  succeeded,
  failedWithoutVerification,
  evidencePending,
  grok,
  running,
]

const workUnits = attemptFixtures.map((attempt) =>
  decodeFleetWorkUnitEntity({
    workUnitRef: attempt.workUnitRef,
    issueRef:
      attempt.attemptRef === succeeded.attemptRef
        ? "#8639"
        : attempt.attemptRef === grok.attemptRef
          ? "#8650"
          : null,
    dependsOnRefs: [],
    state:
      attempt.state === "evidence_pending"
        ? "verification_pending"
        : attempt.state,
    latestAttemptRef: attempt.attemptRef,
    acceptedAttemptRef:
      attempt.state === "succeeded" ? attempt.attemptRef : null,
    updatedAt: attempt.updatedAt,
  }),
)

const run = decodeFleetRunEntity({
  runId: "fleet.run.receipt.fixture",
  status: "running",
  desiredSlots: 3,
  workerKind: "auto",
  startedAt: "2026-07-09T19:49:00.000Z",
  counters: {
    workUnitsTotal: 5,
    activeAssignments: 1,
    completedAssignments: 2,
    failedAssignments: 1,
    blockedAssignments: 1,
  },
  updatedAt: "2026-07-09T19:59:59.000Z",
})

const projection = projectSarahFleetOwnerRun(
  {
    run,
    workUnits,
    attempts: attemptFixtures,
    assignments: [],
    workers: [],
    approvals: [],
    inboxFlags: [],
  },
  NOW,
)

const receipts = () => projectSarahCodingCloseoutReceipts({ projection })

describe("FC-3 attempt-backed coding closeout receipt", () => {
  test("uses exact attempt identity and the fixed comprehension order", () => {
    const result = receipts()
    expect(result).toHaveLength(4)
    for (const receipt of result) {
      expect(receipt.cardRef).toBe(receipt.attemptRef)
      expect(receipt.sections.map((section) => section.kind)).toEqual([
        ...SARAH_CODING_CLOSEOUT_SECTION_ORDER,
      ])
      expect(Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)(receipt)).toEqual(
        receipt,
      )
    }
    const codex = result.find(
      (receipt) => receipt.attemptRef === succeeded.attemptRef,
    )!
    expect(codex.workUnitRef).toBe("unit.receipt.codex")
    expect(codex.workUnitRef).not.toBe("#8639")
    expect(codex.assignmentRef).toBe("assignment.receipt.codex")
  })

  test("emits no receipt for running attempts", () => {
    expect(
      receipts().some((receipt) => receipt.attemptRef === running.attemptRef),
    ).toBe(false)
  })

  test("surfaces an exact pending approval first for a blocked running attempt", () => {
    const blocked = decodeFleetAttemptEntity({
      ...running,
      progressClass: "blocked",
      blockerRefs: ["blocker.receipt.approval"],
      lastEventRef: `event.pylon.fleet_run.${"6".repeat(24)}`,
    })
    const worker = decodeFleetWorkerEntity({
      workerId: "worker.receipt.running",
      phase: "blocked",
      harnessKind: "codex",
      lastProgressAt: blocked.lastObservedAt,
      updatedAt: blocked.updatedAt,
    })
    const approval = decodeFleetApprovalEntity({
      approvalRef: "approval.receipt.running",
      status: "pending",
      runRef: run.runId,
      workUnitRef: blocked.workUnitRef,
      attemptRef: blocked.attemptRef,
      assignmentRef: null,
      workerId: worker.workerId,
      accountRefHash: null,
      requestEventRef: `event.pylon.fleet_run.${"5".repeat(24)}`,
      toolClass: "write_file",
      openedAt: "2026-07-09T19:59:30.000Z",
      updatedAt: "2026-07-09T19:59:59.000Z",
    })
    const unit = workUnits.find(
      (candidate) => candidate.workUnitRef === blocked.workUnitRef,
    )!
    const approvalProjection = projectSarahFleetOwnerRun(
      {
        run,
        workUnits: [unit],
        attempts: [blocked],
        assignments: [],
        workers: [worker],
        approvals: [approval],
        inboxFlags: [],
      },
      NOW,
    )
    const [receipt] = projectSarahCodingCloseoutReceipts({
      projection: approvalProjection,
    })
    expect(receipt?.attemptRef).toBe(blocked.attemptRef)
    expect(receipt?.sections[4]).toMatchObject({
      approvalStatus: "pending",
      approvalRefs: [approval.approvalRef],
    })
    expect(receipt?.sections[5]).toEqual({
      kind: "next_action",
      next: {
        action: "resolve_approval",
        targetRef: approval.approvalRef,
        decisions: ["allow", "deny"],
      },
      summary: "Resolve pending approval",
    })
    const decidedProjection = {
      ...approvalProjection,
      approvals: approvalProjection.approvals.map((candidate) => ({
        ...candidate,
        status: "allowed" as const,
        availableDecisions: [] as const,
        summary: "Approval allowed",
      })),
    }
    expect(
      projectSarahCodingCloseoutReceipts({ projection: decidedProjection }),
    ).toEqual([])
  })

  test("claims pass and success only for a fully proven succeeded attempt", () => {
    const codex = receipts().find(
      (receipt) => receipt.attemptRef === succeeded.attemptRef,
    )!
    expect(codex.sections[0]).toMatchObject({
      status: "succeeded",
      attemptState: "succeeded",
      closeoutRef: "closeout.receipt.codex",
    })
    expect(codex.sections[1]).toEqual({
      kind: "verification",
      status: "passed",
      verificationRef: "verifier.receipt.codex",
      evidenceRefs: ["test.receipt.codex"],
      summary: "Verification passed",
    })
    expect(codex.sections[2]).toMatchObject({
      status: "reported",
      artifactRefs: ["artifact.receipt.codex"],
      proofRefs: ["proof.receipt.codex"],
    })
    expect(codex.sections[3]).toMatchObject({
      status: "reported",
      marginalCostClass: "subscription",
      usageEvidence: { truth: "exact", totalTokens: 13 },
    })
    expect(codex.sections[4]).toMatchObject({
      approvalStatus: "not_reported",
      approvalRefs: [],
      authorityStatus: "reported",
      authorityReceiptRefs: ["authority.receipt.codex"],
    })
  })

  test("keeps failed and evidence-pending terminal states honest", () => {
    const failed = receipts().find(
      (receipt) => receipt.attemptRef === failedWithoutVerification.attemptRef,
    )!
    expect(failed.sections[0]).toMatchObject({
      status: "failed",
      closeoutRef: null,
    })
    expect(failed.sections[1]).toMatchObject({
      status: "not_reported",
      verificationRef: null,
    })
    expect(failed.sections[2]).toMatchObject({
      status: "not_reported",
      artifactRefs: [],
      proofRefs: ["proof.receipt.failed"],
    })
    expect(failed.sections[4]).toMatchObject({
      approvalStatus: "not_reported",
      approvalRefs: [],
    })

    const pending = receipts().find(
      (receipt) => receipt.attemptRef === evidencePending.attemptRef,
    )!
    expect(pending.sections[0]).toMatchObject({
      status: "blocked",
      attemptState: "evidence_pending",
    })
    expect(pending.sections[1].status).toBe("not_reported")
    expect(pending.sections[3].usageEvidence.truth).toBe("pending")
  })

  test("preserves Grok not-measured receipt and caveat evidence with no assignment", () => {
    const grokReceipt = receipts().find(
      (receipt) => receipt.attemptRef === grok.attemptRef,
    )!
    expect(grokReceipt.assignmentRef).toBeNull()
    expect(grokReceipt.sections[0].status).toBe("succeeded")
    expect(grokReceipt.sections[3]).toMatchObject({
      harnessKind: "grok",
      marginalCostClass: "api_metered",
      usageEvidence: {
        truth: "not_measured",
        receiptRef: "receipt.receipt.grok",
        caveatRefs: ["caveat.receipt.grok.not_measured"],
      },
    })
  })

  test("rejects the legacy assignment-keyed evidence channel", () => {
    expect(() =>
      projectSarahCodingCloseoutReceipts({
        projection,
        evidence: [
          {
            assignmentRef: "assignment.receipt.failed",
            verification: { status: "passed" },
          },
        ],
      }),
    ).toThrow("coding receipt input must contain only projection")
  })

  test("schema refuses fallback card identity and private graph refs", () => {
    const codex = receipts().find(
      (receipt) => receipt.attemptRef === succeeded.attemptRef,
    )!
    expect(() =>
      Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
        ...codex,
        cardRef: codex.sections[0].closeoutRef,
      }),
    ).toThrow("coding receipt card identity must equal its attempt ref")
    expect(() =>
      Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
        ...codex,
        assignmentRef: "/Users/operator/private/repo",
      }),
    ).toThrow()
    expect(JSON.stringify(receipts())).not.toMatch(
      /rawPrompt|rawDiff|workspacePath|\/Users\//,
    )
  })
})
