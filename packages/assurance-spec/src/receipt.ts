import { Schema as S } from "effect"

import { canonicalArtifact } from "./artifact.ts"
import { Digest, NonEmptyString, RelativePath, StableRef } from "./schema.ts"
import { sha256Digest } from "./tooling.ts"

export const ASSURANCE_RECEIPT_FORMAT_VERSION = "0.1" as const

export const AssuranceReceiptSchema = S.Struct({
  assurance_receipt_format_version: S.Literal(ASSURANCE_RECEIPT_FORMAT_VERSION),
  receipt_ref: StableRef,
  manifest_digest: Digest,
  product_spec_digest: Digest,
  assurance_spec_digest: Digest,
  admission_digest: Digest,
  obligation_id: StableRef,
  criterion_refs: S.Array(StableRef),
  environment_ref: StableRef,
  adapter_ref: StableRef,
  execution_unit_ref: StableRef,
  producer_ref: StableRef,
  reviewer_ref: StableRef,
  native_report_ref: RelativePath,
  native_report_digest: Digest,
  command_digest: Digest,
  source_digest: Digest,
  axes: S.Struct({
    admission: S.Literals(["proposed", "admitted", "superseded", "retired"]),
    readiness: S.Literals(["needs_design", "planned_red", "blocked", "executable", "not_applicable"]),
    observation: S.Literals(["not_run", "CONFIRMED", "REFUTED", "INCONCLUSIVE"]),
    infrastructure: S.Literals(["ready", "unarmed", "unavailable", "failed"]),
    stability: S.Literals(["unknown", "stable", "flaky"]),
    freshness: S.Literals(["current", "stale"]),
    disposition: S.Literals(["pending_review", "accepted", "rejected", "exception"]),
    exception: S.Literals(["none", "scoped"]),
  }),
  public_safety: S.Struct({
    classification: S.Literals(["private", "reviewed_public_safe"]),
    contains_raw_output: S.Literal(false),
  }),
})
export type AssuranceReceipt = typeof AssuranceReceiptSchema.Type
export const decodeAssuranceReceipt = S.decodeUnknownSync(AssuranceReceiptSchema)

export const ORACLE_SENSITIVITY_RECEIPT_FORMAT_VERSION = "0.1" as const
export const OracleSensitivityReceiptSchema = S.Struct({
  oracle_sensitivity_receipt_format_version: S.Literal(ORACLE_SENSITIVITY_RECEIPT_FORMAT_VERSION),
  receipt_ref: StableRef,
  obligation_id: StableRef,
  candidate_receipt_ref: StableRef,
  falsifier_receipt_ref: StableRef,
  oracle_ref: NonEmptyString,
  falsifier_ref: NonEmptyString,
  candidate_observation: S.Literal("CONFIRMED"),
  falsifier_observation: S.Literal("REFUTED"),
  sensitivity_observation: S.Literal("CONFIRMED"),
  surviving_mutant_refs: S.Array(StableRef),
  diagnostic_refs: S.Array(StableRef),
})
export type OracleSensitivityReceipt = typeof OracleSensitivityReceiptSchema.Type

export const makeOracleSensitivityReceipt = (
  candidate: AssuranceReceipt,
  falsifier: AssuranceReceipt,
  input: Readonly<{ oracleRef: string; falsifierRef: string; survivingMutantRefs?: ReadonlyArray<string> }>,
): OracleSensitivityReceipt => {
  if (candidate.obligation_id !== falsifier.obligation_id) {
    throw new Error("sensitivity_obligation_mismatch")
  }
  if (candidate.axes.observation !== "CONFIRMED" || falsifier.axes.observation !== "REFUTED") {
    throw new Error("sensitivity_observation_mismatch")
  }
  const seed = {
    obligation_id: candidate.obligation_id,
    candidate_receipt_ref: candidate.receipt_ref,
    falsifier_receipt_ref: falsifier.receipt_ref,
    oracle_ref: input.oracleRef,
    falsifier_ref: input.falsifierRef,
  }
  const surviving = [...(input.survivingMutantRefs ?? [])]
  return {
    oracle_sensitivity_receipt_format_version: ORACLE_SENSITIVITY_RECEIPT_FORMAT_VERSION,
    receipt_ref: `oracle.sensitivity.${sha256Digest(JSON.stringify(seed)).slice("sha256:".length)}`,
    ...seed,
    candidate_observation: "CONFIRMED",
    falsifier_observation: "REFUTED",
    sensitivity_observation: "CONFIRMED",
    surviving_mutant_refs: surviving,
    diagnostic_refs: surviving.length === 0 ? [] : ["weak_oracle"],
  }
}

export const assuranceReceiptArtifact = (receipt: AssuranceReceipt) => canonicalArtifact(receipt)

export const ASSURANCE_EVIDENCE_INDEX_FORMAT_VERSION = "0.1" as const
const EvidenceArtifactPointerSchema = S.Struct({
  ref: StableRef,
  digest: Digest,
  path: RelativePath,
})

export const AssuranceEvidenceIndexSchema = S.Struct({
  assurance_evidence_index_format_version: S.Literal(ASSURANCE_EVIDENCE_INDEX_FORMAT_VERSION),
  subject: S.Struct({
    product_spec_digest: Digest,
    assurance_spec_digest: Digest,
    manifest_digest: Digest,
    admission_digest: Digest,
  }),
  gate: S.Struct({
    gate_ref: StableRef,
    admitted: S.Boolean,
    executable: S.Boolean,
    confirmed_obligations: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    total_obligations: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    infrastructure: S.Literals(["ready", "unavailable", "failed"]),
    stability: S.Literals(["stable", "flaky", "unknown"]),
    freshness: S.Literals(["current", "stale"]),
    disposition: S.Literals(["accepted", "pending_review", "rejected", "exception"]),
    exception: S.Literals(["none", "scoped"]),
    full_desktop_gate: S.Literals(["pending_external_run", "green", "failed"]),
  }),
  receipts: S.Array(S.Struct({
    obligation_id: StableRef,
    criterion_refs: S.Array(StableRef),
    candidate: EvidenceArtifactPointerSchema,
    falsifier: EvidenceArtifactPointerSchema,
    sensitivity: EvidenceArtifactPointerSchema,
    axes: AssuranceReceiptSchema.fields.axes,
  })),
  companion_evidence_refs: S.Array(RelativePath),
  public_safety: S.Struct({
    classification: S.Literal("reviewed_public_safe"),
    raw_artifacts_public: S.Literal(false),
  }),
})
export type AssuranceEvidenceIndex = typeof AssuranceEvidenceIndexSchema.Type
export const decodeAssuranceEvidenceIndex = S.decodeUnknownSync(AssuranceEvidenceIndexSchema)
