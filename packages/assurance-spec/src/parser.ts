import { Schema as S } from "effect"

import { analyzeObligationDependencies } from "./graph.ts"
import {
  ASSURANCE_SECTION_LABELS,
  ASSURANCE_SPEC_FORMAT_VERSION,
  ASSURANCE_STRUCTURED_BLOCK_NAMES,
  AssuranceAuthoritySchema,
  AssuranceEnvironmentBlockSchema,
  AssuranceEvidencePolicySchema,
  AssuranceGateSchema,
  AssuranceObligationSchema,
  AssuranceRiskBlockSchema,
  AssuranceSpecFrontmatterSchema,
  AssuranceSubjectSchema,
  CUSTOM_SECTION_ID_PATTERN,
  MANDATORY_ASSURANCE_SECTION_IDS,
  type AssuranceCustomSection,
  type AssuranceDiagnostic,
  type AssuranceSpecDocument,
  type AssuranceSpecFrontmatter,
  type AssuranceSpecSection,
  type AssuranceUnknownFrontmatterEntry,
  type MandatoryAssuranceSectionId,
} from "./schema.ts"

export class AssuranceSpecParseError extends Error {
  constructor(readonly diagnostic: AssuranceDiagnostic) {
    super(diagnostic.message)
    this.name = "AssuranceSpecParseError"
  }
}

const fail = (code: string, message: string, path?: string): never => {
  throw new AssuranceSpecParseError({
    code,
    message,
    severity: "error",
    ...(path === undefined ? {} : { path }),
  })
}

const unquote = (value: string): string => {
  const trimmed = value.trim()
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
    try {
      const decoded = JSON.parse(trimmed)
      if (typeof decoded === "string") return decoded
    } catch {
      return fail("invalid_frontmatter", `Invalid quoted frontmatter value: ${trimmed}`, "frontmatter")
    }
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) return trimmed.slice(1, -1)
  return trimmed
}

/**
 * The bounded profile's known frontmatter keys, derived from the schema so the
 * parser cannot silently drift from it (schema/parser parity is tested).
 */
export const KNOWN_FRONTMATTER_KEYS: ReadonlyArray<string> = Object.keys(
  AssuranceSpecFrontmatterSchema.fields,
)

const knownFrontmatterKeys = new Set(KNOWN_FRONTMATTER_KEYS)

type ParsedFrontmatter = Readonly<{
  frontmatter: AssuranceSpecFrontmatter
  unknownFrontmatter: ReadonlyArray<AssuranceUnknownFrontmatterEntry>
}>

