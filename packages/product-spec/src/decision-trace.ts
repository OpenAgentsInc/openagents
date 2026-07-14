import { Schema as S } from "effect"

import type { ValidationIssue } from "./index.ts"

export const DECISION_TRACE_FORMAT_VERSION = "0.1" as const
export const DECISION_TRACE_EXTENSION = ".decision-trace.json" as const

export const DECISION_TRACE_SUBJECT_TYPES = [
  "product_spec",
  "engineering_spec",
  "design",
  "implementation",
  "eval",
  "experiment",
  "incident",
  "other",
] as const

export const DECISION_TRACE_EVENT_TYPES = [
  "intent_decision",
  "scope_drift",
  "acceptance_criteria_drift",
  "ux_drift",
  "ai_eval_drift",
  "success_metric_review",
  "implementation_tradeoff",
  "spec_revision",
  "outcome_review",
] as const

export const DECISION_TRACE_OUTCOMES = [
  "update_spec",
  "update_implementation",
  "accept_tradeoff",
  "reopen_work",
  "record_learning",
  "no_action",
] as const

export const DECISION_TRACE_LINK_TYPES = [
  "product_spec",
  "engineering_spec",
  "github_issue",
  "github_pr",
  "jira_issue",
  "linear_issue",
  "figma",
  "eval_run",
  "analytics_snapshot",
  "experiment",
  "release",
  "other",
] as const

const NonEmptyString = S.String.check(S.isMinLength(1))

export const DecisionTraceLinkSchema = S.Struct({
  type: S.Literals(DECISION_TRACE_LINK_TYPES),
  url: NonEmptyString,
  title: S.optionalKey(S.String),
})
export type DecisionTraceLink = typeof DecisionTraceLinkSchema.Type

export const DecisionTraceSubjectSchema = S.Struct({
  type: S.Literals(DECISION_TRACE_SUBJECT_TYPES),
  id: NonEmptyString,
  title: S.optionalKey(S.String),
  product_spec_path: S.optionalKey(S.String),
  product_spec_revision: S.optionalKey(S.Number),
})
export type DecisionTraceSubject = typeof DecisionTraceSubjectSchema.Type

export const DecisionTraceSourceSchema = S.Struct({
  product_spec_revision: S.optionalKey(S.Number),
  links: S.optionalKey(S.Array(DecisionTraceLinkSchema)),
})
export type DecisionTraceSource = typeof DecisionTraceSourceSchema.Type

export const DecisionTraceDriftSchema = S.Struct({
  spec_claim: S.optionalKey(S.String),
  observed_reality: S.optionalKey(S.String),
})
export type DecisionTraceDrift = typeof DecisionTraceDriftSchema.Type

export const DecisionTraceDecisionSchema = S.Struct({
  outcome: S.Literals(DECISION_TRACE_OUTCOMES),
  rationale: NonEmptyString,
  approved_by: S.optionalKey(S.Array(NonEmptyString)),
})
export type DecisionTraceDecision = typeof DecisionTraceDecisionSchema.Type

export const DecisionTraceResultSchema = S.Struct({
  new_product_spec_revision: S.optionalKey(S.Number),
  linked_artifacts: S.optionalKey(S.Array(DecisionTraceLinkSchema)),
  learning: S.optionalKey(S.String),
})
export type DecisionTraceResult = typeof DecisionTraceResultSchema.Type

export const DecisionTraceEventSchema = S.Struct({
  event_id: NonEmptyString,
  event_type: S.Literals(DECISION_TRACE_EVENT_TYPES),
  occurred_at: NonEmptyString,
  summary: NonEmptyString,
  source: S.optionalKey(DecisionTraceSourceSchema),
  drift: S.optionalKey(DecisionTraceDriftSchema),
  decision: DecisionTraceDecisionSchema,
  result: S.optionalKey(DecisionTraceResultSchema),
})
export type DecisionTraceEvent = typeof DecisionTraceEventSchema.Type

export const DecisionTraceSchema = S.Struct({
  decision_trace_format_version: S.Literal(DECISION_TRACE_FORMAT_VERSION),
  trace_id: NonEmptyString,
  title: NonEmptyString,
  created_at: NonEmptyString,
  updated_at: NonEmptyString,
  subject: DecisionTraceSubjectSchema,
  events: S.Array(DecisionTraceEventSchema),
})
export type DecisionTrace = typeof DecisionTraceSchema.Type

