import { createHash } from "node:crypto"
import { posix } from "node:path"

import { validateExecutableProductSpec } from "@openagentsinc/product-spec"
import { Effect, Schema as S } from "effect"

import { absentRepositoryInventory } from "./repository-inventory.ts"
import {
  ASSURANCE_SECTION_LABELS,
  ASSURANCE_SPEC_FORMAT_VERSION,
  AssuranceEvaluatorSchema,
  AssuranceEvidenceRequirementSchema,
  AssuranceFalsifierSchema,
  AssuranceGateSchema,
  AssuranceIndependenceSchema,
  AssuranceSeamSchema,
  Digest,
  MANDATORY_ASSURANCE_SECTION_IDS,
  NonEmptyString,
  ProductSpecSubjectSchema,
  RelativePath,
  RepositoryInventorySchema,
  StableRef,
  type AssuranceDiagnostic,
  type AssuranceObligation,
  type AssuranceSpecDocument,
  type ProductSpecSubject,
  type RepositoryInventory,
} from "./schema.ts"
import { serializeAssuranceSpec } from "./serializer.ts"
import { assessAssuranceSpec, validateAssuranceSpec, type AssuranceAdequacyAssessment } from "./validator.ts"

export const SEMANTIC_PLANNER_INPUT_VERSION = "openagents.observer.semantic_planner_input.v1" as const
export const SEMANTIC_PLANNER_OUTPUT_VERSION = "openagents.observer.semantic_planner_output.v1" as const

const CriterionRef = S.String.check(S.isPattern(/^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+$/))

export const SemanticPlannerCriterionSchema = S.Struct({
  criterion_ref: CriterionRef,
  source_claim_snapshot: NonEmptyString,
  source_claim_digest: Digest,
})

export const SemanticPlannerInputSchema = S.Struct({
  format_version: S.Literal(SEMANTIC_PLANNER_INPUT_VERSION),
  subject: ProductSpecSubjectSchema,
  input_digest: Digest,
  criteria: S.Array(SemanticPlannerCriterionSchema),
  risk_source_snapshot: NonEmptyString,
  risk_source_digest: Digest,
  repository_inventory: RepositoryInventorySchema,
})
export type SemanticPlannerInput = typeof SemanticPlannerInputSchema.Type

const NeedsDesignDispositionSchema = S.Struct({
  state: S.Literal("needs_design"),
  criterion_ref: CriterionRef,
  reason: NonEmptyString,
})

const DesignedDispositionSchema = S.Struct({
  state: S.Literal("designed"),
  criterion_ref: CriterionRef,
  obligation_id: StableRef,
  title: NonEmptyString,
  candidate_artifact_refs: S.Array(RelativePath),
  domains: S.Array(StableRef).check(S.isMinLength(1)),
  technique: StableRef,
  environment_refs: S.Array(StableRef).check(S.isMinLength(1)),
  oracle: AssuranceEvaluatorSchema,
  falsifier: AssuranceFalsifierSchema,
  evidence: S.Struct({
    required_kinds: S.Array(StableRef).check(S.isMinLength(1)),
    proof_rung: StableRef,
  }),
  independence: AssuranceIndependenceSchema,
  seam: S.optionalKey(AssuranceSeamSchema),
  dependency_refs: S.Array(StableRef),
  activation_gate: StableRef,
})

export const SemanticPlannerDispositionSchema = S.Union([
  NeedsDesignDispositionSchema,
  DesignedDispositionSchema,
])
export type SemanticPlannerDisposition = typeof SemanticPlannerDispositionSchema.Type

export const SemanticPlannerOutputSchema = S.Struct({
  format_version: S.Literal(SEMANTIC_PLANNER_OUTPUT_VERSION),
  input_digest: Digest,
  subject: ProductSpecSubjectSchema,
  criterion_dispositions: S.Array(SemanticPlannerDispositionSchema),
  risks: S.Array(S.Struct({ id: StableRef, statement: NonEmptyString })),
  environments: S.Array(S.Struct({ id: StableRef, status: S.Literal("proposed") })),
  gates: S.Array(AssuranceGateSchema),
  evidence_policy: S.Struct({
    required_for_ready_obligation: S.Array(StableRef),
    policy_state: S.Literals(["needs_design", "designed"]),
  }),
  proposed_roles: S.Struct({
    admitted_roles: S.Array(StableRef),
    verifier_roles: S.Array(StableRef),
    release_roles: S.Array(StableRef),
    policy_state: S.Literals(["needs_design", "designed"]),
  }),
})
export type SemanticPlannerOutput = typeof SemanticPlannerOutputSchema.Type

export type SemanticPlanner = (
  input: SemanticPlannerInput,
) => Effect.Effect<unknown, unknown>

