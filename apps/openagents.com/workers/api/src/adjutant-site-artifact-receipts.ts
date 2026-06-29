import { Effect, Schema as S } from 'effect'

import { nestedUnknown, safeJsonRecord } from './json-boundary'
import type { OmniEventRecord } from './omni-runs'
import { AutopilotSiteStaticAssetsManifest } from './sites'

export const ADJUTANT_SITE_ARTIFACT_RECEIPT_SCHEMA_VERSION =
  'openagents.adjutant.site_artifact_receipt.v1'

export const AdjutantSiteArtifactReceipt = S.Struct({
  schemaVersion: S.Literal(ADJUTANT_SITE_ARTIFACT_RECEIPT_SCHEMA_VERSION),
  siteId: S.String,
  buildStatus: S.Literals(['build_failed', 'saved']),
  staticAssetsManifest: AutopilotSiteStaticAssetsManifest,
  buildCommand: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  d1BindingName: S.optionalKey(S.String),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  r2BindingName: S.optionalKey(S.String),
  sourceArchiveText: S.optionalKey(S.String),
  sourceCommitSha: S.optionalKey(S.String),
  workerModuleR2Key: S.optionalKey(S.String),
  workerModuleText: S.optionalKey(S.String),
})
export type AdjutantSiteArtifactReceipt =
  typeof AdjutantSiteArtifactReceipt.Type

export class AdjutantSiteArtifactReceiptDecodeError extends S.TaggedErrorClass<AdjutantSiteArtifactReceiptDecodeError>()(
  'AdjutantSiteArtifactReceiptDecodeError',
  {
    eventId: S.String,
    error: S.Defect,
  },
) {}

const hasReceiptSchemaVersion = (value: unknown): boolean =>
  typeof value === 'object' &&
  value !== null &&
  !Array.isArray(value) &&
  (value as Record<string, unknown>).schemaVersion ===
    ADJUTANT_SITE_ARTIFACT_RECEIPT_SCHEMA_VERSION

const receiptCandidates = (
  record: Record<string, unknown>,
): ReadonlyArray<unknown> => [
  record.adjutantSiteArtifactReceipt,
  record.siteArtifactReceipt,
  nestedUnknown(record, ['payload', 'adjutantSiteArtifactReceipt']),
  nestedUnknown(record, ['payload', 'siteArtifactReceipt']),
  hasReceiptSchemaVersion(nestedUnknown(record, ['payload']))
    ? nestedUnknown(record, ['payload'])
    : undefined,
  hasReceiptSchemaVersion(record) ? record : undefined,
]

export const receiptFromOmniEvent = (
  event: OmniEventRecord,
): Effect.Effect<
  AdjutantSiteArtifactReceipt | undefined,
  AdjutantSiteArtifactReceiptDecodeError
> =>
  Effect.gen(function* () {
    const record = safeJsonRecord(event.payloadJson)

    if (record === undefined) {
      return undefined
    }

    const candidate = receiptCandidates(record).find(
      value => value !== undefined,
    )

    if (candidate === undefined) {
      return undefined
    }

    return yield* S.decodeUnknownEffect(AdjutantSiteArtifactReceipt)(
      candidate,
    ).pipe(
      Effect.mapError(
        error =>
          new AdjutantSiteArtifactReceiptDecodeError({
            eventId: event.id,
            error,
          }),
      ),
    )
  })

export const firstReceiptFromOmniEvents = (
  events: ReadonlyArray<OmniEventRecord>,
): Effect.Effect<
  AdjutantSiteArtifactReceipt | undefined,
  AdjutantSiteArtifactReceiptDecodeError
> =>
  Effect.gen(function* () {
    for (const event of events) {
      const receipt = yield* receiptFromOmniEvent(event)

      if (receipt !== undefined) {
        return receipt
      }
    }

    return undefined
  })
