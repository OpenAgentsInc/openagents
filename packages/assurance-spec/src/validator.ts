import { AssuranceSpecParseError, parseAssuranceSpec } from "./parser.ts"
import type {
  AssuranceDiagnostic,
  AssuranceObligation,
  AssuranceSpecDocument,
} from "./schema.ts"

export type AssuranceStructuralValidation = Readonly<{
  valid: boolean
  document?: AssuranceSpecDocument
  errors: ReadonlyArray<AssuranceDiagnostic>
  warnings: ReadonlyArray<AssuranceDiagnostic>
}>

const designFields = [
  "domains",
  "technique",
  "environment_refs",
  "oracle",
  "falsifier",
  "evidence",
  "independence",
  "activation_gate",
] as const

export const missingObligationDesignFields = (
  obligation: AssuranceObligation,
): ReadonlyArray<(typeof designFields)[number]> => designFields.filter((field) => {
  const value = obligation[field]
  return value === undefined || (Array.isArray(value) && value.length === 0)
})

const structuralDiagnostics = (document: AssuranceSpecDocument): ReadonlyArray<AssuranceDiagnostic> => {
  const errors: AssuranceDiagnostic[] = []
  const criterionRefs = document.subject.product_spec.criterion_refs
  const criterionSet = new Set(criterionRefs)
  if (criterionSet.size !== criterionRefs.length) {
    errors.push({ code: "duplicate_subject_criterion_ref", message: "Subject criterion_refs must be unique.", severity: "error", path: "subject.product_spec.criterion_refs" })
  }

  const obligationIds = new Set<string>()
  const coveredCriteria = new Set<string>()
  const environmentIds = new Set(document.environments.profiles.map((profile) => profile.id))
  const gateIds = new Set(document.gates.map((gate) => gate.id))
  for (const obligation of document.obligations) {
    if (obligationIds.has(obligation.id)) {
      errors.push({ code: "duplicate_obligation_id", message: `Duplicate obligation ID: ${obligation.id}`, severity: "error", path: "obligations", obligation_id: obligation.id })
    }
    obligationIds.add(obligation.id)
    if (obligation.criterion_refs.length === 0) {
      errors.push({ code: "missing_obligation_criterion_ref", message: `Obligation ${obligation.id} has no criterion ref.`, severity: "error", path: `obligations.${obligation.id}`, obligation_id: obligation.id })
    }
    for (const criterionRef of obligation.criterion_refs) {
      if (!criterionSet.has(criterionRef)) {
        errors.push({ code: "dangling_source_ref", message: `Obligation ${obligation.id} references unknown criterion ${criterionRef}.`, severity: "error", path: `obligations.${obligation.id}.criterion_refs`, obligation_id: obligation.id })
      } else {
        coveredCriteria.add(criterionRef)
      }
    }
    for (const environmentRef of obligation.environment_refs ?? []) {
      if (!environmentIds.has(environmentRef)) {
        errors.push({ code: "dangling_environment_ref", message: `Obligation ${obligation.id} references unknown environment ${environmentRef}.`, severity: "error", path: `obligations.${obligation.id}.environment_refs`, obligation_id: obligation.id })
      }
    }
    if (obligation.activation_gate !== undefined && !gateIds.has(obligation.activation_gate)) {
      errors.push({ code: "dangling_gate_ref", message: `Obligation ${obligation.id} references unknown gate ${obligation.activation_gate}.`, severity: "error", path: `obligations.${obligation.id}.activation_gate`, obligation_id: obligation.id })
    }
  }
  for (const criterionRef of criterionRefs) {
    if (!coveredCriteria.has(criterionRef)) {
      errors.push({ code: "uncovered_acceptance_criterion", message: `No obligation references ${criterionRef}.`, severity: "error", path: "obligations" })
    }
  }
  return errors
}

export const validateAssuranceSpec = (markdown: string): AssuranceStructuralValidation => {
  try {
    const document = parseAssuranceSpec(markdown)
    const errors = structuralDiagnostics(document)
    return errors.length === 0
      ? { valid: true, document, errors: [], warnings: [] }
      : { valid: false, document, errors, warnings: [] }
  } catch (error) {
    const diagnostic = error instanceof AssuranceSpecParseError
      ? error.diagnostic
      : { code: "invalid_assurance_spec", message: error instanceof Error ? error.message : String(error), severity: "error" as const }
    return { valid: false, errors: [diagnostic], warnings: [] }
  }
}

export type AssuranceAdequacyAssessment = Readonly<{
  design_ready: boolean
  diagnostics: ReadonlyArray<AssuranceDiagnostic>
  coverage: Readonly<{
    criteria: number
    obligations: number
    ready: number
    needs_design: number
  }>
}>

export const assessAssuranceSpec = (document: AssuranceSpecDocument): AssuranceAdequacyAssessment => {
  const diagnostics: AssuranceDiagnostic[] = []
  let ready = 0
  for (const obligation of document.obligations) {
    const missingFields = missingObligationDesignFields(obligation)
    if (missingFields.length === 0) {
      ready += 1
    } else {
      diagnostics.push({
        code: "obligation_needs_design",
        message: `Obligation ${obligation.id} is structurally valid but missing admitted proof design.`,
        severity: "warning",
        path: `obligations.${obligation.id}`,
        obligation_id: obligation.id,
        missing_fields: missingFields,
      })
    }
  }
  if (document.riskModel.risks.length === 0) diagnostics.push({ code: "risk_model_needs_design", message: "No structured assurance risks are defined.", severity: "warning", path: "risk_model" })
  if (document.environments.profiles.length === 0) diagnostics.push({ code: "environment_profiles_need_design", message: "No Environment Profiles are defined.", severity: "warning", path: "environments" })
  if (document.gates.length === 0) diagnostics.push({ code: "gates_need_design", message: "No assurance activation or aggregate gates are defined.", severity: "warning", path: "gates" })
  if (document.evidencePolicy.policy_state === "needs_design") diagnostics.push({ code: "evidence_policy_needs_design", message: "Evidence sufficiency and freshness policy still need design.", severity: "warning", path: "evidence_policy" })
  if (document.authority.policy_state === "needs_design") diagnostics.push({ code: "authority_policy_needs_design", message: "Admission, verification, and release roles still need design.", severity: "warning", path: "authority_boundaries" })
  for (const code of document.environments.repository_inventory.diagnostics) {
    diagnostics.push({
      code,
      message: code === "repository_not_supplied"
        ? "No repository was supplied; the proposal covers ProductSpec criteria without repository candidates."
        : code === "repository_dirty"
          ? "The tracked worktree differs from the committed inventory snapshot; repository context is stale-prone."
          : code === "repository_candidates_unmapped"
            ? "Repository candidates were inventoried but not mapped to criteria or selected as proof."
            : `Repository inventory diagnostic: ${code}`,
      severity: code === "repository_dirty" ? "warning" : "info",
      path: "environments.repository_inventory",
    })
  }
  return {
    design_ready: ready === document.obligations.length && diagnostics.every((entry) => entry.severity !== "warning"),
    diagnostics,
    coverage: {
      criteria: document.subject.product_spec.criterion_refs.length,
      obligations: document.obligations.length,
      ready,
      needs_design: document.obligations.length - ready,
    },
  }
}
