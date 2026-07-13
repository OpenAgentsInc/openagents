/**
 * AssuranceSpec document support for OpenAgents Desktop.
 *
 * Parsing and adequacy semantics come from the browser-safe package surface.
 * This module owns only editor presentation state and a pure Effect Native
 * view. It does not read files, admit proposals, execute checks, verify
 * evidence, waive obligations, or release software.
 */
import {
  missingObligationDesignFields,
  type AssuranceObligation,
  type AssuranceSpecProjection,
  type InvalidAssuranceSpec,
  type ReadyAssuranceSpec,
  projectAssuranceSpecDocument,
} from "../assurance-spec-document.ts"
import {
  Badge,
  Button,
  Divider,
  Icon,
  IntentRef,
  Spacer,
  SplitPane,
  Stack,
  StaticPayload,
  Table,
  Text,
  defineIntent,
  type IntentHandlers,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import { bundledMvpAssuranceSpecProjection } from "./assurance-spec-source.ts"

export const mvpAssuranceSpecRelativePath =
  "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md"

export type AssuranceSpecWorkspaceState = Readonly<{
  projection: AssuranceSpecProjection
  selectedObligationId: string | null
}>

const preferredDogfoodObligation = "AO-CW-AC-04-01"

export const assuranceSpecWorkspaceStateFromSource = (
  source: string,
  relativePath: string = mvpAssuranceSpecRelativePath,
): AssuranceSpecWorkspaceState => {
  const projection = projectAssuranceSpecDocument(source, relativePath)
  const obligations = projection.state === "ready" ? projection.document.obligations : []
  return {
    projection,
    selectedObligationId:
      obligations.find((obligation) => obligation.id === preferredDogfoodObligation)?.id
      ?? obligations[0]?.id
      ?? null,
  }
}

export const initialAssuranceSpecWorkspaceState = (): AssuranceSpecWorkspaceState =>
  ({
    projection: bundledMvpAssuranceSpecProjection,
    selectedObligationId: bundledMvpAssuranceSpecProjection.state === "ready"
      ? bundledMvpAssuranceSpecProjection.document.obligations.find(obligation => obligation.id === preferredDogfoodObligation)?.id
        ?? bundledMvpAssuranceSpecProjection.document.obligations[0]?.id
        ?? null
      : null,
  })

export type AssuranceSpecWorkspaceCapableState = Readonly<{
  assuranceSpec: AssuranceSpecWorkspaceState
}>

export const AssuranceSpecObligationSelected = defineIntent(
  "AssuranceSpecObligationSelected",
  Schema.String,
)

export const assuranceSpecWorkspaceIntents = [AssuranceSpecObligationSelected] as const

export const makeAssuranceSpecWorkspaceHandlers = <S extends AssuranceSpecWorkspaceCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
): IntentHandlers<typeof assuranceSpecWorkspaceIntents> => ({
  AssuranceSpecObligationSelected: (obligationId) => Effect.gen(function* () {
    yield* SubscriptionRef.update(state, current => {
      const projection = current.assuranceSpec.projection
      if (
        projection.state !== "ready"
        || !projection.document.obligations.some(obligation => obligation.id === obligationId)
      ) return current
      return {
        ...current,
        assuranceSpec: {
          ...current.assuranceSpec,
          selectedObligationId: obligationId,
        },
      }
    })
  }),
})

const singleLine = (value: string): string => value.replace(/\s+/g, " ").trim()

const claimPreview = (obligation: AssuranceObligation): string => {
  const claim = singleLine(obligation.source_claim_snapshot)
  return claim.length <= 76 ? claim : `${claim.slice(0, 73).trimEnd()}…`
}

const shortDigest = (digest: string): string =>
  digest.length <= 28 ? digest : `${digest.slice(0, 21)}…${digest.slice(-7)}`

const designFieldLabels: Readonly<Record<string, string>> = {
  domains: "Domains",
  technique: "Technique",
  environment_refs: "Environment",
  oracle: "Oracle",
  falsifier: "Falsifier",
  evidence: "Evidence",
  independence: "Independence",
  activation_gate: "Activation gate",
}

const designFieldValue = (obligation: AssuranceObligation, field: string): string => {
  switch (field) {
    case "domains": return obligation.domains?.join(", ") ?? "Not specified"
    case "technique": return obligation.technique ?? "Not specified"
    case "environment_refs": return obligation.environment_refs?.join(", ") ?? "Not bound"
    case "oracle": return obligation.oracle?.statement ?? "Not specified"
    case "falsifier": return obligation.falsifier === undefined
      ? "Not specified"
      : `${obligation.falsifier.kind} · expects ${obligation.falsifier.expected_verdict}`
    case "evidence": return obligation.evidence === undefined
      ? "Not specified"
      : `${obligation.evidence.required_kinds.join(", ")} · ${obligation.evidence.proof_rung}`
    case "independence": return obligation.independence === undefined
      ? "Not specified"
      : obligation.independence.producer_may_verify ? "Producer may verify" : "Independent verifier required"
    case "activation_gate": return obligation.activation_gate ?? "Not specified"
    default: return "Not specified"
  }
}

