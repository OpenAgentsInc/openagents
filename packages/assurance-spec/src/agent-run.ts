import { Schema } from "effect"

export const AGENT_RUN_FORMAT_VERSION = "0.1" as const
export const AGENT_RUN_INGEST_SCHEMA = "openagents.assurance.agent_run_ingest.v1" as const

const NonEmptyString = Schema.String.check(Schema.isMinLength(1))
const PositiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))

export const AgentRunEvidenceTypeSchema = Schema.Literals([
  "product_spec", "engineering_spec", "github_issue", "github_pr", "jira_issue",
  "linear_issue", "figma", "eval_run", "dashboard", "analytics_snapshot",
  "experiment", "release", "code", "other",
])

export const AgentRunEvidenceLinkSchema = Schema.Struct({
  type: AgentRunEvidenceTypeSchema,
  url: NonEmptyString,
  title: Schema.optionalKey(Schema.String),
})

export const AgentRunCheckedItemSchema = Schema.Struct({
  item_id: NonEmptyString,
  status: Schema.Literals(["passed", "failed", "not_checked", "blocked"]),
  evidence: Schema.optionalKey(Schema.Array(AgentRunEvidenceLinkSchema)),
  notes: Schema.optionalKey(NonEmptyString),
})

export const AgentRunDocumentSchema = Schema.Struct({
  agent_run_format_version: Schema.Literal(AGENT_RUN_FORMAT_VERSION),
  run_id: NonEmptyString,
  agent: Schema.Struct({
    name: NonEmptyString,
    version: Schema.optionalKey(NonEmptyString),
  }),
  product_spec: Schema.Struct({
    path: NonEmptyString,
    spec_revision: PositiveInteger,
    content_hash: Schema.optionalKey(NonEmptyString),
  }),
  started_at: NonEmptyString,
  completed_at: Schema.optionalKey(NonEmptyString),
  status: Schema.Literals(["draft", "completed", "blocked", "failed"]),
  checked_items: Schema.Array(AgentRunCheckedItemSchema),
  drift: Schema.Struct({
    detected: Schema.Boolean,
    decision_trace_path: Schema.optionalKey(NonEmptyString),
    summary: Schema.optionalKey(NonEmptyString),
  }),
  completion_claim: Schema.optionalKey(NonEmptyString),
})

export type AgentRunDocument = typeof AgentRunDocumentSchema.Type
export type AgentRunCheckedItem = typeof AgentRunCheckedItemSchema.Type

export type AgentRunDiagnostic = Readonly<{
  code: string
  message: string
  path?: string
}>

export type AgentRunValidation =
  | Readonly<{ valid: true; document: AgentRunDocument; errors: readonly [] }>
  | Readonly<{ valid: false; errors: ReadonlyArray<AgentRunDiagnostic> }>

const RUN_ID = /^[a-z0-9]+(?:[-_][a-z0-9]+)*$/
const ITEM_ID = /^(AC|EVAL|SM)-[1-9][0-9]*$/
const ISO_DATETIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/
const RUN_STATUSES = new Set(["draft", "completed", "blocked", "failed"])
const ITEM_STATUSES = new Set(["passed", "failed", "not_checked", "blocked"])
const EVIDENCE_TYPES = new Set([
  "product_spec", "engineering_spec", "github_issue", "github_pr", "jira_issue",
  "linear_issue", "figma", "eval_run", "dashboard", "analytics_snapshot",
  "experiment", "release", "code", "other",
])

const record = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const extras = (value: Record<string, unknown>, allowed: ReadonlySet<string>): boolean =>
  Object.keys(value).some((key) => !allowed.has(key))

const nonEmpty = (value: unknown): value is string => typeof value === "string" && value.length > 0
const dateTime = (value: unknown): value is string =>
  typeof value === "string" && ISO_DATETIME.test(value) && !Number.isNaN(Date.parse(value))

