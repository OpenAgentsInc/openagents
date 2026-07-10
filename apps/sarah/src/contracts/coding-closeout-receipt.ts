import {
  FleetAccountRefHash,
  FleetApprovalStatus,
  FleetAttemptMarginalCostClass,
  FleetAttemptUsageEvidence,
  FleetClassToken,
  FleetHarnessKind,
  FleetPublicRef,
} from "@openagentsinc/khala-sync"
import {
  ApprovalDecisionValue,
  FleetRunControlAction,
} from "@openagentsinc/khala-fleet-intents"
import { Schema } from "effect"

import {
  SarahFleetOwnerProjection,
  SarahFleetSafeSummary,
  type SarahFleetOwnerProjection as SarahFleetOwnerProjectionType,
} from "./fleet-owner-projection.ts"

/**
 * Pure FC-3 closeout-card projection. Every card is keyed by one canonical
 * attempt and reads evidence only from that attempt projection. Assignment is
 * an optional graph edge; it is never receipt or evidence identity.
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

const OutcomeSection = Schema.Struct({
  kind: Schema.Literal("outcome"),
  status: Schema.Literals(["succeeded", "failed", "blocked", "in_progress"]),
  attemptState: Schema.Literals([
    "running",
    "evidence_pending",
    "succeeded",
    "failed",
    "stale",
  ]),
  closeoutRef: Schema.NullOr(FleetPublicRef),
  blockerRefs: Schema.Array(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const VerificationSection = Schema.Struct({
  kind: Schema.Literal("verification"),
  status: Schema.Literals(["passed", "failed", "not_reported"]),
  verificationRef: Schema.NullOr(FleetPublicRef),
  evidenceRefs: Schema.Array(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const ChangesSection = Schema.Struct({
  kind: Schema.Literal("changes"),
  status: Schema.Literals(["reported", "not_reported"]),
  changeClass: Schema.NullOr(FleetClassToken),
  artifactRef: Schema.NullOr(FleetPublicRef),
  artifactRefs: Schema.Array(FleetPublicRef),
  proofRefs: Schema.Array(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const CapacityAndCostSection = Schema.Struct({
  kind: Schema.Literal("capacity_and_cost"),
  status: Schema.Literal("reported"),
  harnessKind: FleetHarnessKind,
  pylonRef: FleetPublicRef,
  accountRefHash: Schema.NullOr(FleetAccountRefHash),
  capacityClass: Schema.Literal("owner_local"),
  marginalCostClass: FleetAttemptMarginalCostClass,
  usageEvidence: FleetAttemptUsageEvidence,
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
  authorityReceiptRefs: Schema.Array(FleetPublicRef),
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
  /** Stable view key; exactly the attempt ref, never a fallback identity. */
  cardRef: FleetPublicRef,
  runRef: FleetPublicRef,
  workUnitRef: FleetPublicRef,
  attemptRef: FleetPublicRef,
  assignmentRef: Schema.NullOr(FleetPublicRef),
  sections: Schema.Tuple([
    OutcomeSection,
    VerificationSection,
    ChangesSection,
    CapacityAndCostSection,
    ApprovalAndAuthoritySection,
    NextActionSection,
  ]),
}).pipe(
  Schema.check(
    Schema.makeFilter((receipt) => receipt.cardRef === receipt.attemptRef, {
      message: "coding receipt card identity must equal its attempt ref",
    }),
  ),
)
export type SarahCodingCloseoutReceipt =
  typeof SarahCodingCloseoutReceipt.Type

type FleetAttempt =
  SarahFleetOwnerProjectionType["workUnits"][number]["attempts"][number]
type FleetApproval = SarahFleetOwnerProjectionType["approvals"][number]

const humanizeToken = (token: string): string => token.replaceAll("_", " ")

const verificationSection = (
  attempt: FleetAttempt,
): typeof VerificationSection.Type => {
  if (
    attempt.state === "succeeded" &&
    attempt.verification.status === "ready"
  ) {
    return {
      kind: "verification",
      status: "passed",
      verificationRef: attempt.verification.verificationRef,
      evidenceRefs: [...attempt.verification.evidenceRefs],
      summary: "Verification passed",
    }
  }
  if (attempt.verification.status === "failed") {
    return {
      kind: "verification",
      status: "failed",
      verificationRef: attempt.verification.verificationRef,
      evidenceRefs: [...attempt.verification.evidenceRefs],
      summary: "Verification failed",
    }
  }
  return {
    kind: "verification",
    status: "not_reported",
    verificationRef: attempt.verification.verificationRef,
    evidenceRefs: [...attempt.verification.evidenceRefs],
    summary:
      attempt.verification.verificationRef === null
        ? "Verification not reported"
        : "Verification is not accepted",
  }
}