const sha256 = (value: string): string =>
  `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`

const canonicalJson = (value: unknown): string => `${JSON.stringify(value, null, 2)}\n`

const safeIdPart = (value: string): string => value
  .toLowerCase()
  .replace(/[^a-z0-9]+/g, ".")
  .replace(/^\.|\.$/g, "")
  .slice(0, 160) || "product"

const normalizedSubjectPath = (value: string): string | null => {
  const normalized = value.replaceAll("\\", "/")
  if (normalized.startsWith("/") || normalized.includes("\0")) return null
  const clean = posix.normalize(normalized)
  return clean === ".." || clean.startsWith("../") || !clean.endsWith(".product-spec.md")
    ? null
    : clean
}

const sameSubject = (left: ProductSpecSubject, right: ProductSpecSubject): boolean =>
  canonicalJson(left) === canonicalJson(right)

const diagnostic = (code: string, message: string, path?: string): AssuranceDiagnostic => ({
  code,
  message,
  severity: "error",
  ...(path === undefined ? {} : { path }),
})

export type PrepareSemanticPlannerInputOptions = Readonly<{
  acceptedSubject: ProductSpecSubject
  productSpecPath: string
  productSpecMarkdown: string
  repositoryInventory?: RepositoryInventory
}>

export type SemanticPlannerInputResult =
  | Readonly<{ ok: true; input: SemanticPlannerInput }>
  | Readonly<{ ok: false; diagnostics: ReadonlyArray<AssuranceDiagnostic> }>

/**
 * Constructs the provider-neutral planner request only after checking the
 * caller's explicit accepted-subject pin against the exact ProductSpec bytes.
 */
export const prepareSemanticPlannerInput = (
  options: PrepareSemanticPlannerInputOptions,
): SemanticPlannerInputResult => {
  let acceptedSubject: ProductSpecSubject
  let repositoryInventory: RepositoryInventory
  try {
    acceptedSubject = S.decodeUnknownSync(ProductSpecSubjectSchema)(options.acceptedSubject)
    repositoryInventory = options.repositoryInventory === undefined
      ? absentRepositoryInventory()
      : S.decodeUnknownSync(RepositoryInventorySchema)(options.repositoryInventory)
  } catch {
    return { ok: false, diagnostics: [diagnostic("invalid_semantic_planner_input", "Accepted subject or repository inventory does not conform to the typed planner input contract.")] }
  }
  const path = normalizedSubjectPath(options.productSpecPath)
  if (path === null) {
    return { ok: false, diagnostics: [diagnostic("invalid_subject_path", "Accepted ProductSpec path must be repository-relative and end in .product-spec.md.", "subject.path")] }
  }
  const source = validateExecutableProductSpec(options.productSpecMarkdown)
  if (!source.executable) {
    return {
      ok: false,
      diagnostics: [
        diagnostic("product_spec_not_executable", "Semantic planning requires an executable ProductSpec."),
        ...source.errors.map((error) => ({ ...error, severity: "error" as const })),
      ],
    }
  }
  const actualSubject: ProductSpecSubject = {
    profile: "openagents_executable_v0.1_exact_document",
    path,
    spec_format_version: "0.1",
    spec_revision: source.document.frontmatter.spec_revision!,
    document_digest: sha256(options.productSpecMarkdown),
    criterion_refs: source.criteria.map((criterion) => criterion.id),
  }
  if (!sameSubject(acceptedSubject, actualSubject)) {
    return { ok: false, diagnostics: [diagnostic("semantic_planner_subject_drift", "Accepted ProductSpec identity does not match the exact source path, revision, digest, and criterion set.", "subject")] }
  }
  const risks = source.document.sections.find((section) => section.id === "risks")?.content.trim()
    || "The source ProductSpec contains no Risks section. Assurance risk modeling remains required."
  const body = {
    format_version: SEMANTIC_PLANNER_INPUT_VERSION,
    subject: actualSubject,
    criteria: source.criteria.map((criterion) => ({
      criterion_ref: criterion.id,
      source_claim_snapshot: criterion.body,
      source_claim_digest: sha256(`${criterion.id}\n${criterion.body}`),
    })),
    risk_source_snapshot: risks,
    risk_source_digest: sha256(risks),
    repository_inventory: repositoryInventory,
  }
  const normalized = S.decodeUnknownSync(SemanticPlannerInputSchema)({
    ...body,
    input_digest: sha256("semantic-planner-input-normalization"),
  })
  const { input_digest: _normalizationDigest, ...normalizedBody } = normalized
  return {
    ok: true,
    input: S.decodeUnknownSync(SemanticPlannerInputSchema)({
      ...normalizedBody,
      input_digest: sha256(canonicalJson(normalizedBody)),
    }),
  }
}

