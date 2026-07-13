/**
 * Pure, deterministic derivations over parsed AssuranceSpec documents for the
 * AT-1 agent tooling slice (docs/assurance/AGENT_TOOLING.md §2-§3).
 *
 * Everything in this module is a pure function: no filesystem access, no
 * clock, no randomness, no model calls (Law 2). The CLI and the MCP server
 * share these code paths through the Effect handlers in ./handlers.ts.
 *
 * Honesty posture (Law 7): no receipts exist yet, so the observation axis is
 * `not_run` everywhere, the reachable frontier is `not_computed`, and nothing
 * here rounds a typed gap up into a pass.
 */
import { createHash } from "node:crypto"

import type {
  AssuranceGate,
  AssuranceObligation,
  AssuranceSpecDocument,
} from "./schema.ts"
import { missingObligationDesignFields } from "./validator.ts"

export const sha256Digest = (value: string): string =>
  `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`

// ---------------------------------------------------------------------------
// Subject probe (computed by the IO layer, consumed by pure derivations)
// ---------------------------------------------------------------------------

export type SubjectBindingStatus = "bound" | "stale" | "missing"

export type SubjectProbe = Readonly<{
  status: SubjectBindingStatus
  declared_path: string
  declared_revision: number
  declared_digest: string
  current_digest?: string
  current_revision?: number
  current_executable?: boolean
  errors: ReadonlyArray<{ code: string; message: string }>
}>

// ---------------------------------------------------------------------------
// Sessions (stateless dual-digest pins)
// ---------------------------------------------------------------------------

export const STATELESS_SESSION_NOTES = [
  "Sessions are stateless by design: store this full pin yourself; no daemon or in-memory registry holds it.",
  "intent_digest is not present: the subject profile is document-digest-only today; the field is declared in the format and never faked.",
] as const

export type AssuranceSessionPin = Readonly<{
  session_id: string
  assurance_spec: Readonly<{ path: string; revision: number; document_digest: string }>
  subject: Readonly<{ path: string; revision: number; document_digest: string }>
  subject_binding: SubjectBindingStatus
  criterion_refs: ReadonlyArray<string>
  notes: ReadonlyArray<string>
  message: string
}>

export const buildSessionPin = (options: Readonly<{
  assuranceSpecPath: string
  assuranceSpecDigest: string
  document: AssuranceSpecDocument
  subject: SubjectProbe
}>): AssuranceSessionPin => {
  const subjectDigest = options.subject.current_digest ?? options.subject.declared_digest
  const sessionId = `assurance-session-${createHash("sha256")
    .update(`${options.assuranceSpecPath}\n${options.assuranceSpecDigest}\n${subjectDigest}`, "utf8")
    .digest("hex")
    .slice(0, 24)}`
  return {
    session_id: sessionId,
    assurance_spec: {
      path: options.assuranceSpecPath,
      revision: options.document.frontmatter.assurance_revision,
      document_digest: options.assuranceSpecDigest,
    },
    subject: {
      path: options.subject.declared_path,
      revision: options.subject.current_revision ?? options.subject.declared_revision,
      document_digest: subjectDigest,
    },
    subject_binding: options.subject.status,
    criterion_refs: options.document.subject.product_spec.criterion_refs,
    notes: [...STATELESS_SESSION_NOTES],
    message:
      "Assurance session pinned against the current on-disk digests. Run session check before mutation and before reporting.",
  }
}

export type AssuranceSessionStatus =
  | "unchanged"
  | "assurance_spec_changed"
  | "subject_changed"
  | "both_changed"
  | "invalid_current"

export type AssuranceSessionRecommendedAction =
  | "continue_against_pinned"
  | "replan_before_continuing"
  | "resolve_invalid_current"

export const classifySessionStatus = (options: Readonly<{
  pinnedSpecDigest: string
  pinnedSubjectDigest: string
  currentSpecDigest: string
  currentSubjectDigest: string
}>): Exclude<AssuranceSessionStatus, "invalid_current"> => {
  const specChanged = options.currentSpecDigest !== options.pinnedSpecDigest
  const subjectChanged = options.currentSubjectDigest !== options.pinnedSubjectDigest
  if (specChanged && subjectChanged) return "both_changed"
  if (specChanged) return "assurance_spec_changed"
  if (subjectChanged) return "subject_changed"
  return "unchanged"
}

