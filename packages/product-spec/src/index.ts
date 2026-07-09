/**
 * @openagentsinc/product-spec — OpenAgents implementation of the ProductSpec
 * open standard (format v0.1) for `.product-spec.md` intent artifacts.
 *
 * Own implementation on Effect Schema; the upstream `@productspec/parser` is a
 * conformance reference only, never a runtime dependency. Compatibility is
 * held by the vendored fixtures under `fixtures/conformance/` (MIT, see
 * ATTRIBUTION.md). Adoption design: docs/fable/2026-07-08-productspec-adoption-analysis.md
 * and specs/CONVENTIONS.md (#8593).
 *
 * Specs declare intent. They never carry enforcement authority: behavior
 * contracts and Eval Suites stay the oracles, and the promise registry stays
 * the only authority for public claims. `tool_metadata` must be stripped on
 * any public export (see stripToolMetadata).
 */
import { Schema as S } from "effect"

export const SPEC_FORMAT_VERSION = "0.1" as const
export const PRODUCT_SPEC_EXTENSION = ".product-spec.md" as const

export const MANDATORY_SECTION_IDS = [
  "problem",
  "hypothesis",
  "scope",
  "acceptance_criteria",
  "success_metrics",
] as const

export const OPTIONAL_SECTION_IDS = [
  "user_experience",
  "customer_truth",
  "solution_alternatives",
  "solution",
  "strategic_positioning",
  "adoption",
  "pricing",
  "risks",
  "ai",
  "open_questions",
  "rollout",
] as const

export const CANONICAL_SECTION_IDS = [
  ...MANDATORY_SECTION_IDS,
  ...OPTIONAL_SECTION_IDS,
] as const

export type MandatorySectionId = (typeof MANDATORY_SECTION_IDS)[number]
export type CanonicalSectionId = (typeof CANONICAL_SECTION_IDS)[number]

/** OpenAgents convention sections (custom-* per the standard's extension rule). */
export const OPENAGENTS_CUSTOM_SECTIONS = [
  { id: "custom-owner-gates", label: "Owner Gates", after: "success_metrics" },
  { id: "custom-receipts", label: "Receipts", after: "custom-owner-gates" },
  { id: "custom-promise-links", label: "Promise Links", after: "custom-receipts" },
] as const

export const SECTION_LABELS: Record<CanonicalSectionId, string> = {
  problem: "Problem",
  hypothesis: "Hypothesis",
  scope: "Scope",
  acceptance_criteria: "Acceptance Criteria",
  success_metrics: "Success Metrics",
  user_experience: "User Experience",
  customer_truth: "Customer Truth",
  solution_alternatives: "Solution Alternatives",
  solution: "Solution",
  strategic_positioning: "Strategic Positioning",
  adoption: "Adoption",
  pricing: "Pricing",
  risks: "Risks",
  ai: "AI Details",
  open_questions: "Open Questions",
  rollout: "Rollout",
}

export const ArtifactType = S.Literals(["hypothesis", "prd", "openspec_proposal"])
export type ArtifactType = typeof ArtifactType.Type

const NonEmptyString = S.String.check(S.isMinLength(1))

export const CustomSectionDeclSchema = S.Struct({
  id: NonEmptyString,
  label: NonEmptyString,
  after: NonEmptyString,
})
export type CustomSectionDecl = typeof CustomSectionDeclSchema.Type

export const FrontmatterSchema = S.Struct({
  spec_format_version: S.Literal(SPEC_FORMAT_VERSION),
  title: NonEmptyString,
  artifact_type: ArtifactType,
  author: NonEmptyString,
  created_at: NonEmptyString,
  updated_at: NonEmptyString,
  spec_revision: S.optionalKey(S.Number),
  linked_github_repo: S.optionalKey(S.String),
  custom_sections: S.optionalKey(S.Array(CustomSectionDeclSchema)),
  tool_metadata: S.optionalKey(S.Record(S.String, S.String)),
})
export type ProductSpecFrontmatter = typeof FrontmatterSchema.Type