export type SemanticPlannerProposalOptions = Readonly<{
  assuranceSpecId?: string
  assuranceRevision?: number
  title?: string
  author: string
}>

export type SemanticPlannerProposalResult =
  | Readonly<{ ok: false; diagnostics: ReadonlyArray<AssuranceDiagnostic> }>
  | Readonly<{
      ok: true
      plannerInput: SemanticPlannerInput
      plannerOutput: SemanticPlannerOutput
      document: AssuranceSpecDocument
      markdown: string
      structural: ReturnType<typeof validateAssuranceSpec>
      adequacy: AssuranceAdequacyAssessment
    }>

const decodePlannerOutput = (value: unknown): SemanticPlannerOutput | null => {
  try {
    return S.decodeUnknownSync(SemanticPlannerOutputSchema)(value)
  } catch {
    return null
  }
}

/** Deterministically compiles decoded planner output; it grants no authority. */
export const compileSemanticPlannerProposal = (
  input: SemanticPlannerInput,
  outputValue: unknown,
  options: SemanticPlannerProposalOptions,
): SemanticPlannerProposalResult => {
  let decodedInput: SemanticPlannerInput
  try {
    decodedInput = S.decodeUnknownSync(SemanticPlannerInputSchema)(input)
  } catch {
    return { ok: false, diagnostics: [diagnostic("invalid_semantic_planner_input", "Planner input does not conform to the typed semantic planner input contract.")] }
  }
  const { input_digest: _inputDigest, ...inputBody } = decodedInput
  if (decodedInput.input_digest !== sha256(canonicalJson(inputBody))) {
    return { ok: false, diagnostics: [diagnostic("semantic_planner_input_drift", "Planner input digest does not bind its exact typed request payload.")] }
  }
  const criterionRefs = decodedInput.criteria.map((criterion) => criterion.criterion_ref)
  if (
    canonicalJson(criterionRefs) !== canonicalJson(decodedInput.subject.criterion_refs)
    || new Set(criterionRefs).size !== criterionRefs.length
    || decodedInput.criteria.some((criterion) =>
      criterion.source_claim_digest !== sha256(`${criterion.criterion_ref}\n${criterion.source_claim_snapshot}`))
  ) {
    return { ok: false, diagnostics: [diagnostic("semantic_planner_input_drift", "Planner input criteria do not exactly match the subject criterion set and source snapshots.")] }
  }
  input = decodedInput
  const output = decodePlannerOutput(outputValue)
  if (output === null) {
    return { ok: false, diagnostics: [diagnostic("invalid_semantic_planner_output", "Planner output does not conform to the typed semantic planner output contract.")] }
  }
  if (output.input_digest !== input.input_digest || !sameSubject(output.subject, input.subject)) {
    return { ok: false, diagnostics: [diagnostic("semantic_planner_output_drift", "Planner output is not bound to this exact planner input and ProductSpec identity.")] }
  }
  const expected = new Set(input.criteria.map((criterion) => criterion.criterion_ref))
  const seen = new Set<string>()
  for (const [index, disposition] of output.criterion_dispositions.entries()) {
    if (!expected.has(disposition.criterion_ref)) {
      return { ok: false, diagnostics: [diagnostic("semantic_planner_stale_criterion", `Planner output references unknown criterion ${disposition.criterion_ref}.`, `criterion_dispositions.${index}.criterion_ref`)] }
    }
    if (seen.has(disposition.criterion_ref)) {
      return { ok: false, diagnostics: [diagnostic("semantic_planner_duplicate_disposition", `Planner output disposes ${disposition.criterion_ref} more than once.`, `criterion_dispositions.${index}.criterion_ref`)] }
    }
    seen.add(disposition.criterion_ref)
    if (disposition.state === "designed") {
      if (disposition.independence.producer_may_verify) {
        return { ok: false, diagnostics: [diagnostic("semantic_planner_weak_proof", `Designed disposition ${disposition.criterion_ref} permits self-verification.`)] }
      }
      if (disposition.domains.includes("seam") && (
        disposition.seam === undefined
        || disposition.seam.side_a_ref === disposition.seam.side_b_ref
        || disposition.seam.qualifying_evidence_refs.length === 0
      )) {
        return { ok: false, diagnostics: [diagnostic("semantic_planner_weak_proof", `Designed seam disposition ${disposition.criterion_ref} lacks two real sides and qualifying evidence.`)] }
      }
    }
  }
  const missing = [...expected].filter((criterionRef) => !seen.has(criterionRef))
  if (missing.length > 0) {
    return { ok: false, diagnostics: [diagnostic("semantic_planner_missing_disposition", `Planner output omits criterion dispositions: ${missing.join(", ")}.`)] }
  }

  const sourceByRef = new Map(input.criteria.map((criterion) => [criterion.criterion_ref, criterion]))
  const obligations: AssuranceObligation[] = output.criterion_dispositions.map((disposition) => {
    const source = sourceByRef.get(disposition.criterion_ref)!
    const base: AssuranceObligation = {
      id: disposition.state === "designed" ? disposition.obligation_id : `AO-${disposition.criterion_ref}-01`,
      title: disposition.state === "designed" ? disposition.title : `Assure ${disposition.criterion_ref}`,
      criterion_refs: [disposition.criterion_ref],
      source_claim_snapshot: source.source_claim_snapshot,
      source_claim_digest: source.source_claim_digest,
      disposition: "required",
      candidate_artifact_refs: disposition.state === "designed" ? disposition.candidate_artifact_refs : [],
    }
    return disposition.state === "needs_design" ? base : {
      ...base,
      domains: disposition.domains,
      technique: disposition.technique,
      environment_refs: disposition.environment_refs,
      oracle: disposition.oracle,
      falsifier: disposition.falsifier,
      evidence: disposition.evidence,
      independence: disposition.independence,
      ...(disposition.seam === undefined ? {} : { seam: disposition.seam }),
      dependency_refs: disposition.dependency_refs,
      activation_gate: disposition.activation_gate,
    }
  })
  const document: AssuranceSpecDocument = {
    frontmatter: {
      assurance_spec_format_version: ASSURANCE_SPEC_FORMAT_VERSION,
      assurance_spec_id: options.assuranceSpecId ?? `assurance.${safeIdPart(input.subject.path.replace(/\.product-spec\.md$/, ""))}`,
      assurance_revision: options.assuranceRevision ?? 1,
      title: options.title ?? `${input.subject.path} Assurance Spec`,
      artifact_type: "product_assurance",
      lifecycle_state: "proposed",
      author: options.author,
    },
    unknownFrontmatter: [],
    sections: MANDATORY_ASSURANCE_SECTION_IDS.map((id) => ({ id, label: ASSURANCE_SECTION_LABELS[id], content: "" })),
    customSections: [],
    subject: { product_spec: input.subject },
    riskModel: {
      source_snapshot: input.risk_source_snapshot,
      source_digest: input.risk_source_digest,
      risks: output.risks,
    },
    environments: { profiles: output.environments, repository_inventory: input.repository_inventory },
    obligations,
    gates: output.gates,
    evidencePolicy: {
      links_are_verdicts: false,
      missing_evidence_verdict: "INCONCLUSIVE",
      required_for_ready_obligation: output.evidence_policy.required_for_ready_obligation,
      policy_state: output.evidence_policy.policy_state,
    },
    authority: {
      proposal_may_self_admit: false,
      proposal_may_execute: false,
      proposal_may_verify: false,
      proposal_may_release: false,
      proposal_may_change_public_promises: false,
      ...output.proposed_roles,
    },
  }
  const markdown = serializeAssuranceSpec(document)
  const structural = validateAssuranceSpec(markdown)
  if (!structural.valid || structural.document === undefined) {
    return { ok: false, diagnostics: structural.errors }
  }
  return {
    ok: true,
    plannerInput: input,
    plannerOutput: output,
    document: structural.document,
    markdown,
    structural,
    adequacy: assessAssuranceSpec(structural.document),
  }
}

