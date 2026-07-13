import { Schema as S } from "effect"

import {
  ASSURANCE_SECTION_LABELS,
  ASSURANCE_SPEC_FORMAT_VERSION,
  AssuranceAuthoritySchema,
  AssuranceEnvironmentBlockSchema,
  AssuranceEvidencePolicySchema,
  AssuranceGateSchema,
  AssuranceObligationSchema,
  AssuranceRiskBlockSchema,
  AssuranceSpecFrontmatterSchema,
  AssuranceSubjectSchema,
  MANDATORY_ASSURANCE_SECTION_IDS,
  type AssuranceDiagnostic,
  type AssuranceSpecDocument,
  type AssuranceSpecFrontmatter,
  type AssuranceSpecSection,
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

const parseFrontmatter = (raw: string): AssuranceSpecFrontmatter => {
  const result: Record<string, unknown> = {}
  for (const line of raw.split("\n")) {
    if (line.trim() === "") continue
    const match = /^([a-z0-9_]+):\s*(.+)$/.exec(line)
    if (match === null) fail("invalid_frontmatter", `Invalid AssuranceSpec frontmatter line: ${line}`, "frontmatter")
    const key = match![1]!
    if (key in result) fail("duplicate_frontmatter_key", `Duplicate AssuranceSpec frontmatter key: ${key}`, `frontmatter.${key}`)
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
    return S.decodeUnknownSync(AssuranceSpecFrontmatterSchema)(result)
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

const parseSections = (body: string): ReadonlyArray<AssuranceSpecSection> => {
  const matches = [...body.matchAll(/^##\s+(.+?)\s*$/gm)]
  const sections: AssuranceSpecSection[] = []
  const seen = new Set<string>()
  for (let index = 0; index < matches.length; index += 1) {
    const label = matches[index]![1]!.trim()
    const id = labelToId.get(label)
      ?? fail("unsupported_section", `Unsupported AssuranceSpec section: ${label}`, `sections.${label}`)
    if (seen.has(id)) fail("duplicate_section", `Duplicate AssuranceSpec section: ${id}`, `sections.${id}`)
    seen.add(id)
    const start = matches[index]!.index! + matches[index]![0].length
    const end = matches[index + 1]?.index ?? body.length
    sections.push({ id, label, content: body.slice(start, end).trim() })
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
  return sections
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

export const parseAssuranceSpec = (markdown: string): AssuranceSpecDocument => {
  const frontmatterMatch = /^---\n([\s\S]*?)\n---\n?/.exec(markdown)
  if (frontmatterMatch === null) fail("missing_frontmatter", "AssuranceSpec frontmatter is required.")
  const frontmatter = parseFrontmatter(frontmatterMatch![1]!)
  const sections = parseSections(markdown.slice(frontmatterMatch![0].length))
  const section = (id: MandatoryAssuranceSectionId): AssuranceSpecSection => sections.find((candidate) => candidate.id === id)!

  return {
    frontmatter,
    sections,
    subject: decodeBlock(AssuranceSubjectSchema, structuredJson(section("subject"), "assurancespec-subject"), "subject"),
    riskModel: decodeBlock(AssuranceRiskBlockSchema, structuredJson(section("risk_model"), "assurancespec-risks"), "risk_model"),
    environments: decodeBlock(AssuranceEnvironmentBlockSchema, structuredJson(section("environments"), "assurancespec-environments"), "environments"),
    obligations: decodeBlock(S.Array(AssuranceObligationSchema), structuredJson(section("obligations"), "assurancespec-obligations"), "obligations"),
    gates: decodeBlock(S.Array(AssuranceGateSchema), structuredJson(section("gates"), "assurancespec-gates"), "gates"),
    evidencePolicy: decodeBlock(AssuranceEvidencePolicySchema, structuredJson(section("evidence_policy"), "assurancespec-evidence-policy"), "evidence_policy"),
    authority: decodeBlock(AssuranceAuthoritySchema, structuredJson(section("authority_boundaries"), "assurancespec-authority"), "authority_boundaries"),
  }
}