export const recommendedActionForStatus = (
  status: AssuranceSessionStatus,
): AssuranceSessionRecommendedAction =>
  status === "unchanged"
    ? "continue_against_pinned"
    : status === "invalid_current"
      ? "resolve_invalid_current"
      : "replan_before_continuing"

export type AssuranceSessionCheck = Readonly<{
  session_id?: string
  status: AssuranceSessionStatus
  recommended_action: AssuranceSessionRecommendedAction
  assurance_spec: Readonly<{
    path: string
    pinned_digest: string
    current_digest?: string
    changed: boolean
  }>
  subject: Readonly<{
    path?: string
    pinned_digest: string
    current_digest?: string
    changed: boolean
  }>
  errors: ReadonlyArray<{ code: string; message: string }>
}>

// ---------------------------------------------------------------------------
// Obligations
// ---------------------------------------------------------------------------

export type ObligationDesignStatus = "ready" | "needs_design"

export const obligationDesignStatus = (obligation: AssuranceObligation): ObligationDesignStatus =>
  missingObligationDesignFields(obligation).length === 0 ? "ready" : "needs_design"

export type ObligationSummary = Readonly<{
  id: string
  title: string
  criterion_refs: ReadonlyArray<string>
  disposition: string
  technique: string | null
  environment_refs: ReadonlyArray<string>
  design_status: ObligationDesignStatus
}>

export type ObligationFilter = Readonly<{
  criterionRef?: string
  status?: ObligationDesignStatus
  technique?: string
}>

export const summarizeObligations = (
  document: AssuranceSpecDocument,
  filter: ObligationFilter = {},
): ReadonlyArray<ObligationSummary> =>
  document.obligations
    .filter((obligation) =>
      (filter.criterionRef === undefined || obligation.criterion_refs.includes(filter.criterionRef))
      && (filter.status === undefined || obligationDesignStatus(obligation) === filter.status)
      && (filter.technique === undefined || obligation.technique === filter.technique))
    .map((obligation) => ({
      id: obligation.id,
      title: obligation.title,
      criterion_refs: obligation.criterion_refs,
      disposition: obligation.disposition,
      technique: obligation.technique ?? null,
      environment_refs: obligation.environment_refs ?? [],
      design_status: obligationDesignStatus(obligation),
    }))

export type ObligationDetail = Readonly<{
  obligation: AssuranceObligation
  design_status: ObligationDesignStatus
  unresolved_fields: ReadonlyArray<string>
}>

export const obligationDetail = (
  document: AssuranceSpecDocument,
  obligationId: string,
): ObligationDetail | null => {
  const obligation = document.obligations.find((candidate) => candidate.id === obligationId)
  if (obligation === undefined) return null
  return {
    obligation,
    design_status: obligationDesignStatus(obligation),
    unresolved_fields: missingObligationDesignFields(obligation),
  }
}

// ---------------------------------------------------------------------------
// Seams
// ---------------------------------------------------------------------------

export type SeamReport = Readonly<{
  seams: ReadonlyArray<AssuranceObligation>
  count: number
  message: string
}>

export const seamReport = (document: AssuranceSpecDocument): SeamReport => {
  const seams = document.obligations.filter((obligation) => (obligation.domains ?? []).includes("seam"))
  return {
    seams,
    count: seams.length,
    message: seams.length === 0
      ? "No seam obligations are declared. Absent seam coverage is a queryable fact, not an omission."
      : "Seam obligations name both real sides; mock-only coverage never satisfies a seam (Law 5).",
  }
}

// ---------------------------------------------------------------------------
// Environments
// ---------------------------------------------------------------------------

export type EnvironmentGap = Readonly<{
  code: string
  message: string
  environment_ref?: string
}>

export type EnvironmentReport = Readonly<{
  profile_support: "not_implemented"
  referenced_environments: ReadonlyArray<Readonly<{
    id: string
    declared_in_environments_section: boolean
    referenced_by_obligations: ReadonlyArray<string>
    gaps: ReadonlyArray<EnvironmentGap>
  }>>
  gaps: ReadonlyArray<EnvironmentGap>
  message: string
}>