const parseFrontmatter = (raw: string): ParsedFrontmatter => {
  const result: Record<string, unknown> = {}
  const unknownFrontmatter: AssuranceUnknownFrontmatterEntry[] = []
  const seen = new Set<string>()
  for (const line of raw.split("\n")) {
    if (line.trim() === "") continue
    const match = /^([a-z0-9_]+):\s*(.+)$/.exec(line)
    if (match === null) fail("invalid_frontmatter", `Invalid AssuranceSpec frontmatter line: ${line}`, "frontmatter")
    const key = match![1]!
    if (seen.has(key)) fail("duplicate_frontmatter_key", `Duplicate AssuranceSpec frontmatter key: ${key}`, `frontmatter.${key}`)
    seen.add(key)
    if (!knownFrontmatterKeys.has(key)) {
      // Unknown-but-valid metadata is preserved verbatim, never interpreted.
      unknownFrontmatter.push({ key, raw: match![2]!.trim() })
      continue
    }
    const value = unquote(match![2]!)
    result[key] = key === "assurance_revision" ? Number(value) : value
  }
  if (result.assurance_spec_format_version !== ASSURANCE_SPEC_FORMAT_VERSION) {
    fail(
      "unsupported_version",
      `Unsupported assurance_spec_format_version: ${String(result.assurance_spec_format_version)}`,
      "frontmatter.assurance_spec_format_version",
    )
  }
  try {
    return { frontmatter: S.decodeUnknownSync(AssuranceSpecFrontmatterSchema)(result), unknownFrontmatter }
  } catch (error) {
    return fail(
      "invalid_frontmatter",
      `AssuranceSpec frontmatter failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
      "frontmatter",
    )
  }
}

const labelToId = new Map<string, MandatoryAssuranceSectionId>(
  MANDATORY_ASSURANCE_SECTION_IDS.map((id) => [ASSURANCE_SECTION_LABELS[id], id] as const),
)

type ParsedSections = Readonly<{
  sections: ReadonlyArray<AssuranceSpecSection>
  customSections: ReadonlyArray<AssuranceCustomSection>
}>

const parseSections = (body: string): ParsedSections => {
  const matches = [...body.matchAll(/^##\s+(.+?)\s*$/gm)]
  const sections: AssuranceSpecSection[] = []
  const customSections: AssuranceCustomSection[] = []
  const seen = new Set<string>()
  for (let index = 0; index < matches.length; index += 1) {
    const label = matches[index]![1]!.trim()
    const start = matches[index]!.index! + matches[index]![0].length
    const end = matches[index + 1]?.index ?? body.length
    const content = body.slice(start, end).trim()
    const mandatoryId = labelToId.get(label)
    if (mandatoryId !== undefined) {
      if (customSections.length > 0) {
        fail(
          "invalid_section_order",
          `AssuranceSpec section out of order: mandatory section ${mandatoryId} appears after custom section ${customSections[customSections.length - 1]!.id}.`,
          "sections",
        )
      }
      if (seen.has(mandatoryId)) fail("duplicate_section", `Duplicate AssuranceSpec section: ${mandatoryId}`, `sections.${mandatoryId}`)
      seen.add(mandatoryId)
      sections.push({ id: mandatoryId, label, content })
      continue
    }
    if (/^custom-/i.test(label)) {
      if (!CUSTOM_SECTION_ID_PATTERN.test(label)) {
        fail(
          "invalid_custom_section_id",
          `Custom section headings must use custom-<kebab-name>: ${label}`,
          `sections.${label}`,
        )
      }
      if (seen.has(label)) fail("duplicate_section", `Duplicate AssuranceSpec section: ${label}`, `sections.${label}`)
      seen.add(label)
      customSections.push({ id: label, content })
      continue
    }
    fail("unsupported_section", `Unsupported AssuranceSpec section: ${label}`, `sections.${label}`)
  }
  for (const id of MANDATORY_ASSURANCE_SECTION_IDS) {
    if (!seen.has(id)) fail("missing_required_section", `Missing mandatory AssuranceSpec section: ${id}`, `sections.${id}`)
  }
  const actual = sections.map((section) => section.id)
  const outOfOrder = MANDATORY_ASSURANCE_SECTION_IDS.findIndex((id, index) => actual[index] !== id)
  if (outOfOrder !== -1) {
    fail(
      "invalid_section_order",
      `AssuranceSpec section out of order: expected ${MANDATORY_ASSURANCE_SECTION_IDS[outOfOrder]}, received ${String(actual[outOfOrder])}`,
      "sections",
    )
  }
  return { sections, customSections }
}

const structuredJson = (section: AssuranceSpecSection, blockName: string): unknown => {
  const matches = [...section.content.matchAll(new RegExp(`\`\`\`${blockName}\\n([\\s\\S]*?)\\n\`\`\``, "g"))]
  if (matches.length === 0) {
    return fail("missing_structured_block", `Missing ${blockName} block in ${section.id}.`, `sections.${section.id}`)
  }
  if (matches.length > 1) {
    return fail("duplicate_structured_block", `Duplicate ${blockName} block in ${section.id}.`, `sections.${section.id}`)
  }
  try {
    return JSON.parse(matches[0]![1]!)
  } catch (error) {
    return fail(
      "invalid_structured_block",
      `Invalid canonical JSON in ${blockName}: ${error instanceof Error ? error.message : String(error)}`,
      `sections.${section.id}`,
    )
  }
}

