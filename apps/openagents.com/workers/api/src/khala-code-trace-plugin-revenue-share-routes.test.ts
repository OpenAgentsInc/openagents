import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_OPERATOR_ENDPOINT,
  KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_ENDPOINT,
  handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi,
  handlePublicKhalaCodeTracePluginRevenueSharePrecedentRead,
  khalaCodeTracePluginRevenueSharePrecedentRecordFromSql,
  type KhalaCodeTracePluginRevenueSharePrecedentDraft,
  type KhalaCodeTracePluginRevenueSharePrecedentRecord,
  type KhalaCodeTracePluginRevenueShareStore,
} from './khala-code-trace-plugin-revenue-share-routes'

const nowIso = '2026-07-04T16:00:00.000Z'

const validBody = (
  idempotencyKey = 'trace-plugin-rs-test-1',
): Record<string, unknown> => ({
  schemaVersion:
    'openagents.khala_code.trace_plugin_revenue_share_precedent_intake.v1',
  consent: {
    publicReceipt: true,
    noPrivateDataIncluded: true,
    realSettlementReceiptSupplied: true,
  },
  consentedTraceReceiptRef: 'receipt.khala_code.trace_capture.redacted_001',
  traceDigestRef: 'digest.khala_code.trace.sha256_redacted_001',
  pluginAdmissionReceiptRef: 'receipt.khala_code.plugin_admission.redacted_001',
  pluginRegistryReceiptRef: 'receipt.khala_code.plugin_registry.redacted_001',
  pluginRef: 'plugin.khala_code.trace_derived.redacted_001',
  pluginDigestRef: 'digest.khala_code.plugin.sha256_redacted_001',
  pluginRouteRef: 'route.khala_code.plugin.redacted_001',
  routedRequestRef: 'request.khala_code.plugin.redacted_001',
  usageEventRef: 'usage.khala_code.plugin.redacted_001',
  usageIdempotencyRef: 'idempotency.khala_code.plugin.redacted_001',
  contributorAttributionRef: 'attribution.khala_code.contributor.redacted_001',
  grossRevenueMsats: 5_000,
  contributorShareMsats: 1_000,
  amountEnvelopeRef: 'envelope.khala_code.plugin_revenue_share.one_sat_001',
  payoutRail: 'spark',
  payoutReceiptRef: 'receipt.khala_code.plugin_revenue_share.payout_001',
  settlementReceiptRef:
    'settlement.public.khala_code.plugin_revenue_share.one_sat_001',
  idempotencyKey,
})

const operatorRequest = (method: string, body?: unknown): Request => {
  const init =
    body === undefined
      ? { method }
      : {
          method,
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
        }

  return new Request(
    `https://openagents.com${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_OPERATOR_ENDPOINT}`,
    init,
  )
}

const publicRequest = (receiptRef: string, method = 'GET'): Request =>
  new Request(
    `https://openagents.com${KHALA_CODE_TRACE_PLUGIN_REVENUE_SHARE_PUBLIC_ENDPOINT}/${encodeURIComponent(
      receiptRef,
    )}`,
    { method },
  )

const toRecord = (
  draft: KhalaCodeTracePluginRevenueSharePrecedentDraft,
): KhalaCodeTracePluginRevenueSharePrecedentRecord => ({
  receiptRef: draft.receiptRef,
  consentedTraceReceiptRef: draft.consentedTraceReceiptRef,
  traceDigestRef: draft.traceDigestRef,
  pluginAdmissionReceiptRef: draft.pluginAdmissionReceiptRef,
  pluginRegistryReceiptRef: draft.pluginRegistryReceiptRef,
  pluginRef: draft.pluginRef,
  pluginDigestRef: draft.pluginDigestRef,
  pluginRouteRef: draft.pluginRouteRef,
  routedRequestRef: draft.routedRequestRef,
  usageEventRef: draft.usageEventRef,
  usageIdempotencyRef: draft.usageIdempotencyRef,
  contributorAttributionRef: draft.contributorAttributionRef,
  grossRevenueMsats: draft.grossRevenueMsats,
  contributorShareMsats: draft.contributorShareMsats,
  amountEnvelopeRef: draft.amountEnvelopeRef,
  payoutRail: draft.payoutRail,
  payoutReceiptRef: draft.payoutReceiptRef,
  settlementReceiptRef: draft.settlementReceiptRef,
  recordedAt: draft.recordedAt,
})