export const StructuredScopeSchema = S.Struct({
  in: S.Array(S.String),
  out: S.Array(S.String),
  cut: S.Array(S.String),
})
export type StructuredScope = typeof StructuredScopeSchema.Type

export const AiEvalSchema = S.Struct({
  id: NonEmptyString,
  type: NonEmptyString,
  input_set: NonEmptyString,
  evaluator: NonEmptyString,
  pass_threshold: S.Number,
  checks: S.Array(S.String),
})
export type ProductSpecAiEval = typeof AiEvalSchema.Type

export const SuccessMetricSchema = S.Struct({
  id: NonEmptyString,
  metric: NonEmptyString,
  target: NonEmptyString,
  window: NonEmptyString,
  segment: NonEmptyString,
  source: NonEmptyString,
})
export type ProductSpecSuccessMetric = typeof SuccessMetricSchema.Type

export type ProductSpecSection = {
  id: string
  label: string
  content: string
  scope?: StructuredScope
  ai_evals?: ReadonlyArray<ProductSpecAiEval>
  success_metrics?: ReadonlyArray<ProductSpecSuccessMetric>
}

export type ProductSpecDocument = {
  frontmatter: ProductSpecFrontmatter
  sections: ProductSpecSection[]
}

export type ValidationIssue = {
  code: string
  message: string
  path?: string
}

export type ValidationResult =
  | { valid: true; document: ProductSpecDocument; errors: []; warnings: ValidationIssue[] }
  | { valid: false; document?: undefined; errors: ValidationIssue[]; warnings: ValidationIssue[] }

class SpecParseError extends Error {
  constructor(readonly issue: ValidationIssue) {
    super(issue.message)
    this.name = "SpecParseError"
  }
}

const fail = (code: string, message: string, path?: string): never => {
  throw new SpecParseError(path === undefined ? { code, message } : { code, message, path })
}

// ---------------------------------------------------------------------------
// Frontmatter parsing (deterministic parsing of the bounded YAML-like subset
// the standard defines: scalars, one custom_sections list, one flat
// tool_metadata map — an explicitly modeled parser, per the routing invariant)
// ---------------------------------------------------------------------------

const unquote = (value: string): string => {
  const trimmed = value.trim()
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"') && trimmed.length >= 2) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'") && trimmed.length >= 2)
  ) {
    return trimmed.slice(1, -1)
  }
  return trimmed
}

const KEY_VALUE = /^([A-Za-z0-9_]+):\s*(.*)$/