const policyRow = (key: string, label: string, value: string, tone: "textMuted" | "warning" = "textMuted"): View =>
  Stack({ key, direction: "row", gap: "2", align: "start", style: { width: "full", minWidth: 0 } }, [
    Text({ key: `${key}-label`, content: label, variant: "caption", color: "textFaint" }),
    Text({ key: `${key}-value`, content: value, variant: "caption", color: tone, style: { flex: 1, minWidth: 0 } }),
  ])

const assuranceSummary = (projection: ReadyAssuranceSpec): View => {
  const { coverage } = projection.assessment
  return Stack({ key: "assurance-summary-strip", direction: "row", gap: "1", style: { width: "full" } }, [
    Stack({ key: "assurance-summary-criteria", direction: "column", gap: "0.5", style: { flex: 1, minWidth: 0, padding: "2" } }, [
      Text({ key: "assurance-summary-criteria-value", content: String(coverage.criteria), variant: "heading", color: "textPrimary", weight: "semibold" }),
      Text({ key: "assurance-summary-criteria-label", content: "criteria mapped", variant: "caption", color: "textMuted" }),
    ]),
    Stack({ key: "assurance-summary-obligations", direction: "column", gap: "0.5", style: { flex: 1, minWidth: 0, padding: "2" } }, [
      Text({ key: "assurance-summary-obligations-value", content: String(coverage.obligations), variant: "heading", color: "textPrimary", weight: "semibold" }),
      Text({ key: "assurance-summary-obligations-label", content: "obligations", variant: "caption", color: "textMuted" }),
    ]),
    Stack({ key: "assurance-summary-ready", direction: "column", gap: "0.5", style: { flex: 1, minWidth: 0, padding: "2" } }, [
      Text({ key: "assurance-summary-ready-value", content: String(coverage.ready), variant: "heading", color: "textPrimary", weight: "semibold" }),
      Text({ key: "assurance-summary-ready-label", content: "proof-ready", variant: "caption", color: "textMuted" }),
    ]),
    Stack({ key: "assurance-summary-needs-design", direction: "column", gap: "0.5", style: { flex: 1, minWidth: 0, padding: "2" } }, [
      Text({ key: "assurance-summary-needs-design-value", content: String(coverage.needs_design), variant: "heading", color: "warning", weight: "semibold" }),
      Text({ key: "assurance-summary-needs-design-label", content: "need proof design", variant: "caption", color: "warning" }),
    ]),
  ])
}

const obligationNavigator = (
  projection: ReadyAssuranceSpec,
  selectedObligationId: string,
): View => Stack({ key: "assurance-obligation-navigator", direction: "column", gap: "1", style: { height: "full", minHeight: 0, minWidth: 0 } }, [
  Stack({ key: "assurance-obligation-navigator-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
    Text({ key: "assurance-obligation-navigator-title", content: "Obligations", variant: "label", color: "textPrimary", weight: "semibold" }),
    Badge({ key: "assurance-obligation-count", label: String(projection.document.obligations.length), tone: "neutral" }),
  ]),
  Text({ key: "assurance-obligation-navigator-copy", content: "One proposed obligation per ProductSpec criterion", variant: "caption", color: "textFaint" }),
  Stack({ key: "assurance-obligation-list", direction: "column", gap: "0.5", style: { flex: 1, minHeight: 0, minWidth: 0 } },
    projection.document.obligations.map(obligation => {
      const selected = obligation.id === selectedObligationId
      const criterion = obligation.criterion_refs.join(", ")
      const missing = missingObligationDesignFields(obligation).length
      return Button({
        key: selected ? `assurance-obligation-selected-${obligation.id}` : `assurance-obligation-${obligation.id}`,
        label: `${criterion}  ${claimPreview(obligation)}`,
        variant: "ghost",
        onPress: IntentRef("AssuranceSpecObligationSelected", StaticPayload(obligation.id)),
        a11y: { label: `${criterion}, ${missing === 0 ? "proof design ready" : `${missing} proof-design fields missing`}` },
        style: {
          width: "full",
          minWidth: 0,
          borderRadius: "md",
          textAlign: "left",
          ...(selected ? { backgroundColor: "stateSelected", color: "textPrimary" } : { color: "textMuted" }),
        },
      })
    })),
])

