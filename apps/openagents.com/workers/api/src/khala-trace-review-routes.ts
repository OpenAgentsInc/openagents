// Khala trace-review operator route (#6356).
//
//   GET /api/operator/khala/trace-review
//
// Owner/admin scoped. Builds a recurring review report over agent traces,
// token usage events, and Pylon/Codex raw-event metadata. The response is
// aggregate/ref-only: it never returns raw trajectories or raw SDK payloads.

import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  normalizeIsoTimestamp,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

const KHALA_TRACE_REVIEW_DEFAULT_HOURS = 24
const KHALA_TRACE_REVIEW_MAX_HOURS = 24 * 14
const KHALA_TRACE_REVIEW_MAX_LIMIT = 50

export type KhalaTraceReviewWindow = Readonly<{
  since: string
  until: string
  hours: number
}>

export type KhalaTraceReviewBucket = Readonly<{
  label: string
  count: number
  totalTokens: number
}>

export type KhalaTraceReviewModelBucket = Readonly<{
  provider: string
  model: string
  count: number
  totalTokens: number
}>

export type KhalaTraceReviewOutcomeBucket = Readonly<{
  outcome: string
  count: number
  totalTokens: number
}>

export type KhalaTraceReviewNotableTrace = Readonly<{
  traceUuid: string
  traceRef: string
  createdAt: string
  demandKind: string
  demandSource: string
  stepCount: number
  visibility: string
  reasonRefs: ReadonlyArray<string>
}>

export type KhalaTraceReviewRawEventHighlight = Readonly<{
  assignmentRef: string
  byteLength: number
  eventCount: number
  observedAt: string
  rawEventRef: string
}>

export type KhalaTraceReviewBackendSignalKind =
  | 'unhandled_exception'
  | 'gateway_timeout'
  | 'agent_crash'

export type KhalaTraceReviewBackendSignalBucket = Readonly<{
  signalKind: KhalaTraceReviewBackendSignalKind
  surface: string
  count: number
  latestObservedAt: string
  evidenceRefs: ReadonlyArray<string>
}>

export type KhalaTraceReviewBackendSignalSummary = Readonly<{
  signalCount: number
  unhandledExceptionCount: number
  gatewayTimeoutCount: number
  agentCrashCount: number
}>

export type KhalaTraceReviewFailureMode = Readonly<{
  failureRef: string
  label: string
  count: number
  severity: 'info' | 'warning' | 'critical'
  evidenceRefs: ReadonlyArray<string>
}>

export type KhalaTraceReviewUserIntent = Readonly<{
  intentRef: string
  label: string
  count: number
  evidenceRefs: ReadonlyArray<string>
}>

export type KhalaTraceReviewTriageItem = Readonly<{
  triageRef: string
  kind: 'bug' | 'investigation' | 'missing_capability'
  priority: 'low' | 'medium' | 'high'
  title: string
  evidenceRefs: ReadonlyArray<string>
  suggestedIssueTitle: string
}>

export type KhalaTraceReviewFacts = Readonly<{
  tokenSummary: {
    eventCount: number
    totalTokens: number
    inputTokens: number
    outputTokens: number
    reasoningTokens: number
    zeroOutputCount: number
    estimatedUsageCount: number
  }
  tokenByDemandSource: ReadonlyArray<KhalaTraceReviewBucket>
  modelMix: ReadonlyArray<KhalaTraceReviewModelBucket>
  outcomes: ReadonlyArray<KhalaTraceReviewOutcomeBucket>
  traceSummary: {
    traceCount: number
    zeroStepCount: number
    trainingConsentCount: number
    ownerOnlyCount: number
    publicCount: number
    unlistedCount: number
  }
  traceByDemandSource: ReadonlyArray<KhalaTraceReviewBucket>
  notableTraces: ReadonlyArray<KhalaTraceReviewNotableTrace>
  rawEventSummary: {
    rowCount: number
    assignmentCount: number
    eventCount: number
    byteLength: number
  }
  rawEventHighlights: ReadonlyArray<KhalaTraceReviewRawEventHighlight>
  backendSignalSummary: KhalaTraceReviewBackendSignalSummary
  backendSignalBuckets: ReadonlyArray<KhalaTraceReviewBackendSignalBucket>
}>

