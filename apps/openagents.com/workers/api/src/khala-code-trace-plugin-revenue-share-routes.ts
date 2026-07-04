import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { notFound } from '@openagentsinc/sync-worker'
import { Data, Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_OPERATOR_ENDPOINT =
  '/api/operator/khala-code/trace-plugin-revenue-share-precedents' as const
export const KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_ENDPOINT =
  '/api/public/khala-code/trace-plugin-revenue-share-precedents' as const
export const KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_RECEIPT_PATH =
  '/api/public/khala-code/trace-plugin-revenue-share-precedents/:receiptRef' as const
export const KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_TABLE =
  'khala_code_trace_plugin_revenue_share_precedents' as const

export const KhalaCodeTracePluginRevenueSharePayoutRail = S.Literals([
  'spark',
])
export type KhalaCodeTracePluginRevenueSharePayoutRail =
  typeof KhalaCodeTracePluginRevenueSharePayoutRail.Type

export const KhalaCodeTracePluginRevenueSharePrecedentConsent = S.Struct({
  publicReceipt: S.Literal(true),
  noPrivateDataIncluded: S.Literal(true),
  realSettlementReceiptSupplied: S.Literal(true),
})

export const KhalaCodeTracePluginRevenueSharePrecedentIntakeRequest = S.Struct(
  {
    schemaVersion: S.Literal(
      'openagents.khala_code.trace_plugin_revenue_share_precedent_intake.v1',
    ),
    consent: KhalaCodeTracePluginRevenueSharePrecedentConsent,
    consentedTraceReceiptRef: S.String,
    traceDigestRef: S.String,
    pluginAdmissionReceiptRef: S.String,
    pluginRegistryReceiptRef: S.String,
    pluginRef: S.String,
    pluginDigestRef: S.String,
    pluginRouteRef: S.String,
    routedRequestRef: S.String,
    usageEventRef: S.String,
    usageIdempotencyRef: S.String,
    contributorAttributionRef: S.String,
    grossRevenueMsats: S.Number,
    contributorShareMsats: S.Number,
    amountEnvelopeRef: S.String,
    payoutRail: KhalaCodeTracePluginRevenueSharePayoutRail,
    payoutReceiptRef: S.String,
    settlementReceiptRef: S.String,
    idempotencyKey: S.optionalKey(S.String),
  },
)
export type KhalaCodeTracePluginRevenueSharePrecedentIntakeRequest =
  typeof KhalaCodeTracePluginRevenueSharePrecedentIntakeRequest.Type

export const PublicKhalaCodeTracePluginRevenueSharePrecedentReceipt = S.Struct({
  schemaVersion: S.Literal(
    'openagents.khala_code.trace_plugin_revenue_share_precedent_receipt.v1',
  ),
  product: S.Literal('khala-code'),
  promiseIds: S.Array(S.String),
  receiptRef: S.String,
  receiptUrl: S.String,
  generatedAt: S.String,
  recordedAt: S.String,
  trace: S.Struct({
    consentedTraceReceiptRef: S.String,
    traceDigestRef: S.String,
    rawTraceIncluded: S.Literal(false),
  }),
  plugin: S.Struct({
    pluginAdmissionReceiptRef: S.String,
    pluginRegistryReceiptRef: S.String,
    pluginRef: S.String,
    pluginDigestRef: S.String,
    pluginRouteRef: S.String,
    registered: S.Literal(true),
    routable: S.Literal(true),
  }),
  routing: S.Struct({
    routedRequestRef: S.String,
    usageEventRef: S.String,
    usageIdempotencyRef: S.String,
    meteringTruth: S.Literal('exact'),
    meteredUsageEventCount: S.Literal(1),
  }),
  attribution: S.Struct({
    contributorAttributionRef: S.String,
  }),
  revenueShare: S.Struct({
    grossRevenueMsats: S.Number,
    contributorShareMsats: S.Number,
    payoutRail: KhalaCodeTracePluginRevenueSharePayoutRail,
    amountEnvelopeRef: S.String,
    payoutReceiptRef: S.String,
    settlementReceiptRef: S.String,
    state: S.Literal('settled'),
  }),
  publicSafety: S.Struct({
    noRawTrace: S.Literal(true),
    noRawPrompt: S.Literal(true),
    noRawUsagePayload: S.Literal(true),
    noRawPaymentMaterial: S.Literal(true),
    noPayoutDestination: S.Literal(true),
    noWalletMaterial: S.Literal(true),
  }),
  evidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaCodeTracePluginRevenueSharePrecedentReceipt =
  typeof PublicKhalaCodeTracePluginRevenueSharePrecedentReceipt.Type

export const PublicKhalaCodeTracePluginRevenueSharePrecedentEnvelope = S.Struct(
  {
    generatedAt: S.String,
    staleness: PublicProjectionStalenessContract,
    receipt: PublicKhalaCodeTracePluginRevenueSharePrecedentReceipt,
  },
)
export type PublicKhalaCodeTracePluginRevenueSharePrecedentEnvelope =
  typeof PublicKhalaCodeTracePluginRevenueSharePrecedentEnvelope.Type

export const OperatorKhalaCodeTracePluginRevenueSharePrecedentIntakeEnvelope =
  S.Struct({
    ok: S.Literal(true),
    idempotent: S.Boolean,
    generatedAt: S.String,
    staleness: PublicProjectionStalenessContract,
    receipt: PublicKhalaCodeTracePluginRevenueSharePrecedentReceipt,
  })
export type OperatorKhalaCodeTracePluginRevenueSharePrecedentIntakeEnvelope =
  typeof OperatorKhalaCodeTracePluginRevenueSharePrecedentIntakeEnvelope.Type

export type KhalaCodeTracePluginRevenueSharePrecedentRecord = Readonly<{
  receiptRef: string
  consentedTraceReceiptRef: string
  traceDigestRef: string
  pluginAdmissionReceiptRef: string
  pluginRegistryReceiptRef: string
  pluginRef: string
  pluginDigestRef: string
  pluginRouteRef: string
  routedRequestRef: string
  usageEventRef: string
  usageIdempotencyRef: string
  contributorAttributionRef: string
  grossRevenueMsats: number
  contributorShareMsats: number
  amountEnvelopeRef: string
  payoutRail: KhalaCodeTracePluginRevenueSharePayoutRail
  payoutReceiptRef: string
  settlementReceiptRef: string
  recordedAt: string
}>

export type KhalaCodeTracePluginRevenueSharePrecedentDraft =
  KhalaCodeTracePluginRevenueSharePrecedentRecord &
    Readonly<{ idempotencyKey: string }>

export type KhalaCodeTracePluginRevenueShareStore = Readonly<{
  recordPrecedent: (
    draft: KhalaCodeTracePluginRevenueSharePrecedentDraft,
  ) => Effect.Effect<
    Readonly<{
      record: KhalaCodeTracePluginRevenueSharePrecedentRecord
      idempotent: boolean
    }>,
    unknown
  >
  readPrecedent: (
    receiptRef: string,
  ) => Effect.Effect<
    KhalaCodeTracePluginRevenueSharePrecedentRecord | null,
    unknown
  >
}>

type RouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: (() => string) | undefined
  store?: KhalaCodeTracePluginRevenueShareStore | undefined
}>

type OperatorRouteInput = RouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
  }>

type SqlRow = Readonly<{
  receipt_ref: unknown
  consented_trace_receipt_ref: unknown
  trace_digest_ref: unknown
  plugin_admission_receipt_ref: unknown
  plugin_registry_receipt_ref: unknown
  plugin_ref: unknown
  plugin_digest_ref: unknown
  plugin_route_ref: unknown
  routed_request_ref: unknown
  usage_event_ref: unknown
  usage_idempotency_ref: unknown
  contributor_attribution_ref: unknown
  gross_revenue_msats: unknown
  contributor_share_msats: unknown
  amount_envelope_ref: unknown
  payout_rail: unknown
  payout_receipt_ref: unknown
  settlement_receipt_ref: unknown
  recorded_at: unknown
}>

export class KhalaCodeTracePluginRevenueShareStoreUnavailable extends Data.TaggedError(
  'KhalaCodeTracePluginRevenueShareStoreUnavailable',
)<{ readonly reason: string }> {}

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const integerValue = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isSafeInteger(value) ? value : undefined

const isPayoutRail = (
  value: string | undefined,
): value is KhalaCodeTracePluginRevenueSharePayoutRail => value === 'spark'

export const khalaCodeTracePluginRevenueSharePrecedentRecordFromSql = (
  row: SqlRow,
): KhalaCodeTracePluginRevenueSharePrecedentRecord | null => {
  const receiptRef = stringValue(row.receipt_ref)
  const consentedTraceReceiptRef = stringValue(row.consented_trace_receipt_ref)
  const traceDigestRef = stringValue(row.trace_digest_ref)
  const pluginAdmissionReceiptRef = stringValue(
    row.plugin_admission_receipt_ref,
  )
  const pluginRegistryReceiptRef = stringValue(row.plugin_registry_receipt_ref)
  const pluginRef = stringValue(row.plugin_ref)
  const pluginDigestRef = stringValue(row.plugin_digest_ref)
  const pluginRouteRef = stringValue(row.plugin_route_ref)
  const routedRequestRef = stringValue(row.routed_request_ref)
  const usageEventRef = stringValue(row.usage_event_ref)
  const usageIdempotencyRef = stringValue(row.usage_idempotency_ref)
  const contributorAttributionRef = stringValue(row.contributor_attribution_ref)
  const grossRevenueMsats = integerValue(row.gross_revenue_msats)
  const contributorShareMsats = integerValue(row.contributor_share_msats)
  const amountEnvelopeRef = stringValue(row.amount_envelope_ref)
  const payoutRail = stringValue(row.payout_rail)
  const payoutReceiptRef = stringValue(row.payout_receipt_ref)
  const settlementReceiptRef = stringValue(row.settlement_receipt_ref)
  const recordedAt = stringValue(row.recorded_at)

  if (
    receiptRef === undefined ||
    consentedTraceReceiptRef === undefined ||
    traceDigestRef === undefined ||
    pluginAdmissionReceiptRef === undefined ||
    pluginRegistryReceiptRef === undefined ||
    pluginRef === undefined ||
    pluginDigestRef === undefined ||
    pluginRouteRef === undefined ||
    routedRequestRef === undefined ||
    usageEventRef === undefined ||
    usageIdempotencyRef === undefined ||
    contributorAttributionRef === undefined ||
    grossRevenueMsats === undefined ||
    contributorShareMsats === undefined ||
    amountEnvelopeRef === undefined ||
    !isPayoutRail(payoutRail) ||
    payoutReceiptRef === undefined ||
    settlementReceiptRef === undefined ||
    recordedAt === undefined
  ) {
    return null
  }

  return {
    receiptRef,
    consentedTraceReceiptRef,
    traceDigestRef,
    pluginAdmissionReceiptRef,
    pluginRegistryReceiptRef,
    pluginRef,
    pluginDigestRef,
    pluginRouteRef,
    routedRequestRef,
    usageEventRef,
    usageIdempotencyRef,
    contributorAttributionRef,
    grossRevenueMsats,
    contributorShareMsats,
    amountEnvelopeRef,
    payoutRail,
    payoutReceiptRef,
    settlementReceiptRef,
    recordedAt,
  }
}

const readSql = `
  SELECT
    receipt_ref,
    consented_trace_receipt_ref,
    trace_digest_ref,
    plugin_admission_receipt_ref,
    plugin_registry_receipt_ref,
    plugin_ref,
    plugin_digest_ref,
    plugin_route_ref,
    routed_request_ref,
    usage_event_ref,
    usage_idempotency_ref,
    contributor_attribution_ref,
    gross_revenue_msats,
    contributor_share_msats,
    amount_envelope_ref,
    payout_rail,
    payout_receipt_ref,
    settlement_receipt_ref,
    recorded_at
  FROM khala_code_trace_plugin_revenue_share_precedents
`

const recordFromDbRow = (
  row: SqlRow | null | undefined,
): KhalaCodeTracePluginRevenueSharePrecedentRecord | null =>
  row === null || row === undefined
    ? null
    : khalaCodeTracePluginRevenueSharePrecedentRecordFromSql(row)

export const makeD1KhalaCodeTracePluginRevenueShareStore = (
  db: D1Database | undefined,
): KhalaCodeTracePluginRevenueShareStore => {
  const readByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<KhalaCodeTracePluginRevenueSharePrecedentRecord | null> => {
    if (db === undefined) {
      throw new KhalaCodeTracePluginRevenueShareStoreUnavailable({
        reason: 'OPENAGENTS_DB missing',
      })
    }
    const row = await db
      .prepare(`${readSql} WHERE idempotency_key = ? LIMIT 1`)
      .bind(idempotencyKey)
      .first<SqlRow>()
    return recordFromDbRow(row)
  }

  return {
    recordPrecedent: draft =>
      Effect.tryPromise({
        try: async () => {
          if (db === undefined) {
            throw new KhalaCodeTracePluginRevenueShareStoreUnavailable({
              reason: 'OPENAGENTS_DB missing',
            })
          }

          const existing = await readByIdempotencyKey(draft.idempotencyKey)
          if (existing !== null) {
            return { record: existing, idempotent: true }
          }

          await db
            .prepare(
              `
                INSERT INTO khala_code_trace_plugin_revenue_share_precedents (
                  receipt_ref,
                  idempotency_key,
                  consented_trace_receipt_ref,
                  trace_digest_ref,
                  plugin_admission_receipt_ref,
                  plugin_registry_receipt_ref,
                  plugin_ref,
                  plugin_digest_ref,
                  plugin_route_ref,
                  routed_request_ref,
                  usage_event_ref,
                  usage_idempotency_ref,
                  contributor_attribution_ref,
                  gross_revenue_msats,
                  contributor_share_msats,
                  amount_envelope_ref,
                  payout_rail,
                  payout_receipt_ref,
                  settlement_receipt_ref,
                  recorded_at,
                  created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
              `,
            )
            .bind(
              draft.receiptRef,
              draft.idempotencyKey,
              draft.consentedTraceReceiptRef,
              draft.traceDigestRef,
              draft.pluginAdmissionReceiptRef,
              draft.pluginRegistryReceiptRef,
              draft.pluginRef,
              draft.pluginDigestRef,
              draft.pluginRouteRef,
              draft.routedRequestRef,
              draft.usageEventRef,
              draft.usageIdempotencyRef,
              draft.contributorAttributionRef,
              draft.grossRevenueMsats,
              draft.contributorShareMsats,
              draft.amountEnvelopeRef,
              draft.payoutRail,
              draft.payoutReceiptRef,
              draft.settlementReceiptRef,
              draft.recordedAt,
              draft.recordedAt,
            )
            .run()

          const inserted = await readByIdempotencyKey(draft.idempotencyKey)
          if (inserted === null) {
            throw new KhalaCodeTracePluginRevenueShareStoreUnavailable({
              reason: 'inserted precedent could not be read',
            })
          }
          return { record: inserted, idempotent: false }
        },
        catch: error =>
          new KhalaCodeTracePluginRevenueShareStoreUnavailable({
            reason: error instanceof Error ? error.message : String(error),
          }),
      }),
    readPrecedent: receiptRef =>
      Effect.tryPromise({
        try: async () => {
          if (db === undefined) {
            throw new KhalaCodeTracePluginRevenueShareStoreUnavailable({
              reason: 'OPENAGENTS_DB missing',
            })
          }
          const row = await db
            .prepare(`${readSql} WHERE receipt_ref = ? LIMIT 1`)
            .bind(receiptRef)
            .first<SqlRow>()
          return recordFromDbRow(row)
        },
        catch: error =>
          new KhalaCodeTracePluginRevenueShareStoreUnavailable({
            reason: error instanceof Error ? error.message : String(error),
          }),
      }),
  }
}

const promiseIds = [
  'khala_code.trace_derived_plugins.v1',
  'khala_code.plugin_backend_revenue_share.v1',
] as const

const publicSafety = {
  noRawTrace: true,
  noRawPrompt: true,
  noRawUsagePayload: true,
  noRawPaymentMaterial: true,
  noPayoutDestination: true,
  noWalletMaterial: true,
} as const

const evidenceRefs = [
  'issue:OpenAgentsInc/openagents#8251',
  'promise:khala_code.trace_derived_plugins.v1',
  'promise:khala_code.plugin_backend_revenue_share.v1',
  'docs/fable/2026-07-02-khala-code-business-opportunity-and-openagents-analysis.md',
  'docs/fable/ROADMAP_AFTER.md',
] as const

const caveatRefs = [
  'caveat.khala_code_trace_plugin_revenue_share.n_equals_one_precedent',
  'caveat.khala_code_trace_plugin_revenue_share.internal_route_is_not_market_demand',
  'caveat.khala_code_trace_plugin_revenue_share.no_rate_or_pool_policy_claim',
  'caveat.khala_code_trace_plugin_revenue_share.no_promise_state_change',
  'caveat.khala_code_trace_plugin_revenue_share.no_raw_trace_or_payment_material',
] as const

const sourceRefs = [
  `table:${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_TABLE}`,
  `route:${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_OPERATOR_ENDPOINT}`,
  `route:${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_RECEIPT_PATH}`,
] as const

export const publicKhalaCodeTracePluginRevenueSharePrecedentReceipt = (
  record: KhalaCodeTracePluginRevenueSharePrecedentRecord,
  generatedAt: string,
): PublicKhalaCodeTracePluginRevenueSharePrecedentReceipt => ({
  schemaVersion:
    'openagents.khala_code.trace_plugin_revenue_share_precedent_receipt.v1',
  product: 'khala-code',
  promiseIds: [...promiseIds],
  receiptRef: record.receiptRef,
  receiptUrl: `${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_ENDPOINT}/${encodeURIComponent(
    record.receiptRef,
  )}`,
  generatedAt,
  recordedAt: record.recordedAt,
  trace: {
    consentedTraceReceiptRef: record.consentedTraceReceiptRef,
    traceDigestRef: record.traceDigestRef,
    rawTraceIncluded: false,
  },
  plugin: {
    pluginAdmissionReceiptRef: record.pluginAdmissionReceiptRef,
    pluginRegistryReceiptRef: record.pluginRegistryReceiptRef,
    pluginRef: record.pluginRef,
    pluginDigestRef: record.pluginDigestRef,
    pluginRouteRef: record.pluginRouteRef,
    registered: true,
    routable: true,
  },
  routing: {
    routedRequestRef: record.routedRequestRef,
    usageEventRef: record.usageEventRef,
    usageIdempotencyRef: record.usageIdempotencyRef,
    meteringTruth: 'exact',
    meteredUsageEventCount: 1,
  },
  attribution: {
    contributorAttributionRef: record.contributorAttributionRef,
  },
  revenueShare: {
    grossRevenueMsats: record.grossRevenueMsats,
    contributorShareMsats: record.contributorShareMsats,
    payoutRail: record.payoutRail,
    amountEnvelopeRef: record.amountEnvelopeRef,
    payoutReceiptRef: record.payoutReceiptRef,
    settlementReceiptRef: record.settlementReceiptRef,
    state: 'settled',
  },
  publicSafety,
  evidenceRefs: [...evidenceRefs],
  caveatRefs: [...caveatRefs],
  sourceRefs: [...sourceRefs],
  staleness: liveAtReadStaleness([
    KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_TABLE,
  ]),
})

const boundedIdempotencyKey = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()
  return trimmed !== undefined &&
    trimmed !== '' &&
    /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(trimmed)
    ? trimmed
    : undefined
}

