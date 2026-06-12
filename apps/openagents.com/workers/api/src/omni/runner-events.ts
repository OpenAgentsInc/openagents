import {
  normalizeOmniRunnerEventPayload,
  type OmniRunnerEvent,
  decodeOmniRunnerEvent,
} from '@openagentsinc/sync-schema'
import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Context, Effect, Layer } from 'effect'

import { nestedUnknown, optionalString, safeJsonRecord } from '../json-boundary'
import { type OmniEventRecord, eventFromRunnerPayload } from '../omni-runs'
import {
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
} from '../runtime-primitives'
import {
  type OmniError,
  omniRunnerCallbackDecodeErrorFromUnknown,
} from './errors'

export type OmniRunnerEventServiceShape = Readonly<{
  decodeCallbackEvent: (
    payload: unknown,
  ) => Effect.Effect<OmniRunnerEvent, OmniError>
  eventFromCallbackPayload: (
    parentId: string,
    fallbackSequence: number,
    payload: Record<string, unknown>,
  ) => Effect.Effect<OmniEventRecord, OmniError>
  eventsFromCallbackPayloads: (
    parentId: string,
    fallbackStart: number,
    payloads: ReadonlyArray<Record<string, unknown>>,
  ) => Effect.Effect<ReadonlyArray<OmniEventRecord>, OmniError>
  providerReauthReason: (
    event: OmniEventRecord,
  ) => Effect.Effect<string | undefined>
}>

export class OmniRunnerEventService extends Context.Service<
  OmniRunnerEventService,
  OmniRunnerEventServiceShape
>()('openagents/OmniRunnerEventService') {}

const eventPayloadRecord = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => safeJsonRecord(event.payloadJson)

const callbackRecord = (payload: unknown): Record<string, unknown> | undefined =>
  typeof payload === 'object' && payload !== null && !Array.isArray(payload)
    ? (payload as Record<string, unknown>)
    : undefined

const callbackDataJsonRecord = (
  record: Record<string, unknown>,
): Record<string, unknown> | undefined =>
  safeJsonRecord(optionalString(record.dataJson))

const callbackNormalizationRecord = (
  record: Record<string, unknown>,
): Record<string, unknown> => {
  const dataJsonRecord = callbackDataJsonRecord(record)

  const merged = dataJsonRecord === undefined
    ? record
    : {
        ...dataJsonRecord,
        ...record,
        createdAtMs: record.createdAtMs ?? dataJsonRecord.createdAtMs,
        emittedAtMs: record.emittedAtMs ?? dataJsonRecord.emittedAtMs,
        emitted_at_ms: record.emitted_at_ms ?? dataJsonRecord.emitted_at_ms,
        sequence: record.sequence ?? dataJsonRecord.sequence,
      }

  if (!containsProviderSecretMaterial(JSON.stringify(merged))) {
    return merged
  }

  return {
    createdAtMs: merged.createdAtMs,
    emittedAtMs: merged.emittedAtMs,
    emitted_at_ms: merged.emitted_at_ms,
    redacted: true,
    sequence: merged.sequence,
    source: merged.source,
    status: merged.status,
    summary: merged.summary,
    type: merged.type,
  }
}

const callbackCreatedAt = (
  record: Record<string, unknown>,
): string | undefined => {
  const explicit =
    optionalString(record.createdAt) ??
    optionalString(record.created_at) ??
    optionalString(record.emittedAt) ??
    optionalString(record.emitted_at)

  if (explicit !== undefined) {
    return explicit
  }

  const emittedAtMs =
    record.emittedAtMs ?? record.emitted_at_ms ?? record.createdAtMs

  return typeof emittedAtMs === 'number' && Number.isFinite(emittedAtMs)
    ? epochMillisToIsoTimestamp(emittedAtMs)
    : undefined
}