export type KhalaTraceReviewReport = Readonly<{
  schemaVersion: 'openagents.khala.trace_review.v1'
  reportRef: string
  generatedAt: string
  window: KhalaTraceReviewWindow
  sourceTables: ReadonlyArray<
    | 'agent_traces'
    | 'token_usage_events'
    | 'pylon_codex_raw_events'
    | 'backend_exception_signals'
  >
  aggregates: {
    tokens: KhalaTraceReviewFacts['tokenSummary']
    traces: KhalaTraceReviewFacts['traceSummary']
    rawCodexEvents: KhalaTraceReviewFacts['rawEventSummary']
    backendSignals: KhalaTraceReviewFacts['backendSignalSummary']
  }
  backendSignals: ReadonlyArray<KhalaTraceReviewBackendSignalBucket>
  modelMix: ReadonlyArray<KhalaTraceReviewModelBucket>
  demandSources: ReadonlyArray<KhalaTraceReviewBucket>
  outcomes: ReadonlyArray<KhalaTraceReviewOutcomeBucket>
  notableTraces: ReadonlyArray<KhalaTraceReviewNotableTrace>
  userIntents: ReadonlyArray<KhalaTraceReviewUserIntent>
  failureModes: ReadonlyArray<KhalaTraceReviewFailureMode>
  triageItems: ReadonlyArray<KhalaTraceReviewTriageItem>
  backlogFeed: {
    producedItemCount: number
    itemRefs: ReadonlyArray<string>
  }
}>

export type KhalaTraceReviewStore = Readonly<{
  readFacts: (input: {
    limit: number
    window: KhalaTraceReviewWindow
  }) => Promise<KhalaTraceReviewFacts>
}>

export type BackendExceptionSignalInput = Readonly<{
  signalRef: string
  observedAt: string
  signalKind: KhalaTraceReviewBackendSignalKind
  source: 'worker_tail' | 'workers_logs' | 'logpush' | 'queue' | 'worker_catch'
  surface: string
  outcome: string
  statusCode?: number | null | undefined
  traceRef?: string | null | undefined
  requestRef?: string | null | undefined
  agentRef?: string | null | undefined
  assignmentRef?: string | null | undefined
  errorClass?: string | null | undefined
  sanitizedMessage?: string | null | undefined
  fingerprint: string
}>

export type BackendExceptionSignalRuntimeInput = Readonly<{
  error: unknown
  observedAt: string
  request: Request
  signalRef: string
}>

export type OperatorKhalaTraceReviewDependencies = Readonly<{
  requireAdminApiToken: (request: Request) => Promise<boolean>
  store: KhalaTraceReviewStore
  nowIso?: (() => string) | undefined
}>

class KhalaTraceReviewStorageError extends S.TaggedErrorClass<KhalaTraceReviewStorageError>()(
  'KhalaTraceReviewStorageError',
  {
    reason: S.String,
  },
) {}

const n = (value: unknown): number => {
  const parsed = typeof value === 'number' ? value : Number(value ?? 0)
  return Number.isFinite(parsed) ? parsed : 0
}

const s = (value: unknown, fallback = 'unknown'): string => {
  const text = typeof value === 'string' ? value.trim() : ''
  return text.length === 0 ? fallback : text
}

const clampInteger = (value: number, min: number, max: number): number =>
  Math.min(Math.max(Math.trunc(value), min), max)

const normalizeIsoTimestampOr = (
  timestamp: string | null,
  fallback: string,
): string => {
  if (timestamp === null) {
    return fallback
  }

  try {
    return normalizeIsoTimestamp(timestamp)
  } catch {
    return fallback
  }
}

const parseWindow = (
  request: Request,
  nowIso: () => string,
): KhalaTraceReviewWindow => {
  const url = new URL(request.url)
  const fallbackUntil = nowIso()
  const safeUntil = normalizeIsoTimestampOr(
    url.searchParams.get('until'),
    fallbackUntil,
  )
  const requestedHours = Number(url.searchParams.get('hours') ?? KHALA_TRACE_REVIEW_DEFAULT_HOURS)
  const hours = Number.isFinite(requestedHours)
    ? clampInteger(requestedHours, 1, KHALA_TRACE_REVIEW_MAX_HOURS)
    : KHALA_TRACE_REVIEW_DEFAULT_HOURS
  return {
    hours,
    since: isoTimestampAfterIso(safeUntil, -hours * 60 * 60 * 1000),
    until: safeUntil,
  }
}

