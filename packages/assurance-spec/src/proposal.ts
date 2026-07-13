import { createHash } from "node:crypto"
import { posix } from "node:path"

import { validateExecutableProductSpec } from "@openagentsinc/product-spec"

import { absentRepositoryInventory } from "./repository-inventory.ts"
import {
  ASSURANCE_SECTION_LABELS,
  ASSURANCE_SPEC_FORMAT_VERSION,
  MANDATORY_ASSURANCE_SECTION_IDS,
  type AssuranceDiagnostic,
  type AssuranceObligation,
  type AssuranceSpecDocument,
  type RepositoryInventory,
} from "./schema.ts"
import { serializeAssuranceSpec } from "./serializer.ts"
import { assessAssuranceSpec, validateAssuranceSpec, type AssuranceAdequacyAssessment } from "./validator.ts"

const sha256 = (value: string): string =>
  `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`

const safeIdPart = (value: string): string => value
  .toLowerCase()
  .replace(/[^a-z0-9]+/g, ".")
  .replace(/^\.|\.$/g, "")
  .slice(0, 160) || "product"

const normalizeProductSpecPath = (value: string): string | null => {
  const normalized = value.replaceAll("\\", "/")
  if (normalized.startsWith("/") || normalized.includes("\0")) return null
  const clean = posix.normalize(normalized)
  if (clean === ".." || clean.startsWith("../") || !clean.endsWith(".product-spec.md")) return null
  return clean
}

export type AssuranceProposalOptions = Readonly<{
  productSpecPath: string
  productSpecMarkdown: string
  repositoryInventory?: RepositoryInventory
  assuranceSpecId?: string
  assuranceRevision?: number
  title?: string
  author?: string
}>

export type AssuranceProposalResult =
  | Readonly<{
      ok: false
      diagnostics: ReadonlyArray<AssuranceDiagnostic>
    }>
  | Readonly<{
      ok: true
      document: AssuranceSpecDocument
      markdown: string
      structural: ReturnType<typeof validateAssuranceSpec>
      adequacy: AssuranceAdequacyAssessment
    }>

export const proposeAssuranceSpec = (options: AssuranceProposalOptions): AssuranceProposalResult => {
  const path = normalizeProductSpecPath(options.productSpecPath)
  if (path === null) {
    return { ok: false, diagnostics: [{ code: "invalid_subject_path", message: "ProductSpec path must be repository-relative and end in .product-spec.md.", severity: "error", path: "subject.product_spec.path" }] }
  }
  const source = validateExecutableProductSpec(options.productSpecMarkdown)
  if (!source.executable) {
    return {
      ok: false,
      diagnostics: [
        { code: "product_spec_not_executable", message: "Assurance proposal requires an executable ProductSpec with a positive revision and stable unique criterion IDs.", severity: "error", path: "subject.product_spec" },
        ...source.errors.map((error) => ({ ...error, severity: "error" as const })),
      ],
    }
  }
  const revision = source.document.frontmatter.spec_revision!
  const subjectDigest = sha256(options.productSpecMarkdown)
  const risks = source.document.sections.find((section) => section.id === "risks")?.content.trim()
    || "The source ProductSpec contains no Risks section. Assurance risk modeling remains required."
  const obligations: AssuranceObligation[] = source.criteria.map((criterion) => ({
    id: `AO-${criterion.id}-01`,
    title: `Assure ${criterion.id}`,
    criterion_refs: [criterion.id],
    source_claim_snapshot: criterion.body,
    source_claim_digest: sha256(`${criterion.id}\n${criterion.body}`),
    disposition: "required",
    candidate_artifact_refs: [],
  }))
  const document: AssuranceSpecDocument = {
    frontmatter: {
      assurance_spec_format_version: ASSURANCE_SPEC_FORMAT_VERSION,
      assurance_spec_id: options.assuranceSpecId ?? `assurance.${safeIdPart(source.document.frontmatter.title)}`,
      assurance_revision: options.assuranceRevision ?? 1,
      title: options.title ?? `${source.document.frontmatter.title} Assurance Spec`,
      artifact_type: "product_assurance",
      lifecycle_state: "proposed",
      author: options.author ?? source.document.frontmatter.author,
    },
    unknownFrontmatter: [],
    sections: MANDATORY_ASSURANCE_SECTION_IDS.map((id) => ({ id, label: ASSURANCE_SECTION_LABELS[id], content: "" })),
    customSections: [],
    subject: {
      product_spec: {
        profile: "openagents_executable_v0.1_exact_document",
        path,
        spec_format_version: "0.1",
        spec_revision: revision,
        document_digest: subjectDigest,
        criterion_refs: source.criteria.map((criterion) => criterion.id),
      },
    },
    riskModel: {
      source_snapshot: risks,
      source_digest: sha256(risks),
      risks: [],
    },
    environments: {
      profiles: [],
      repository_inventory: options.repositoryInventory ?? absentRepositoryInventory(),
    },
    obligations,
    gates: [],
    evidencePolicy: {
      links_are_verdicts: false,
      missing_evidence_verdict: "INCONCLUSIVE",
      required_for_ready_obligation: ["oracle_observation", "falsifier_observation", "environment_binding", "independent_review"],
      policy_state: "needs_design",
    },
    authority: {
      proposal_may_self_admit: false,
      proposal_may_execute: false,
      proposal_may_verify: false,
      proposal_may_release: false,
      proposal_may_change_public_promises: false,
      admitted_roles: [],
      verifier_roles: [],
      release_roles: [],
      policy_state: "needs_design",
    },
  }
  const markdown = serializeAssuranceSpec(document)
  const structural = validateAssuranceSpec(markdown)
  if (!structural.valid || structural.document === undefined) {
    return { ok: false, diagnostics: structural.errors }
  }
  return {
    ok: true,
    document: structural.document,
    markdown,
    structural,
    adequacy: assessAssuranceSpec(structural.document),
  }
}
