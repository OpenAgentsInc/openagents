/**
 * @openagentsinc/product-spec — OpenAgents implementation of the ProductSpec
 * open standard (format v0.1) for `.product-spec.md` intent artifacts.
 *
 * Own implementation on Effect Schema; the upstream `@productspec/parser` is a
 * conformance reference only, never a runtime dependency. Compatibility is
 * held by the vendored fixtures under `fixtures/conformance/` (MIT, see
 * ATTRIBUTION.md), pinned to the upstream commit named in
 * UPSTREAM_COMPATIBILITY below. Adoption design:
 * docs/fable/2026-07-08-productspec-adoption-analysis.md and
 * specs/CONVENTIONS.md (#8593); evidence-loop and dual-digest design:
 * docs/assurance/PRODUCTSPEC_EVIDENCE_LOOP.md and
 * docs/assurance/ASSURANCE_SPEC.md §4 (PSEL-0/PSEL-1, #8757).
 *
 * Specs declare intent. They never carry enforcement authority: behavior
 * contracts and Eval Suites stay the oracles, and the promise registry stays
 * the only authority for public claims. `tool_metadata` must be stripped on
 * any public export (see stripToolMetadata).
 */
import { createHash } from "node:crypto"
import { readFileSync, writeFileSync } from "node:fs"
import { Schema as S } from "effect"

export const SPEC_FORMAT_VERSION = "0.1" as const
export const PRODUCT_SPEC_EXTENSION = ".product-spec.md" as const

/**
 * The pinned upstream compatibility target. PSEL policy pins one upstream
 * release per deliberate re-vendor instead of chasing upstream minors
 * reactively. This constant is the exact statement of which upstream
 * semantics the `upstream` validation profile implements.
 */
export const UPSTREAM_COMPATIBILITY = {
  package: "@productspec/parser",
  version: "0.19.0",
  commit: "9ef2654bdd01aef3985fef6ed5a9ab66365999e1",
  supported: [
    "frontmatter and section model (format 0.1)",
    "applies_to frontmatter",
    "custom_sections and tool_metadata",
    "productspec-scope blocks",
    "productspec-acceptance-criteria items (AC-<n>)",
    "productspec-ai-evals items (EVAL-<n>, cases + checks)",
    "productspec-success-metrics items (SM-<n>, target_status/target_owner)",
    "productspec-related-artifacts (typed, dangling-ID errors, unusual-target warnings)",
    "duplicate item-id detection across AC/EVAL/SM",
    "fence-aware section heading and block extraction",
    "ISO 8601 created_at/updated_at validation",
    "markdown serialization round trips",
  ],
  unsupported: [
    "Decision Trace JSON validation",
    "ProductSpec dependency graph resolution",
    "MCP evidence checklist / session behavior",
  ],
} as const

/**
 * Validation profiles.
 *
 * - `openagents` (default): the OpenAgents local profile. Preserves the
 *   pre-PSEL-1 behavior for legacy documents (prose acceptance criteria
 *   allowed; success metrics may use the OpenAgents shape with snake_case
 *   ids plus `segment`/`source`) while validating every upstream structured
 *   construct strictly when a document uses it.
 * - `upstream`: the pinned upstream `0.19.0` semantics. Structured
 *   `productspec-acceptance-criteria` and `productspec-success-metrics`
 *   items are mandatory, ids must use `AC-<n>`/`EVAL-<n>`/`SM-<n>`, and the
 *   OpenAgents-only success-metric fields are rejected.
 *
 * There is no silent ID aliasing between profiles: a legacy `CW-AC-*`
 * document is simply not valid under `upstream`, and the incompatibility is
 * recorded as typed errors (see the package tests).
 */
export type ProductSpecProfile = "openagents" | "upstream"
export const PRODUCT_SPEC_PROFILES = ["openagents", "upstream"] as const

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
  "related_artifacts",
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
  related_artifacts: "Related Artifacts",
}

export const ArtifactType = S.Literals(["hypothesis", "prd", "openspec_proposal"])
export type ArtifactType = typeof ArtifactType.Type

export const AI_EVAL_TYPES = [
  "exact_match",
  "contains",
  "regex",
  "llm_judge",
  "human_review",
] as const
export type AiEvalType = (typeof AI_EVAL_TYPES)[number]

export const AI_EVAL_EVALUATORS = ["deterministic", "llm", "human"] as const
export type AiEvalEvaluator = (typeof AI_EVAL_EVALUATORS)[number]

export const RELATED_ARTIFACT_TYPES = [
  "github_issue",
  "github_pr",
  "jira_issue",
  "linear_issue",
  "figma",
  "engineering_spec",
  "eval_run",
  "dashboard",
  "analytics_snapshot",
  "experiment",
  "release",
  "code",
  "product_spec",
  "other",
] as const
export type RelatedArtifactType = (typeof RELATED_ARTIFACT_TYPES)[number]

export const RELATED_ARTIFACT_RELATIONS = [
  "depends_on",
  "blocks",
  "supersedes",
  "relates_to",
] as const
export type RelatedArtifactRelation = (typeof RELATED_ARTIFACT_RELATIONS)[number]

const NonEmptyString = S.String.check(S.isMinLength(1))

export const CustomSectionDeclSchema = S.Struct({
  id: NonEmptyString,
  label: NonEmptyString,
  after: NonEmptyString,
})
export type CustomSectionDecl = typeof CustomSectionDeclSchema.Type

/** `applies_to` entry: exactly one of `path` or `component` (validated). */
export const AppliesToSchema = S.Struct({
  path: S.optionalKey(S.String),
  component: S.optionalKey(S.String),
})
export type ProductSpecAppliesTo = typeof AppliesToSchema.Type