const parseLimit = (request: Request): number => {
  const url = new URL(request.url)
  const requestedLimit = Number(url.searchParams.get('limit') ?? '10')
  return Number.isFinite(requestedLimit)
    ? clampInteger(requestedLimit, 1, KHALA_TRACE_REVIEW_MAX_LIMIT)
    : 10
}

const stableLabelRef = (prefix: string, label: string): string =>
  `${prefix}.${label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 80) || 'unknown'}`

const stableSignalRefPart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 80) || 'unknown'

const backendSignalKindLabel = (
  kind: KhalaTraceReviewBackendSignalKind,
): string => {
  switch (kind) {
    case 'agent_crash':
      return 'Silent agent crash signals'
    case 'gateway_timeout':
      return 'Gateway timeout signals'
    case 'unhandled_exception':
      return 'Unhandled backend exceptions'
  }
}

const backendSignalKindRef = (
  kind: KhalaTraceReviewBackendSignalKind,
): string => `failure.khala_trace_review.backend_${kind}`

const backendSignalKindFrom = (
  value: unknown,
): KhalaTraceReviewBackendSignalKind => {
  switch (value) {
    case 'agent_crash':
    case 'gateway_timeout':
    case 'unhandled_exception':
      return value
    default:
      return 'unhandled_exception'
  }
}

const nullableString = (value: string | null | undefined): string | null => {
  const text = typeof value === 'string' ? value.trim() : ''
  return text.length === 0 ? null : text
}

export const backendExceptionSignalFromWorkerError = (
  input: BackendExceptionSignalRuntimeInput,
): BackendExceptionSignalInput => {
  const url = new URL(input.request.url)
  const errorClass =
    input.error instanceof Error && input.error.name.trim().length > 0
      ? input.error.name.trim().slice(0, 80)
      : 'Error'
  const surface = url.pathname === '' ? '/' : url.pathname

  return {
    errorClass,
    fingerprint: `worker_catch:${surface}:${errorClass}`,
    observedAt: input.observedAt,
    outcome: 'exception',
    requestRef: input.request.headers.get('cf-ray'),
    sanitizedMessage: 'Unhandled Worker exception captured by backend signal sink.',
    signalKind: 'unhandled_exception',
    signalRef: input.signalRef,
    source: 'worker_catch',
    statusCode: 500,
    surface,
  }
}