const parseFrontmatter = (raw: string): ProductSpecFrontmatter => {
  const lines = raw.split("\n")
  const result: Record<string, unknown> = {}

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? ""
    if (!line.trim()) continue

    if (line.startsWith("custom_sections:")) {
      const customSections: Array<Record<string, string>> = []
      while (lines[index + 1]?.startsWith("  - ")) {
        index += 1
        const item: Record<string, string> = {}
        const first = KEY_VALUE.exec((lines[index] ?? "").replace(/^ {2}- /, ""))
        if (first) item[first[1] ?? ""] = unquote(first[2] ?? "")
        while (lines[index + 1]?.startsWith("    ")) {
          index += 1
          const next = KEY_VALUE.exec((lines[index] ?? "").trim())
          if (next) item[next[1] ?? ""] = unquote(next[2] ?? "")
        }
        customSections.push(item)
      }
      result.custom_sections = customSections
      continue
    }

    if (line.startsWith("tool_metadata:")) {
      const metadata: Record<string, string> = {}
      while (lines[index + 1]?.startsWith("  ") && !lines[index + 1]?.startsWith("  - ")) {
        index += 1
        const entry = KEY_VALUE.exec((lines[index] ?? "").trim())
        if (entry) metadata[entry[1] ?? ""] = unquote(entry[2] ?? "")
      }
      result.tool_metadata = metadata
      continue
    }

    const match = KEY_VALUE.exec(line)
    if (match) result[match[1] ?? ""] = unquote(match[2] ?? "")
  }

  if (result.spec_format_version !== SPEC_FORMAT_VERSION) {
    fail(
      "unsupported_version",
      `Unsupported spec_format_version: ${String(result.spec_format_version)}`,
      "frontmatter.spec_format_version",
    )
  }
  for (const field of ["title", "artifact_type", "author", "created_at", "updated_at"]) {
    if (!String(result[field] ?? "").trim()) {
      fail(
        "missing_required_frontmatter",
        `Missing required Product Spec frontmatter: ${field}`,
        "frontmatter",
      )
    }
  }
  if (!["hypothesis", "prd", "openspec_proposal"].includes(String(result.artifact_type))) {
    fail(
      "unsupported_artifact_type",
      `Unsupported artifact_type: ${String(result.artifact_type)}`,
      "frontmatter.artifact_type",
    )
  }
  if (result.spec_revision !== undefined) {
    const revision = Number(result.spec_revision)
    if (!Number.isInteger(revision) || revision < 1) {
      fail(
        "invalid_spec_revision",
        "Invalid spec_revision: use a positive integer.",
        "frontmatter.spec_revision",
      )
    }
    result.spec_revision = revision
  }

  try {
    return S.decodeUnknownSync(FrontmatterSchema)(result)
  } catch (error) {
    return fail(
      "invalid_product_spec",
      `Frontmatter failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
      "frontmatter",
    )
  }
}

// ---------------------------------------------------------------------------
// Section + structured block parsing
// ---------------------------------------------------------------------------

const sectionIdForLabel = (
  label: string,
  customSections: ReadonlyArray<CustomSectionDecl>,
): string => {
  const custom = customSections.find((section) => section.label === label)
  if (custom) return custom.id
  const normalized = label
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_|_$/g, "")
  if (normalized === "ai_details") return "ai"
  if ((CANONICAL_SECTION_IDS as ReadonlyArray<string>).includes(normalized)) return normalized
  return `custom-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`
}

const parseScopeBlocks = (content: string): StructuredScope | undefined => {
  const blocks = [...content.matchAll(/```productspec-scope\n([\s\S]*?)\n```/g)]
  if (blocks.length === 0) return undefined
  const scope: { in: string[]; out: string[]; cut: string[] } = { in: [], out: [], cut: [] }
  for (const block of blocks) {
    let category: "in" | "out" | "cut" | undefined
    for (const line of (block[1] ?? "").split("\n")) {
      if (!line.trim()) continue
      const heading = /^(in|out|cut):$/.exec(line.trim())
      if (heading) {
        category = heading[1] as "in" | "out" | "cut"
        continue
      }
      if (category && line.startsWith("  - ")) {
        scope[category].push(unquote(line.replace(/^ {2}- /, "")))
        continue
      }
      fail("invalid_structured_scope", `Invalid structured scope block line: ${line}`, "sections.scope.scope")
    }
  }
  return scope
}

type RawItemListOptions = {
  code: string
  path: string
  listKey?: string
}

/** Parse `- key: value` item lists with optional nested `checks:` string lists. */
const parseItemList = (
  raw: string,
  options: RawItemListOptions,
): Array<Record<string, unknown>> => {
  const items: Array<Record<string, unknown>> = []
  let current: Record<string, unknown> | undefined
  let inNestedList = false

  for (const line of raw.split("\n")) {
    if (!line.trim()) continue
    if (line.startsWith("- ")) {
      current = {}
      items.push(current)
      inNestedList = false
      const match = KEY_VALUE.exec(line.slice(2))
      if (!match) fail(options.code, `Invalid block line: ${line}`, options.path)
      else current[match[1] ?? ""] = unquote(match[2] ?? "")
      continue
    }
    if (!current) {
      fail(options.code, "Invalid block: expected a list item.", options.path)
      continue
    }
    if (options.listKey && line.trim() === `${options.listKey}:`) {
      current[options.listKey] = []
      inNestedList = true
      continue
    }
    if (inNestedList && options.listKey && line.startsWith("    - ")) {
      ;(current[options.listKey] as string[]).push(unquote(line.replace(/^ {4}- /, "")))
      continue
    }
    if (line.startsWith("  ")) {
      inNestedList = false
      const match = KEY_VALUE.exec(line.trim())
      if (!match) fail(options.code, `Invalid block line: ${line}`, options.path)
      else current[match[1] ?? ""] = unquote(match[2] ?? "")
      continue
    }
    fail(options.code, `Invalid block line: ${line}`, options.path)
  }

  return items
}

const parseAiEvalBlocks = (content: string): ProductSpecAiEval[] =>
  [...content.matchAll(/```productspec-ai-evals\n([\s\S]*?)\n```/g)].flatMap((match) =>
    parseItemList(match[1] ?? "", {
      code: "invalid_ai_eval",
      path: "sections.acceptance_criteria.ai_evals",
      listKey: "checks",
    }).map((item) => ({
      id: String(item.id ?? ""),
      type: String(item.type ?? ""),
      input_set: String(item.input_set ?? ""),
      evaluator: String(item.evaluator ?? ""),
      pass_threshold: Number(item.pass_threshold),
      checks: (item.checks as string[] | undefined) ?? [],
    })),
  )

const parseSuccessMetricBlocks = (content: string): ProductSpecSuccessMetric[] =>
  [...content.matchAll(/```productspec-success-metrics\n([\s\S]*?)\n```/g)].flatMap((match) =>
    parseItemList(match[1] ?? "", {
      code: "invalid_success_metric",
      path: "sections.success_metrics.success_metrics",
    }).map((item) => ({
      id: String(item.id ?? ""),
      metric: String(item.metric ?? ""),
      target: String(item.target ?? ""),
      window: String(item.window ?? ""),
      segment: String(item.segment ?? ""),
      source: String(item.source ?? ""),
    })),
  )

const parseSections = (
  body: string,
  customSections: ReadonlyArray<CustomSectionDecl>,
): ProductSpecSection[] => {
  const matches = [...body.matchAll(/^##\s+(.+)$/gm)]
  return matches.map((match, index) => {
    const label = (match[1] ?? "").trim()
    const start = (match.index ?? 0) + match[0].length
    const end = matches[index + 1]?.index ?? body.length
    const content = body.slice(start, end).trim()
    const scope = parseScopeBlocks(content)
    const ai_evals = parseAiEvalBlocks(content)
    const success_metrics = parseSuccessMetricBlocks(content)
    return {
      id: sectionIdForLabel(label, customSections),
      label,
      content,
      ...(scope ? { scope } : {}),
      ...(ai_evals.length ? { ai_evals } : {}),
      ...(success_metrics.length ? { success_metrics } : {}),
    }
  })
}

// ---------------------------------------------------------------------------
// Parse + validate
// ---------------------------------------------------------------------------

export const parseProductSpec = (markdown: string): ProductSpecDocument => {
  const frontmatterMatch = /^---\n([\s\S]*?)\n---\n?/.exec(markdown)
  if (!frontmatterMatch) {
    fail("missing_frontmatter", "Product Spec frontmatter is required.")
  }
  const frontmatter = parseFrontmatter(frontmatterMatch![1] ?? "")
  const body = markdown.slice(frontmatterMatch![0].length)
  const sections = parseSections(body, frontmatter.custom_sections ?? [])

  for (const sectionId of MANDATORY_SECTION_IDS) {
    if (!sections.some((section) => section.id === sectionId)) {
      fail(
        "missing_required_section",
        `Missing mandatory section: ${sectionId}`,
        `sections.${sectionId}`,
      )
    }
  }

  return { frontmatter, sections }
}

const CUSTOM_SECTION_ID = /^custom-[a-z0-9]+(?:-[a-z0-9]+)*$/
const SNAKE_CASE = /^[a-z0-9]+(?:_[a-z0-9]+)*$/

const validateDocument = (document: ProductSpecDocument): {
  errors: ValidationIssue[]
  warnings: ValidationIssue[]
} => {
  const errors: ValidationIssue[] = []
  const warnings: ValidationIssue[] = []

  const seen = new Set<string>()
  for (const section of document.sections) {
    if (seen.has(section.id)) {
      errors.push({
        code: "duplicate_section",
        message: `Duplicate section: ${section.id}`,
        path: `sections.${section.id}`,
      })
    }
    seen.add(section.id)
  }

  let lastRequiredIndex = -1
  for (const section of document.sections) {
    const requiredIndex = (MANDATORY_SECTION_IDS as ReadonlyArray<string>).indexOf(section.id)
    if (requiredIndex === -1) continue
    if (requiredIndex < lastRequiredIndex) {
      errors.push({
        code: "invalid_section_order",
        message: `Required section out of order: ${section.id}`,
        path: `sections.${section.id}`,
      })
    }
    lastRequiredIndex = Math.max(lastRequiredIndex, requiredIndex)
  }

  for (const custom of document.frontmatter.custom_sections ?? []) {
    if (!CUSTOM_SECTION_ID.test(custom.id)) {
      errors.push({
        code: "invalid_custom_section_id",
        message: `Custom section id must use custom-<kebab-name>: ${custom.id}`,
        path: `frontmatter.custom_sections.${custom.id}`,
      })
    }
  }

  for (const sectionId of MANDATORY_SECTION_IDS) {
    const section = document.sections.find((candidate) => candidate.id === sectionId)
    if (!section) continue
    const meaningful = section.content.replace(/[`*_#>\-\s\d.()[\]]/g, " ").trim()
    const wordCount = meaningful.split(/\s+/).filter(Boolean).length
    if (!meaningful || /^tbd$/i.test(meaningful)) {
      warnings.push({
        code: "empty_required_section",
        message: `Required section has no meaningful content: ${sectionId}`,
        path: `sections.${sectionId}`,
      })
      continue
    }
    if (wordCount < 6) {
      warnings.push({
        code: "thin_required_section",
        message: `Required section is very short: ${sectionId}`,
        path: `sections.${sectionId}`,
      })
    }
  }

  for (const section of document.sections) {
    if (section.scope) {
      const path = `sections.${section.id}.scope`
      if (section.id !== "scope") {
        errors.push({
          code: "invalid_structured_scope",
          message: "Structured scope blocks belong in Scope.",
          path,
        })
      }
      const items = [...section.scope.in, ...section.scope.out, ...section.scope.cut]
      if (items.length === 0 || items.some((item) => !item.trim())) {
        errors.push({
          code: "invalid_structured_scope",
          message: "Invalid structured scope: include at least one non-empty in, out, or cut item.",
          path,
        })
      }
    }

    for (const [index, aiEval] of (section.ai_evals ?? []).entries()) {
      const path = `sections.${section.id}.ai_evals.${index}`
      if (section.id !== "acceptance_criteria") {
        errors.push({
          code: "invalid_ai_eval",
          message: "AI eval blocks belong in Acceptance Criteria.",
          path,
        })
      }
      const missing = (["id", "type", "input_set", "evaluator"] as const).filter(
        (field) => !String(aiEval[field] ?? "").trim(),
      )
      if (missing.length) {
        errors.push({
          code: "invalid_ai_eval",
          message: `Invalid AI eval: missing ${missing.join(", ")}.`,
          path,
        })
      }
      if (
        !Number.isFinite(aiEval.pass_threshold) ||
        aiEval.pass_threshold <= 0 ||
        aiEval.pass_threshold > 1
      ) {
        errors.push({
          code: "invalid_ai_eval",
          message:
            "Invalid AI eval: pass_threshold must be a number greater than 0 and less than or equal to 1.",
          path,
        })
      }
      if (aiEval.checks.length === 0 || aiEval.checks.some((check) => !check.trim())) {
        errors.push({
          code: "invalid_ai_eval",
          message: "Invalid AI eval: checks must include at least one non-empty item.",
          path,
        })
      }
    }

    for (const [index, metric] of (section.success_metrics ?? []).entries()) {
      const path = `sections.${section.id}.success_metrics.${index}`
      if (section.id !== "success_metrics") {
        errors.push({
          code: "invalid_success_metric",
          message: "Structured success metric blocks belong in Success Metrics.",
          path,
        })
      }
      const missing = (["id", "metric", "target", "window", "segment", "source"] as const).filter(
        (field) => !String(metric[field] ?? "").trim(),
      )
      if (missing.length) {
        errors.push({
          code: "invalid_success_metric",
          message: `Invalid success metric: missing ${missing.join(", ")}.`,
          path,
        })
      }
      if (metric.id && !SNAKE_CASE.test(metric.id)) {
        errors.push({
          code: "invalid_success_metric",
          message: "Invalid success metric: id must use snake_case.",
          path,
        })
      }
    }
  }

  return { errors, warnings }
}

