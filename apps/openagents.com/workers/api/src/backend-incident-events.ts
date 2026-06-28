import { createHash } from 'node:crypto'

import { redactProviderAccountLogValue } from '@openagentsinc/provider-account-schema'

import { currentIsoTimestamp } from './runtime-primitives'

export type BackendIncidentKind =
  | 'unhandled_exception'
  | 'gateway_timeout'
  | 'silent_agent_crash'

export type BackendIncidentSource =
  | 'worker_fetch'
  | 'tail_worker'
  | 'workers_logs'
  | 'logpush'
  | 'queue_consumer'
  | 'durable_object'
  | 'pylon_local_runner'

export type BackendIncidentSeverity = 'warning' | 'critical'

export type BackendIncidentEventInput = Readonly<{
  errorName?: string | undefined
  kind: BackendIncidentKind
  method?: string | undefined
  observedAt?: string | undefined
  routePattern?: string | undefined
  safeMetadata?: Readonly<Record<string, unknown>> | undefined
  severity?: BackendIncidentSeverity | undefined
  source: BackendIncidentSource
  statusCode?: number | undefined
}>

const routeSegmentPatterns: ReadonlyArray<RegExp> = [
  /[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}/gi,
  /\b(?:chatcmpl|run|assignment|request|trace|pylon|agent|user|team)_[A-Za-z0-9_-]{8,}\b/g,
  /\b[0-9a-f]{24,64}\b/gi,
]

const stableRefSuffix = (value: string): string =>
  createHash('sha256').update(value).digest('hex').slice(0, 24)

const bounded = (value: string, fallback: string, maxLength: number): string => {
  const text = value.trim()
  if (text.length === 0) {
    return fallback
  }
  return text.slice(0, maxLength)
}

export const routePatternFromRequest = (request: Request): string => {
  const url = new URL(request.url)
  const normalizedPath = routeSegmentPatterns.reduce(
    (path, pattern) => path.replace(pattern, ':ref'),
    url.pathname,
  )
  return bounded(normalizedPath, 'unknown', 240)
}

const safeMethod = (method: string | undefined): string =>
  bounded((method ?? 'UNKNOWN').toUpperCase().replace(/[^A-Z]/g, ''), 'UNKNOWN', 12)

const safeStatusCode = (statusCode: number | undefined): number | null => {
  if (statusCode === undefined || !Number.isFinite(statusCode)) {
    return null
  }
  return Math.trunc(statusCode)
}

const safeMetadataJson = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): string => {
  const safeMetadata = Object.fromEntries(
    Object.entries(metadata ?? {})
      .filter(([, value]) => value !== undefined)
      .map(([key, value]) => [
        bounded(key.replace(/[^A-Za-z0-9_.:-]/g, '_'), 'field', 80),
        redactProviderAccountLogValue(value).slice(0, 240),
      ]),
  )
  return JSON.stringify(safeMetadata)
}

export const recordBackendIncidentEvent = async (
  db: D1Database,
  input: BackendIncidentEventInput,
  nowIso: () => string = currentIsoTimestamp,
): Promise<void> => {
  const observedAt = input.observedAt ?? nowIso()
  const routePattern = bounded(input.routePattern ?? 'unknown', 'unknown', 240)
  const method = safeMethod(input.method)
  const statusCode = safeStatusCode(input.statusCode)
  const errorName = bounded(
    (input.errorName ?? 'unknown').replace(/[^A-Za-z0-9_.:-]/g, '_'),
    'unknown',
    120,
  )
  const severity = input.severity ?? 'critical'
  const idempotencySeed = [
    observedAt,
    input.source,
    input.kind,
    routePattern,
    method,
    statusCode ?? 'none',
    errorName,
    safeMetadataJson(input.safeMetadata),
  ].join('\0')
  const suffix = stableRefSuffix(idempotencySeed)
  const id = `backend_incident_event_${suffix}`
  const incidentRef = `backend_incident.${input.kind}.${suffix}`
  const createdAt = nowIso()

  await db
    .prepare(
      `INSERT OR IGNORE INTO backend_incident_events (
         id,
         incident_ref,
         observed_at,
         source,
         kind,
         severity,
         route_pattern,
         method,
         status_code,
         error_name,
         runtime_name,
         occurrence_count,
         safe_metadata_json,
         created_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      id,
      incidentRef,
      observedAt,
      input.source,
      input.kind,
      severity,
      routePattern,
      method,
      statusCode,
      errorName,
      'cloudflare_workers',
      1,
      safeMetadataJson(input.safeMetadata),
      createdAt,
    )
    .run()
}