export const insertBackendExceptionSignal = async (
  db: D1Database,
  input: BackendExceptionSignalInput,
): Promise<void> => {
  const sanitizedMessage = nullableString(input.sanitizedMessage)
  if (
    sanitizedMessage !== null &&
    containsProviderSecretMaterial(sanitizedMessage)
  ) {
    throw new KhalaTraceReviewStorageError({
      reason: 'backend exception signal contained private-data-shaped material',
    })
  }

  await db
    .prepare(
      `INSERT OR IGNORE INTO backend_exception_signals (
         signal_ref, observed_at, signal_kind, source, surface, outcome,
         status_code, trace_ref, request_ref, agent_ref, assignment_ref,
         error_class, sanitized_message, fingerprint, created_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      input.signalRef,
      input.observedAt,
      input.signalKind,
      input.source,
      input.surface,
      input.outcome,
      input.statusCode ?? null,
      nullableString(input.traceRef),
      nullableString(input.requestRef),
      nullableString(input.agentRef),
      nullableString(input.assignmentRef),
      nullableString(input.errorClass),
      sanitizedMessage,
      input.fingerprint,
      currentIsoTimestamp(),
    )
    .run()
}

const publicSafeReport = (report: KhalaTraceReviewReport): KhalaTraceReviewReport => {
  // Defensive secret-material backstop. The report is aggregate/ref-only by
  // construction, but we double-check before returning. We EXCLUDE the bounded
  // model/provider IDENTIFIER fields from the scan: those are controlled routing
  // identifiers read straight from `token_usage_events.provider/model` (never
  // credential input), and a legitimate serving identifier such as
  // `hydralisk-vllm-glm-5p2-reap-504b` contains an `sk-...`-shaped substring
  // (`sk-vllm-glm-5p2-reap-504b`) that the blunt OpenAI-key heuristic would
  // otherwise false-positive on, throwing and taking the whole operator report
  // down with a 500. Every OTHER field (refs, demand sources, outcomes, titles,
  // notable traces) is still scanned for real secret material.
  const scanProjection = {
    ...report,
    modelMix: report.modelMix.map(bucket => ({
      ...bucket,
      model: '',
      provider: '',
    })),
  }
  const serialized = JSON.stringify(scanProjection)
  if (containsProviderSecretMaterial(serialized)) {
    throw new KhalaTraceReviewStorageError({
      reason: 'trace review report contained private-data-shaped material',
    })
  }
  return report
}

export const makeD1KhalaTraceReviewStore = (
  db: D1Database,
): KhalaTraceReviewStore => ({
  readFacts: async input => {
    const [tokenSummary, demandRows, modelRows, outcomeRows, traceSummary, traceDemandRows, notableRows, rawSummary, rawRows, backendSignalSummary, backendSignalRows] = await Promise.all([
      db
        .prepare(
          `SELECT COUNT(*) AS event_count,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(CASE WHEN output_tokens = 0 THEN 1 ELSE 0 END), 0) AS zero_output_count,
                  COALESCE(SUM(CASE WHEN usage_truth != 'exact' THEN 1 ELSE 0 END), 0) AS estimated_usage_count
             FROM token_usage_events
            WHERE observed_at >= ? AND observed_at < ?`,
        )
        .bind(input.window.since, input.window.until)
        .first<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COALESCE(NULLIF(demand_source, ''), 'unknown') AS label,
                  COUNT(*) AS count,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens
             FROM token_usage_events
            WHERE observed_at >= ? AND observed_at < ?
            GROUP BY COALESCE(NULLIF(demand_source, ''), 'unknown')
            ORDER BY total_tokens DESC, count DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COALESCE(NULLIF(provider, ''), 'unknown') AS provider,
                  COALESCE(NULLIF(model, ''), 'unknown') AS model,
                  COUNT(*) AS count,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens
             FROM token_usage_events
            WHERE observed_at >= ? AND observed_at < ?
            GROUP BY COALESCE(NULLIF(provider, ''), 'unknown'),
                     COALESCE(NULLIF(model, ''), 'unknown')
            ORDER BY total_tokens DESC, count DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COALESCE(
                    NULLIF(json_extract(safe_metadata_json, '$.finishReason'), ''),
                    NULLIF(json_extract(safe_metadata_json, '$.finish'), ''),
                    NULLIF(json_extract(safe_metadata_json, '$.finish_reason'), ''),
                    'unknown'
                  ) AS outcome,
                  COUNT(*) AS count,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens
             FROM token_usage_events
            WHERE observed_at >= ? AND observed_at < ?
            GROUP BY outcome
            ORDER BY count DESC, total_tokens DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COUNT(*) AS trace_count,
                  COALESCE(SUM(CASE WHEN step_count = 0 THEN 1 ELSE 0 END), 0) AS zero_step_count,
                  COALESCE(SUM(CASE WHEN training_consent = 1 THEN 1 ELSE 0 END), 0) AS training_consent_count,
                  COALESCE(SUM(CASE WHEN visibility = 'owner_only' THEN 1 ELSE 0 END), 0) AS owner_only_count,
                  COALESCE(SUM(CASE WHEN visibility = 'public' THEN 1 ELSE 0 END), 0) AS public_count,
                  COALESCE(SUM(CASE WHEN visibility = 'unlisted' THEN 1 ELSE 0 END), 0) AS unlisted_count
             FROM agent_traces
            WHERE created_at >= ? AND created_at < ?`,
        )
        .bind(input.window.since, input.window.until)
        .first<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COALESCE(NULLIF(demand_source, ''), 'unknown') AS label,
                  COUNT(*) AS count,
                  0 AS total_tokens
             FROM agent_traces
            WHERE created_at >= ? AND created_at < ?
            GROUP BY COALESCE(NULLIF(demand_source, ''), 'unknown')
            ORDER BY count DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT trace_uuid,
                  created_at,
                  COALESCE(demand_kind, 'unlabeled') AS demand_kind,
                  COALESCE(NULLIF(demand_source, ''), 'unknown') AS demand_source,
                  step_count,
                  visibility,
                  training_consent
             FROM agent_traces
            WHERE created_at >= ? AND created_at < ?
            ORDER BY step_count DESC, created_at DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COUNT(*) AS row_count,
                  COUNT(DISTINCT assignment_ref) AS assignment_count,
                  COALESCE(SUM(event_count), 0) AS event_count,
                  COALESCE(SUM(byte_length), 0) AS byte_length
             FROM pylon_codex_raw_events
            WHERE observed_at >= ? AND observed_at < ?`,
        )
        .bind(input.window.since, input.window.until)
        .first<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT raw_event_ref,
                  assignment_ref,
                  event_count,
                  byte_length,
                  observed_at
             FROM pylon_codex_raw_events
            WHERE observed_at >= ? AND observed_at < ?
            ORDER BY byte_length DESC, event_count DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT COUNT(*) AS signal_count,
                  COALESCE(SUM(CASE WHEN signal_kind = 'unhandled_exception' THEN 1 ELSE 0 END), 0) AS unhandled_exception_count,
                  COALESCE(SUM(CASE WHEN signal_kind = 'gateway_timeout' THEN 1 ELSE 0 END), 0) AS gateway_timeout_count,
                  COALESCE(SUM(CASE WHEN signal_kind = 'agent_crash' THEN 1 ELSE 0 END), 0) AS agent_crash_count
             FROM backend_exception_signals
            WHERE observed_at >= ? AND observed_at < ?`,
        )
        .bind(input.window.since, input.window.until)
        .first<Record<string, unknown>>(),
      db
        .prepare(
          `SELECT signal_kind,
                  COALESCE(NULLIF(surface, ''), 'unknown') AS surface,
                  COUNT(*) AS count,
                  MAX(observed_at) AS latest_observed_at,
                  GROUP_CONCAT(signal_ref, ',') AS signal_refs
             FROM backend_exception_signals
            WHERE observed_at >= ? AND observed_at < ?
            GROUP BY signal_kind, COALESCE(NULLIF(surface, ''), 'unknown')
            ORDER BY count DESC, latest_observed_at DESC
            LIMIT ?`,
        )
        .bind(input.window.since, input.window.until, input.limit)
        .all<Record<string, unknown>>(),
    ])

    return {
      backendSignalBuckets: (backendSignalRows.results ?? []).map(row => ({
        count: n(row.count),
        evidenceRefs: s(row.signal_refs, '')
          .split(',')
          .map(ref => ref.trim())
          .filter(ref => ref.length > 0)
          .slice(0, 5),
        latestObservedAt: s(row.latest_observed_at),
        signalKind: backendSignalKindFrom(row.signal_kind),
        surface: s(row.surface),
      })),
      backendSignalSummary: {
        agentCrashCount: n(backendSignalSummary?.agent_crash_count),
        gatewayTimeoutCount: n(backendSignalSummary?.gateway_timeout_count),
        signalCount: n(backendSignalSummary?.signal_count),
        unhandledExceptionCount: n(
          backendSignalSummary?.unhandled_exception_count,
        ),
      },
      modelMix: (modelRows.results ?? []).map(row => ({
        count: n(row.count),
        model: s(row.model),
        provider: s(row.provider),
        totalTokens: n(row.total_tokens),
      })),
      notableTraces: (notableRows.results ?? []).map(row => {
        const traceUuid = s(row.trace_uuid)
        const reasonRefs = [
          n(row.step_count) === 0 ? 'reason.trace_review.zero_step_trace' : null,
          row.training_consent === 1 ? 'reason.trace_review.training_consent' : null,
          row.visibility === 'owner_only' ? 'reason.trace_review.owner_only_trace' : null,
        ].filter((ref): ref is string => ref !== null)
        return {
          createdAt: s(row.created_at),
          demandKind: s(row.demand_kind, 'unlabeled'),
          demandSource: s(row.demand_source),
          reasonRefs,
          stepCount: n(row.step_count),
          traceRef: `trace.${traceUuid}`,
          traceUuid,
          visibility: s(row.visibility),
        }
      }),
      outcomes: (outcomeRows.results ?? []).map(row => ({
        count: n(row.count),
        outcome: s(row.outcome),
        totalTokens: n(row.total_tokens),
      })),
      rawEventHighlights: (rawRows.results ?? []).map(row => ({
        assignmentRef: s(row.assignment_ref),
        byteLength: n(row.byte_length),
        eventCount: n(row.event_count),
        observedAt: s(row.observed_at),
        rawEventRef: s(row.raw_event_ref),
      })),
      rawEventSummary: {
        assignmentCount: n(rawSummary?.assignment_count),
        byteLength: n(rawSummary?.byte_length),
        eventCount: n(rawSummary?.event_count),
        rowCount: n(rawSummary?.row_count),
      },
      tokenByDemandSource: (demandRows.results ?? []).map(row => ({
        count: n(row.count),
        label: s(row.label),
        totalTokens: n(row.total_tokens),
      })),
      tokenSummary: {
        estimatedUsageCount: n(tokenSummary?.estimated_usage_count),
        eventCount: n(tokenSummary?.event_count),
        inputTokens: n(tokenSummary?.input_tokens),
        outputTokens: n(tokenSummary?.output_tokens),
        reasoningTokens: n(tokenSummary?.reasoning_tokens),
        totalTokens: n(tokenSummary?.total_tokens),
        zeroOutputCount: n(tokenSummary?.zero_output_count),
      },
      traceByDemandSource: (traceDemandRows.results ?? []).map(row => ({
        count: n(row.count),
        label: s(row.label),
        totalTokens: 0,
      })),
      traceSummary: {
        ownerOnlyCount: n(traceSummary?.owner_only_count),
        publicCount: n(traceSummary?.public_count),
        traceCount: n(traceSummary?.trace_count),
        trainingConsentCount: n(traceSummary?.training_consent_count),
        unlistedCount: n(traceSummary?.unlisted_count),
        zeroStepCount: n(traceSummary?.zero_step_count),
      },
    }
  },
})