const schemaPayloadFromCallback = (
  payload: unknown,
  fallbackSequence: number,
): unknown => {
  const record = callbackRecord(payload)

  if (record === undefined) {
    return payload
  }

  const normalizationRecord = callbackNormalizationRecord(record)
  const normalized = normalizeOmniRunnerEventPayload(
    normalizationRecord,
    fallbackSequence,
  )
  const createdAt =
    callbackCreatedAt(normalizationRecord) ?? currentIsoTimestamp()

  return normalized === undefined || createdAt === undefined
    ? payload
    : {
        artifactRefs: normalized.artifactRefs,
        createdAt,
        ...(normalized.externalEventId === undefined
          ? {}
          : { externalEventId: normalized.externalEventId }),
        payload: normalized.payload,
        sequence: normalized.sequence,
        source: normalized.source,
        ...(normalized.status === undefined ? {} : { status: normalized.status }),
        summary: normalized.summary,
        type: normalized.type,
      }
}

export const providerReauthReasonFromRunnerEvent = (
  event: OmniEventRecord,
): string | undefined => {
  const haystack = [
    event.type,
    event.summary,
    event.status ?? '',
    event.payloadJson ?? '',
  ]
    .join('\n')
    .toLowerCase()
  const isTokenInvalidated =
    haystack.includes('token_invalidated') ||
    haystack.includes('authentication token has been invalidated')

  if (isTokenInvalidated) {
    const payload = eventPayloadRecord(event)
    const raw = safeJsonRecord(optionalString(payload?.dataJson)) ?? payload
    const statusValue =
      nestedUnknown(raw, ['error', 'data', 'statusCode']) ??
      nestedUnknown(raw, ['data', 'error', 'data', 'statusCode']) ??
      nestedUnknown(raw, ['payload', 'error', 'data', 'statusCode']) ??
      nestedUnknown(raw, ['payload', 'data', 'error', 'data', 'statusCode'])
    const status =
      optionalString(statusValue) ??
      (typeof statusValue === 'number' ? String(statusValue) : undefined)
    const detail = [
      'token_invalidated',
      status === undefined ? undefined : `HTTP ${status}`,
    ]
      .filter((value): value is string => value !== undefined)
      .join(', ')

    return detail === ''
      ? 'ChatGPT/Codex account token was invalidated by OpenAI.'
      : `ChatGPT/Codex account token was invalidated by OpenAI (${detail}).`
  }

  if (haystack.includes('x-openai-authorization-error')) {
    return 'ChatGPT/Codex account authorization failed.'
  }

  return undefined
}

const decodeCallbackEvent = (
  payload: unknown,
  fallbackSequence = 1,
): Effect.Effect<OmniRunnerEvent, OmniError> =>
  decodeOmniRunnerEvent(
    schemaPayloadFromCallback(payload, fallbackSequence),
  ).pipe(
    Effect.mapError(error =>
      omniRunnerCallbackDecodeErrorFromUnknown('decode_callback_event', error),
    ),
    Effect.withSpan('OmniRunnerEventService.decodeCallbackEvent'),
  )

const eventFromCallbackPayload = (
  parentId: string,
  fallbackSequence: number,
  payload: Record<string, unknown>,
): Effect.Effect<OmniEventRecord, OmniError> =>
  Effect.try({
    try: () =>
      eventFromRunnerPayload(
        parentId,
        fallbackSequence,
        callbackNormalizationRecord(payload),
      ),
    catch: error =>
      omniRunnerCallbackDecodeErrorFromUnknown(
        'event_from_callback_payload',
        error,
      ),
  }).pipe(Effect.withSpan('OmniRunnerEventService.eventFromCallbackPayload'))

export const makeOmniRunnerEventService = (): OmniRunnerEventServiceShape => ({
  decodeCallbackEvent,
  eventFromCallbackPayload,
  eventsFromCallbackPayloads: (parentId, fallbackStart, payloads) =>
    Effect.all(
      payloads.map((payload, index) =>
        decodeCallbackEvent(payload, fallbackStart + index).pipe(
          Effect.flatMap(() =>
            eventFromCallbackPayload(parentId, fallbackStart + index, payload),
          ),
        ),
      ),
    ).pipe(
      Effect.withSpan('OmniRunnerEventService.eventsFromCallbackPayloads'),
    ),
  providerReauthReason: event =>
    Effect.sync(() => providerReauthReasonFromRunnerEvent(event)).pipe(
      Effect.withSpan('OmniRunnerEventService.providerReauthReason'),
    ),
})

export const OmniRunnerEventServiceLive = Layer.succeed(
  OmniRunnerEventService,
  makeOmniRunnerEventService(),
)