const makeReceiptRef = (): string =>
  compactRandomId('receipt.khala_code.trace_plugin_revenue_share').replace(
    'receipt.khala_code.trace_plugin_revenue_share_',
    'receipt.khala_code.trace_plugin_revenue_share.',
  )

const privateMaterialKeyPattern =
  /(^|_|\b)(authorization|bearer|cwd|destination|home|invoice|log|mnemonic|path|paymentHash|preimage|prompt|rawPayment|secret|token|wallet)(_|$|\b)/i

const containsPrivateMaterialKey = (value: unknown): boolean => {
  if (Array.isArray(value)) {
    return value.some(item => containsPrivateMaterialKey(item))
  }
  if (typeof value !== 'object' || value === null) {
    return false
  }
  for (const [key, child] of Object.entries(value)) {
    if (privateMaterialKeyPattern.test(key)) return true
    if (containsPrivateMaterialKey(child)) return true
  }
  return false
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|checkout[_-]?session|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|customer|fixture|invoice|log|package|payment|payload|prompt|provider|receipt|runner|run[_-]?log|schema|source|trace|usage|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|spark[_-]?(address|invoice|request|secret)|token|usage[_-]?(event[_-]?raw|payload|raw)|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const refIsSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !containsProviderSecretMaterial(ref) &&
  !unsafeRefPattern.test(ref) &&
  !rawTimestampPattern.test(ref)

const validateRef = (
  label: string,
  value: string,
): string | { error: string } => {
  const trimmed = value.trim()
  return trimmed !== '' && refIsSafe(trimmed)
    ? trimmed
    : { error: `${label}_unsafe` }
}

const decodeRequest = (
  value: unknown,
): KhalaCodeTracePluginRevenueSharePrecedentIntakeRequest | undefined => {
  try {
    return S.decodeUnknownSync(
      KhalaCodeTracePluginRevenueSharePrecedentIntakeRequest,
    )(value)
  } catch {
    return undefined
  }
}

const readBody = (request: Request) =>
  Effect.promise(() => request.text().catch(() => ''))

const normalizeBody = (
  body: KhalaCodeTracePluginRevenueSharePrecedentIntakeRequest,
): KhalaCodeTracePluginRevenueSharePrecedentRecord | { error: string } => {
  const refFields = [
    ['consented_trace_receipt_ref', body.consentedTraceReceiptRef],
    ['trace_digest_ref', body.traceDigestRef],
    ['plugin_admission_receipt_ref', body.pluginAdmissionReceiptRef],
    ['plugin_registry_receipt_ref', body.pluginRegistryReceiptRef],
    ['plugin_ref', body.pluginRef],
    ['plugin_digest_ref', body.pluginDigestRef],
    ['plugin_route_ref', body.pluginRouteRef],
    ['routed_request_ref', body.routedRequestRef],
    ['usage_event_ref', body.usageEventRef],
    ['usage_idempotency_ref', body.usageIdempotencyRef],
    ['contributor_attribution_ref', body.contributorAttributionRef],
    ['amount_envelope_ref', body.amountEnvelopeRef],
    ['payout_receipt_ref', body.payoutReceiptRef],
    ['settlement_receipt_ref', body.settlementReceiptRef],
  ] as const
  const normalized = new Map<string, string>()

  for (const [label, value] of refFields) {
    const ref = validateRef(label, value)
    if (typeof ref !== 'string') {
      return ref
    }
    normalized.set(label, ref)
  }

  if (
    !Number.isSafeInteger(body.grossRevenueMsats) ||
    !Number.isSafeInteger(body.contributorShareMsats) ||
    body.grossRevenueMsats <= 0 ||
    body.contributorShareMsats <= 0
  ) {
    return { error: 'amounts_must_be_positive_safe_integers' }
  }

  if (body.contributorShareMsats > body.grossRevenueMsats) {
    return { error: 'contributor_share_exceeds_gross_revenue' }
  }

  if (body.contributorShareMsats % 1_000 !== 0) {
    return { error: 'contributor_share_must_be_whole_sats' }
  }

  return {
    receiptRef: makeReceiptRef(),
    consentedTraceReceiptRef: normalized.get('consented_trace_receipt_ref')!,
    traceDigestRef: normalized.get('trace_digest_ref')!,
    pluginAdmissionReceiptRef: normalized.get('plugin_admission_receipt_ref')!,
    pluginRegistryReceiptRef: normalized.get('plugin_registry_receipt_ref')!,
    pluginRef: normalized.get('plugin_ref')!,
    pluginDigestRef: normalized.get('plugin_digest_ref')!,
    pluginRouteRef: normalized.get('plugin_route_ref')!,
    routedRequestRef: normalized.get('routed_request_ref')!,
    usageEventRef: normalized.get('usage_event_ref')!,
    usageIdempotencyRef: normalized.get('usage_idempotency_ref')!,
    contributorAttributionRef: normalized.get('contributor_attribution_ref')!,
    grossRevenueMsats: body.grossRevenueMsats,
    contributorShareMsats: body.contributorShareMsats,
    amountEnvelopeRef: normalized.get('amount_envelope_ref')!,
    payoutRail: body.payoutRail,
    payoutReceiptRef: normalized.get('payout_receipt_ref')!,
    settlementReceiptRef: normalized.get('settlement_receipt_ref')!,
    recordedAt: '',
  }
}

const receiptRefFromPath = (pathname: string): string | null => {
  const prefix = `${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_ENDPOINT}/`
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

const receiptRefPattern =
  /^receipt\.khala_code\.trace_plugin_revenue_share\.[A-Za-z0-9_-]+$/

export const handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi = (
  request: Request,
  input: OperatorRouteInput,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request).catch(() => false),
    )

    if (!authorized) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const text = yield* readBody(request)
    const parsed = yield* Effect.try({
      try: () => parseJsonUnknown(text),
      catch: () => undefined,
    }).pipe(Effect.catch(() => Effect.void))

    if (parsed === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    if (containsPrivateMaterialKey(parsed)) {
      return noStoreJsonResponse(
        { error: 'private_material_not_allowed' },
        { status: 400 },
      )
    }

    const body = decodeRequest(parsed)
    if (body === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    const normalized = normalizeBody(body)
    if ('error' in normalized) {
      return noStoreJsonResponse(
        { error: 'invalid_public_safe_evidence', reason: normalized.error },
        { status: 400 },
      )
    }

    const clientKey = boundedIdempotencyKey(body.idempotencyKey)
    if (body.idempotencyKey !== undefined && clientKey === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_idempotency_key' },
        { status: 400 },
      )
    }

    const nowIso = input.nowIso ?? currentIsoTimestamp
    const recordedAt = nowIso()
    const draft = { ...normalized, recordedAt }
    const idempotencyKey = `khala-code-trace-plugin-revenue-share:${
      clientKey ?? draft.receiptRef
    }`
    const store =
      input.store ??
      makeD1KhalaCodeTracePluginRevenueShareStore(input.OPENAGENTS_DB)
    const recorded = yield* store
      .recordPrecedent({ ...draft, idempotencyKey })
      .pipe(
        Effect.catch(() =>
          Effect.succeed<
            Readonly<{
              record: KhalaCodeTracePluginRevenueSharePrecedentRecord
              idempotent: boolean
            }> | null
          >(null),
        ),
      )

    if (recorded === null) {
      return noStoreJsonResponse(
        { error: 'khala_code_trace_plugin_revenue_share_receipt_unavailable' },
        { status: 503 },
      )
    }

    const generatedAt = nowIso()
    const receipt = publicKhalaCodeTracePluginRevenueSharePrecedentReceipt(
      recorded.record,
      generatedAt,
    )

    return noStoreJsonResponse(
      {
        ok: true,
        idempotent: recorded.idempotent,
        generatedAt,
        staleness: receipt.staleness,
        receipt,
      } satisfies OperatorKhalaCodeTracePluginRevenueSharePrecedentIntakeEnvelope,
      { status: recorded.idempotent ? 200 : 201 },
    )
  })