export const validateProductSpec = (markdown: string): ValidationResult => {
  try {
    const document = parseProductSpec(markdown)
    const { errors, warnings } = validateDocument(document)
    if (errors.length) return { valid: false, errors, warnings }
    return { valid: true, document, errors: [], warnings }
  } catch (error) {
    if (error instanceof SpecParseError) {
      return { valid: false, errors: [error.issue], warnings: [] }
    }
    return {
      valid: false,
      errors: [
        {
          code: "invalid_product_spec",
          message: error instanceof Error ? error.message : "Invalid Product Spec.",
        },
      ],
      warnings: [],
    }
  }
}

// ---------------------------------------------------------------------------
// OpenAgents helpers
// ---------------------------------------------------------------------------

/**
 * Public-safe export: strip `tool_metadata` (the standard's own default for
 * exports intended for sharing; our projection law makes it mandatory).
 */
export const stripToolMetadata = (markdown: string): string => {
  const frontmatterMatch = /^---\n([\s\S]*?)\n---\n?/.exec(markdown)
  if (!frontmatterMatch) return markdown
  const lines = (frontmatterMatch[1] ?? "").split("\n")
  const kept: string[] = []
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? ""
    if (line.startsWith("tool_metadata:")) {
      while (lines[index + 1]?.startsWith("  ")) index += 1
      continue
    }
    kept.push(line)
  }
  return `---\n${kept.join("\n")}\n---\n${markdown.slice(frontmatterMatch[0].length)}`
}

