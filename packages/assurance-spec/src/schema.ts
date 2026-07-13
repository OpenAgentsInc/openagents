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

const NonEmptyString = S.String.check(S.isMinLength(1))
const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThan(0))
const Digest = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/))
const StableRef = S.String.check(S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/))
const CriterionRef = S.String.check(S.isPattern(/^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+$/))
const RelativePath = S.String.check(
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

export type AssuranceSpecDocument = Readonly<{
  frontmatter: AssuranceSpecFrontmatter
  sections: ReadonlyArray<AssuranceSpecSection>
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