export const handlePublicKhalaCodeTracePluginRevenueSharePrecedentRead = (
  request: Request,
  input: RouteInput & Readonly<{ receiptRef: string }>,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  if (!receiptRefPattern.test(input.receiptRef)) {
    return Effect.succeed(notFound())
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp
  const store =
    input.store ??
    makeD1KhalaCodeTracePluginRevenueShareStore(input.OPENAGENTS_DB)

  return store.readPrecedent(input.receiptRef).pipe(
    Effect.map(record => {
      if (record === null) {
        return notFound()
      }

      const generatedAt = nowIso()
      const receipt = publicKhalaCodeTracePluginRevenueSharePrecedentReceipt(
        record,
        generatedAt,
      )

      return noStoreJsonResponse({
        generatedAt,
        staleness: receipt.staleness,
        receipt,
      } satisfies PublicKhalaCodeTracePluginRevenueSharePrecedentEnvelope)
    }),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          {
            error:
              'khala_code_trace_plugin_revenue_share_receipt_unavailable',
          },
          { status: 503 },
        ),
      ),
    ),
  )
}

export const makeKhalaCodeTracePluginRevenueShareRoutes = <Bindings>(
  dependencies: Readonly<{
    makeStore: (env: Bindings) => KhalaCodeTracePluginRevenueShareStore
    nowIso: () => string
  }>,
) => ({
  routePublicKhalaCodeTracePluginRevenueShareRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)
    return receiptRef === null
      ? undefined
      : handlePublicKhalaCodeTracePluginRevenueSharePrecedentRead(request, {
          receiptRef,
          store: dependencies.makeStore(env),
          nowIso: dependencies.nowIso,
        })
  },
})
