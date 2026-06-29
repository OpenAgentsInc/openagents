import { validateAssignmentClaim } from "./assignment-claim-validate.js"

export type BuildAssignmentClaimReceiptInput = {
  assignmentRef: string
  claimedByHash: string
  claimedAt: string
}

export type AssignmentClaimReceipt = {
  kind: "assignment_claim_receipt"
  assignmentRef: string
  claimedByHash: string
  claimedAt: string
  line: string
}

export function buildClaimReceipt(input: BuildAssignmentClaimReceiptInput): AssignmentClaimReceipt {
  const claim = validateAssignmentClaim({
    assignmentRef: input.assignmentRef,
    state: "open",
  })

  return {
    kind: "assignment_claim_receipt",
    assignmentRef: claim.assignmentRef,
    claimedByHash: input.claimedByHash,
    claimedAt: input.claimedAt,
    line: formatClaimReceiptLine({
      assignmentRef: claim.assignmentRef,
      claimedByHash: input.claimedByHash,
      claimedAt: input.claimedAt,
    }),
  }
}

export function validate(receipt: unknown): boolean {
  if (!isReceiptRecord(receipt)) return false
  if (receipt.kind !== "assignment_claim_receipt") return false
  if (typeof receipt.assignmentRef !== "string") return false
  if (typeof receipt.claimedByHash !== "string") return false
  if (typeof receipt.claimedAt !== "string") return false
  if (typeof receipt.line !== "string") return false

  const claim = validateAssignmentClaim({
    assignmentRef: receipt.assignmentRef,
    state: "open",
  })
  if (!claim.ok || receipt.assignmentRef !== claim.assignmentRef) return false

  return receipt.line === formatClaimReceiptLine({
    assignmentRef: receipt.assignmentRef,
    claimedByHash: receipt.claimedByHash,
    claimedAt: receipt.claimedAt,
  })
}

function formatClaimReceiptLine(receipt: {
  assignmentRef: string
  claimedByHash: string
  claimedAt: string
}): string {
  return `Assignment ${receipt.assignmentRef} claimed by ${receipt.claimedByHash} at ${receipt.claimedAt}.`
}

function isReceiptRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