const proofDesignTable = (obligation: AssuranceObligation): View => {
  const missing = new Set(missingObligationDesignFields(obligation))
  return Table({
    key: "assurance-proof-design-table",
    columns: [
      { id: "field", header: "Proof field", width: "xs" },
      { id: "design", header: "Current design" },
    ],
    rows: Object.keys(designFieldLabels).map(field => ({
      id: field,
      cells: [
        Text({ key: `assurance-proof-field-${field}`, content: designFieldLabels[field]!, variant: "caption", color: "textMuted", weight: "medium" }),
        Stack({ key: `assurance-proof-value-wrap-${field}`, direction: "row", gap: "2", align: "center", style: { minWidth: 0, width: "full" } }, [
          Text({ key: `assurance-proof-value-${field}`, content: designFieldValue(obligation, field), variant: "caption", color: missing.has(field as never) ? "warning" : "textPrimary", style: { flex: 1, minWidth: 0 } }),
          ...(missing.has(field as never) ? [Badge({ key: `assurance-proof-missing-${field}`, label: "Needs design", tone: "warn" })] : []),
        ]),
      ],
    })),
    style: { width: "full", minWidth: 0 },
  })
}

const selectedObligationDetail = (
  projection: ReadyAssuranceSpec,
  obligation: AssuranceObligation,
): View => {
  const subject = projection.document.subject.product_spec
  const repository = projection.document.repositoryInventory
  const missing = missingObligationDesignFields(obligation)
  return Stack({ key: "assurance-obligation-detail", direction: "column", gap: "3", style: { flex: 1, minWidth: 0, minHeight: 0 } }, [
    Stack({ key: "assurance-obligation-detail-scroll", direction: "column", gap: "3", style: { flex: 1, minWidth: 0, minHeight: 0, maxWidth: 840 } }, [
      Stack({ key: "assurance-obligation-identity", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
        Badge({ key: "assurance-selected-criterion", label: obligation.criterion_refs.join(", "), tone: "info" }),
        Text({ key: "assurance-selected-obligation", content: obligation.id, variant: "caption", color: "textFaint" }),
        Spacer({ key: "assurance-selected-spacer", flex: true }),
        Badge({ key: "assurance-selected-status", label: `${missing.length} missing fields`, tone: "warn" }),
      ]),
      Stack({ key: "assurance-source-claim", direction: "column", gap: "1.5", style: { width: "full", minWidth: 0 } }, [
        Text({ key: "assurance-source-claim-label", content: "SOURCE CLAIM", variant: "caption", color: "textFaint", weight: "semibold" }),
        Text({ key: "assurance-source-claim-value", content: singleLine(obligation.source_claim_snapshot), variant: "body", color: "textPrimary" }),
      ]),
      Divider({ key: "assurance-detail-divider-one" }),
      Stack({ key: "assurance-proof-design", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
        Stack({ key: "assurance-proof-design-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
          Text({ key: "assurance-proof-design-title", content: "Proof design", variant: "heading", color: "textPrimary", weight: "semibold" }),
          Text({ key: "assurance-proof-design-copy", content: "Define how this claim could be proven—and refuted.", variant: "caption", color: "textMuted" }),
        ]),
        proofDesignTable(obligation),
      ]),
      Divider({ key: "assurance-detail-divider-two" }),
      Stack({ key: "assurance-binding-and-policy", direction: "row", gap: "4", align: "start", style: { width: "full", minWidth: 0 } }, [
        Stack({ key: "assurance-subject-binding", direction: "column", gap: "1.5", style: { flex: 1, minWidth: 0 } }, [
          Text({ key: "assurance-subject-binding-title", content: "ProductSpec binding", variant: "label", color: "textPrimary", weight: "semibold" }),
          policyRow("assurance-subject-path", "Path", subject.path),
          policyRow("assurance-subject-revision", "Revision", String(subject.spec_revision)),
          policyRow("assurance-subject-digest", "Digest", shortDigest(subject.document_digest)),
        ]),
        Stack({ key: "assurance-policy-state", direction: "column", gap: "1.5", style: { flex: 1, minWidth: 0 } }, [
          Text({ key: "assurance-policy-state-title", content: "Proposal context", variant: "label", color: "textPrimary", weight: "semibold" }),
          policyRow("assurance-risk-policy", "Risk model", projection.document.riskCount === 0 ? "Not designed" : `${projection.document.riskCount} risks` , "warning"),
          policyRow("assurance-environment-policy", "Environments", projection.document.environmentProfileCount === 0 ? "None bound" : `${projection.document.environmentProfileCount} bound`, "warning"),
          policyRow("assurance-gate-policy", "Gates", projection.document.gateCount === 0 ? "None" : String(projection.document.gateCount), "warning"),
          policyRow("assurance-repository-policy", "Repository", `${repository.candidateCount} candidates · unmapped`),
        ]),
      ]),
    ]),
  ])
}