export const DECISION_TRACE_ERROR_CODES = [
  "invalid_decision_trace_json",
  "invalid_decision_trace",
  "missing_decision_trace_field",
  "unsupported_decision_trace_version",
  "unexpected_decision_trace_field",
  "invalid_trace_id",
  "invalid_decision_trace_title",
  "invalid_decision_trace_datetime",
  "invalid_decision_trace_subject",
  "empty_decision_trace_events",
  "invalid_decision_trace_event",
  "missing_decision_trace_event_field",
  "invalid_decision_trace_event_id",
  "duplicate_decision_trace_event_id",
  "invalid_decision_trace_event_type",
  "invalid_decision_trace_source",
  "invalid_decision_trace_drift",
  "invalid_decision_trace_decision",
  "invalid_decision_trace_result",
  "invalid_decision_trace_link",
] as const
export type DecisionTraceErrorCode = (typeof DECISION_TRACE_ERROR_CODES)[number]

export type DecisionTraceValidationResult =
  | { valid: true; trace: DecisionTrace; errors: [] }
  | { valid: false; trace?: undefined; errors: ValidationIssue[] }

export class DecisionTraceValidationError extends Error {
  constructor(readonly issues: ReadonlyArray<ValidationIssue>) {
    super(issues.map((issue) => `${issue.code}: ${issue.message}`).join("\n"))
    this.name = "DecisionTraceValidationError"
  }
}

const IDENTIFIER = /^[a-z0-9]+(?:[-_][a-z0-9]+)*$/
const ISO_DATETIME =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const isNonEmptyString = (value: unknown): value is string =>
  typeof value === "string" && value.length > 0

const isIsoDateTime = (value: unknown): value is string =>
  typeof value === "string" && ISO_DATETIME.test(value) && !Number.isNaN(Date.parse(value))

const isPositiveInteger = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 1

const includes = (values: ReadonlyArray<string>, value: unknown): value is string =>
  typeof value === "string" && values.includes(value)

const issue = (
  code: DecisionTraceErrorCode,
  message: string,
  path?: string,
): ValidationIssue => path === undefined ? { code, message } : { code, message, path }

const rejectUnexpectedKeys = (
  value: Record<string, unknown>,
  allowed: ReadonlyArray<string>,
  path: string,
  errors: ValidationIssue[],
): void => {
  for (const key of Object.keys(value)) {
    if (!allowed.includes(key)) {
      errors.push(issue(
        "unexpected_decision_trace_field",
        `Unexpected Decision Trace field: ${key}.`,
        `${path}.${key}`,
      ))
    }
  }
}

const validateLink = (value: unknown, path: string, errors: ValidationIssue[]): void => {
  if (!isRecord(value)) {
    errors.push(issue("invalid_decision_trace_link", "Decision Trace link must be an object.", path))
    return
  }
  rejectUnexpectedKeys(value, ["type", "url", "title"], path, errors)
  if (!includes(DECISION_TRACE_LINK_TYPES, value.type)) {
    errors.push(issue("invalid_decision_trace_link", "Decision Trace link has an invalid type.", `${path}.type`))
  }
  if (!isNonEmptyString(value.url)) {
    errors.push(issue("invalid_decision_trace_link", "Decision Trace link requires a non-empty url.", `${path}.url`))
  }
  if (value.title !== undefined && typeof value.title !== "string") {
    errors.push(issue("invalid_decision_trace_link", "Decision Trace link title must be a string.", `${path}.title`))
  }
}

const validateLinks = (value: unknown, path: string, errors: ValidationIssue[]): void => {
  if (!Array.isArray(value)) {
    errors.push(issue("invalid_decision_trace_link", "Decision Trace links must be an array.", path))
    return
  }
  value.forEach((link, index) => validateLink(link, `${path}.${index}`, errors))
}

const validateSubject = (value: unknown, errors: ValidationIssue[]): void => {
  const path = "subject"
  if (!isRecord(value)) {
    errors.push(issue("invalid_decision_trace_subject", "Decision Trace subject must be an object.", path))
    return
  }
  rejectUnexpectedKeys(
    value,
    ["type", "id", "title", "product_spec_path", "product_spec_revision"],
    path,
    errors,
  )
  if (!includes(DECISION_TRACE_SUBJECT_TYPES, value.type)) {
    errors.push(issue("invalid_decision_trace_subject", "Decision Trace subject has an invalid type.", `${path}.type`))
  }
  if (!isNonEmptyString(value.id)) {
    errors.push(issue("invalid_decision_trace_subject", "Decision Trace subject requires a non-empty id.", `${path}.id`))
  }
  if (value.title !== undefined && typeof value.title !== "string") {
    errors.push(issue("invalid_decision_trace_subject", "Decision Trace subject title must be a string.", `${path}.title`))
  }
  if (value.product_spec_path !== undefined && typeof value.product_spec_path !== "string") {
    errors.push(issue("invalid_decision_trace_subject", "Decision Trace subject product_spec_path must be a string.", `${path}.product_spec_path`))
  }
  if (value.product_spec_revision !== undefined && !isPositiveInteger(value.product_spec_revision)) {
    errors.push(issue("invalid_decision_trace_subject", "Decision Trace subject product_spec_revision must be a positive integer.", `${path}.product_spec_revision`))
  }
}

