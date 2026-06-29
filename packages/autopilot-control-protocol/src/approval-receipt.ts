import type { ApprovalDecision } from "./approval-answer.js"

export type BuildApprovalReceiptInput = {
  ref: string
  decision: ApprovalDecision
  answer?: string
  decidedAt: string
  actor: string
}

export type ApprovalReceipt = {
  kind: "approval_receipt"
  ref: string
  decision: string
  hasAnswer: boolean
  decidedAt: string
  actor: string
  line: string
}

export function buildApprovalReceipt(input: BuildApprovalReceiptInput): ApprovalReceipt {
  const hasAnswer = input.decision === "answer" && (input.answer?.length ?? 0) > 0

  return {
    kind: "approval_receipt",
    ref: input.ref,
    decision: input.decision,
    hasAnswer,
    decidedAt: input.decidedAt,
    actor: input.actor,
    line: formatApprovalReceiptLine({
      ref: input.ref,
      decision: input.decision,
      hasAnswer,
      decidedAt: input.decidedAt,
      actor: input.actor,
    }),
  }
}

export function validateApprovalReceipt(receipt: unknown): boolean {
  if (!isReceiptRecord(receipt)) return false
  if (receipt.kind !== "approval_receipt") return false
  if (typeof receipt.ref !== "string") return false
  if (!isApprovalDecision(receipt.decision)) return false
  if (typeof receipt.hasAnswer !== "boolean") return false
  if (typeof receipt.decidedAt !== "string") return false
  if (typeof receipt.actor !== "string") return false
  if (typeof receipt.line !== "string") return false

  // Each field is validated above; build the typed shape explicitly so the
  // formatter receives the exact type rather than a broad Record.
  return receipt.line === formatApprovalReceiptLine({
    ref: receipt.ref,
    decision: receipt.decision,
    hasAnswer: receipt.hasAnswer,
    decidedAt: receipt.decidedAt,
    actor: receipt.actor,
  })
}

function formatApprovalReceiptLine(receipt: {
  ref: string
  decision: ApprovalDecision
  hasAnswer: boolean
  decidedAt: string
  actor: string
}): string {
  const answer = receipt.hasAnswer ? " with answer" : ""

  return `Approval ${receipt.ref} ${receipt.decision}${answer} by ${receipt.actor} at ${receipt.decidedAt}.`
}

function isApprovalDecision(value: unknown): value is ApprovalDecision {
  return value === "approve" || value === "deny" || value === "answer"
}

function isReceiptRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