export const environmentReport = (document: AssuranceSpecDocument): EnvironmentReport => {
  const declared = new Set(document.environments.profiles.map((profile) => profile.id))
  const referencedBy = new Map<string, Array<string>>()
  for (const obligation of document.obligations) {
    for (const environmentRef of obligation.environment_refs ?? []) {
      const existing = referencedBy.get(environmentRef) ?? []
      existing.push(obligation.id)
      referencedBy.set(environmentRef, existing)
    }
  }
  const ids = [...new Set([...declared, ...referencedBy.keys()])].sort()
  const referenced = ids.map((id) => {
    const gaps: EnvironmentGap[] = [{
      code: "environment_profile_missing",
      message: `No Environment Profile exists for ${id}; AT-1 does not read assurance/environments/*.assurance-environment.json profiles.`,
      environment_ref: id,
    }]
    return {
      id,
      declared_in_environments_section: declared.has(id),
      referenced_by_obligations: (referencedBy.get(id) ?? []).sort(),
      gaps,
    }
  })
  const gaps: EnvironmentGap[] = ids.length === 0
    ? [{
        code: "environment_profiles_need_design",
        message: "No Environment Profiles are defined and no obligation references an environment.",
      }]
    : referenced.flatMap((entry) => entry.gaps)
  return {
    profile_support: "not_implemented",
    referenced_environments: referenced,
    gaps,
    message:
      "Environment Profile reading ships in a later slice; missing profiles are typed gaps, never empty successes.",
  }
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

export type GateReport = Readonly<{
  gates: ReadonlyArray<AssuranceGate & { arms: ReadonlyArray<string> }>
  count: number
  message: string
}>

export const gateReport = (document: AssuranceSpecDocument): GateReport => ({
  gates: document.gates.map((gate) => ({
    ...gate,
    arms: document.obligations
      .filter((obligation) => obligation.activation_gate === gate.id)
      .map((obligation) => obligation.id),
  })),
  count: document.gates.length,
  message: document.gates.length === 0
    ? "No gates are defined. Gate design remains a reviewable change to the committed artifact."
    : "Gates report which obligations they arm; a gate never admits, approves, or releases (Law 10).",
})

// ---------------------------------------------------------------------------
// Coverage ledgers (three, separately; never a blended score)
// ---------------------------------------------------------------------------

export type CoverageLedgers = Readonly<{
  criterion_traceability: Readonly<{
    total_criteria: number
    traceable_criteria: number
    entries: ReadonlyArray<Readonly<{ criterion_ref: string; obligation_refs: ReadonlyArray<string> }>>
  }>
  execution: Readonly<{
    total_obligations: number
    executed_obligations: number
    receipt_source: "none"
    entries: ReadonlyArray<Readonly<{
      obligation_id: string
      environment_ref: string | null
      observation: "not_run"
    }>>
  }>
  reachable_frontier: Readonly<{
    status: "not_computed"
    reason: string
  }>
  message: string
}>

export const coverageLedgers = (document: AssuranceSpecDocument): CoverageLedgers => {
  const traceabilityEntries = document.subject.product_spec.criterion_refs.map((criterionRef) => ({
    criterion_ref: criterionRef,
    obligation_refs: document.obligations
      .filter((obligation) => obligation.criterion_refs.includes(criterionRef))
      .map((obligation) => obligation.id),
  }))
  type ExecutionEntry = Readonly<{
    obligation_id: string
    environment_ref: string | null
    observation: "not_run"
  }>
  const executionEntries = document.obligations.flatMap((obligation): ReadonlyArray<ExecutionEntry> => {
    const environments = obligation.environment_refs ?? []
    return environments.length === 0
      ? [{ obligation_id: obligation.id, environment_ref: null, observation: "not_run" }]
      : environments.map((environmentRef) => ({
          obligation_id: obligation.id,
          environment_ref: environmentRef,
          observation: "not_run",
        }))
  })
  return {
    criterion_traceability: {
      total_criteria: traceabilityEntries.length,
      traceable_criteria: traceabilityEntries.filter((entry) => entry.obligation_refs.length > 0).length,
      entries: traceabilityEntries,
    },
    execution: {
      total_obligations: document.obligations.length,
      executed_obligations: 0,
      receipt_source: "none",
      entries: executionEntries,
    },
    reachable_frontier: {
      status: "not_computed",
      reason: "No compiler exists; the reachable frontier is a compiler projection, not a repository guess.",
    },
    message: "Three ledgers reported separately. No blended coverage percentage is computed (Law 7).",
  }
}

// ---------------------------------------------------------------------------
// Evidence checklist
// ---------------------------------------------------------------------------

export type EvidenceChecklist = Readonly<{
  criteria: ReadonlyArray<Readonly<{
    criterion_ref: string
    obligations: ReadonlyArray<Readonly<{
      obligation_id: string
      evidence_state: "designed" | "needs_design"
      required_kinds: ReadonlyArray<string>
      proof_rung: string | null
      environment_refs: ReadonlyArray<string>
      missing: ReadonlyArray<Readonly<{
        kind: string
        environment_ref: string | null
        status: "missing"
      }>>
      present: ReadonlyArray<never>
      gaps: ReadonlyArray<Readonly<{ code: string; message: string }>>
    }>>
  }>>
  message: string
}>

export const evidenceChecklist = (
  document: AssuranceSpecDocument,
  criterionRef?: string,
): EvidenceChecklist | null => {
  const criterionRefs = document.subject.product_spec.criterion_refs
  if (criterionRef !== undefined && !criterionRefs.includes(criterionRef)) return null
  const selected = criterionRef === undefined ? criterionRefs : [criterionRef]
  return {
    criteria: selected.map((criterion) => ({
      criterion_ref: criterion,
      obligations: document.obligations
        .filter((obligation) => obligation.criterion_refs.includes(criterion))
        .map((obligation) => {
          const evidence = obligation.evidence
          const environments = obligation.environment_refs ?? []
          const requiredKinds = evidence?.required_kinds ?? []
          type MissingEntry = Readonly<{
            kind: string
            environment_ref: string | null
            status: "missing"
          }>
          const missing = requiredKinds.flatMap((kind): ReadonlyArray<MissingEntry> =>
            environments.length === 0
              ? [{ kind, environment_ref: null, status: "missing" }]
              : environments.map((environmentRef) => ({
                  kind,
                  environment_ref: environmentRef,
                  status: "missing",
                })))
          return {
            obligation_id: obligation.id,
            evidence_state: evidence === undefined ? ("needs_design" as const) : ("designed" as const),
            required_kinds: requiredKinds,
            proof_rung: evidence?.proof_rung ?? null,
            environment_refs: environments,
            missing,
            present: [],
            gaps: evidence === undefined
              ? [{
                  code: "evidence_requirements_undesigned",
                  message: `Obligation ${obligation.id} has no designed evidence requirements yet.`,
                }]
              : [],
          }
        }),
    })),
    message:
      "This checklist reports required evidence and what is missing. It collects nothing and attaches no verdicts to links (Law 13).",
  }
}

// ---------------------------------------------------------------------------
// Completion-claim audit (all eight status axes; rounds nothing up)
// ---------------------------------------------------------------------------

export type ObligationStatusAxes = Readonly<{
  admission: string
  readiness: "needs_design" | "not_computed"
  observation: "not_run"
  infrastructure: "not_computed"
  stability: "unknown"
  freshness: "current" | "stale"
  disposition: "pending_review"
  exception: "none"
}>

export type CompletionClaimAudit = Readonly<{
  claim: string | null
  claim_evaluated: false
  admission_state: string
  subject_binding: SubjectBindingStatus
  obligations: ReadonlyArray<Readonly<{
    obligation_id: string
    title: string
    criterion_refs: ReadonlyArray<string>
    axes: ObligationStatusAxes
    unresolved_fields: ReadonlyArray<string>
  }>>
  notes: ReadonlyArray<string>
  message: string
}>

export const completionClaimAudit = (
  document: AssuranceSpecDocument,
  subject: SubjectProbe,
  claim?: string,
): CompletionClaimAudit => {
  const admission = document.frontmatter.lifecycle_state
  const freshness: "current" | "stale" = subject.status === "bound" ? "current" : "stale"
  return {
    claim: claim ?? null,
    claim_evaluated: false,
    admission_state: admission,
    subject_binding: subject.status,
    obligations: document.obligations.map((obligation) => {
      const unresolved = missingObligationDesignFields(obligation)
      return {
        obligation_id: obligation.id,
        title: obligation.title,
        criterion_refs: obligation.criterion_refs,
        axes: {
          admission,
          readiness: unresolved.length > 0 ? ("needs_design" as const) : ("not_computed" as const),
          observation: "not_run" as const,
          infrastructure: "not_computed" as const,
          stability: "unknown" as const,
          freshness,
          disposition: "pending_review" as const,
          exception: "none" as const,
        },
        unresolved_fields: unresolved,
      }
    }),
    notes: [
      "observation is not_run for every obligation: no Assurance Receipts exist, and repository state, test files, and claim text never become observations.",
      "readiness beyond needs_design and infrastructure are not_computed: no compiler or runner receipts exist.",
      "freshness projects only the subject document binding (declared digest vs current bytes).",
      "The claim text is echoed for the record, not evaluated; semantic claim evaluation is reviewable planner work, not this tool (Law 2).",
    ],
    message:
      "Acceptance is a human/policy decision. This audit reports every obligation across all eight status axes and rounds nothing up (Law 7).",
  }
}

// ---------------------------------------------------------------------------
// Typed gaps
// ---------------------------------------------------------------------------

export type TypedGap = Readonly<{
  code: string
  message: string
  obligation_id?: string
  environment_ref?: string
  missing_fields?: ReadonlyArray<string>
}>

export type TypedGapReport = Readonly<{
  gaps: ReadonlyArray<TypedGap>
  count: number
  message: string
}>

export const typedGapReport = (
  document: AssuranceSpecDocument,
  subject: SubjectProbe,
): TypedGapReport => {
  const gaps: TypedGap[] = []
  if (subject.status === "missing") {
    gaps.push({
      code: "subject_missing",
      message: `The bound ProductSpec subject ${subject.declared_path} is not readable at the declared path.`,
    })
  } else if (subject.status === "stale") {
    gaps.push({
      code: "subject_document_digest_mismatch",
      message: `The bound ProductSpec subject ${subject.declared_path} no longer matches the declared document digest.`,
    })
  }
  for (const obligation of document.obligations) {
    const missingFields = missingObligationDesignFields(obligation)
    if (missingFields.length > 0) {
      gaps.push({
        code: "obligation_needs_design",
        message: `Obligation ${obligation.id} is missing admitted proof design.`,
        obligation_id: obligation.id,
        missing_fields: missingFields,
      })
    }
    if (obligation.oracle === undefined) {
      gaps.push({
        code: "missing_oracle",
        message: `Obligation ${obligation.id} names no oracle.`,
        obligation_id: obligation.id,
      })
    }
    if (obligation.falsifier === undefined) {
      gaps.push({
        code: "missing_falsifier",
        message: `Obligation ${obligation.id} names no falsifier.`,
        obligation_id: obligation.id,
      })
    }
  }
  if (document.riskModel.risks.length === 0) {
    gaps.push({ code: "risk_model_needs_design", message: "No structured assurance risks are defined." })
  }
  const environments = environmentReport(document)
  if (environments.referenced_environments.length === 0) {
    gaps.push({ code: "environment_profiles_need_design", message: "No Environment Profiles are defined." })
  } else {
    for (const entry of environments.referenced_environments) {
      for (const gap of entry.gaps) {
        gaps.push({ code: gap.code, message: gap.message, environment_ref: entry.id })
      }
    }
  }
  if (document.gates.length === 0) {
    gaps.push({ code: "gates_need_design", message: "No assurance activation or aggregate gates are defined." })
  }
  if (document.evidencePolicy.policy_state === "needs_design") {
    gaps.push({ code: "evidence_policy_needs_design", message: "Evidence sufficiency and freshness policy still need design." })
  }
  if (document.authority.policy_state === "needs_design") {
    gaps.push({ code: "authority_policy_needs_design", message: "Admission, verification, and release roles still need design." })
  }
  return {
    gaps,
    count: gaps.length,
    message:
      "Machine-readable report of what would have to exist before this spec could be admitted. Gaps are typed; none is skipped-and-green.",
  }
}
