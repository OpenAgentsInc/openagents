/**
 * Browser-safe AssuranceSpec document boundary for Desktop.
 *
 * Domain parsing stays outside the renderer folder. The renderer receives a
 * bounded presentation projection rather than raw repository inventory, while
 * future editor-opened source can use the same projection function.
 */
import {
  assessAssuranceSpec,
  missingObligationDesignFields as missingDomainObligationDesignFields,
  validateAssuranceSpec,
  type AssuranceAdequacyAssessment,
  type AssuranceAuthority,
  type AssuranceDiagnostic,
  type AssuranceEvidencePolicy,
  type AssuranceObligation as DomainAssuranceObligation,
  type AssuranceSpecFrontmatter,
  type AssuranceSubject,
} from "@openagentsinc/assurance-spec/browser"

export type {
  AssuranceAdequacyAssessment,
  AssuranceDiagnostic,
}

export type AssuranceObligation = Omit<DomainAssuranceObligation, "candidate_artifact_refs">

export const missingObligationDesignFields = (
  obligation: AssuranceObligation,
): ReturnType<typeof missingDomainObligationDesignFields> =>
  missingDomainObligationDesignFields({
    ...obligation,
    candidate_artifact_refs: [],
  })

export type AssuranceSpecRenderDocument = Readonly<{
  frontmatter: AssuranceSpecFrontmatter
  subject: AssuranceSubject
  obligations: ReadonlyArray<AssuranceObligation>
  riskCount: number
  environmentProfileCount: number
  gateCount: number
  repositoryInventory: Readonly<{
    state: "absent" | "clean" | "dirty" | "not_git" | "unavailable"
    candidateCount: number
    declaredScriptCount: number
    truncated: boolean
    diagnostics: ReadonlyArray<string>
  }>
  evidencePolicy: AssuranceEvidencePolicy
  authority: AssuranceAuthority
}>

export type ReadyAssuranceSpec = Readonly<{
  state: "ready"
  relativePath: string
  document: AssuranceSpecRenderDocument
  assessment: AssuranceAdequacyAssessment
}>

export type InvalidAssuranceSpec = Readonly<{
  state: "invalid"
  relativePath: string
  diagnostics: ReadonlyArray<AssuranceDiagnostic>
}>

export type AssuranceSpecProjection = ReadyAssuranceSpec | InvalidAssuranceSpec

export const projectAssuranceSpecDocument = (
  source: string,
  relativePath: string,
): AssuranceSpecProjection => {
  const validation = validateAssuranceSpec(source)
  if (!validation.valid || validation.document === undefined) {
    return {
      state: "invalid",
      relativePath,
      diagnostics: validation.errors,
    }
  }
  const document = validation.document
  const inventory = document.environments.repository_inventory
  return {
    state: "ready",
    relativePath,
    document: {
      frontmatter: document.frontmatter,
      subject: document.subject,
      obligations: document.obligations.map(({ candidate_artifact_refs: _, ...obligation }) => obligation),
      riskCount: document.riskModel.risks.length,
      environmentProfileCount: document.environments.profiles.length,
      gateCount: document.gates.length,
      repositoryInventory: {
        state: inventory.state,
        candidateCount: inventory.candidate_artifact_refs.length,
        declaredScriptCount: inventory.declared_scripts.length,
        truncated: inventory.truncated,
        diagnostics: inventory.diagnostics,
      },
      evidencePolicy: document.evidencePolicy,
      authority: document.authority,
    },
    assessment: assessAssuranceSpec(document),
  }
}

const invalidBundledProjection = (): InvalidAssuranceSpec => ({
  state: "invalid",
  relativePath: "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
  diagnostics: [{
    code: "bundled_assurance_spec_unavailable",
    message: "The bundled AssuranceSpec presentation snapshot is unavailable.",
    severity: "error",
  }],
})

export const decodeBundledAssuranceSpecProjection = (raw: string): AssuranceSpecProjection => {
  try {
    const value = JSON.parse(raw) as Partial<AssuranceSpecProjection>
    if (value.state === "ready" && typeof value.relativePath === "string" && value.document !== undefined && value.assessment !== undefined) {
      return value as ReadyAssuranceSpec
    }
    if (value.state === "invalid" && typeof value.relativePath === "string" && Array.isArray(value.diagnostics)) {
      return value as InvalidAssuranceSpec
    }
  } catch {
    // The build owns this constant; malformed or absent bytes fail closed.
  }
  return invalidBundledProjection()
}
