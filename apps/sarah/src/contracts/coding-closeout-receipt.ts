import {
  FleetAccountRefHash,
  FleetApprovalStatus,
  FleetClassToken,
  FleetHarnessKind,
  FleetIssueRef,
  FleetPublicRef,
} from "@openagentsinc/khala-sync"
import {
  ApprovalDecisionValue,
  FleetRunControlAction,
  MarginalCostClass,
} from "@openagentsinc/khala-fleet-intents"
import { Schema } from "effect"

import {
  SarahFleetOwnerProjection,
  SarahFleetSafeSummary,
  type SarahFleetOwnerProjection as SarahFleetOwnerProjectionType,
} from "./fleet-owner-projection.ts"

/**
 * Pure FC-3 closeout-card projection. Its fixed tuple is the reading order,
 * and every optional fact fails honest: no verdict becomes `not_reported`, no
 * measured marginal cost becomes `not_measured`, and no change evidence means
 * no artifact claim. Authority refs are evidence labels, never authority
 * grants; actions still cross the authenticated fleet-intent boundary.
 */
export const SARAH_CODING_CLOSEOUT_RECEIPT_SCHEMA =
  "sarah.coding_closeout_receipt.v1" as const

export const SARAH_CODING_CLOSEOUT_SECTION_ORDER = [
  "outcome",
  "verification",
  "changes",
  "capacity_and_cost",
  "approval_and_authority",
  "next_action",
] as const

const ReceiptVerificationEvidence = Schema.Struct({
  status: Schema.Literals(["passed", "failed"]),
  verificationRef: FleetPublicRef,
})

const ReceiptChangeEvidence = Schema.Struct({
  changeClass: FleetClassToken,
  artifactRef: FleetPublicRef,
})

const ReceiptCapacityEvidence = Schema.Struct({
  capacityClass: FleetClassToken,
  marginalCostClass: Schema.optionalKey(MarginalCostClass),
})

const ReceiptAuthorityEvidence = Schema.Struct({
  authorityClass: FleetClassToken,
  authorityRef: FleetPublicRef,
})

export const SarahCodingCloseoutEvidence = Schema.Struct({
  assignmentRef: FleetPublicRef,
  verification: Schema.optionalKey(ReceiptVerificationEvidence),
  changes: Schema.optionalKey(ReceiptChangeEvidence),
  capacity: Schema.optionalKey(ReceiptCapacityEvidence),
  authority: Schema.optionalKey(ReceiptAuthorityEvidence),
})
export type SarahCodingCloseoutEvidence =
  typeof SarahCodingCloseoutEvidence.Type

const ReceiptWorkUnitRef = Schema.Union([FleetPublicRef, FleetIssueRef])

const OutcomeSection = Schema.Struct({
  kind: Schema.Literal("outcome"),
  status: Schema.Literals(["succeeded", "failed", "blocked", "in_progress"]),
  assignmentStatus: FleetClassToken,
  summary: SarahFleetSafeSummary,
})