const validateEvent = (
  value: unknown,
  index: number,
  eventIds: Set<string>,
  errors: ValidationIssue[],
): void => {
  const path = `events.${index}`
  if (!isRecord(value)) {
    errors.push(issue("invalid_decision_trace_event", "Decision Trace event must be an object.", path))
    return
  }
  rejectUnexpectedKeys(
    value,
    ["event_id", "event_type", "occurred_at", "summary", "source", "drift", "decision", "result"],
    path,
    errors,
  )
  for (const field of ["event_id", "event_type", "occurred_at", "summary", "decision"] as const) {
    if (!(field in value)) {
      errors.push(issue(
        "missing_decision_trace_event_field",
        `Decision Trace event is missing required field: ${field}.`,
        `${path}.${field}`,
      ))
    }
  }
  if (!isNonEmptyString(value.event_id) || !IDENTIFIER.test(value.event_id)) {
    errors.push(issue("invalid_decision_trace_event_id", "Decision Trace event_id must use lowercase kebab-case or snake_case.", `${path}.event_id`))
  } else if (eventIds.has(value.event_id)) {
    errors.push(issue("duplicate_decision_trace_event_id", `Duplicate Decision Trace event_id: ${value.event_id}.`, `${path}.event_id`))
  } else {
    eventIds.add(value.event_id)
  }
  if (!includes(DECISION_TRACE_EVENT_TYPES, value.event_type)) {
    errors.push(issue("invalid_decision_trace_event_type", "Decision Trace event has an invalid event_type.", `${path}.event_type`))
  }
  if (!isIsoDateTime(value.occurred_at)) {
    errors.push(issue("invalid_decision_trace_datetime", "Decision Trace occurred_at must be an ISO 8601 date-time.", `${path}.occurred_at`))
  }
  if (!isNonEmptyString(value.summary)) {
    errors.push(issue("invalid_decision_trace_event", "Decision Trace event requires a non-empty summary.", `${path}.summary`))
  }

  if (value.source !== undefined) {
    if (!isRecord(value.source)) {
      errors.push(issue("invalid_decision_trace_source", "Decision Trace event source must be an object.", `${path}.source`))
    } else {
      rejectUnexpectedKeys(value.source, ["product_spec_revision", "links"], `${path}.source`, errors)
      if (value.source.product_spec_revision !== undefined && !isPositiveInteger(value.source.product_spec_revision)) {
        errors.push(issue("invalid_decision_trace_source", "Decision Trace source product_spec_revision must be a positive integer.", `${path}.source.product_spec_revision`))
      }
      if (value.source.links !== undefined) validateLinks(value.source.links, `${path}.source.links`, errors)
    }
  }

  if (value.drift !== undefined) {
    if (!isRecord(value.drift)) {
      errors.push(issue("invalid_decision_trace_drift", "Decision Trace event drift must be an object.", `${path}.drift`))
    } else {
      rejectUnexpectedKeys(value.drift, ["spec_claim", "observed_reality"], `${path}.drift`, errors)
      for (const field of ["spec_claim", "observed_reality"] as const) {
        if (value.drift[field] !== undefined && typeof value.drift[field] !== "string") {
          errors.push(issue("invalid_decision_trace_drift", `Decision Trace drift ${field} must be a string.`, `${path}.drift.${field}`))
        }
      }
    }
  }

  if (!isRecord(value.decision)) {
    errors.push(issue("invalid_decision_trace_decision", "Decision Trace event decision must be an object.", `${path}.decision`))
  } else {
    rejectUnexpectedKeys(value.decision, ["outcome", "rationale", "approved_by"], `${path}.decision`, errors)
    if (!includes(DECISION_TRACE_OUTCOMES, value.decision.outcome)) {
      errors.push(issue("invalid_decision_trace_decision", "Decision Trace decision has an invalid outcome.", `${path}.decision.outcome`))
    }
    if (!isNonEmptyString(value.decision.rationale)) {
      errors.push(issue("invalid_decision_trace_decision", "Decision Trace decision requires a non-empty rationale.", `${path}.decision.rationale`))
    }
    if (value.decision.approved_by !== undefined && (
      !Array.isArray(value.decision.approved_by) ||
      value.decision.approved_by.some((approver) => !isNonEmptyString(approver))
    )) {
      errors.push(issue("invalid_decision_trace_decision", "Decision Trace approved_by must contain only non-empty strings.", `${path}.decision.approved_by`))
    }
  }

  if (value.result !== undefined) {
    if (!isRecord(value.result)) {
      errors.push(issue("invalid_decision_trace_result", "Decision Trace event result must be an object.", `${path}.result`))
    } else {
      rejectUnexpectedKeys(value.result, ["new_product_spec_revision", "linked_artifacts", "learning"], `${path}.result`, errors)
      if (value.result.new_product_spec_revision !== undefined && !isPositiveInteger(value.result.new_product_spec_revision)) {
        errors.push(issue("invalid_decision_trace_result", "Decision Trace result new_product_spec_revision must be a positive integer.", `${path}.result.new_product_spec_revision`))
      }
      if (value.result.linked_artifacts !== undefined) validateLinks(value.result.linked_artifacts, `${path}.result.linked_artifacts`, errors)
      if (value.result.learning !== undefined && typeof value.result.learning !== "string") {
        errors.push(issue("invalid_decision_trace_result", "Decision Trace result learning must be a string.", `${path}.result.learning`))
      }
    }
  }
}

