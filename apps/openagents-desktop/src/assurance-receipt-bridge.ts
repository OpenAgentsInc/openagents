import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { resolve } from "node:path"

import { canonicalArtifact, decodeAssuranceReceipt, sha256Digest, type AssuranceReceipt } from "@openagentsinc/assurance-spec"

import type { ProductSpecIdentity, ProductSpecRun } from "./product-spec-workroom-contract.ts"
import type { ProductSpecWorkroom } from "./product-spec-workroom.ts"

export const ASSURANCE_RECEIPT_BRIDGE_VERSION = "openagents.assurance_receipt_bridge.v1" as const

export type AssuranceReceiptBridgeErrorCode =
  | "invalid_receipt"
  | "receipt_digest_mismatch"
  | "binding_mismatch"
  | "receipt_not_qualifying"
  | "reviewer_not_independent"
  | "reviewer_not_authorized"
  | "immutable_resolver_conflict"
  | "workroom_rejected"

export type AssuranceReceiptBridgeResult =
  | Readonly<{ ok: true; handle: string; resolverPath: string; evidenceReceiptRef: string; run: ProductSpecRun }>
  | Readonly<{ ok: false; code: AssuranceReceiptBridgeErrorCode; message: string }>

const failure = (code: AssuranceReceiptBridgeErrorCode, message: string): AssuranceReceiptBridgeResult =>
  ({ ok: false, code, message })

const exactArray = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean =>
  left.length === right.length && left.every((value, index) => value === right[index])

const decodeReceipt = (bytes: string): AssuranceReceipt | null => {
  try {
    return decodeAssuranceReceipt(JSON.parse(bytes))
  } catch {
    return null
  }
}

export const bridgeAssuranceReceipt = (input: Readonly<{
  workroom: ProductSpecWorkroom
  resolverRoot: string
  receiptBytes: string
  expectedReceiptDigest: string
  expectedManifestDigest: string
  expectedAssuranceSpecDigest: string
  expectedAdmissionDigest: string
  expectedObligationId: string
  expectedSpec: ProductSpecIdentity
  expectedCriterionIds: ReadonlyArray<string>
  authorizedReviewerRefs: ReadonlyArray<string>
  runRef: string
  packetRef: string
  leaseRef: string
  verifierRef: string
}>): AssuranceReceiptBridgeResult => {
  const receipt = decodeReceipt(input.receiptBytes)
  if (receipt === null) return failure("invalid_receipt", "Assurance Receipt is not schema-valid.")
  const actualDigest = sha256Digest(input.receiptBytes)
  if (actualDigest !== input.expectedReceiptDigest) {
    return failure("receipt_digest_mismatch", "Assurance Receipt bytes do not match the expected immutable digest.")
  }
  if (
    receipt.manifest_digest !== input.expectedManifestDigest ||
    receipt.assurance_spec_digest !== input.expectedAssuranceSpecDigest ||
    receipt.admission_digest !== input.expectedAdmissionDigest ||
    receipt.product_spec_digest !== input.expectedSpec.digest ||
    receipt.obligation_id !== input.expectedObligationId ||
    !exactArray(receipt.criterion_refs, input.expectedCriterionIds)
  ) return failure("binding_mismatch", "Receipt does not bind the exact ProductSpec/AssuranceSpec/Manifest/obligation chain.")

  if (receipt.producer_ref === receipt.reviewer_ref || receipt.producer_ref === input.verifierRef) {
    return failure("reviewer_not_independent", "Assurance producer, reviewer, and host verifier must be independent.")
  }
  if (receipt.reviewer_ref !== input.verifierRef || !input.authorizedReviewerRefs.includes(receipt.reviewer_ref)) {
    return failure("reviewer_not_authorized", "Receipt reviewer is not authorized by the bridge policy.")
  }
  const axes = receipt.axes
  if (
    axes.admission !== "admitted" || axes.readiness !== "executable" ||
    axes.observation !== "CONFIRMED" || axes.infrastructure !== "ready" ||
    axes.stability !== "stable" || axes.freshness !== "current" ||
    axes.disposition !== "accepted" || axes.exception !== "none"
  ) return failure("receipt_not_qualifying", "Non-confirming, stale, flaky, unreviewed, or excepted receipts cannot enter host verification.")

  const handle = `assurance.receipt.${actualDigest.slice("sha256:".length)}`
  const resolverRoot = resolve(input.resolverRoot)
  const resolverPath = resolve(resolverRoot, `${handle}.json`)
  mkdirSync(resolverRoot, { recursive: true })
  if (existsSync(resolverPath)) {
    if (readFileSync(resolverPath, "utf8") !== input.receiptBytes) {
      return failure("immutable_resolver_conflict", "Immutable receipt handle already resolves to different bytes.")
    }
  } else {
    writeFileSync(resolverPath, input.receiptBytes, { encoding: "utf8", flag: "wx", mode: 0o600 })
  }

  const evidenced = input.workroom.recordEvidence({
    runRef: input.runRef,
    packetRef: input.packetRef,
    leaseRef: input.leaseRef,
    evidenceRef: handle,
    evidenceKind: "receipt",
    expectedSpec: input.expectedSpec,
  })
  if (!evidenced.ok) return failure("workroom_rejected", evidenced.message)
  const packet = evidenced.value.plan.packets.find((candidate) => candidate.packetRef === input.packetRef)
  const evidenceReceiptRef = packet?.evidenceReceipts.find((candidate) => candidate.evidenceRef === handle)?.receiptRef
  if (evidenceReceiptRef === undefined) return failure("workroom_rejected", "Workroom did not retain the exact Assurance receipt handle.")
  const outputRef = `assurance.review.${sha256Digest(JSON.stringify({ handle, verifier: input.verifierRef })).slice("sha256:".length)}`
  const verified = input.workroom.verifyEvidence({
    runRef: input.runRef,
    packetRef: input.packetRef,
    verifierRef: input.verifierRef,
    outputRef,
    evidenceReceiptRefs: [evidenceReceiptRef],
    expectedSpec: input.expectedSpec,
  })
  if (!verified.ok) return failure("workroom_rejected", verified.message)
  return { ok: true, handle, resolverPath, evidenceReceiptRef, run: verified.value }
}

export const assuranceReceiptBytes = (receipt: AssuranceReceipt): string => canonicalArtifact(receipt).bytes