const VerificationSection = Schema.Struct({
  kind: Schema.Literal("verification"),
  status: Schema.Literals(["passed", "failed", "not_reported"]),
  verificationRef: Schema.NullOr(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const ChangesSection = Schema.Struct({
  kind: Schema.Literal("changes"),
  status: Schema.Literals(["reported", "not_reported"]),
  changeClass: Schema.NullOr(FleetClassToken),
  artifactRef: Schema.NullOr(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const CapacityAndCostSection = Schema.Struct({
  kind: Schema.Literal("capacity_and_cost"),
  status: Schema.Literals(["reported", "not_reported"]),
  harnessKind: Schema.NullOr(FleetHarnessKind),
  accountRefHash: Schema.NullOr(FleetAccountRefHash),
  capacityClass: Schema.NullOr(FleetClassToken),
  marginalCostClass: MarginalCostClass,
  summary: SarahFleetSafeSummary,
})

const ReceiptApprovalStatus = Schema.Union([
  FleetApprovalStatus,
  Schema.Literals(["not_required", "not_reported"]),
])

const ApprovalAndAuthoritySection = Schema.Struct({
  kind: Schema.Literal("approval_and_authority"),
  approvalStatus: ReceiptApprovalStatus,
  approvalRefs: Schema.Array(FleetPublicRef),
  authorityStatus: Schema.Literals(["reported", "not_reported"]),
  authorityClass: Schema.NullOr(FleetClassToken),
  authorityRef: Schema.NullOr(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const ReceiptNextAction = Schema.Union([
  Schema.Struct({
    action: Schema.Literal("resolve_approval"),
    targetRef: FleetPublicRef,
    decisions: Schema.Array(ApprovalDecisionValue),
  }),
  Schema.Struct({
    action: Schema.Literal("open_artifact"),
    targetRef: FleetPublicRef,
  }),
  Schema.Struct({
    action: Schema.Literal("open_verification"),
    targetRef: FleetPublicRef,
  }),
  Schema.Struct({
    action: Schema.Literal("open_closeout"),
    targetRef: FleetPublicRef,
  }),
  Schema.Struct({
    action: Schema.Literal("control_run"),
    targetRef: FleetPublicRef,
    runControl: FleetRunControlAction,
  }),
  Schema.Struct({
    action: Schema.Literal("none"),
    targetRef: Schema.Null,
  }),
])

const NextActionSection = Schema.Struct({
  kind: Schema.Literal("next_action"),
  next: ReceiptNextAction,
  summary: SarahFleetSafeSummary,
})

export const SarahCodingCloseoutReceipt = Schema.Struct({
  schema: Schema.Literal(SARAH_CODING_CLOSEOUT_RECEIPT_SCHEMA),
  cardRef: FleetPublicRef,
  runRef: FleetPublicRef,
  workUnitRef: ReceiptWorkUnitRef,
  assignmentRef: FleetPublicRef,
  sections: Schema.Tuple([
    OutcomeSection,
    VerificationSection,
    ChangesSection,
    CapacityAndCostSection,
    ApprovalAndAuthoritySection,
    NextActionSection,
  ]),
})
export type SarahCodingCloseoutReceipt =
  typeof SarahCodingCloseoutReceipt.Type

type FleetWorkUnit = SarahFleetOwnerProjectionType["workUnits"][number]
type FleetWorker = SarahFleetOwnerProjectionType["workers"][number]
type FleetApproval = SarahFleetOwnerProjectionType["approvals"][number]

const decodeEvidence = Schema.decodeUnknownSync(SarahCodingCloseoutEvidence)

const humanizeToken = (token: string): string => token.replaceAll("_", " ")

const verificationSection = (
  workUnit: FleetWorkUnit,
  evidence: SarahCodingCloseoutEvidence | undefined,
): typeof VerificationSection.Type => {
  if (
    workUnit.verification.status === "failed" &&
    workUnit.verification.verificationRef !== null
  ) {
    return {
      kind: "verification",
      status: "failed",
      verificationRef: workUnit.verification.verificationRef,
      summary: "Verification failed",
    }
  }
  if (evidence?.verification !== undefined) {
    return {
      kind: "verification",
      status: evidence.verification.status,
      verificationRef: evidence.verification.verificationRef,
      summary:
        evidence.verification.status === "passed"
          ? "Verification passed"
          : "Verification failed",
    }
  }
  return {
    kind: "verification",
    status: "not_reported",
    verificationRef: workUnit.verification.verificationRef,
    summary:
      workUnit.verification.verificationRef === null
        ? "Verification not reported"
        : "Verification verdict not reported",
  }
}

const changesSection = (
  evidence: SarahCodingCloseoutEvidence | undefined,
): typeof ChangesSection.Type => {
  if (evidence?.changes === undefined) {
    return {
      kind: "changes",
      status: "not_reported",
      changeClass: null,
      artifactRef: null,
      summary: "Changes not reported",
    }
  }
  return {
    kind: "changes",
    status: "reported",
    changeClass: evidence.changes.changeClass,
    artifactRef: evidence.changes.artifactRef,
    summary: `Changed ${humanizeToken(evidence.changes.changeClass)}`,
  }
}

const capacityAndCostSection = (
  worker: FleetWorker | undefined,
  evidence: SarahCodingCloseoutEvidence | undefined,
): typeof CapacityAndCostSection.Type => {
  if (
    evidence?.capacity === undefined ||
    worker?.harnessKind === null ||
    worker?.harnessKind === undefined ||
    worker.accountRefHash === null
  ) {
    return {
      kind: "capacity_and_cost",
      status: "not_reported",
      harnessKind: worker?.harnessKind ?? null,
      accountRefHash: worker?.accountRefHash ?? null,
      capacityClass: null,
      marginalCostClass: "not_measured",
      summary: "Capacity not reported. Cost not measured.",
    }
  }
  const marginalCostClass =
    evidence.capacity.marginalCostClass ?? "not_measured"
  return {
    kind: "capacity_and_cost",
    status: "reported",
    harnessKind: worker?.harnessKind ?? null,
    accountRefHash: worker?.accountRefHash ?? null,
    capacityClass: evidence.capacity.capacityClass,
    marginalCostClass,
    summary:
      marginalCostClass === "not_measured"
        ? "Capacity reported. Cost not measured."
        : `Capacity reported. Cost ${humanizeToken(marginalCostClass)}.`,
  }
}

const approvalStatus = (
  approvals: ReadonlyArray<FleetApproval>,
  approvalRefs: ReadonlyArray<string>,
): typeof ReceiptApprovalStatus.Type => {
  if (approvalRefs.length === 0) {
    return "not_required"
  }
  if (approvals.length === 0) {
    return "not_reported"
  }
  if (approvals.some((approval) => approval.status === "denied")) {
    return "denied"
  }
  if (approvals.some((approval) => approval.status === "pending")) {
    return "pending"
  }
  return "allowed"
}

const approvalAndAuthoritySection = (
  workUnit: FleetWorkUnit,
  approvals: ReadonlyArray<FleetApproval>,
  evidence: SarahCodingCloseoutEvidence | undefined,
): typeof ApprovalAndAuthoritySection.Type => {
  const status = approvalStatus(approvals, workUnit.approvalRefs)
  const authorityReported = evidence?.authority !== undefined
  return {
    kind: "approval_and_authority",
    approvalStatus: status,
    approvalRefs: [...workUnit.approvalRefs],
    authorityStatus: authorityReported ? "reported" : "not_reported",
    authorityClass: evidence?.authority?.authorityClass ?? null,
    authorityRef: evidence?.authority?.authorityRef ?? null,
    summary: authorityReported
      ? `Approval ${humanizeToken(status)}. Authority reported.`
      : `Approval ${humanizeToken(status)}. Authority not reported.`,
  }
}

const outcomeSection = (
  workUnit: FleetWorkUnit,
  verification: typeof VerificationSection.Type,
): typeof OutcomeSection.Type => {
  if (
    verification.status === "failed" ||
    workUnit.closeout.status === "rejected"
  ) {
    return {
      kind: "outcome",
      status: "failed",
      assignmentStatus: workUnit.assignmentStatus,
      summary: "Work unit failed",
    }
  }
  if (workUnit.closeout.status === "accepted") {
    return {
      kind: "outcome",
      status: "succeeded",
      assignmentStatus: workUnit.assignmentStatus,
      summary: "Work unit succeeded",
    }
  }
  if (
    workUnit.progress.status === "blocked" ||
    workUnit.progress.status === "stalled"
  ) {
    return {
      kind: "outcome",
      status: "blocked",
      assignmentStatus: workUnit.assignmentStatus,
      summary: "Work unit blocked",
    }
  }
  return {
    kind: "outcome",
    status: "in_progress",
    assignmentStatus: workUnit.assignmentStatus,
    summary: "Work unit in progress",
  }
}

const nextActionSection = (
  projection: SarahFleetOwnerProjectionType,
  workUnit: FleetWorkUnit,
  verification: typeof VerificationSection.Type,
  changes: typeof ChangesSection.Type,
  approval: typeof ApprovalAndAuthoritySection.Type,
): typeof NextActionSection.Type => {
  if (approval.approvalStatus === "pending") {
    const targetRef = approval.approvalRefs[0]
    if (targetRef !== undefined) {
      return {
        kind: "next_action",
        next: {
          action: "resolve_approval",
          targetRef,
          decisions: ["allow", "deny"],
        },
        summary: "Resolve approval",
      }
    }
  }
  if (changes.artifactRef !== null) {
    return {
      kind: "next_action",
      next: { action: "open_artifact", targetRef: changes.artifactRef },
      summary: "Open safe artifact",
    }
  }
  if (verification.verificationRef !== null) {
    return {
      kind: "next_action",
      next: {
        action: "open_verification",
        targetRef: verification.verificationRef,
      },
      summary: "Open verification",
    }
  }
  if (workUnit.closeout.closeoutRef !== null) {
    return {
      kind: "next_action",
      next: {
        action: "open_closeout",
        targetRef: workUnit.closeout.closeoutRef,
      },
      summary: "Open closeout",
    }
  }
  const runControl = projection.run.availableControls[0]
  if (runControl !== undefined) {
    return {
      kind: "next_action",
      next: {
        action: "control_run",
        targetRef: projection.run.runRef,
        runControl,
      },
      summary: `Run action ${humanizeToken(runControl)}`,
    }
  }
  return {
    kind: "next_action",
    next: { action: "none", targetRef: null },
    summary: "No action available",
  }
}

export function projectSarahCodingCloseoutReceipts(input: Readonly<{
  projection: SarahFleetOwnerProjectionType
  evidence: ReadonlyArray<SarahCodingCloseoutEvidence>
}>): ReadonlyArray<SarahCodingCloseoutReceipt> {
  const projection = Schema.decodeUnknownSync(SarahFleetOwnerProjection)(
    input.projection,
  )
  const evidence = input.evidence.map((entry) => decodeEvidence(entry))
  const evidenceByAssignment = new Map(
    evidence.map((entry) => [entry.assignmentRef, entry]),
  )
  const workerByRef = new Map(
    projection.workers.map((worker) => [worker.workerRef, worker]),
  )
  const approvalByRef = new Map(
    projection.approvals.map((approval) => [approval.approvalRef, approval]),
  )

  return [...projection.workUnits]
    .sort((left, right) => left.assignmentRef.localeCompare(right.assignmentRef))
    .map((workUnit) => {
      const worker =
        workUnit.workerRef === null
          ? undefined
          : workerByRef.get(workUnit.workerRef)
      const workUnitApprovals = workUnit.approvalRefs.flatMap((approvalRef) => {
        const approval = approvalByRef.get(approvalRef)
        return approval === undefined ? [] : [approval]
      })
      const workUnitEvidence = evidenceByAssignment.get(workUnit.assignmentRef)
      const verification = verificationSection(workUnit, workUnitEvidence)
      const changes = changesSection(workUnitEvidence)
      const capacityAndCost = capacityAndCostSection(worker, workUnitEvidence)
      const approvalAndAuthority = approvalAndAuthoritySection(
        workUnit,
        workUnitApprovals,
        workUnitEvidence,
      )
      const receipt = {
        schema: SARAH_CODING_CLOSEOUT_RECEIPT_SCHEMA,
        cardRef: workUnit.closeout.closeoutRef ?? workUnit.assignmentRef,
        runRef: projection.run.runRef,
        workUnitRef: workUnit.workUnitRef,
        assignmentRef: workUnit.assignmentRef,
        sections: [
          outcomeSection(workUnit, verification),
          verification,
          changes,
          capacityAndCost,
          approvalAndAuthority,
          nextActionSection(
            projection,
            workUnit,
            verification,
            changes,
            approvalAndAuthority,
          ),
        ],
      }
      return Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)(receipt)
    })
}