/** Starter Product Spec with the OpenAgents custom sections pre-stubbed. */
export const starterProductSpec = (options: {
  title: string
  artifactType?: ArtifactType
  author?: string
  now?: string
}): string => {
  const now = options.now ?? new Date().toISOString()
  const artifactType = options.artifactType ?? "prd"
  const author = options.author ?? "OpenAgents"
  const customSections = OPENAGENTS_CUSTOM_SECTIONS.map(
    (section) =>
      `  - id: "${section.id}"\n    label: "${section.label}"\n    after: "${section.after}"`,
  ).join("\n")
  return `---
spec_format_version: "0.1"
title: "${options.title}"
artifact_type: "${artifactType}"
spec_revision: 1
author: "${author}"
created_at: "${now}"
updated_at: "${now}"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
${customSections}
---

## Problem

Who is hurting, what pain do they feel, and why does it matter?

## Hypothesis

If X ships, behavior Y will change because Z.

## Scope

\`\`\`productspec-scope
in:
  - first shipped capability
out:
  - explicitly not this version
cut:
  - considered and deliberately removed
\`\`\`

## Acceptance Criteria

- Pass/fail build gates before launch. Link behavior-contract IDs and Eval
  Suite names — never duplicate their content.

## Success Metrics

\`\`\`productspec-success-metrics
- id: example_metric
  metric: describe_the_real_user_behavior
  target: ">= 1"
  window: within 30 days of launch
  segment: which users or accounts
  source: which exact-only counter or receipt
\`\`\`

## Owner Gates

- NEEDS_OWNER items this work will hit, stated up front.

## Receipts

- Receipt kinds that will prove the acceptance criteria (Eval Suite names,
  behavior-contract IDs, counters).

## Promise Links

- Promise-registry IDs this work feeds. The registry remains the only
  authority for public claims.
`
}