/** Shape validation pinned to ProductSpec Agent Run 0.1, including duplicate item rejection. */
export const validateAgentRunJson = (source: string): AgentRunValidation => {
  let value: unknown
  try {
    value = JSON.parse(source)
  } catch {
    return { valid: false, errors: [{ code: "invalid_json", message: "Agent Run must be valid JSON." }] }
  }
  if (!record(value)) {
    return { valid: false, errors: [{ code: "invalid_agent_run", message: "Agent Run must be a JSON object." }] }
  }
  const required = ["agent_run_format_version", "run_id", "agent", "product_spec", "started_at", "status", "checked_items", "drift"]
  for (const key of required) {
    if (!(key in value)) return { valid: false, errors: [{ code: "missing_required_agent_run_field", message: `Missing required Agent Run field: ${key}.`, path: key }] }
  }
  if (extras(value, new Set([...required, "completed_at", "completion_claim"]))) {
    return { valid: false, errors: [{ code: "invalid_agent_run", message: "Agent Run contains an unsupported field." }] }
  }
  if (value.agent_run_format_version !== AGENT_RUN_FORMAT_VERSION) {
    return { valid: false, errors: [{ code: "unsupported_agent_run_version", message: `Only Agent Run format ${AGENT_RUN_FORMAT_VERSION} is supported.`, path: "agent_run_format_version" }] }
  }
  if (typeof value.run_id !== "string" || !RUN_ID.test(value.run_id)) {
    return { valid: false, errors: [{ code: "invalid_agent_run_id", message: "run_id must be lowercase alphanumeric segments separated by '-' or '_'.", path: "run_id" }] }
  }
  if (!RUN_STATUSES.has(String(value.status))) {
    return { valid: false, errors: [{ code: "invalid_agent_run_status", message: "Agent Run status is invalid.", path: "status" }] }
  }
  if (!dateTime(value.started_at) || (value.completed_at !== undefined && !dateTime(value.completed_at))) {
    const path = !dateTime(value.started_at) ? "started_at" : "completed_at"
    return { valid: false, errors: [{ code: "invalid_datetime", message: `${path} must be an ISO 8601 date-time.`, path }] }
  }
  if (value.completion_claim !== undefined && !nonEmpty(value.completion_claim)) {
    return { valid: false, errors: [{ code: "invalid_agent_run", message: "completion_claim must be non-empty when present.", path: "completion_claim" }] }
  }

  const agent = value.agent
  if (!record(agent) || !nonEmpty(agent.name) || (agent.version !== undefined && !nonEmpty(agent.version)) || extras(agent, new Set(["name", "version"]))) {
    return { valid: false, errors: [{ code: "invalid_agent_run_agent", message: "agent must contain only a non-empty name and optional non-empty version.", path: "agent" }] }
  }
  const productSpec = value.product_spec
  if (!record(productSpec) || !nonEmpty(productSpec.path) || !Number.isInteger(productSpec.spec_revision) || Number(productSpec.spec_revision) < 1 || (productSpec.content_hash !== undefined && !nonEmpty(productSpec.content_hash)) || extras(productSpec, new Set(["path", "spec_revision", "content_hash"]))) {
    const code = record(productSpec) && (!Number.isInteger(productSpec.spec_revision) || Number(productSpec.spec_revision) < 1)
      ? "invalid_agent_run_revision"
      : "invalid_agent_run_product_spec"
    return { valid: false, errors: [{ code, message: "product_spec must contain a path, positive integer spec_revision, and optional non-empty content_hash.", path: "product_spec" }] }
  }
  const drift = value.drift
  if (!record(drift) || typeof drift.detected !== "boolean" || (drift.decision_trace_path !== undefined && !nonEmpty(drift.decision_trace_path)) || (drift.summary !== undefined && !nonEmpty(drift.summary)) || extras(drift, new Set(["detected", "decision_trace_path", "summary"]))) {
    return { valid: false, errors: [{ code: "invalid_agent_run_drift", message: "drift must contain detected and only optional non-empty decision trace fields.", path: "drift" }] }
  }
  if (!Array.isArray(value.checked_items)) {
    return { valid: false, errors: [{ code: "invalid_agent_run_item", message: "checked_items must be an array.", path: "checked_items" }] }
  }
  const seen = new Set<string>()
  for (let index = 0; index < value.checked_items.length; index += 1) {
    const item = value.checked_items[index]
    const path = `checked_items.${index}`
    if (!record(item) || typeof item.item_id !== "string" || !ITEM_ID.test(item.item_id) || !ITEM_STATUSES.has(String(item.status)) || (item.notes !== undefined && !nonEmpty(item.notes)) || extras(item, new Set(["item_id", "status", "evidence", "notes"]))) {
      return { valid: false, errors: [{ code: "invalid_agent_run_item", message: "Checked item is invalid.", path }] }
    }
    if (seen.has(item.item_id)) {
      return { valid: false, errors: [{ code: "duplicate_agent_run_item_id", message: `Duplicate checked item id: ${item.item_id}.`, path: `${path}.item_id` }] }
    }
    seen.add(item.item_id)
    if (item.evidence !== undefined) {
      if (!Array.isArray(item.evidence)) return { valid: false, errors: [{ code: "invalid_evidence_link", message: "evidence must be an array.", path: `${path}.evidence` }] }
      for (let evidenceIndex = 0; evidenceIndex < item.evidence.length; evidenceIndex += 1) {
        const link = item.evidence[evidenceIndex]
        const linkPath = `${path}.evidence.${evidenceIndex}`
        if (!record(link) || !EVIDENCE_TYPES.has(String(link.type)) || !nonEmpty(link.url) || (link.title !== undefined && typeof link.title !== "string") || extras(link, new Set(["type", "url", "title"]))) {
          return { valid: false, errors: [{ code: "invalid_evidence_link", message: "Evidence link is invalid.", path: linkPath }] }
        }
      }
    }
  }
  try {
    return { valid: true, document: Schema.decodeUnknownSync(AgentRunDocumentSchema)(value), errors: [] }
  } catch (error) {
    return { valid: false, errors: [{ code: "invalid_agent_run", message: error instanceof Error ? error.message : "Agent Run is invalid." }] }
  }
}

