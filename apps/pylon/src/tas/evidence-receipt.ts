import { Schema as S } from "effect"

export const EvidenceReceiptKind = S.Literals([
  "schedule",
  "task",
  "decision",
  "notification",
  "review",
  "smoke",
])
export type EvidenceReceiptKind = typeof EvidenceReceiptKind.Type

export const EvidenceReceiptStatus = S.Literals([
  "produced",
  "accepted",
  "rejected",
])
export type EvidenceReceiptStatus = typeof EvidenceReceiptStatus.Type

export const EvidenceReceipt = S.Struct({
  receiptKind: EvidenceReceiptKind,
  subjectRef: S.String,
  digestRef: S.String,
  producedAt: S.String,
  status: EvidenceReceiptStatus,
})
export type EvidenceReceipt = typeof EvidenceReceipt.Type

export type EvidenceReceiptInput = EvidenceReceipt & Record<string, unknown>

const EVIDENCE_RECEIPT_KEYS = [
  "receiptKind",
  "subjectRef",
  "digestRef",
  "producedAt",
  "status",
] as const

const EVIDENCE_RECEIPT_KEY_SET = new Set<string>(EVIDENCE_RECEIPT_KEYS)
const DIGEST_REF_PATTERN = /^sha256:[a-f0-9]{64}$/i

function assertEvidenceReceipt(receipt: EvidenceReceipt): EvidenceReceipt {
  const keys = Object.keys(receipt)
  const unknownKeys = keys.filter((key) => !EVIDENCE_RECEIPT_KEY_SET.has(key))
  if (unknownKeys.length > 0) {
    throw new Error(`evidence receipt includes non-ref fields: ${unknownKeys.join(", ")}`)
  }
  if (!receipt.subjectRef.trim()) {
    throw new Error("evidence receipt subjectRef is required")
  }
  if (!DIGEST_REF_PATTERN.test(receipt.digestRef)) {
    throw new Error("evidence receipt digestRef must be a sha256: digest ref")
  }
  if (Number.isNaN(Date.parse(receipt.producedAt))) {
    throw new Error("evidence receipt producedAt must be an ISO-compatible timestamp")
  }
  return receipt
}

export function decodeEvidenceReceipt(input: unknown): EvidenceReceipt {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new Error("evidence receipt must be an object")
  }
  const unknownKeys = Object.keys(input).filter((key) => !EVIDENCE_RECEIPT_KEY_SET.has(key))
  if (unknownKeys.length > 0) {
    throw new Error(`evidence receipt includes non-ref fields: ${unknownKeys.join(", ")}`)
  }
  return assertEvidenceReceipt(S.decodeUnknownSync(EvidenceReceipt)(input))
}

export function buildEvidenceReceipt(input: EvidenceReceiptInput): EvidenceReceipt {
  return assertEvidenceReceipt({
    receiptKind: input.receiptKind,
    subjectRef: input.subjectRef,
    digestRef: input.digestRef,
    producedAt: input.producedAt,
    status: input.status,
  })
}
