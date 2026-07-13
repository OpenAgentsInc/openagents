import {
  ASSURANCE_SECTION_LABELS,
  MANDATORY_ASSURANCE_SECTION_IDS,
  type AssuranceSpecDocument,
  type MandatoryAssuranceSectionId,
} from "./schema.ts"

const canonicalize = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(canonicalize)
  if (typeof value !== "object" || value === null) return value
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(([, entry]) => entry !== undefined)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, canonicalize(entry)]),
  )
}

export const canonicalJson = (value: unknown): string =>
  `${JSON.stringify(canonicalize(value), null, 2)}\n`

const quote = (value: string): string => JSON.stringify(value)

const frontmatter = (document: AssuranceSpecDocument): string => [
  "---",
  `assurance_spec_format_version: ${quote(document.frontmatter.assurance_spec_format_version)}`,
  `assurance_spec_id: ${quote(document.frontmatter.assurance_spec_id)}`,
  `assurance_revision: ${document.frontmatter.assurance_revision}`,
  `title: ${quote(document.frontmatter.title)}`,
  `artifact_type: ${quote(document.frontmatter.artifact_type)}`,
  `lifecycle_state: ${quote(document.frontmatter.lifecycle_state)}`,
  `author: ${quote(document.frontmatter.author)}`,
  "---",
].join("\n")

const proseBySection: Record<MandatoryAssuranceSectionId, string> = {
  assurance_objective: "This proposed AssuranceSpec creates exact criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete.",
  subject: "The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.",
  risk_model: "No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.",
  assurance_scope: "Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.",
  environments: "Repository facts are proposal context only. No Environment Profile, adapter, capability, or permission is selected by inventory.",
  obligations: "Each criterion receives one incomplete proposed obligation. Missing proof-design fields project as needs_design and prevent admission or execution.",
  gates: "No execution or release gates are inferred. Gate design remains blocked pending review.",
  evidence_policy: "Links are pointers, not verdicts. Missing or unreviewed evidence remains INCONCLUSIVE.",
  authority_boundaries: "This proposal cannot admit, execute, verify, waive, release, or change a public promise.",
}

const structuredBlock = (name: string, value: unknown): string =>
  `\`\`\`${name}\n${canonicalJson(value)}\`\`\``

const structuredBlockName = (id: MandatoryAssuranceSectionId): string | null => {
  switch (id) {
    case "subject": return "assurancespec-subject"
    case "risk_model": return "assurancespec-risks"
    case "environments": return "assurancespec-environments"
    case "obligations": return "assurancespec-obligations"
    case "gates": return "assurancespec-gates"
    case "evidence_policy": return "assurancespec-evidence-policy"
    case "authority_boundaries": return "assurancespec-authority"
    default: return null
  }
}

const blockValue = (document: AssuranceSpecDocument, id: MandatoryAssuranceSectionId): unknown => {
  switch (id) {
    case "subject": return document.subject
    case "risk_model": return document.riskModel
    case "environments": return document.environments
    case "obligations": return document.obligations
    case "gates": return document.gates
    case "evidence_policy": return document.evidencePolicy
    case "authority_boundaries": return document.authority
    default: return null
  }
}

const narrativeBySection = (
  document: AssuranceSpecDocument,
  id: MandatoryAssuranceSectionId,
): string => {
  const content = document.sections.find((section) => section.id === id)?.content.trim() ?? ""
  const blockName = structuredBlockName(id)
  if (content === "" || blockName === null) return content || proseBySection[id]
  const narrative = content
    .replace(new RegExp(`\n?\`\`\`${blockName}\\n[\\s\\S]*?\\n\`\`\`\n?`, "g"), "\n")
    .trim()
  return narrative || proseBySection[id]
}

export const serializeAssuranceSpec = (document: AssuranceSpecDocument): string => {
  const sections = MANDATORY_ASSURANCE_SECTION_IDS.map((id) => {
    const blockName = structuredBlockName(id)
    const block = blockName === null ? null : structuredBlock(blockName, blockValue(document, id))
    return [
      `## ${ASSURANCE_SECTION_LABELS[id]}`,
      "",
      narrativeBySection(document, id),
      ...(block === null ? [] : ["", block]),
    ].join("\n")
  })
  return `${frontmatter(document)}\n\n${sections.join("\n\n")}\n`
}