const invalidAssuranceSpecView = (projection: InvalidAssuranceSpec): View =>
  Stack({ key: "assurance-spec-invalid", direction: "column", gap: "3", style: { width: "full", minWidth: 0, padding: "4" } }, [
    Badge({ key: "assurance-spec-invalid-badge", label: "Invalid AssuranceSpec", tone: "danger" }),
    Text({ key: "assurance-spec-invalid-title", content: "This document cannot be visualized safely.", variant: "title", color: "textPrimary" }),
    Text({ key: "assurance-spec-invalid-path", content: projection.relativePath, variant: "caption", color: "textMuted" }),
    ...projection.diagnostics.map((diagnostic, index) => Text({
      key: `assurance-spec-invalid-diagnostic-${index}`,
      content: `${diagnostic.code}: ${diagnostic.message}`,
      variant: "body",
      color: "danger",
    })),
  ])

export const assuranceSpecDocumentView = (
  projection: ReadyAssuranceSpec,
  selectedObligationId: string | null,
): View => {
  const document = projection.document
  const obligation = document.obligations.find(candidate => candidate.id === selectedObligationId)
    ?? document.obligations[0]
  if (obligation === undefined) return invalidAssuranceSpecView({
    state: "invalid",
    relativePath: projection.relativePath,
    diagnostics: [{ code: "missing_obligations", message: "The AssuranceSpec contains no obligations.", severity: "error" }],
  })
  const fileName = projection.relativePath.split("/").at(-1) ?? projection.relativePath
  return Stack({ key: "assurance-spec-document", direction: "column", gap: "0", style: { flex: 1, width: "full", minWidth: 0, minHeight: 0 } }, [
    Stack({ key: "assurance-spec-file-header", direction: "column", gap: "2", style: { width: "full", minWidth: 0, padding: "4" } }, [
      Stack({ key: "assurance-spec-file-line", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
        Icon({ key: "assurance-spec-file-icon", name: "Compare", size: "sm", color: "textMuted", label: "AssuranceSpec document" }),
        Text({ key: "assurance-spec-file-name", content: fileName, variant: "caption", color: "textMuted" }),
        Text({ key: "assurance-spec-file-revision", content: `r${document.frontmatter.assurance_revision}`, variant: "caption", color: "textFaint" }),
        Spacer({ key: "assurance-spec-file-spacer", flex: true }),
        Badge({ key: "assurance-spec-lifecycle", label: "Proposed", tone: "info" }),
        Badge({ key: "assurance-spec-structure", label: "Structure valid", tone: "success" }),
      ]),
      Stack({ key: "assurance-spec-title-line", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
        Text({ key: "assurance-spec-title", content: document.frontmatter.title, variant: "title", color: "textPrimary", weight: "semibold", style: { minWidth: 0 } }),
        Spacer({ key: "assurance-spec-title-spacer", flex: true }),
        Badge({ key: "assurance-spec-design-state", label: `${projection.assessment.coverage.needs_design} need proof design`, tone: "warn" }),
        Badge({ key: "assurance-spec-execution-state", label: "Not executable", tone: "neutral" }),
      ]),
      Stack({ key: "assurance-spec-authority-notice", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0, padding: "2", backgroundColor: "surfaceRaised", borderRadius: "md" } }, [
        Icon({ key: "assurance-spec-authority-icon", name: "InfoCircle", size: "sm", color: "warning", label: "Proposal authority boundary" }),
        Text({ key: "assurance-spec-authority-copy", content: "Proposal only. It cannot admit work, execute checks, verify evidence, waive obligations, release software, or change public promises.", variant: "caption", color: "textMuted" }),
      ]),
      assuranceSummary(projection),
    ]),
    Divider({ key: "assurance-spec-header-divider" }),
    SplitPane({
      key: "assurance-spec-workbench",
      orientation: "row",
      panes: [
        {
          id: "assurance-obligations",
          min: 240,
          max: 400,
          size: 320,
          content: obligationNavigator(projection, obligation.id),
        },
        {
          id: "assurance-detail",
          min: 360,
          content: selectedObligationDetail(projection, obligation),
        },
      ],
      style: { flex: 1, width: "full", minWidth: 0, minHeight: 0 },
    }),
  ])
}

export const assuranceSpecWorkspaceView = (workspace: AssuranceSpecWorkspaceState): View =>
  workspace.projection.state === "invalid"
    ? invalidAssuranceSpecView(workspace.projection)
    : assuranceSpecDocumentView(workspace.projection, workspace.selectedObligationId)
