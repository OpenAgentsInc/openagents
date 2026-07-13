import { Schema as S } from "effect"

export const ASSURANCE_SPEC_FORMAT_VERSION = "0.1" as const
export const ASSURANCE_SPEC_EXTENSION = ".assurance-spec.md" as const

export const MANDATORY_ASSURANCE_SECTION_IDS = [
  "assurance_objective",
  "subject",
  "risk_model",
  "assurance_scope",
  "environments",
  "obligations",
  "gates",
  "evidence_policy",
  "authority_boundaries",
] as const

export type MandatoryAssuranceSectionId = (typeof MANDATORY_ASSURANCE_SECTION_IDS)[number]

export const ASSURANCE_SECTION_LABELS: Record<MandatoryAssuranceSectionId, string> = {
  assurance_objective: "Assurance Objective",
  subject: "Subject",
  risk_model: "Risk Model",
  assurance_scope: "Assurance Scope",
  environments: "Environments",
  obligations: "Obligations",
  gates: "Gates",
  evidence_policy: "Evidence Policy",
  authority_boundaries: "Authority Boundaries",
}

/**
 * Custom extension sections use the id itself as the `##` heading in the
 * bounded profile (`## custom-owner-gates`). Ids are `custom-<kebab-name>`
 * per ASSURANCE_SPEC.md §3.2 and round-trip byte-stable.
 */
export const CUSTOM_SECTION_ID_PATTERN = /^custom-[a-z0-9]+(?:-[a-z0-9]+)*$/

/**
 * Typed fenced-block names per mandatory section (ASSURANCE_SPEC.md §3.3).
 * The parser and the serializer share this single map so block-name parity is
 * structural, not a convention two files must remember to keep aligned.
 */
export const ASSURANCE_STRUCTURED_BLOCK_NAMES = {
  subject: "assurancespec-subject",
  risk_model: "assurancespec-risks",
  environments: "assurancespec-environments",
  obligations: "assurancespec-obligations",
  gates: "assurancespec-gates",
  evidence_policy: "assurancespec-evidence-policy",
  authority_boundaries: "assurancespec-authority",
} as const satisfies Partial<Record<MandatoryAssuranceSectionId, string>>

export const structuredBlockNameForSection = (
  id: MandatoryAssuranceSectionId,
): string | null =>
  (ASSURANCE_STRUCTURED_BLOCK_NAMES as Partial<Record<MandatoryAssuranceSectionId, string>>)[id] ?? null

// ---------------------------------------------------------------------------
// Stable diagnostic codes (codes are API, ASSURANCE_SPEC.md §12.2)
//
// Every code below is covered by at least one committed conformance fixture in
// packages/assurance-spec/conformance/, enforced by test. Removing a code or
// changing its meaning is a format/tool version change, never a refactor.
// Any change that can make a previously valid document invalid must bump
// ASSURANCE_SPEC_FORMAT_VERSION and freeze the previous corpus per version.
// ---------------------------------------------------------------------------

/** Format-plane errors thrown while parsing markdown into the document model. */
export const ASSURANCE_FORMAT_ERROR_CODES = [
  "missing_frontmatter",
  "invalid_frontmatter",
  "duplicate_frontmatter_key",
  "unsupported_version",
  "unsupported_section",
  "invalid_custom_section_id",
  "duplicate_section",
  "missing_required_section",
  "invalid_section_order",
  "missing_structured_block",
  "duplicate_structured_block",
  "invalid_structured_block",
] as const

/** Referential-integrity errors computed at parse time over the parsed model. */
export const ASSURANCE_REFERENTIAL_ERROR_CODES = [
  "duplicate_subject_criterion_ref",
  "duplicate_obligation_id",
  "missing_obligation_criterion_ref",
  "dangling_source_ref",
  "dangling_environment_ref",
  "dangling_gate_ref",
  "uncovered_acceptance_criterion",
  "self_obligation_dependency",
  "dangling_dependency_ref",
  "cyclic_obligation_dependency",
] as const

export const ASSURANCE_STRUCTURAL_ERROR_CODES = [
  ...ASSURANCE_FORMAT_ERROR_CODES,
  ...ASSURANCE_REFERENTIAL_ERROR_CODES,
] as const

/**
 * Codes that cannot be produced from markdown bytes alone and therefore carry
 * no conformance fixture: `invalid_assurance_spec` is the typed wrapper for a
 * non-parse exception escaping the validator.
 */