export const FrontmatterSchema = S.Struct({
  spec_format_version: S.Literal(SPEC_FORMAT_VERSION),
  title: NonEmptyString,
  artifact_type: ArtifactType,
  author: NonEmptyString,
  created_at: NonEmptyString,
  updated_at: NonEmptyString,
  spec_revision: S.optionalKey(S.Number),
  linked_github_repo: S.optionalKey(S.String),
  applies_to: S.optionalKey(S.Array(AppliesToSchema)),
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

/** Structured `productspec-acceptance-criteria` item (upstream `AC-<n>`). */
export const AcceptanceCriterionItemSchema = S.Struct({
  id: NonEmptyString,
  criterion: NonEmptyString,
})
export type ProductSpecAcceptanceCriterionItem = typeof AcceptanceCriterionItemSchema.Type

export const AiEvalCaseSchema = S.Struct({
  input: S.String,
  expected: S.String,
})
export type ProductSpecAiEvalCase = typeof AiEvalCaseSchema.Type

/**
 * AI eval item. Superset of the upstream 0.19.0 shape (`cases` + enum
 * `type`/`evaluator`) and the legacy OpenAgents shape (`input_set` + free
 * `type`/`evaluator`). The validation profile decides which rules apply.
 */
export const AiEvalSchema = S.Struct({
  id: NonEmptyString,
  type: NonEmptyString,
  evaluator: NonEmptyString,
  pass_threshold: S.Number,
  checks: S.Array(S.String),
  cases: S.optionalKey(S.Array(AiEvalCaseSchema)),
  input_set: S.optionalKey(S.String),
})
export type ProductSpecAiEval = typeof AiEvalSchema.Type

/**
 * Success metric item. Superset of the upstream 0.19.0 shape
 * (`target_status`/`target_owner`) and the legacy OpenAgents shape
 * (`segment`/`source`). The validation profile decides which rules apply.
 */
export const SuccessMetricSchema = S.Struct({
  id: NonEmptyString,
  metric: NonEmptyString,
  target: NonEmptyString,
  window: NonEmptyString,
  target_status: S.optionalKey(S.Literals(["committed", "provisional"])),
  target_owner: S.optionalKey(S.String),
  segment: S.optionalKey(S.String),
  source: S.optionalKey(S.String),
})
export type ProductSpecSuccessMetric = typeof SuccessMetricSchema.Type

export const RelatedArtifactSchema = S.Struct({
  type: NonEmptyString,
  url: S.optionalKey(S.String),
  title: S.optionalKey(S.String),
  section_id: S.optionalKey(S.String),
  item_id: S.optionalKey(S.String),
  product_spec_path: S.optionalKey(S.String),
  product_spec_revision: S.optionalKey(S.Number),
  relation: S.optionalKey(S.String),
})
export type ProductSpecRelatedArtifact = typeof RelatedArtifactSchema.Type

export type ProductSpecSection = {
  id: string
  label: string
  content: string
  scope?: StructuredScope
  acceptance_criteria?: ReadonlyArray<ProductSpecAcceptanceCriterionItem>
  ai_evals?: ReadonlyArray<ProductSpecAiEval>
  success_metrics?: ReadonlyArray<ProductSpecSuccessMetric>
  related_artifacts?: ReadonlyArray<ProductSpecRelatedArtifact>
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

/**
 * Author-visible criterion identity required by the OpenAgents executable
 * ProductSpec profile. The base ProductSpec format intentionally permits prose
 * acceptance criteria; OpenAgents keeps those documents viewable but refuses
 * to dispatch work until every top-level criterion bullet has one unique ID.
 */
export type ProductSpecAcceptanceCriterion = Readonly<{
  id: string
  body: string
  ordinal: number
}>

export type ExecutableProductSpecResult =
  | Readonly<{
      executable: true
      document: ProductSpecDocument
      criteria: ReadonlyArray<ProductSpecAcceptanceCriterion>
      errors: readonly []
      warnings: ReadonlyArray<ValidationIssue>
    }>
  | Readonly<{
      executable: false
      document?: ProductSpecDocument
      criteria: ReadonlyArray<ProductSpecAcceptanceCriterion>
      errors: ReadonlyArray<ValidationIssue>
      warnings: ReadonlyArray<ValidationIssue>
    }>

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
// the standard defines: scalars, one custom_sections list, one applies_to
// list, one flat tool_metadata map — an explicitly modeled parser, per the
// routing invariant)
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

    if (line.startsWith("applies_to:")) {
      const appliesTo: Array<Record<string, string>> = []
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
        appliesTo.push(item)
      }
      result.applies_to = appliesTo.map((item) => ({
        ...(item.path !== undefined ? { path: item.path } : {}),
        ...(item.component !== undefined ? { component: item.component } : {}),
      }))
      continue
    }

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
// Fence-aware body scanning (mirrors the upstream reference: headings inside
// fenced code blocks never split sections, and structured blocks accept
// backtick or tilde fences of length >= 3 with up to three leading spaces)
// ---------------------------------------------------------------------------

const FENCE_OPEN = /^ {0,3}(`{3,}|~{3,})/

const fencedRanges = (body: string): Array<[number, number]> => {
  const ranges: Array<[number, number]> = []
  let offset = 0
  let openMarker: string | null = null
  let openStart = 0

  for (const line of body.split("\n")) {
    const fence = FENCE_OPEN.exec(line)
    if (openMarker === null) {
      if (fence) {
        openMarker = fence[1] ?? ""
        openStart = offset
      }
    } else if (
      fence &&
      (fence[1] ?? "")[0] === openMarker[0] &&
      (fence[1] ?? "").length >= openMarker.length &&
      line.slice(fence[0].length).trim() === ""
    ) {
      ranges.push([openStart, offset + line.length])
      openMarker = null
    }
    offset += line.length + 1
  }

  if (openMarker !== null) ranges.push([openStart, body.length])
  return ranges
}

/** Extract the bodies of ```<info> fenced blocks (fence-aware, per upstream). */
const productSpecBlocks = (content: string, info: string): string[] => {
  const blocks: string[] = []
  let openFence: string | undefined
  let body: string[] = []

  for (const line of content.split("\n")) {
    if (openFence === undefined) {
      const open = /^ {0,3}(`{3,}|~{3,})([A-Za-z0-9_-]+)[ \t]*$/.exec(line)
      if (open?.[2] === info) {
        openFence = open[1] ?? ""
        body = []
      }
      continue
    }

    const close = /^ {0,3}(`{3,}|~{3,})[ \t]*$/.exec(line)
    if (close && (close[1] ?? "")[0] === openFence[0] && (close[1] ?? "").length >= openFence.length) {
      blocks.push(body.join("\n"))
      openFence = undefined
      body = []
      continue
    }

    body.push(line)
  }

  return blocks
}

/** Remove every ```<info> fenced block (fence line to fence line) from content. */
const stripProductSpecBlocks = (content: string, info: string): string => {
  const kept: string[] = []
  let openFence: string | undefined

  for (const line of content.split("\n")) {
    if (openFence === undefined) {
      const open = /^ {0,3}(`{3,}|~{3,})([A-Za-z0-9_-]+)[ \t]*$/.exec(line)
      if (open?.[2] === info) {
        openFence = open[1] ?? ""
        continue
      }
      kept.push(line)
      continue
    }

    const close = /^ {0,3}(`{3,}|~{3,})[ \t]*$/.exec(line)
    if (close && (close[1] ?? "")[0] === openFence[0] && (close[1] ?? "").length >= openFence.length) {
      openFence = undefined
    }
  }

  return kept.join("\n").replace(/\n{3,}/g, "\n\n").trim()
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
  const blocks = productSpecBlocks(content, "productspec-scope")
  if (blocks.length === 0) return undefined
  const scope: { in: string[]; out: string[]; cut: string[] } = { in: [], out: [], cut: [] }
  for (const block of blocks) {
    let category: "in" | "out" | "cut" | undefined
    for (const line of block.split("\n")) {
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

type ItemListOptions = {
  code: string
  path: string
  /** Keys accepted as `key: value` fields on an item; others are parse errors. */
  allowedKeys?: ReadonlyArray<string>
  /** Keys that introduce a nested `    - value` string list. */
  stringListKeys?: ReadonlyArray<string>
  /** Keys that introduce a nested `    - key: value` object list. */
  objectListKeys?: ReadonlyArray<string>
}

/**
 * Parse `- key: value` item lists with optional nested string lists (such as
 * `checks:`) and nested object lists (such as AI eval `cases:`).
 */
const parseItemList = (
  raw: string,
  options: ItemListOptions,
): Array<Record<string, unknown>> => {
  const items: Array<Record<string, unknown>> = []
  let current: Record<string, unknown> | undefined
  let nestedKey: string | undefined
  let nestedMode: "strings" | "objects" | undefined
  let currentObject: Record<string, string> | undefined

  const assign = (target: Record<string, unknown>, line: string): void => {
    const match = KEY_VALUE.exec(line)
    if (!match) {
      fail(options.code, `Invalid block line: ${line}`, options.path)
      return
    }
    const key = match[1] ?? ""
    if (options.allowedKeys && !options.allowedKeys.includes(key)) {
      fail(options.code, `Invalid block field: ${key}`, options.path)
      return
    }
    target[key] = unquote(match[2] ?? "")
  }

  for (const line of raw.split("\n")) {
    if (!line.trim()) continue
    if (line.startsWith("- ")) {
      current = {}
      items.push(current)
      nestedKey = undefined
      nestedMode = undefined
      currentObject = undefined
      assign(current, line.slice(2))
      continue
    }
    if (!current) {
      fail(options.code, "Invalid block: expected a list item.", options.path)
      continue
    }
    const bareKey = /^([A-Za-z0-9_]+):$/.exec(line.trim())?.[1]
    if (bareKey && options.stringListKeys?.includes(bareKey)) {
      current[bareKey] = []
      nestedKey = bareKey
      nestedMode = "strings"
      currentObject = undefined
      continue
    }
    if (bareKey && options.objectListKeys?.includes(bareKey)) {
      current[bareKey] = []
      nestedKey = bareKey
      nestedMode = "objects"
      currentObject = undefined
      continue
    }
    if (nestedMode === "strings" && nestedKey && line.startsWith("    - ")) {
      ;(current[nestedKey] as string[]).push(unquote(line.replace(/^ {4}- /, "")))
      continue
    }
    if (nestedMode === "objects" && nestedKey && line.startsWith("    - ")) {
      currentObject = {}
      ;(current[nestedKey] as Array<Record<string, string>>).push(currentObject)
      const match = KEY_VALUE.exec(line.replace(/^ {4}- /, ""))
      if (!match) fail(options.code, `Invalid block line: ${line}`, options.path)
      else currentObject[match[1] ?? ""] = unquote(match[2] ?? "")
      continue
    }
    if (nestedMode === "objects" && currentObject && line.startsWith("      ")) {
      const match = KEY_VALUE.exec(line.trim())
      if (!match) fail(options.code, `Invalid block line: ${line}`, options.path)
      else currentObject[match[1] ?? ""] = unquote(match[2] ?? "")
      continue
    }
    if (line.startsWith("  ")) {
      nestedKey = undefined
      nestedMode = undefined
      currentObject = undefined
      assign(current, line.trim())
      continue
    }
    fail(options.code, `Invalid block line: ${line}`, options.path)
  }

  return items
}

const parseAcceptanceCriterionBlocks = (
  content: string,
): ProductSpecAcceptanceCriterionItem[] =>
  productSpecBlocks(content, "productspec-acceptance-criteria").flatMap((block) =>
    parseItemList(block, {
      code: "invalid_acceptance_criterion",
      path: "sections.acceptance_criteria.acceptance_criteria",
      allowedKeys: ["id", "criterion"],
    }).map((item) => ({
      id: String(item.id ?? ""),
      criterion: String(item.criterion ?? ""),
    })),
  )

const parseAiEvalBlocks = (content: string): ProductSpecAiEval[] =>
  productSpecBlocks(content, "productspec-ai-evals").flatMap((block) =>
    parseItemList(block, {
      code: "invalid_ai_eval",
      path: "sections.acceptance_criteria.ai_evals",
      stringListKeys: ["checks"],
      objectListKeys: ["cases"],
    }).map((item) => ({
      id: String(item.id ?? ""),
      type: String(item.type ?? ""),
      evaluator: String(item.evaluator ?? ""),
      pass_threshold: Number(item.pass_threshold),
      checks: (item.checks as string[] | undefined) ?? [],
      ...(item.cases !== undefined
        ? {
            cases: (item.cases as Array<Record<string, string>>).map((entry) => ({
              input: String(entry.input ?? ""),
              expected: String(entry.expected ?? ""),
            })),
          }
        : {}),
      ...(item.input_set !== undefined ? { input_set: String(item.input_set) } : {}),
    })),
  )

const SUCCESS_METRIC_KEYS = [
  "id",
  "metric",
  "target",
  "target_status",
  "target_owner",
  "window",
  // OpenAgents legacy-profile extension fields (not upstream):
  "segment",
  "source",
] as const

const parseSuccessMetricBlocks = (content: string): ProductSpecSuccessMetric[] =>
  productSpecBlocks(content, "productspec-success-metrics").flatMap((block) =>
    parseItemList(block, {
      code: "invalid_success_metric",
      path: "sections.success_metrics.success_metrics",
      allowedKeys: SUCCESS_METRIC_KEYS,
    }).map((item) => ({
      id: String(item.id ?? ""),
      metric: String(item.metric ?? ""),
      target: String(item.target ?? ""),
      window: String(item.window ?? ""),
      ...(item.target_status !== undefined
        ? { target_status: String(item.target_status) as "committed" | "provisional" }
        : {}),
      ...(item.target_owner !== undefined ? { target_owner: String(item.target_owner) } : {}),
      ...(item.segment !== undefined ? { segment: String(item.segment) } : {}),
      ...(item.source !== undefined ? { source: String(item.source) } : {}),
    })),
  )

const RELATED_ARTIFACT_KEYS = [
  "type",
  "url",
  "title",
  "section_id",
  "item_id",
  "product_spec_path",
  "product_spec_revision",
  "relation",
] as const

const parseRelatedArtifactBlocks = (content: string): ProductSpecRelatedArtifact[] =>
  productSpecBlocks(content, "productspec-related-artifacts").flatMap((block) =>
    parseItemList(block, {
      code: "invalid_related_artifact",
      path: "sections.related_artifacts.related_artifacts",
      allowedKeys: RELATED_ARTIFACT_KEYS,
    }).map((item) => ({
      type: String(item.type ?? ""),
      ...(item.url !== undefined ? { url: String(item.url) } : {}),
      ...(item.title !== undefined ? { title: String(item.title) } : {}),
      ...(item.section_id !== undefined ? { section_id: String(item.section_id) } : {}),
      ...(item.item_id !== undefined ? { item_id: String(item.item_id) } : {}),
      ...(item.product_spec_path !== undefined
        ? { product_spec_path: String(item.product_spec_path) }
        : {}),
      ...(item.product_spec_revision !== undefined
        ? { product_spec_revision: Number(item.product_spec_revision) }
        : {}),
      // Upstream defaults product_spec dependencies to relates_to.
      ...(item.relation !== undefined
        ? { relation: String(item.relation) }
        : String(item.type ?? "") === "product_spec"
          ? { relation: "relates_to" }
          : {}),
    })),
  )

const parseSections = (
  body: string,
  customSections: ReadonlyArray<CustomSectionDecl>,
): ProductSpecSection[] => {
  const fenced = fencedRanges(body)
  const matches = [...body.matchAll(/^##\s+(.+)$/gm)].filter((match) => {
    const index = match.index ?? 0
    return !fenced.some(([start, end]) => index >= start && index < end)
  })
  return matches.map((match, index) => {
    const label = (match[1] ?? "").trim()
    const start = (match.index ?? 0) + match[0].length
    const end = matches[index + 1]?.index ?? body.length
    const content = body.slice(start, end).trim()
    const scope = parseScopeBlocks(content)
    const acceptance_criteria = parseAcceptanceCriterionBlocks(content)
    const ai_evals = parseAiEvalBlocks(content)
    const success_metrics = parseSuccessMetricBlocks(content)
    const related_artifacts = parseRelatedArtifactBlocks(content)
    return {
      id: sectionIdForLabel(label, customSections),
      label,
      content,
      ...(scope ? { scope } : {}),
      ...(acceptance_criteria.length ? { acceptance_criteria } : {}),
      ...(ai_evals.length ? { ai_evals } : {}),
      ...(success_metrics.length ? { success_metrics } : {}),
      ...(related_artifacts.length ? { related_artifacts } : {}),
    }
  })
}

// ---------------------------------------------------------------------------
// Parse + validate
// ---------------------------------------------------------------------------

export const parseProductSpec = (markdown: string): ProductSpecDocument => {
  const normalized = markdown.replace(/\r\n/g, "\n")
  const frontmatterMatch = /^---\n([\s\S]*?)\n---\n?/.exec(normalized)
  if (!frontmatterMatch) {
    fail("missing_frontmatter", "Product Spec frontmatter is required.")
  }
  const frontmatter = parseFrontmatter(frontmatterMatch![1] ?? "")
  const body = normalized.slice(frontmatterMatch![0].length)
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
const AC_ITEM_ID = /^AC-[1-9]\d*$/
const EVAL_ITEM_ID = /^EVAL-[1-9]\d*$/
const SM_ITEM_ID = /^SM-[1-9]\d*$/
const ANY_ITEM_ID = /^(AC|SM|EVAL)-[1-9]\d*$/
const ISO_DATETIME =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/

const isIsoDateTime = (value: string): boolean =>
  ISO_DATETIME.test(value) && !Number.isNaN(Date.parse(value))

const expectedItemIdPrefixForArtifact = (type: string): "AC" | "EVAL" | "SM" | undefined => {
  switch (type) {
    case "github_pr":
    case "code":
    case "release":
    case "engineering_spec":
      return "AC"
    case "eval_run":
      return "EVAL"
    case "dashboard":
    case "analytics_snapshot":
    case "experiment":
      return "SM"
    default:
      return undefined
  }
}

/**
 * Deterministic per-item dialect selection under the `openagents` profile:
 * an item written with the upstream 0.19.0 vocabulary is validated with the
 * upstream rules, a legacy OpenAgents item keeps the legacy rules. This is a
 * validation-rule selection only — it never rewrites or aliases IDs.
 */
const isUpstreamSuccessMetric = (metric: ProductSpecSuccessMetric): boolean =>
  metric.target_status !== undefined ||
  metric.target_owner !== undefined ||
  SM_ITEM_ID.test(metric.id)

const isUpstreamAiEval = (aiEval: ProductSpecAiEval): boolean =>
  aiEval.cases !== undefined || EVAL_ITEM_ID.test(aiEval.id)

const validateDocument = (
  document: ProductSpecDocument,
  profile: ProductSpecProfile,
): {
  errors: ValidationIssue[]
  warnings: ValidationIssue[]
} => {
  const errors: ValidationIssue[] = []
  const warnings: ValidationIssue[] = []
  const upstream = profile === "upstream"

  for (const field of ["created_at", "updated_at"] as const) {
    if (!isIsoDateTime(document.frontmatter[field])) {
      errors.push({
        code: "invalid_datetime",
        message: `Invalid Product Spec date-time: ${field} must be ISO 8601.`,
        path: `frontmatter.${field}`,
      })
    }
  }

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

  for (const [index, appliesTo] of (document.frontmatter.applies_to ?? []).entries()) {
    const hasPath = Boolean(appliesTo.path?.trim())
    const hasComponent = Boolean(appliesTo.component?.trim())
    if (hasPath === hasComponent) {
      errors.push({
        code: "invalid_applies_to",
        message: "Invalid applies_to: each item must include exactly one non-empty path or component.",
        path: `frontmatter.applies_to.${index}`,
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

  // Item ids are unique across AC, EVAL, and SM items (upstream semantics).
  const seenItemIds = new Set<string>()
  const recordItemId = (id: string, path: string): void => {
    if (!id) return
    if (seenItemIds.has(id)) {
      errors.push({
        code: "duplicate_item_id",
        message: `Duplicate Product Spec item id: ${id}.`,
        path,
      })
      return
    }
    seenItemIds.add(id)
  }

  const knownItemIds = new Set(
    document.sections.flatMap((section) => [
      ...(section.acceptance_criteria ?? []).map((criterion) => criterion.id),
      ...(section.ai_evals ?? []).map((aiEval) => aiEval.id),
      ...(section.success_metrics ?? []).map((metric) => metric.id),
    ]),
  )

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

    for (const [index, criterion] of (section.acceptance_criteria ?? []).entries()) {
      const path = `sections.${section.id}.acceptance_criteria.${index}`
      if (section.id !== "acceptance_criteria") {
        errors.push({
          code: "invalid_acceptance_criterion",
          message: "Structured acceptance criteria blocks belong in Acceptance Criteria.",
          path,
        })
      }
      const missing = (["id", "criterion"] as const).filter(
        (field) => !String(criterion[field] ?? "").trim(),
      )
      if (missing.length) {
        errors.push({
          code: "invalid_acceptance_criterion",
          message: `Invalid acceptance criterion: missing ${missing.join(", ")}.`,
          path,
        })
      }
      if (criterion.id && !AC_ITEM_ID.test(criterion.id)) {
        errors.push({
          code: "invalid_acceptance_criterion",
          message: "Invalid acceptance criterion: id must use AC-<number>.",
          path,
        })
      }
      recordItemId(criterion.id, `${path}.id`)
    }

    if (
      upstream &&
      section.id === "acceptance_criteria" &&
      !section.acceptance_criteria?.length
    ) {
      errors.push({
        code: "invalid_acceptance_criterion",
        message:
          "Invalid acceptance criterion: include at least one productspec-acceptance-criteria item.",
        path: "sections.acceptance_criteria.acceptance_criteria",
      })
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

      if (upstream || isUpstreamAiEval(aiEval)) {
        const missing = (["id", "type", "evaluator"] as const).filter(
          (field) => !String(aiEval[field] ?? "").trim(),
        )
        if (missing.length) {
          errors.push({
            code: "invalid_ai_eval",
            message: `Invalid AI eval: missing ${missing.join(", ")}.`,
            path,
          })
        }
        if (aiEval.id && !EVAL_ITEM_ID.test(aiEval.id)) {
          errors.push({
            code: "invalid_ai_eval",
            message: "Invalid AI eval: id must use EVAL-<number>.",
            path,
          })
        }
        if (aiEval.type && !(AI_EVAL_TYPES as ReadonlyArray<string>).includes(aiEval.type)) {
          errors.push({
            code: "invalid_ai_eval",
            message: `Invalid AI eval: type must be one of ${AI_EVAL_TYPES.join(", ")}.`,
            path,
          })
        }
        if (
          aiEval.evaluator &&
          !(AI_EVAL_EVALUATORS as ReadonlyArray<string>).includes(aiEval.evaluator)
        ) {
          errors.push({
            code: "invalid_ai_eval",
            message: `Invalid AI eval: evaluator must be one of ${AI_EVAL_EVALUATORS.join(", ")}.`,
            path,
          })
        }
        const cases = aiEval.cases ?? []
        if (
          cases.length === 0 ||
          cases.some((testCase) => !testCase.input.trim() || !testCase.expected.trim())
        ) {
          errors.push({
            code: "invalid_ai_eval",
            message:
              "Invalid AI eval: cases must include at least one item with non-empty input and expected values.",
            path,
          })
        }
        if (aiEval.checks.some((check) => !check.trim())) {
          errors.push({
            code: "invalid_ai_eval",
            message: "Invalid AI eval: checks must be non-empty when provided.",
            path,
          })
        }
        if (aiEval.checks.some((check) => /^id\s*:/i.test(check.trim()))) {
          errors.push({
            code: "invalid_ai_eval",
            message: "Invalid AI eval: checks do not use standalone IDs.",
            path,
          })
        }
      } else {
        // OpenAgents legacy dialect (pre-PSEL-1): free-form type/evaluator,
        // input_set required, at least one non-empty check.
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
        if (aiEval.checks.length === 0 || aiEval.checks.some((check) => !check.trim())) {
          errors.push({
            code: "invalid_ai_eval",
            message: "Invalid AI eval: checks must include at least one non-empty item.",
            path,
          })
        }
      }
      recordItemId(aiEval.id, `${path}.id`)
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

      if (upstream || isUpstreamSuccessMetric(metric)) {
        const missing = (["id", "metric", "target", "window"] as const).filter(
          (field) => !String(metric[field] ?? "").trim(),
        )
        if (missing.length) {
          errors.push({
            code: "invalid_success_metric",
            message: `Invalid success metric: missing ${missing.join(", ")}.`,
            path,
          })
        }
        if (metric.id && !SM_ITEM_ID.test(metric.id)) {
          errors.push({
            code: "invalid_success_metric",
            message: "Invalid success metric: id must use SM-<number>.",
            path,
          })
        }
        const targetStatus = metric.target_status ?? "committed"
        if (!["committed", "provisional"].includes(targetStatus)) {
          errors.push({
            code: "invalid_success_metric",
            message: "Invalid success metric: target_status must be committed or provisional.",
            path,
          })
        }
        if (targetStatus === "provisional" && !metric.target_owner?.trim()) {
          errors.push({
            code: "invalid_success_metric",
            message: "Invalid success metric: provisional targets require target_owner.",
            path,
          })
        }
        if (upstream) {
          for (const field of ["segment", "source"] as const) {
            if (metric[field] !== undefined) {
              errors.push({
                code: "invalid_success_metric",
                message: `Invalid success metric field: ${field}.`,
                path,
              })
            }
          }
        }
      } else {
        // OpenAgents legacy dialect (pre-PSEL-1): snake_case semantic ids
        // plus required segment/source provenance fields.
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
      recordItemId(metric.id, `${path}.id`)
    }

    if (upstream && section.id === "success_metrics" && !section.success_metrics?.length) {
      errors.push({
        code: "invalid_success_metric",
        message: "Invalid success metric: include at least one productspec-success-metrics item.",
        path: "sections.success_metrics.success_metrics",
      })
    }

    for (const [index, artifact] of (section.related_artifacts ?? []).entries()) {
      const path = `sections.${section.id}.related_artifacts.${index}`
      if (section.id !== "related_artifacts") {
        errors.push({
          code: "invalid_related_artifact",
          message: "Related artifact blocks belong in Related Artifacts.",
          path,
        })
      }
      const requiredFields =
        artifact.type === "product_spec"
          ? (["type", "product_spec_path"] as const)
          : (["type", "url"] as const)
      const missing = requiredFields.filter((field) => !String(artifact[field] ?? "").trim())
      if (missing.length) {
        errors.push({
          code: "invalid_related_artifact",
          message: `Invalid related artifact: missing ${missing.join(", ")}.`,
          path,
        })
      }
      if (
        artifact.type &&
        !(RELATED_ARTIFACT_TYPES as ReadonlyArray<string>).includes(artifact.type)
      ) {
        errors.push({
          code: "invalid_related_artifact",
          message: `Invalid related artifact: type must be one of ${RELATED_ARTIFACT_TYPES.join(", ")}.`,
          path,
        })
      }
      if (artifact.url && artifact.type === "product_spec") {
        errors.push({
          code: "invalid_related_artifact",
          message: "Invalid related artifact: product_spec entries use product_spec_path, not url.",
          path,
        })
      }
      if (artifact.product_spec_path && artifact.type !== "product_spec") {
        errors.push({
          code: "invalid_related_artifact",
          message: "Invalid related artifact: product_spec_path only applies to type product_spec.",
          path,
        })
      }
      if (artifact.product_spec_revision !== undefined && artifact.type !== "product_spec") {
        errors.push({
          code: "invalid_related_artifact",
          message: "Invalid related artifact: product_spec_revision only applies to type product_spec.",
          path,
        })
      }
      if (artifact.relation && artifact.type !== "product_spec") {
        errors.push({
          code: "invalid_related_artifact",
          message: "Invalid related artifact: relation only applies to type product_spec.",
          path,
        })
      }
      if (
        artifact.product_spec_revision !== undefined &&
        (!Number.isInteger(artifact.product_spec_revision) || artifact.product_spec_revision < 1)
      ) {
        errors.push({
          code: "invalid_related_artifact",
          message: "Invalid related artifact: product_spec_revision must be a positive integer.",
          path,
        })
      }
      if (
        artifact.relation &&
        !(RELATED_ARTIFACT_RELATIONS as ReadonlyArray<string>).includes(artifact.relation)
      ) {
        errors.push({
          code: "invalid_related_artifact",
          message: `Invalid related artifact: relation must be one of ${RELATED_ARTIFACT_RELATIONS.join(", ")}.`,
          path,
        })
      }
      if (artifact.section_id) {
        const validSection =
          (CANONICAL_SECTION_IDS as ReadonlyArray<string>).includes(artifact.section_id) ||
          CUSTOM_SECTION_ID.test(artifact.section_id)
        if (!validSection) {
          errors.push({
            code: "invalid_related_artifact",
            message: "Invalid related artifact: section_id must be canonical or custom-<kebab-name>.",
            path,
          })
        }
      }
      if (artifact.item_id && !ANY_ITEM_ID.test(artifact.item_id)) {
        errors.push({
          code: "invalid_related_artifact",
          message:
            "Invalid related artifact: item_id must use AC-<number>, SM-<number>, or EVAL-<number>.",
          path,
        })
      }
      if (
        artifact.item_id &&
        ANY_ITEM_ID.test(artifact.item_id) &&
        !knownItemIds.has(artifact.item_id)
      ) {
        errors.push({
          code: "invalid_related_artifact",
          message: `Invalid related artifact: item_id ${artifact.item_id} does not match any Acceptance Criterion, Success Metric, or AI Eval.`,
          path,
        })
      }
      const expectedPrefix = artifact.type
        ? expectedItemIdPrefixForArtifact(artifact.type)
        : undefined
      if (artifact.item_id && expectedPrefix && !artifact.item_id.startsWith(`${expectedPrefix}-`)) {
        warnings.push({
          code: "unusual_related_artifact_target",
          message: `Related artifact type ${artifact.type} usually attaches to ${expectedPrefix}-<number>.`,
          path,
        })
      }
    }
  }

  return { errors, warnings }
}

export type ValidateProductSpecOptions = {
  profile?: ProductSpecProfile
}

export const validateProductSpec = (
  markdown: string,
  options?: ValidateProductSpecOptions,
): ValidationResult => {
  const profile = options?.profile ?? "openagents"
  try {
    const document = parseProductSpec(markdown)
    const { errors, warnings } = validateDocument(document, profile)
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
// Serialization (round-trip: serialize(parse(x)) reparses to the same
// document; structured blocks live inside section content and are preserved
// verbatim)
// ---------------------------------------------------------------------------

export const serializeProductSpec = (document: ProductSpecDocument): string => {
  const { frontmatter } = document
  let output = ""
  for (const key of [
    "spec_format_version",
    "title",
    "artifact_type",
    "spec_revision",
    "author",
    "created_at",
    "updated_at",
    "linked_github_repo",
  ] as const) {
    const value = frontmatter[key]
    if (value === undefined || value === "") continue
    output += typeof value === "number" ? `${key}: ${value}\n` : `${key}: "${value}"\n`
  }
  if (frontmatter.applies_to?.length) {
    output += "applies_to:\n"
    for (const item of frontmatter.applies_to) {
      if (item.path !== undefined) output += `  - path: "${item.path}"\n`
      if (item.component !== undefined) output += `  - component: "${item.component}"\n`
    }
  }
  if (frontmatter.custom_sections?.length) {
    output += "custom_sections:\n"
    for (const section of frontmatter.custom_sections) {
      output += `  - id: "${section.id}"\n`
      output += `    label: "${section.label}"\n`
      output += `    after: "${section.after}"\n`
    }
  }
  if (frontmatter.tool_metadata && Object.keys(frontmatter.tool_metadata).length) {
    output += "tool_metadata:\n"
    for (const [key, value] of Object.entries(frontmatter.tool_metadata)) {
      output += `  ${key}: "${value}"\n`
    }
  }
  const body = document.sections
    .map((section) => `## ${section.label}\n\n${section.content.trim()}`)
    .join("\n\n")
  return `---\n${output}---\n\n${body}\n`
}

// ---------------------------------------------------------------------------
// Dual digests (PSEL-1, docs/assurance/ASSURANCE_SPEC.md §4)
//
// document digest — SHA-256 over the exact authored UTF-8 bytes. Changes on
// every edit, including evidence-link maintenance. Retained for provenance
// and recheck-based race detection.
//
// intent digest — SHA-256 over the canonical intent projection. Excludes
// only Related Artifact attachments that are not product_spec dependencies
// (typed evidence classification) plus the created_at/updated_at provenance
// timestamps. Unknown fields are intent-bound by default. The projection is
// versioned and conformance-tested; it is not a Markdown deletion heuristic.
// ---------------------------------------------------------------------------

export const INTENT_PROJECTION_VERSION = "1" as const

export type ProductSpecIntentProjection = {
  projection_version: typeof INTENT_PROJECTION_VERSION
  frontmatter: {
    spec_format_version: string
    title: string
    artifact_type: ArtifactType
    author: string
    spec_revision?: number
    linked_github_repo?: string
    applies_to?: ReadonlyArray<ProductSpecAppliesTo>
    custom_sections?: ReadonlyArray<CustomSectionDecl>
    tool_metadata?: Readonly<Record<string, string>>
  }
  sections: Array<{ id: string; label: string; content: string }>
  product_spec_dependencies: Array<{
    section_id: string
    artifact: ProductSpecRelatedArtifact
  }>
}

const sha256Hex = (payload: string | Uint8Array): string =>
  `sha256:${createHash("sha256").update(payload).digest("hex")}`

const canonicalJson = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, entryValue]) => entryValue !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    return `{${entries.map(([key, entryValue]) => `${JSON.stringify(key)}:${canonicalJson(entryValue)}`).join(",")}}`
  }
  return JSON.stringify(value)
}

/** SHA-256 over the exact UTF-8 bytes of the authored document. */
export const computeProductSpecDocumentDigest = (markdown: string | Uint8Array): string =>
  sha256Hex(typeof markdown === "string" ? new TextEncoder().encode(markdown) : markdown)

/**
 * The canonical intent projection of a parsed ProductSpec document.
 *
 * - `created_at`/`updated_at` are excluded (explicit non-intent provenance).
 * - `productspec-related-artifacts` blocks are structurally removed from
 *   section content; `product_spec` dependency entries are re-included as
 *   typed intent (they change what the spec means), while all other Related
 *   Artifact entries are classified evidence attachments and excluded.
 * - A `related_artifacts` section whose content is nothing but attachment
 *   blocks is dropped entirely, so introducing the section for evidence
 *   links does not read as intent drift. Any residual prose in it stays
 *   intent-bound (unknown content is intent by default).
 * - Everything else — including every `tool_metadata` entry, `applies_to`,
 *   custom sections, and all structured item text — is intent-bound.
 */
export const productSpecIntentProjection = (
  document: ProductSpecDocument,
): ProductSpecIntentProjection => {
  const { frontmatter } = document
  const sections: ProductSpecIntentProjection["sections"] = []
  const dependencies: ProductSpecIntentProjection["product_spec_dependencies"] = []

  for (const section of document.sections) {
    const strippedContent = stripProductSpecBlocks(section.content, "productspec-related-artifacts")
    for (const artifact of section.related_artifacts ?? []) {
      if (artifact.type === "product_spec") {
        dependencies.push({ section_id: section.id, artifact })
      }
    }
    if (section.id === "related_artifacts" && strippedContent === "") continue
    sections.push({ id: section.id, label: section.label, content: strippedContent })
  }

  return {
    projection_version: INTENT_PROJECTION_VERSION,
    frontmatter: {
      spec_format_version: frontmatter.spec_format_version,
      title: frontmatter.title,
      artifact_type: frontmatter.artifact_type,
      author: frontmatter.author,
      ...(frontmatter.spec_revision !== undefined
        ? { spec_revision: frontmatter.spec_revision }
        : {}),
      ...(frontmatter.linked_github_repo !== undefined
        ? { linked_github_repo: frontmatter.linked_github_repo }
        : {}),
      ...(frontmatter.applies_to !== undefined ? { applies_to: frontmatter.applies_to } : {}),
      ...(frontmatter.custom_sections !== undefined
        ? { custom_sections: frontmatter.custom_sections }
        : {}),
      ...(frontmatter.tool_metadata !== undefined
        ? { tool_metadata: frontmatter.tool_metadata }
        : {}),
    },
    sections,
    product_spec_dependencies: dependencies,
  }
}

/** SHA-256 over the canonical JSON of the versioned intent projection. */
export const computeProductSpecIntentDigest = (input: string | ProductSpecDocument): string => {
  const document = typeof input === "string" ? parseProductSpec(input) : input
  return sha256Hex(canonicalJson(productSpecIntentProjection(document)))
}

// ---------------------------------------------------------------------------
// Typed evidence-attachment-only edit path (PSEL-1)
//
// An evidence-attachment-only edit changes the document digest but provably
// not the intent digest or spec_revision: the typed semantic proof is intent
// projection equality, because the projection is intent-total (it excludes
// only classified evidence attachments and the two provenance timestamps).
// This path never relaxes the generic edit/revision rule — any other change
// still requires a spec_revision bump.
// ---------------------------------------------------------------------------

export type EvidenceAttachmentEditPlan =
  | Readonly<{
      ok: true
      kind: "evidence_attachment_only"
      before: Readonly<{ documentDigest: string; intentDigest: string }>
      after: Readonly<{ documentDigest: string; intentDigest: string }>
    }>
  | Readonly<{
      ok: false
      code:
        | "invalid_current_document"
        | "invalid_proposed_document"
        | "document_unchanged"
        | "spec_revision_changed"
        | "created_at_changed"
        | "intent_changed"
      message: string
      errors?: ReadonlyArray<ValidationIssue>
    }>

export const planProductSpecEvidenceAttachmentEdit = (options: {
  currentMarkdown: string
  proposedMarkdown: string
}): EvidenceAttachmentEditPlan => {
  const current = validateProductSpec(options.currentMarkdown)
  if (!current.valid) {
    return {
      ok: false,
      code: "invalid_current_document",
      message: "The current document is not a valid Product Spec.",
      errors: current.errors,
    }
  }
  const proposed = validateProductSpec(options.proposedMarkdown)
  if (!proposed.valid) {
    return {
      ok: false,
      code: "invalid_proposed_document",
      message: "The proposed document is not a valid Product Spec.",
      errors: proposed.errors,
    }
  }

  const beforeDocumentDigest = computeProductSpecDocumentDigest(options.currentMarkdown)
  const afterDocumentDigest = computeProductSpecDocumentDigest(options.proposedMarkdown)
  if (beforeDocumentDigest === afterDocumentDigest) {
    return {
      ok: false,
      code: "document_unchanged",
      message: "The proposed document is byte-identical to the current document.",
    }
  }

  if (current.document.frontmatter.spec_revision !== proposed.document.frontmatter.spec_revision) {
    return {
      ok: false,
      code: "spec_revision_changed",
      message:
        "spec_revision changed; a revision bump is an intent change and never an evidence-attachment-only edit.",
    }
  }
  if (current.document.frontmatter.created_at !== proposed.document.frontmatter.created_at) {
    return {
      ok: false,
      code: "created_at_changed",
      message: "created_at is immutable provenance; only updated_at may move on an evidence edit.",
    }
  }

  const beforeIntentDigest = computeProductSpecIntentDigest(current.document)
  const afterIntentDigest = computeProductSpecIntentDigest(proposed.document)
  if (beforeIntentDigest !== afterIntentDigest) {
    return {
      ok: false,
      code: "intent_changed",
      message:
        "The canonical intent projection changed; this edit is intent drift and requires a spec_revision bump (and usually a Decision Trace), not the evidence-attachment path.",
    }
  }

  return {
    ok: true,
    kind: "evidence_attachment_only",
    before: { documentDigest: beforeDocumentDigest, intentDigest: beforeIntentDigest },
    after: { documentDigest: afterDocumentDigest, intentDigest: afterIntentDigest },
  }
}

export type EvidenceAttachmentEditResult =
  | Readonly<{
      ok: true
      path: string
      before: Readonly<{ documentDigest: string; intentDigest: string }>
      after: Readonly<{ documentDigest: string; intentDigest: string }>
    }>
  | Readonly<{
      ok: false
      code:
        | "owner_confirmation_required"
        | "document_digest_mismatch"
        | Extract<EvidenceAttachmentEditPlan, { ok: false }>["code"]
      message: string
      errors?: ReadonlyArray<ValidationIssue>
    }>

/**
 * Apply an owner-confirmed evidence-attachment-only edit to a spec file.
 *
 * The caller pins the exact document digest it reviewed. The file bytes are
 * re-read and re-hashed immediately before write, so an intervening edit by
 * anyone else fails typed (`document_digest_mismatch`) instead of being
 * clobbered, and the typed semantic proof (intent projection equality) runs
 * against those exact rechecked bytes.
 */
export const applyProductSpecEvidenceAttachmentEdit = (options: {
  path: string
  expectedDocumentDigest: string
  proposedMarkdown: string
  ownerConfirmed: boolean
}): EvidenceAttachmentEditResult => {
  if (options.ownerConfirmed !== true) {
    return {
      ok: false,
      code: "owner_confirmation_required",
      message: "Evidence-attachment edits are owner-confirmed; pass ownerConfirmed: true.",
    }
  }

  const currentBytes = readFileSync(options.path)
  const currentDigest = computeProductSpecDocumentDigest(currentBytes)
  if (currentDigest !== options.expectedDocumentDigest) {
    return {
      ok: false,
      code: "document_digest_mismatch",
      message: `The file no longer matches the reviewed document digest (expected ${options.expectedDocumentDigest}, found ${currentDigest}).`,
    }
  }

  const plan = planProductSpecEvidenceAttachmentEdit({
    currentMarkdown: currentBytes.toString("utf8"),
    proposedMarkdown: options.proposedMarkdown,
  })
  if (!plan.ok) return plan

  writeFileSync(options.path, options.proposedMarkdown, "utf8")
  return { ok: true, path: options.path, before: plan.before, after: plan.after }
}

// ---------------------------------------------------------------------------
// OpenAgents executable profile (author-visible criterion identity)
// ---------------------------------------------------------------------------

const EXECUTABLE_CRITERION_ID = /^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+$/
const BOLD_CRITERION_PREFIX = /^\*\*([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+):\*\*\s*([\s\S]*)$/
const PLAIN_CRITERION_PREFIX = /^([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+):\s*([\s\S]*)$/

/** Parse top-level Acceptance Criteria bullets after ProductSpec routing. */
export const extractProductSpecAcceptanceCriteria = (
  document: ProductSpecDocument,
): Readonly<{
  criteria: ReadonlyArray<ProductSpecAcceptanceCriterion>
  errors: ReadonlyArray<ValidationIssue>
}> => {
  const section = document.sections.find(candidate => candidate.id === "acceptance_criteria")
  if (section === undefined) {
    return {
      criteria: [],
      errors: [{
        code: "missing_required_section",
        message: "Missing mandatory section: acceptance_criteria",
        path: "sections.acceptance_criteria",
      }],
    }
  }

  const rawItems: string[] = []
  let current: string[] | null = null
  let fenced = false
  const flush = (): void => {
    if (current === null) return
    rawItems.push(current.join("\n").trim())
    current = null
  }

  for (const line of section.content.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      flush()
      fenced = !fenced
      continue
    }
    if (fenced) continue
    if (line.startsWith("- ")) {
      flush()
      current = [line.slice(2).trim()]
      continue
    }
    if (current !== null && /^(?: {2,}|\t)\S/.test(line)) {
      current.push(line.trim())
      continue
    }
    if (line.trim().length !== 0) flush()
  }
  flush()

  const criteria: ProductSpecAcceptanceCriterion[] = []
  const errors: ValidationIssue[] = []
  const seen = new Set<string>()

  rawItems.forEach((item, ordinal) => {
    const match = BOLD_CRITERION_PREFIX.exec(item) ?? PLAIN_CRITERION_PREFIX.exec(item)
    if (match === null || !EXECUTABLE_CRITERION_ID.test(match[1] ?? "")) {
      errors.push({
        code: "missing_acceptance_criterion_id",
        message: `Executable acceptance criterion ${ordinal + 1} requires an author-visible ID such as CW-AC-01.`,
        path: `sections.acceptance_criteria.criteria.${ordinal}`,
      })
      return
    }
    const id = match[1]!
    const body = (match[2] ?? "").trim()
    if (body.length === 0) {
      errors.push({
        code: "empty_acceptance_criterion",
        message: `Acceptance criterion ${id} has no requirement text.`,
        path: `sections.acceptance_criteria.criteria.${ordinal}`,
      })
      return
    }
    if (seen.has(id)) {
      errors.push({
        code: "duplicate_acceptance_criterion_id",
        message: `Duplicate acceptance criterion ID: ${id}`,
        path: `sections.acceptance_criteria.criteria.${ordinal}`,
      })
      return
    }
    seen.add(id)
    criteria.push({ id, body, ordinal })
  })

  if (rawItems.length === 0) {
    errors.push({
      code: "missing_acceptance_criteria",
      message: "Executable ProductSpecs require at least one top-level acceptance criterion bullet.",
      path: "sections.acceptance_criteria",
    })
  }

  return { criteria, errors }
}

/**
 * OpenAgents executable profile: standard-valid plus positive revision and
 * unique author-visible criterion IDs. A standard-valid legacy document can
 * still be rendered when this returns `executable: false`.
 */
export const validateExecutableProductSpec = (
  markdown: string,
): ExecutableProductSpecResult => {
  const validation = validateProductSpec(markdown)
  if (!validation.valid) {
    return {
      executable: false,
      criteria: [],
      errors: validation.errors,
      warnings: validation.warnings,
    }
  }
  const extracted = extractProductSpecAcceptanceCriteria(validation.document)
  const errors = [...extracted.errors]
  if (validation.document.frontmatter.spec_revision === undefined) {
    errors.unshift({
      code: "missing_spec_revision",
      message: "Executable ProductSpecs require a positive spec_revision.",
      path: "frontmatter.spec_revision",
    })
  }
  return errors.length === 0
    ? {
        executable: true,
        document: validation.document,
        criteria: extracted.criteria,
        errors: [],
        warnings: validation.warnings,
      }
    : {
        executable: false,
        document: validation.document,
        criteria: extracted.criteria,
        errors,
        warnings: validation.warnings,
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
