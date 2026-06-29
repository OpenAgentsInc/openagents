import { Effect } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import type {
  BoundedDispatchFailureTelemetry,
  DispatchFailureTelemetryClassifier,
  DispatchFailureTelemetryEvent,
} from './model-router'

const ALL_FAILURE_CLASSES: ReadonlyArray<DispatchFailureTelemetryClassifier> = [
  'provider_error',
  'empty_content',
  'fallback',
  'invalid_tool',
  'rate_limited_429',
]

type RedactedDispatchFailureEvent = Readonly<{
  classifier: DispatchFailureTelemetryClassifier
  stage: DispatchFailureTelemetryEvent['stage']
  retryable: boolean
  statusClass: 'http_429' | 'http_4xx' | 'http_5xx' | 'none'
}>

export type PublicDispatchFailureTelemetryReadout = Readonly<{
  schemaVersion: 'openagents.dispatch_failure_telemetry.v1'
  generatedAtMs: number
  staleness: 'live_at_read'
  windowMs: number
  counts: Readonly<Record<DispatchFailureTelemetryClassifier, number>>
  recentEvents: ReadonlyArray<RedactedDispatchFailureEvent>
}>

const emptyCounts = (): Record<DispatchFailureTelemetryClassifier, number> => ({
  empty_content: 0,
  fallback: 0,
  invalid_tool: 0,
  provider_error: 0,
  rate_limited_429: 0,
})

const statusClassFor = (
  httpStatus: number | undefined,
): RedactedDispatchFailureEvent['statusClass'] => {
  if (httpStatus === 429) {
    return 'http_429'
  }
  if (httpStatus !== undefined && httpStatus >= 500 && httpStatus <= 599) {
    return 'http_5xx'
  }
  if (httpStatus !== undefined && httpStatus >= 400 && httpStatus <= 499) {
    return 'http_4xx'
  }
  return 'none'
}

const redactEvent = (
  event: DispatchFailureTelemetryEvent,
): RedactedDispatchFailureEvent => ({
  classifier: event.classifier,
  retryable: event.retryable,
  stage: event.stage,
  statusClass: statusClassFor(event.httpStatus),
})

export const publicDispatchFailureTelemetryReadout = (
  telemetry: BoundedDispatchFailureTelemetry,
  nowMs: number,
): PublicDispatchFailureTelemetryReadout => {
  const snapshot = telemetry.snapshot(nowMs)
  const counts = emptyCounts()
  for (const classifier of ALL_FAILURE_CLASSES) {
    counts[classifier] = snapshot.counts[classifier] ?? 0
  }
  return {
    counts,
    generatedAtMs: nowMs,
    recentEvents: snapshot.events.map(redactEvent),
    schemaVersion: 'openagents.dispatch_failure_telemetry.v1',
    staleness: 'live_at_read',
    windowMs: snapshot.windowMs,
  }
}

export const handleDispatchFailureTelemetryReadout = (
  request: Request,
  input: Readonly<{
    enabled: boolean
    nowMs: () => number
    telemetry: BoundedDispatchFailureTelemetry
  }>,
) =>
  Effect.sync(() => {
    if (!input.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }
    if (request.method !== 'GET') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }
    try {
      return noStoreJsonResponse(
        publicDispatchFailureTelemetryReadout(input.telemetry, input.nowMs()),
      )
    } catch {
      return noStoreJsonResponse({
        counts: emptyCounts(),
        generatedAtMs: input.nowMs(),
        recentEvents: [],
        schemaVersion: 'openagents.dispatch_failure_telemetry.v1',
        staleness: 'live_at_read',
        windowMs: 0,
      })
    }
  })