export const ASSURANCE_NON_FIXTURE_ERROR_CODES = ["invalid_assurance_spec"] as const

/** Structural warnings: honesty about skeleton documents, never validity. */
export const ASSURANCE_STRUCTURAL_WARNING_CODES = [
  "empty_required_section",
  "thin_required_section",
] as const

/**
 * A mandatory section whose narrative (structured block excluded) has fewer
 * meaningful words than this is a skeleton and warns `thin_required_section`.
 * The deterministic proposal generator emits one boilerplate sentence per
 * section, and a generated-but-unreviewed proposal must warn, not pass silent.
 */
export const THIN_SECTION_WORD_COUNT = 25

export const NonEmptyString = S.String.check(S.isMinLength(1))
export const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThan(0))
export const Digest = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/))
export const StableRef = S.String.check(S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/))
const CriterionRef = S.String.check(S.isPattern(/^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+$/))
export const RelativePath = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(1_024),
  S.isPattern(/^(?!\/)(?!.*(?:^|\/)\.\.(?:\/|$)).+$/),
)

export const AssuranceSpecFrontmatterSchema = S.Struct({
  assurance_spec_format_version: S.Literal(ASSURANCE_SPEC_FORMAT_VERSION),
  assurance_spec_id: StableRef,
  assurance_revision: PositiveInteger,
  title: NonEmptyString,
  artifact_type: S.Literal("product_assurance"),
  lifecycle_state: S.Literal("proposed"),
  author: NonEmptyString,
})
export type AssuranceSpecFrontmatter = typeof AssuranceSpecFrontmatterSchema.Type

export const ProductSpecSubjectSchema = S.Struct({
  profile: S.Literal("openagents_executable_v0.1_exact_document"),
  path: RelativePath,
  spec_format_version: S.Literal("0.1"),
  spec_revision: PositiveInteger,
  document_digest: Digest,
  criterion_refs: S.Array(CriterionRef),
})
export type ProductSpecSubject = typeof ProductSpecSubjectSchema.Type

export const AssuranceSubjectSchema = S.Struct({
  product_spec: ProductSpecSubjectSchema,
})
export type AssuranceSubject = typeof AssuranceSubjectSchema.Type

export const RepositoryDeclaredScriptSchema = S.Struct({
  manifest_path: RelativePath,
  name: NonEmptyString,
  command: NonEmptyString,
})
export type RepositoryDeclaredScript = typeof RepositoryDeclaredScriptSchema.Type

export const RepositoryInventorySchema = S.Struct({
  state: S.Literals(["absent", "clean", "dirty", "not_git", "unavailable"]),
  repository_label: NonEmptyString,
  head: S.optionalKey(NonEmptyString),
  tree: S.optionalKey(NonEmptyString),
  tracked_file_count: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  candidate_artifact_refs: S.Array(RelativePath),
  declared_scripts: S.Array(RepositoryDeclaredScriptSchema),
  inventory_digest: Digest,
  truncated: S.Boolean,
  diagnostics: S.Array(StableRef),
})
export type RepositoryInventory = typeof RepositoryInventorySchema.Type

export const AssuranceEnvironmentProfileSchema = S.Struct({
  id: StableRef,
  status: S.Literal("proposed"),
})
export type AssuranceEnvironmentProfile = typeof AssuranceEnvironmentProfileSchema.Type

export const AssuranceEnvironmentBlockSchema = S.Struct({
  profiles: S.Array(AssuranceEnvironmentProfileSchema),
  repository_inventory: RepositoryInventorySchema,
})
export type AssuranceEnvironmentBlock = typeof AssuranceEnvironmentBlockSchema.Type

export const AssuranceRiskBlockSchema = S.Struct({
  source_snapshot: NonEmptyString,
  source_digest: Digest,
  risks: S.Array(S.Struct({ id: StableRef, statement: NonEmptyString })),
})
export type AssuranceRiskBlock = typeof AssuranceRiskBlockSchema.Type

export const AssuranceEvaluatorSchema = S.Struct({
  statement: NonEmptyString,
  evaluator_ref: RelativePath,
})
export type AssuranceEvaluator = typeof AssuranceEvaluatorSchema.Type

