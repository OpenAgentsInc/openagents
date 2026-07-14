import { AssuranceSpecParseError, parseAssuranceSpecDocument } from "./parser.ts"
import {
  MANDATORY_ASSURANCE_SECTION_IDS,
  THIN_SECTION_WORD_COUNT,
  structuredBlockNameForSection,
  type AssuranceDiagnostic,
  type AssuranceObligation,
  type AssuranceSpecDocument,
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

/**
 * Portable false-green guards. These run for every consumer of the generic
 * validator; adapter-specific validation must not be the only place where a
 * self-verifying or label-only seam design is rejected.
 */
export const obligationFalseGreenDiagnostics = (
  obligation: AssuranceObligation,
): ReadonlyArray<AssuranceDiagnostic> => {
  const diagnostics: AssuranceDiagnostic[] = []
  if (obligation.independence?.producer_may_verify === true) {
    diagnostics.push({
      code: "false_green_api_mirror",
      message: `Obligation ${obligation.id} lets its producer verify the same claim.`,
      severity: "error",
      path: `obligations.${obligation.id}.independence.producer_may_verify`,
      obligation_id: obligation.id,
    })
  }
  if (obligation.domains?.includes("seam") === true) {
    const seam = obligation.seam
    if (
      seam === undefined ||
      seam.side_a_ref === seam.side_b_ref ||
      seam.qualifying_evidence_refs.length === 0
    ) {
      diagnostics.push({
        code: "false_green_mocked_seam",
        message: `Seam obligation ${obligation.id} must name two distinct real sides and at least one qualifying evidence ref.`,
        severity: "error",
        path: `obligations.${obligation.id}.seam`,
        obligation_id: obligation.id,
      })
    }
  }
  return diagnostics
}

export const falseGreenDiagnostics = (
  document: AssuranceSpecDocument,
): ReadonlyArray<AssuranceDiagnostic> => document.obligations.flatMap(obligationFalseGreenDiagnostics)

/**
 * Skeleton-document honesty (GAP_ANALYSIS §2): a mandatory section whose
 * narrative — the prose outside its typed structured block — is empty or a
 * single boilerplate sentence warns without affecting validity. The
 * deterministically generated MVP proposal rightly warns on every section
 * until a human writes real reasoning into it.
 */
const structuralWarnings = (document: AssuranceSpecDocument): ReadonlyArray<AssuranceDiagnostic> => {
  const warnings: AssuranceDiagnostic[] = []
  for (const id of MANDATORY_ASSURANCE_SECTION_IDS) {
    const section = document.sections.find((candidate) => candidate.id === id)
    if (section === undefined) continue
    const blockName = structuredBlockNameForSection(id)
    const narrative = blockName === null
      ? section.content
      : section.content.replace(new RegExp(`\n?\`\`\`${blockName}\\n[\\s\\S]*?\n\`\`\`\n?`, "g"), "\n")
    const meaningful = narrative.replace(/[`*_#>\-\s\d.()[\]]/g, " ").trim()
    const wordCount = meaningful.split(/\s+/).filter(Boolean).length
    if (meaningful === "" || /^tbd$/i.test(meaningful)) {
      warnings.push({
        code: "empty_required_section",
        message: `Mandatory section has no narrative content: ${id}`,
        severity: "warning",
        path: `sections.${id}`,
      })
    } else if (wordCount < THIN_SECTION_WORD_COUNT) {
      warnings.push({
        code: "thin_required_section",
        message: `Mandatory section narrative is skeleton-thin (${wordCount} words): ${id}`,
        severity: "warning",
        path: `sections.${id}`,
      })
    }
  }
  return warnings
}

export const validateAssuranceSpec = (markdown: string): AssuranceStructuralValidation => {
  try {
    const { document, integrity } = parseAssuranceSpecDocument(markdown)
    const warnings = structuralWarnings(document)
    return integrity.length === 0
      ? { valid: true, document, errors: [], warnings }
      : { valid: false, document, errors: integrity, warnings }
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
    const falseGreen = obligationFalseGreenDiagnostics(obligation)
    diagnostics.push(...falseGreen)
    if (missingFields.length === 0 && falseGreen.length === 0) {
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
