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
export const ORACLE_SENSITIVITY_RECEIPT_KIND = "oracle_sensitivity_receipt.v1" as const
export const OracleSensitivityMutantResultSchema = S.Struct({
  mutant_ref: StableRef,
  operator: S.Literal("replace_exact"),
  observation: S.Literals(["killed", "survived", "inconclusive"]),
  assurance_receipt_ref: S.optionalKey(StableRef),
})
export const OracleSensitivityReceiptSchema = S.Struct({
  receipt_kind: S.Literal(ORACLE_SENSITIVITY_RECEIPT_KIND),
  oracle_sensitivity_receipt_format_version: S.Literal(ORACLE_SENSITIVITY_RECEIPT_FORMAT_VERSION),
  receipt_ref: StableRef,
  manifest_digest: Digest,
  admission_digest: Digest,
  obligation_id: StableRef,
  candidate_receipt_ref: StableRef,
  adapter_ref: S.Literal("openagents.mutation.v1"),
  oracle_ref: NonEmptyString,
  mutation_set_digest: Digest,
  candidate_observation: S.Literal("CONFIRMED"),
  sensitivity_observation: S.Literals(["CONFIRMED", "REFUTED", "INCONCLUSIVE"]),
  mutant_results: S.Array(OracleSensitivityMutantResultSchema),
  killed_mutant_refs: S.Array(StableRef),
  surviving_mutant_refs: S.Array(StableRef),
  inconclusive_mutant_refs: S.Array(StableRef),
  diagnostic_refs: S.Array(StableRef),
  authority: S.Literal("evidence_only"),
})
export type OracleSensitivityReceipt = typeof OracleSensitivityReceiptSchema.Type
export type OracleSensitivityMutantResult = typeof OracleSensitivityMutantResultSchema.Type
export const decodeOracleSensitivityReceipt = S.decodeUnknownSync(OracleSensitivityReceiptSchema)

type MutationSensitivityInput = Readonly<{
  oracleRef: string
  mutationSetDigest: string
  mutantResults: ReadonlyArray<OracleSensitivityMutantResult>
}>

type LegacySensitivityInput = Readonly<{
  oracleRef: string
  falsifierRef: string
  survivingMutantRefs?: ReadonlyArray<string>
}>

export function makeOracleSensitivityReceipt(
  candidate: AssuranceReceipt,
  input: MutationSensitivityInput,
): OracleSensitivityReceipt
export function makeOracleSensitivityReceipt(
  candidate: AssuranceReceipt,
  falsifier: AssuranceReceipt,
  input: LegacySensitivityInput,
): OracleSensitivityReceipt
export function makeOracleSensitivityReceipt(
  candidate: AssuranceReceipt,
  inputOrFalsifier: MutationSensitivityInput | AssuranceReceipt,
  legacyInput?: LegacySensitivityInput,
): OracleSensitivityReceipt {
  if (candidate.axes.observation !== "CONFIRMED") {
    throw new Error("sensitivity_observation_mismatch")
  }
  const input: MutationSensitivityInput = legacyInput === undefined
    ? inputOrFalsifier as MutationSensitivityInput
    : {
      oracleRef: legacyInput.oracleRef,
      mutationSetDigest: sha256Digest(JSON.stringify({
        oracle_ref: legacyInput.oracleRef,
        falsifier_ref: legacyInput.falsifierRef,
        surviving_mutant_refs: legacyInput.survivingMutantRefs ?? [],
      })),
      mutantResults: [
        {
          mutant_ref: /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(legacyInput.falsifierRef)
            ? legacyInput.falsifierRef
            : `falsifier.${sha256Digest(legacyInput.falsifierRef).slice("sha256:".length)}`,
          operator: "replace_exact",
          observation: "killed",
          assurance_receipt_ref: (inputOrFalsifier as AssuranceReceipt).receipt_ref,
        },
        ...(legacyInput.survivingMutantRefs ?? []).map((mutantRef) => ({
          mutant_ref: mutantRef,
          operator: "replace_exact" as const,
          observation: "survived" as const,
        })),
      ],
    }
  if (
    legacyInput !== undefined &&
    (inputOrFalsifier as AssuranceReceipt).axes.observation !== "REFUTED"
  ) {
    throw new Error("sensitivity_observation_mismatch")
  }
  const results = [...input.mutantResults].sort((left, right) =>
    left.mutant_ref < right.mutant_ref ? -1 : left.mutant_ref > right.mutant_ref ? 1 : 0)
  if (results.length === 0 || new Set(results.map((result) => result.mutant_ref)).size !== results.length) {
    throw new Error("sensitivity_mutant_set_invalid")
  }
  const killed = results.filter((result) => result.observation === "killed").map((result) => result.mutant_ref)
  const surviving = results.filter((result) => result.observation === "survived").map((result) => result.mutant_ref)
  const inconclusive = results.filter((result) => result.observation === "inconclusive").map((result) => result.mutant_ref)
  const sensitivity = inconclusive.length > 0 ? ("INCONCLUSIVE" as const) : surviving.length > 0 ? ("REFUTED" as const) : ("CONFIRMED" as const)
  const seed = {
    manifest_digest: candidate.manifest_digest,
    admission_digest: candidate.admission_digest,
    obligation_id: candidate.obligation_id,
    candidate_receipt_ref: candidate.receipt_ref,
    adapter_ref: "openagents.mutation.v1" as const,
    oracle_ref: input.oracleRef,
    mutation_set_digest: input.mutationSetDigest,
    mutant_results: results,
  }
  return {
    receipt_kind: ORACLE_SENSITIVITY_RECEIPT_KIND,
    oracle_sensitivity_receipt_format_version: ORACLE_SENSITIVITY_RECEIPT_FORMAT_VERSION,
    receipt_ref: `oracle.sensitivity.${sha256Digest(JSON.stringify(seed)).slice("sha256:".length)}`,
    ...seed,
    candidate_observation: "CONFIRMED",
    sensitivity_observation: sensitivity,
    killed_mutant_refs: killed,
    surviving_mutant_refs: surviving,
    inconclusive_mutant_refs: inconclusive,
    diagnostic_refs: surviving.length === 0 ? [] : ["weak_oracle"],
    authority: "evidence_only",
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