const decodeBlock = <A>(
  schema: S.Decoder<A>,
  value: unknown,
  sectionId: MandatoryAssuranceSectionId,
): A => {
  try {
    return S.decodeUnknownSync(schema)(value)
  } catch (error) {
    return fail(
      "invalid_structured_block",
      `Structured block failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
      `sections.${sectionId}`,
    )
  }
}

/**
 * Referential integrity over the parsed model: unique subject criteria and
 * obligation IDs, no dangling criterion/environment/gate references, and
 * exact criterion coverage. Runs at parse time as part of the same pass —
 * `parseAssuranceSpec` throws the first violation, and the validator collects
 * the full set (ASSURANCE_SPEC.md §12.2; GAP_ANALYSIS §2).
 */
export const referentialIntegrityDiagnostics = (
  document: AssuranceSpecDocument,
): ReadonlyArray<AssuranceDiagnostic> => {
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
  for (const issue of analyzeObligationDependencies(document.obligations).issues) {
    errors.push({
      code: issue.code,
      message: issue.message,
      severity: "error",
      path: issue.code === "cyclic_obligation_dependency"
        ? "obligations"
        : `obligations.${issue.obligation_id}.dependency_refs`,
      obligation_id: issue.obligation_id,
    })
  }
  return errors
}

export type AssuranceSpecParseResult = Readonly<{
  document: AssuranceSpecDocument
  /** Referential-integrity errors; empty when the document is structurally valid. */
  integrity: ReadonlyArray<AssuranceDiagnostic>
}>

/**
 * Format-plane parse plus the referential-integrity pass, without throwing on
 * integrity violations. The validator uses this to report the complete error
 * set; format errors still throw `AssuranceSpecParseError`.
 */
export const parseAssuranceSpecDocument = (markdown: string): AssuranceSpecParseResult => {
  const frontmatterMatch = /^---\n([\s\S]*?)\n---\n?/.exec(markdown)
  if (frontmatterMatch === null) fail("missing_frontmatter", "AssuranceSpec frontmatter is required.")
  const { frontmatter, unknownFrontmatter } = parseFrontmatter(frontmatterMatch![1]!)
  const { sections, customSections } = parseSections(markdown.slice(frontmatterMatch![0].length))
  const section = (id: MandatoryAssuranceSectionId): AssuranceSpecSection => sections.find((candidate) => candidate.id === id)!
  const block = (id: keyof typeof ASSURANCE_STRUCTURED_BLOCK_NAMES): unknown =>
    structuredJson(section(id), ASSURANCE_STRUCTURED_BLOCK_NAMES[id])

  const document: AssuranceSpecDocument = {
    frontmatter,
    unknownFrontmatter,
    sections,
    customSections,
    subject: decodeBlock(AssuranceSubjectSchema, block("subject"), "subject"),
    riskModel: decodeBlock(AssuranceRiskBlockSchema, block("risk_model"), "risk_model"),
    environments: decodeBlock(AssuranceEnvironmentBlockSchema, block("environments"), "environments"),
    obligations: decodeBlock(S.Array(AssuranceObligationSchema), block("obligations"), "obligations"),
    gates: decodeBlock(S.Array(AssuranceGateSchema), block("gates"), "gates"),
    evidencePolicy: decodeBlock(AssuranceEvidencePolicySchema, block("evidence_policy"), "evidence_policy"),
    authority: decodeBlock(AssuranceAuthoritySchema, block("authority_boundaries"), "authority_boundaries"),
  }
  return { document, integrity: referentialIntegrityDiagnostics(document) }
}

export const parseAssuranceSpec = (markdown: string): AssuranceSpecDocument => {
  const { document, integrity } = parseAssuranceSpecDocument(markdown)
  const first = integrity[0]
  if (first !== undefined) throw new AssuranceSpecParseError(first)
  return document
}