export const AssuranceFalsifierSchema = S.Struct({
  kind: StableRef,
  ref: RelativePath,
  expected_verdict: S.Literal("REFUTED"),
})
export type AssuranceFalsifier = typeof AssuranceFalsifierSchema.Type

export const AssuranceEvidenceRequirementSchema = S.Struct({
  required_kinds: S.Array(StableRef),
  proof_rung: StableRef,
})
export type AssuranceEvidenceRequirement = typeof AssuranceEvidenceRequirementSchema.Type

export const AssuranceIndependenceSchema = S.Struct({
  producer_may_verify: S.Boolean,
})
export type AssuranceIndependence = typeof AssuranceIndependenceSchema.Type

export const AssuranceObligationSchema = S.Struct({
  id: StableRef,
  title: NonEmptyString,
  criterion_refs: S.Array(CriterionRef),
  source_claim_snapshot: NonEmptyString,
  source_claim_digest: Digest,
  disposition: S.Literal("required"),
  candidate_artifact_refs: S.Array(RelativePath),
  domains: S.optionalKey(S.Array(StableRef)),
  technique: S.optionalKey(StableRef),
  environment_refs: S.optionalKey(S.Array(StableRef)),
  oracle: S.optionalKey(AssuranceEvaluatorSchema),
  falsifier: S.optionalKey(AssuranceFalsifierSchema),
  evidence: S.optionalKey(AssuranceEvidenceRequirementSchema),
  independence: S.optionalKey(AssuranceIndependenceSchema),
  dependency_refs: S.optionalKey(S.Array(StableRef)),
  activation_gate: S.optionalKey(StableRef),
})
export type AssuranceObligation = typeof AssuranceObligationSchema.Type

export const AssuranceGateSchema = S.Struct({
  id: StableRef,
  expression: NonEmptyString,
})
export type AssuranceGate = typeof AssuranceGateSchema.Type

export const AssuranceEvidencePolicySchema = S.Struct({
  links_are_verdicts: S.Literal(false),
  missing_evidence_verdict: S.Literal("INCONCLUSIVE"),
  required_for_ready_obligation: S.Array(StableRef),
  policy_state: S.Literals(["needs_design", "designed"]),
})
export type AssuranceEvidencePolicy = typeof AssuranceEvidencePolicySchema.Type

export const AssuranceAuthoritySchema = S.Struct({
  proposal_may_self_admit: S.Literal(false),
  proposal_may_execute: S.Literal(false),
  proposal_may_verify: S.Literal(false),
  proposal_may_release: S.Literal(false),
  proposal_may_change_public_promises: S.Literal(false),
  admitted_roles: S.Array(StableRef),
  verifier_roles: S.Array(StableRef),
  release_roles: S.Array(StableRef),
  policy_state: S.Literals(["needs_design", "designed"]),
})
export type AssuranceAuthority = typeof AssuranceAuthoritySchema.Type

export type AssuranceSpecSection = Readonly<{
  id: MandatoryAssuranceSectionId
  label: string
  content: string
}>

/**
 * An unknown `custom-<kebab-name>` extension section, preserved verbatim
 * (id-as-heading) instead of rejected. Custom sections carry no portable
 * semantics; tools must round-trip them without interpreting them.
 */
export type AssuranceCustomSection = Readonly<{
  id: string
  content: string
}>

/**
 * A frontmatter key outside the bounded profile, preserved in authored order
 * with its raw value text so unknown-but-valid metadata survives round trips.
 */
export type AssuranceUnknownFrontmatterEntry = Readonly<{
  key: string
  raw: string
}>

export type AssuranceSpecDocument = Readonly<{
  frontmatter: AssuranceSpecFrontmatter
  unknownFrontmatter: ReadonlyArray<AssuranceUnknownFrontmatterEntry>
  sections: ReadonlyArray<AssuranceSpecSection>
  customSections: ReadonlyArray<AssuranceCustomSection>
  subject: AssuranceSubject
  riskModel: AssuranceRiskBlock
  environments: AssuranceEnvironmentBlock
  obligations: ReadonlyArray<AssuranceObligation>
  gates: ReadonlyArray<AssuranceGate>
  evidencePolicy: AssuranceEvidencePolicy
  authority: AssuranceAuthority
}>

export type AssuranceDiagnostic = Readonly<{
  code: string
  message: string
  severity: "error" | "warning" | "info"
  path?: string
  obligation_id?: string
  missing_fields?: ReadonlyArray<string>
}>