const memoryStore = (): KhalaCodeTracePluginRevenueShareStore => {
  const byIdempotencyKey =
    new Map<string, KhalaCodeTracePluginRevenueSharePrecedentRecord>()
  const byReceiptRef =
    new Map<string, KhalaCodeTracePluginRevenueSharePrecedentRecord>()

  return {
    recordPrecedent: draft =>
      Effect.sync(() => {
        const existing = byIdempotencyKey.get(draft.idempotencyKey)
        if (existing !== undefined) {
          return { record: existing, idempotent: true }
        }
        const record = toRecord(draft)
        byIdempotencyKey.set(draft.idempotencyKey, record)
        byReceiptRef.set(record.receiptRef, record)
        return { record, idempotent: false }
      }),
    readPrecedent: receiptRef =>
      Effect.sync(() => byReceiptRef.get(receiptRef) ?? null),
  }
}

describe('Khala Code trace plugin revenue-share precedent routes', () => {
  test('records one admin-gated settled trace plugin revenue-share precedent', async () => {
    const store = memoryStore()
    const response = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', validBody()),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as {
      ok: boolean
      idempotent: boolean
      generatedAt: string
      receipt: {
        schemaVersion: string
        promiseIds: ReadonlyArray<string>
        receiptRef: string
        receiptUrl: string
        trace: Record<string, unknown>
        plugin: Record<string, unknown>
        routing: Record<string, unknown>
        attribution: Record<string, unknown>
        revenueShare: Record<string, unknown>
        publicSafety: Record<string, boolean>
        caveatRefs: ReadonlyArray<string>
        sourceRefs: ReadonlyArray<string>
        staleness: { composition: string; maxStalenessSeconds: number }
        rawTrace?: string
        prompt?: string
        invoice?: string
        payoutTarget?: string
      }
    }

    expect(body.ok).toBe(true)
    expect(body.idempotent).toBe(false)
    expect(body.generatedAt).toBe(nowIso)
    expect(body.receipt.schemaVersion).toBe(
      'openagents.khala_code.trace_plugin_revenue_share_precedent_receipt.v1',
    )
    expect(body.receipt.promiseIds).toEqual([
      'khala_code.trace_derived_plugins.v1',
      'khala_code.plugin_backend_revenue_share.v1',
    ])
    expect(body.receipt.receiptRef).toMatch(
      /^receipt\.khala_code\.trace_plugin_revenue_share\./,
    )
    expect(body.receipt.receiptUrl).toContain(
      '/api/public/khala-code/trace-plugin-revenue-share-precedents/',
    )
    expect(body.receipt.trace).toMatchObject({
      rawTraceIncluded: false,
      traceDigestRef: 'digest.khala_code.trace.sha256_redacted_001',
    })
    expect(body.receipt.plugin).toMatchObject({
      registered: true,
      routable: true,
      pluginRegistryReceiptRef:
        'receipt.khala_code.plugin_registry.redacted_001',
    })
    expect(body.receipt.routing).toMatchObject({
      meteringTruth: 'exact',
      meteredUsageEventCount: 1,
      usageEventRef: 'usage.khala_code.plugin.redacted_001',
    })
    expect(body.receipt.attribution).toEqual({
      contributorAttributionRef: 'attribution.khala_code.contributor.redacted_001',
    })
    expect(body.receipt.revenueShare).toMatchObject({
      grossRevenueMsats: 5_000,
      contributorShareMsats: 1_000,
      payoutRail: 'spark',
      state: 'settled',
    })
    expect(body.receipt.publicSafety).toMatchObject({
      noRawTrace: true,
      noRawPaymentMaterial: true,
      noPayoutDestination: true,
      noWalletMaterial: true,
    })
    expect(body.receipt.rawTrace).toBeUndefined()
    expect(body.receipt.prompt).toBeUndefined()
    expect(body.receipt.invoice).toBeUndefined()
    expect(body.receipt.payoutTarget).toBeUndefined()
    expect(body.receipt.caveatRefs).toContain(
      'caveat.khala_code_trace_plugin_revenue_share.n_equals_one_precedent',
    )
    expect(body.receipt.sourceRefs).toContain(
      'table:khala_code_trace_plugin_revenue_share_precedents',
    )
    expect(body.receipt.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
  })

  test('replays by idempotency key and publicly dereferences the receipt', async () => {
    const store = memoryStore()
    const first = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', validBody()),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )
    const second = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', validBody()),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )

    const firstBody = (await first.json()) as {
      receipt: { receiptRef: string }
    }
    const secondBody = (await second.json()) as {
      idempotent: boolean
      receipt: { receiptRef: string }
    }

    expect(second.status).toBe(200)
    expect(secondBody.idempotent).toBe(true)
    expect(secondBody.receipt.receiptRef).toBe(firstBody.receipt.receiptRef)

    const read = await Effect.runPromise(
      handlePublicKhalaCodeTracePluginRevenueSharePrecedentRead(
        publicRequest(firstBody.receipt.receiptRef),
        {
          receiptRef: firstBody.receipt.receiptRef,
          nowIso: () => nowIso,
          store,
        },
      ),
    )

    expect(read.status).toBe(200)
    await expect(read.json()).resolves.toMatchObject({
      generatedAt: nowIso,
      receipt: {
        receiptRef: firstBody.receipt.receiptRef,
        revenueShare: {
          settlementReceiptRef:
            'settlement.public.khala_code.plugin_revenue_share.one_sat_001',
        },
      },
    })
  })

  test('requires admin authorization and the expected methods', async () => {
    const unauthorized = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', validBody()),
        {
          nowIso: () => nowIso,
          store: memoryStore(),
          requireAdminApiToken: async () => false,
        },
      ),
    )
    expect(unauthorized.status).toBe(401)

    const wrongOperatorMethod = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('GET'),
        {
          nowIso: () => nowIso,
          store: memoryStore(),
          requireAdminApiToken: async () => true,
        },
      ),
    )
    expect(wrongOperatorMethod.status).toBe(405)

    const wrongPublicMethod = await Effect.runPromise(
      handlePublicKhalaCodeTracePluginRevenueSharePrecedentRead(
        publicRequest('receipt.khala_code.trace_plugin_revenue_share.any', 'POST'),
        {
          receiptRef: 'receipt.khala_code.trace_plugin_revenue_share.any',
          nowIso: () => nowIso,
          store: memoryStore(),
        },
      ),
    )
    expect(wrongPublicMethod.status).toBe(405)
  })

  test('rejects raw/private refs and impossible accounting', async () => {
    const unsafe = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', {
          ...validBody(),
          settlementReceiptRef: 'lnbc1rawinvoice',
        }),
        {
          nowIso: () => nowIso,
          store: memoryStore(),
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(unsafe.status).toBe(400)
    await expect(unsafe.json()).resolves.toMatchObject({
      error: 'invalid_public_safe_evidence',
    })

    const impossible = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', {
          ...validBody('trace-plugin-rs-test-2'),
          grossRevenueMsats: 1_000,
          contributorShareMsats: 2_000,
        }),
        {
          nowIso: () => nowIso,
          store: memoryStore(),
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(impossible.status).toBe(400)
    await expect(impossible.json()).resolves.toMatchObject({
      error: 'invalid_public_safe_evidence',
      reason: 'contributor_share_exceeds_gross_revenue',
    })

    const fractionalSat = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', {
          ...validBody('trace-plugin-rs-test-3'),
          contributorShareMsats: 1,
        }),
        {
          nowIso: () => nowIso,
          store: memoryStore(),
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(fractionalSat.status).toBe(400)
    await expect(fractionalSat.json()).resolves.toMatchObject({
      error: 'invalid_public_safe_evidence',
      reason: 'contributor_share_must_be_whole_sats',
    })
  })

  test('rejects request bodies carrying private-material keys', async () => {
    const response = await Effect.runPromise(
      handleOperatorKhalaCodeTracePluginRevenueSharePrecedentsApi(
        operatorRequest('POST', {
          ...validBody(),
          rawTrace: 'private prompt',
          wallet: 'secret',
        }),
        {
          nowIso: () => nowIso,
          store: memoryStore(),
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'private_material_not_allowed',
    })
  })

  test('SQL row normalization rejects unsafe shape drift', () => {
    expect(
      khalaCodeTracePluginRevenueSharePrecedentRecordFromSql({
        receipt_ref: 'receipt.khala_code.trace_plugin_revenue_share.abc',
        consented_trace_receipt_ref: 'receipt.khala_code.trace_capture.redacted',
        trace_digest_ref: 'digest.khala_code.trace.sha256_redacted',
        plugin_admission_receipt_ref:
          'receipt.khala_code.plugin_admission.redacted',
        plugin_registry_receipt_ref:
          'receipt.khala_code.plugin_registry.redacted',
        plugin_ref: 'plugin.khala_code.trace_derived.redacted',
        plugin_digest_ref: 'digest.khala_code.plugin.sha256_redacted',
        plugin_route_ref: 'route.khala_code.plugin.redacted',
        routed_request_ref: 'request.khala_code.plugin.redacted',
        usage_event_ref: 'usage.khala_code.plugin.redacted',
        usage_idempotency_ref: 'idempotency.khala_code.plugin.redacted',
        contributor_attribution_ref: 'attribution.khala_code.contributor.redacted',
        gross_revenue_msats: 5_000,
        contributor_share_msats: 1_000,
        amount_envelope_ref: 'envelope.khala_code.one_sat',
        payout_rail: 'spark',
        payout_receipt_ref: 'receipt.khala_code.plugin_revenue_share.payout',
        settlement_receipt_ref:
          'settlement.public.khala_code.plugin_revenue_share.one_sat',
        recorded_at: nowIso,
      }),
    ).toMatchObject({
      payoutRail: 'spark',
      contributorShareMsats: 1_000,
    })

    expect(
      khalaCodeTracePluginRevenueSharePrecedentRecordFromSql({
        receipt_ref: 'receipt.khala_code.trace_plugin_revenue_share.bad',
        consented_trace_receipt_ref: 'receipt.khala_code.trace_capture.redacted',
        trace_digest_ref: 'digest.khala_code.trace.sha256_redacted',
        plugin_admission_receipt_ref:
          'receipt.khala_code.plugin_admission.redacted',
        plugin_registry_receipt_ref:
          'receipt.khala_code.plugin_registry.redacted',
        plugin_ref: 'plugin.khala_code.trace_derived.redacted',
        plugin_digest_ref: 'digest.khala_code.plugin.sha256_redacted',
        plugin_route_ref: 'route.khala_code.plugin.redacted',
        routed_request_ref: 'request.khala_code.plugin.redacted',
        usage_event_ref: 'usage.khala_code.plugin.redacted',
        usage_idempotency_ref: 'idempotency.khala_code.plugin.redacted',
        contributor_attribution_ref: 'attribution.khala_code.contributor.redacted',
        gross_revenue_msats: 5_000,
        contributor_share_msats: 1_000,
        amount_envelope_ref: 'envelope.khala_code.one_sat',
        payout_rail: 'mdk',
        payout_receipt_ref: 'receipt.khala_code.plugin_revenue_share.payout',
        settlement_receipt_ref:
          'settlement.public.khala_code.plugin_revenue_share.one_sat',
        recorded_at: nowIso,
      }),
    ).toBeNull()
  })
})