export const AgentRunSelfReportEvidenceSchema = Schema.Struct({
  schema: Schema.Literal(AGENT_RUN_INGEST_SCHEMA),
  source: Schema.Struct({ path: NonEmptyString, document_digest: NonEmptyString }),
  agent_run_format_version: Schema.Literal(AGENT_RUN_FORMAT_VERSION),
  run_id: NonEmptyString,
  run_status: Schema.Literals(["draft", "completed", "blocked", "failed"]),
  started_at: NonEmptyString,
  completed_at: Schema.optionalKey(NonEmptyString),
  proof_rung: Schema.Literal("self_report"),
  producer: Schema.Struct({ name: NonEmptyString, version: Schema.optionalKey(NonEmptyString) }),
  claimant: Schema.Struct({ name: NonEmptyString, version: Schema.optionalKey(NonEmptyString) }),
  producer_equals_claimant: Schema.Literal(true),
  independently_verified: Schema.Literal(false),
  observation_axis: Schema.Literal("not_promoted"),
  spec_pin: Schema.Struct({
    path: NonEmptyString,
    spec_revision: PositiveInteger,
    declared_content_hash: Schema.NullOr(NonEmptyString),
    computed_content_hash: NonEmptyString,
    digest_status: Schema.Literals(["matched", "missing"]),
  }),
  claimed_items: Schema.Array(AgentRunCheckedItemSchema),
  drift: AgentRunDocumentSchema.fields.drift,
  completion_claim: Schema.optionalKey(NonEmptyString),
  gaps: Schema.Array(Schema.Struct({ code: NonEmptyString, message: NonEmptyString })),
  authority: Schema.Struct({
    can_promote_observation: Schema.Literal(false),
    can_verify: Schema.Literal(false),
    can_satisfy_independent_producer: Schema.Literal(false),
  }),
})

export type AgentRunSelfReportEvidence = typeof AgentRunSelfReportEvidenceSchema.Type