const changesSection = (
  attempt: FleetAttempt,
): typeof ChangesSection.Type => {
  if (attempt.artifactRefs.length === 0) {
    return {
      kind: "changes",
      status: "not_reported",
      changeClass: null,
      artifactRef: null,
      artifactRefs: [],
      proofRefs: [...attempt.proofRefs],
      summary: "Changes not reported",
    }
  }
  return {
    kind: "changes",
    status: "reported",
    changeClass: "attempt_evidence",
    artifactRef: attempt.artifactRefs[0] ?? null,
    artifactRefs: [...attempt.artifactRefs],
    proofRefs: [...attempt.proofRefs],
    summary: "Attempt artifacts and proofs reported",
  }
}

const capacityAndCostSection = (
  attempt: FleetAttempt,
): typeof CapacityAndCostSection.Type => ({
  kind: "capacity_and_cost",
  status: "reported",
  harnessKind: attempt.capacity.harnessKind,
  pylonRef: attempt.capacity.pylonRef,
  accountRefHash: attempt.capacity.accountRefHash,
  capacityClass: attempt.capacity.capacityClass,
  marginalCostClass: attempt.marginalCostClass,
  usageEvidence: attempt.usageEvidence,
  summary:
    attempt.usageEvidence.truth === "exact"
      ? `Capacity reported. Exact usage ${attempt.usageEvidence.totalTokens} tokens.`
      : attempt.usageEvidence.truth === "not_measured"
        ? "Capacity reported. Usage not measured."
        : "Capacity reported. Usage pending.",
})