const validateUnknownDecisionTrace = (value: unknown): DecisionTraceValidationResult => {
  if (!isRecord(value)) {
    return { valid: false, errors: [issue("invalid_decision_trace", "Decision Trace must be a JSON object.")] }
  }

  const errors: ValidationIssue[] = []
  const required = [
    "decision_trace_format_version",
    "trace_id",
    "title",
    "created_at",
    "updated_at",
    "subject",
    "events",
  ] as const
  rejectUnexpectedKeys(value, required, "decision_trace", errors)
  for (const field of required) {
    if (!(field in value)) {
      errors.push(issue(
        "missing_decision_trace_field",
        `Decision Trace is missing required field: ${field}.`,
        field,
      ))
    }
  }
  if (value.decision_trace_format_version !== DECISION_TRACE_FORMAT_VERSION) {
    errors.push(issue(
      "unsupported_decision_trace_version",
      `Unsupported decision_trace_format_version: ${String(value.decision_trace_format_version)}.`,
      "decision_trace_format_version",
    ))
  }
  if (!isNonEmptyString(value.trace_id) || !IDENTIFIER.test(value.trace_id)) {
    errors.push(issue("invalid_trace_id", "Decision Trace trace_id must use lowercase kebab-case or snake_case.", "trace_id"))
  }
  if (!isNonEmptyString(value.title)) {
    errors.push(issue("invalid_decision_trace_title", "Decision Trace title must be a non-empty string.", "title"))
  }
  for (const field of ["created_at", "updated_at"] as const) {
    if (!isIsoDateTime(value[field])) {
      errors.push(issue("invalid_decision_trace_datetime", `Decision Trace ${field} must be an ISO 8601 date-time.`, field))
    }
  }
  validateSubject(value.subject, errors)
  if (!Array.isArray(value.events) || value.events.length === 0) {
    errors.push(issue("empty_decision_trace_events", "Decision Trace events must contain at least one event.", "events"))
  } else {
    const eventIds = new Set<string>()
    value.events.forEach((event, index) => validateEvent(event, index, eventIds, errors))
  }

  if (errors.length > 0) return { valid: false, errors }
  try {
    return { valid: true, trace: S.decodeUnknownSync(DecisionTraceSchema)(value), errors: [] }
  } catch (error) {
    return {
      valid: false,
      errors: [issue(
        "invalid_decision_trace",
        `Decision Trace failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
      )],
    }
  }
}

export const validateDecisionTrace = (input: string | unknown): DecisionTraceValidationResult => {
  if (typeof input !== "string") return validateUnknownDecisionTrace(input)
  try {
    return validateUnknownDecisionTrace(JSON.parse(input))
  } catch {
    return {
      valid: false,
      errors: [issue("invalid_decision_trace_json", "Decision Trace must contain valid JSON.")],
    }
  }
}

export const parseDecisionTrace = (input: string | unknown): DecisionTrace => {
  const result = validateDecisionTrace(input)
  if (!result.valid) throw new DecisionTraceValidationError(result.errors)
  return result.trace
}