export const buildKhalaTraceReviewReport = (input: {
  facts: KhalaTraceReviewFacts
  generatedAt: string
  window: KhalaTraceReviewWindow
}): KhalaTraceReviewReport => {
  const failureModes: KhalaTraceReviewFailureMode[] = []

  if (input.facts.tokenSummary.zeroOutputCount > 0) {
    failureModes.push({
      count: input.facts.tokenSummary.zeroOutputCount,
      evidenceRefs: ['table.token_usage_events.output_tokens_zero'],
      failureRef: 'failure.khala_trace_review.empty_response',
      label: 'Token rows with zero completion/output tokens',
      severity: 'warning',
    })
  }

  if (input.facts.tokenSummary.estimatedUsageCount > 0) {
    failureModes.push({
      count: input.facts.tokenSummary.estimatedUsageCount,
      evidenceRefs: ['table.token_usage_events.usage_truth_not_exact'],
      failureRef: 'failure.khala_trace_review.estimated_usage',
      label: 'Token rows without exact usage truth',
      severity: 'warning',
    })
  }

  if (input.facts.traceSummary.zeroStepCount > 0) {
    failureModes.push({
      count: input.facts.traceSummary.zeroStepCount,
      evidenceRefs: ['table.agent_traces.step_count_zero'],
      failureRef: 'failure.khala_trace_review.empty_trace',
      label: 'Captured traces with no steps',
      severity: 'warning',
    })
  }

  if (
    input.facts.rawEventSummary.rowCount > 0 &&
    input.facts.rawEventSummary.assignmentCount === 0
  ) {
    failureModes.push({
      count: input.facts.rawEventSummary.rowCount,
      evidenceRefs: ['table.pylon_codex_raw_events.assignment_ref_missing'],
      failureRef: 'failure.khala_trace_review.raw_event_unassigned',
      label: 'Raw Codex event rows without assignment grouping',
      severity: 'critical',
    })
  }

  for (const bucket of input.facts.backendSignalBuckets) {
    failureModes.push({
      count: bucket.count,
      evidenceRefs:
        bucket.evidenceRefs.length === 0
          ? [
              `table.backend_exception_signals.${bucket.signalKind}.${stableSignalRefPart(bucket.surface)}`,
            ]
          : bucket.evidenceRefs,
      failureRef: `${backendSignalKindRef(bucket.signalKind)}.${stableSignalRefPart(bucket.surface)}`,
      label: `${backendSignalKindLabel(bucket.signalKind)} on ${bucket.surface}`,
      severity: 'critical',
    })
  }

  const userIntents = input.facts.traceByDemandSource.map(bucket => ({
    count: bucket.count,
    evidenceRefs: [`demand_source.${bucket.label}`],
    intentRef: stableLabelRef('intent.khala_trace_review', bucket.label),
    label: bucket.label,
  }))

  const triageItems: KhalaTraceReviewTriageItem[] = failureModes.map(failure => ({
    evidenceRefs: failure.evidenceRefs,
    kind: failure.failureRef.includes('empty') ? 'bug' : 'investigation',
    priority: failure.severity === 'critical' ? 'high' : 'medium',
    suggestedIssueTitle: `[Khala trace review] ${failure.label}`,
    title: failure.label,
    triageRef: failure.failureRef.replace('failure.', 'triage.'),
  }))

  for (const intent of userIntents.slice(0, 5)) {
    if (intent.label !== 'unknown' && intent.count > 0) {
      triageItems.push({
        evidenceRefs: intent.evidenceRefs,
        kind: 'missing_capability',
        priority: 'low',
        suggestedIssueTitle: `[Khala intent] Review ${intent.label} traces`,
        title: `Review recurring user intent: ${intent.label}`,
        triageRef: intent.intentRef.replace('intent.', 'triage.intent.'),
      })
    }
  }

  const demandSources = [
    ...input.facts.tokenByDemandSource,
    ...input.facts.traceByDemandSource.filter(
      traceBucket =>
        !input.facts.tokenByDemandSource.some(
          tokenBucket => tokenBucket.label === traceBucket.label,
        ),
    ),
  ]

  return publicSafeReport({
    aggregates: {
      backendSignals: input.facts.backendSignalSummary,
      rawCodexEvents: input.facts.rawEventSummary,
      tokens: input.facts.tokenSummary,
      traces: input.facts.traceSummary,
    },
    backlogFeed: {
      itemRefs: triageItems.map(item => item.triageRef),
      producedItemCount: triageItems.length,
    },
    backendSignals: input.facts.backendSignalBuckets,
    demandSources,
    failureModes,
    generatedAt: input.generatedAt,
    modelMix: input.facts.modelMix,
    notableTraces: input.facts.notableTraces.map(trace => ({
      ...trace,
      reasonRefs:
        trace.reasonRefs.length === 0
          ? ['reason.trace_review.high_step_count']
          : trace.reasonRefs,
    })),
    outcomes: input.facts.outcomes,
    reportRef: `khala_trace_review.${input.window.until.replace(/[^0-9A-Za-z]+/g, '_')}`,
    schemaVersion: 'openagents.khala.trace_review.v1',
    sourceTables: [
      'agent_traces',
      'token_usage_events',
      'pylon_codex_raw_events',
      'backend_exception_signals',
    ],
    triageItems,
    userIntents,
    window: input.window,
  })
}

export const handleOperatorKhalaTraceReview = (
  request: Request,
  dependencies: OperatorKhalaTraceReviewDependencies,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = dependencies.nowIso ?? currentIsoTimestamp

  return Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      try: () => dependencies.requireAdminApiToken(request),
      catch: () => false,
    })
    if (!authorized) {
      return unauthorized()
    }

    const window = parseWindow(request, nowIso)
    const limit = parseLimit(request)
    const facts = yield* Effect.tryPromise({
      try: () => dependencies.store.readFacts({ limit, window }),
      catch: error =>
        new KhalaTraceReviewStorageError({
          reason: error instanceof Error ? error.message : String(error),
        }),
    })

    const report = yield* Effect.try({
      try: () => buildKhalaTraceReviewReport({
        facts,
        generatedAt: nowIso(),
        window,
      }),
      catch: error =>
        error instanceof KhalaTraceReviewStorageError
          ? error
          : new KhalaTraceReviewStorageError({
              reason: error instanceof Error ? error.message : String(error),
            }),
    })

    return noStoreJsonResponse(report)
  }).pipe(
    Effect.catchTag('KhalaTraceReviewStorageError', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'khala_trace_review_unavailable' },
          { status: 500 },
        ),
      ),
    ),
  )
}