const approvalStatus = (
  approvals: ReadonlyArray<FleetApproval>,
  approvalRefs: ReadonlyArray<string>,
): typeof ReceiptApprovalStatus.Type => {
  // An empty association means this attempt has no exact approval evidence.
  // It does not prove that approval was unnecessary.
  if (approvalRefs.length === 0) return "not_reported"
  if (approvals.length !== approvalRefs.length) return "not_reported"
  if (approvals.some((approval) => approval.bindingStatus !== "exact")) {
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
  attempt: FleetAttempt,
  approvals: ReadonlyArray<FleetApproval>,
): typeof ApprovalAndAuthoritySection.Type => {
  const status = approvalStatus(approvals, attempt.approvalRefs)
  const authorityReported = attempt.authorityReceiptRefs.length > 0
  return {
    kind: "approval_and_authority",
    approvalStatus: status,
    approvalRefs: [...attempt.approvalRefs],
    authorityStatus: authorityReported ? "reported" : "not_reported",
    authorityClass: authorityReported ? "attempt_authority_receipt" : null,
    authorityRef: attempt.authorityReceiptRefs[0] ?? null,
    authorityReceiptRefs: [...attempt.authorityReceiptRefs],
    summary: authorityReported
      ? `Approval ${humanizeToken(status)}. Authority reported.`
      : `Approval ${humanizeToken(status)}. Authority not reported.`,
  }
}

const outcomeSection = (
  attempt: FleetAttempt,
  verification: typeof VerificationSection.Type,
): typeof OutcomeSection.Type => {
  if (attempt.state === "succeeded" && verification.status === "passed") {
    return {
      kind: "outcome",
      status: "succeeded",
      attemptState: attempt.state,
      closeoutRef: attempt.closeout.closeoutRef,
      blockerRefs: [...attempt.blockerRefs],
      summary: "Attempt succeeded",
    }
  }
  if (attempt.state === "failed" || verification.status === "failed") {
    return {
      kind: "outcome",
      status: "failed",
      attemptState: attempt.state,
      closeoutRef: attempt.closeout.closeoutRef,
      blockerRefs: [...attempt.blockerRefs],
      summary: "Attempt failed",
    }
  }
  if (
    attempt.state === "stale" ||
    attempt.state === "evidence_pending" ||
    attempt.progress.status === "blocked" ||
    attempt.progress.status === "stalled"
  ) {
    return {
      kind: "outcome",
      status: "blocked",
      attemptState: attempt.state,
      closeoutRef: attempt.closeout.closeoutRef,
      blockerRefs: [...attempt.blockerRefs],
      summary:
        attempt.state === "evidence_pending"
          ? "Attempt evidence pending"
          : "Attempt blocked",
    }
  }
  return {
    kind: "outcome",
    status: "in_progress",
    attemptState: attempt.state,
    closeoutRef: attempt.closeout.closeoutRef,
    blockerRefs: [...attempt.blockerRefs],
    summary: "Attempt in progress",
  }
}

const nextActionSection = (
  projection: SarahFleetOwnerProjectionType,
  attempt: FleetAttempt,
  approvals: ReadonlyArray<FleetApproval>,
  verification: typeof VerificationSection.Type,
  changes: typeof ChangesSection.Type,
): typeof NextActionSection.Type => {
  const pendingApproval = approvals.find(
    (approval) =>
      approval.bindingStatus === "exact" &&
      approval.status === "pending" &&
      approval.attemptRef === attempt.attemptRef &&
      approval.availableDecisions.length > 0,
  )
  if (pendingApproval !== undefined) {
    return {
      kind: "next_action",
      next: {
        action: "resolve_approval",
        targetRef: pendingApproval.approvalRef,
        decisions: [...pendingApproval.availableDecisions],
      },
      summary: "Resolve pending approval",
    }
  }
  const artifactRef = changes.artifactRefs[0]
  if (artifactRef !== undefined) {
    return {
      kind: "next_action",
      next: { action: "open_artifact", targetRef: artifactRef },
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
  if (attempt.closeout.closeoutRef !== null) {
    return {
      kind: "next_action",
      next: {
        action: "open_closeout",
        targetRef: attempt.closeout.closeoutRef,
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

const SarahCodingCloseoutReceiptProjectionInput = Schema.Struct({
  projection: SarahFleetOwnerProjection,
})

const decodeExactProjectionInput = (
  input: unknown,
): typeof SarahCodingCloseoutReceiptProjectionInput.Type => {
  if (
    typeof input !== "object" ||
    input === null ||
    Array.isArray(input) ||
    Object.keys(input).some((key) => key !== "projection") ||
    Object.keys(input).length !== 1
  ) {
    throw new Error("coding receipt input must contain only projection")
  }
  return Schema.decodeUnknownSync(SarahCodingCloseoutReceiptProjectionInput)(
    input,
  )
}

export function projectSarahCodingCloseoutReceipts(
  input: unknown,
): ReadonlyArray<SarahCodingCloseoutReceipt> {
  const { projection } = decodeExactProjectionInput(input)
  const approvalByRef = new Map(
    projection.approvals.map((approval) => [approval.approvalRef, approval]),
  )
  const hasActionableApproval = (attempt: FleetAttempt): boolean =>
    attempt.approvalRefs.some((approvalRef) => {
      const approval = approvalByRef.get(approvalRef)
      return (
        approval?.bindingStatus === "exact" &&
        approval.status === "pending" &&
        approval.attemptRef === attempt.attemptRef &&
        approval.availableDecisions.length > 0
      )
    })

  return projection.workUnits
    .flatMap((workUnit) =>
      workUnit.attempts
        .filter(
          (attempt) =>
            attempt.state !== "running" || hasActionableApproval(attempt),
        )
        .map((attempt) => ({
          workUnitRef: workUnit.workUnitRef,
          attempt,
        })),
    )
    .sort((left, right) =>
      left.attempt.attemptRef.localeCompare(right.attempt.attemptRef),
    )
    .map(({ workUnitRef, attempt }) => {
      const attemptApprovals = attempt.approvalRefs.flatMap((approvalRef) => {
        const approval = approvalByRef.get(approvalRef)
        return approval === undefined ? [] : [approval]
      })
      const verification = verificationSection(attempt)
      const changes = changesSection(attempt)
      const approvalAndAuthority = approvalAndAuthoritySection(
        attempt,
        attemptApprovals,
      )
      const receipt = {
        schema: SARAH_CODING_CLOSEOUT_RECEIPT_SCHEMA,
        cardRef: attempt.attemptRef,
        runRef: projection.run.runRef,
        workUnitRef,
        attemptRef: attempt.attemptRef,
        assignmentRef: attempt.assignmentRef,
        sections: [
          outcomeSection(attempt, verification),
          verification,
          changes,
          capacityAndCostSection(attempt),
          approvalAndAuthority,
          nextActionSection(
            projection,
            attempt,
            attemptApprovals,
            verification,
            changes,
          ),
        ],
      }
      return Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)(receipt)
    })
}