export const runSemanticPlannerProposal = (
  input: SemanticPlannerInput,
  planner: SemanticPlanner,
  options: SemanticPlannerProposalOptions,
): Effect.Effect<SemanticPlannerProposalResult> =>
  Effect.match(planner(input), {
    onFailure: () => ({ ok: false as const, diagnostics: [diagnostic("semantic_planner_unavailable", "Injected semantic planner failed before returning a proposal.")] }),
    onSuccess: (output) => compileSemanticPlannerProposal(input, output, options),
  })

/** Deterministic provider-free planner used by CLI smoke tests and agents. */
export const fixtureSemanticPlanner: SemanticPlanner = (input) => Effect.succeed({
  format_version: SEMANTIC_PLANNER_OUTPUT_VERSION,
  input_digest: input.input_digest,
  subject: input.subject,
  criterion_dispositions: input.criteria.map((criterion) => ({
    state: "needs_design",
    criterion_ref: criterion.criterion_ref,
    reason: "Deterministic fixture planner preserves this criterion as explicit needs-design.",
  })),
  risks: [],
  environments: [],
  gates: [],
  evidence_policy: { required_for_ready_obligation: [], policy_state: "needs_design" },
  proposed_roles: { admitted_roles: [], verifier_roles: [], release_roles: [], policy_state: "needs_design" },
})
