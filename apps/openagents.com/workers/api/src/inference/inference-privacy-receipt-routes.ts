import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { parseJsonUnknown } from '../json-boundary'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from '../public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from '../runtime-primitives'
import {
  PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT,
  PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE,
} from './inference-privacy-entitlement'

export const PRIVACY_ENTITLEMENT_RECEIPT_SCHEMA_VERSION =
  'openagents.inference.privacy_entitlement_receipt.v1' as const
export const CONFIDENTIAL_COMPUTE_RECEIPT_SCHEMA_VERSION =
  'openagents.inference.confidential_compute_execution_receipt.v1' as const

export type PrivacyReceiptSession = Readonly<{ accountRef: string }>

export type PrivacyReceiptRoutesDeps = Readonly<{
  authenticate: (
    request: Request,
  ) => Promise<PrivacyReceiptSession | undefined>
  confidentialComputeEnabled: boolean
  db: D1Database
  nowIso?: (() => string) | undefined
}>

type PrivacyEntitlementReceiptRow = Readonly<{
  account_ref: string
  capture_excluded: number
  created_at: string
  entitlement_ref: string
  privacy_tier: string
  purchase_ref: string
  reason_ref: string
  receipt_ref: string
  updated_at: string
}>

type ConfidentialComputeReceiptRow = Readonly<{
  account_ref: string
  capture_excluded: number
  created_at: string
  execution_ref: string
  reason_ref: string
  receipt_ref: string
  request_ref: string
  updated_at: string
}>

export type PublicPrivacyReceiptProjection = Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  receipt:
    | Readonly<{
        schemaVersion: typeof PRIVACY_ENTITLEMENT_RECEIPT_SCHEMA_VERSION
        receiptRef: string
        entitlementRef: string
        purchaseRef: string
        privacyTier: string
        captureExcluded: true
        reasonRef: string
        createdAt: string
        updatedAt: string
      }>
    | Readonly<{
        schemaVersion: typeof CONFIDENTIAL_COMPUTE_RECEIPT_SCHEMA_VERSION
        receiptRef: string
        executionRef: string
        requestRef: string
        captureExcluded: true
        reasonRef: string
        createdAt: string
        updatedAt: string
      }>
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
}>

const PurchaseBody = S.Struct({
  idempotencyKey: S.optionalKey(S.String),
})

const ConfidentialBody = S.Struct({
  idempotencyKey: S.optionalKey(S.String),
})

const privacyReceiptStaleness = liveAtReadStaleness([
  'inference_privacy_entitlement_receipts',
  'inference_privacy_entitlements',
  'inference_confidential_compute_execution_receipts',
])

const safeJsonParse = (text: string): unknown => {
  try {
    return parseJsonUnknown(text)
  } catch {
    return null
  }
}

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

const decodePurchaseBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(PurchaseBody)(value)
  } catch {
    return undefined
  }
}

const decodeConfidentialBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(ConfidentialBody)(value)
  } catch {
    return undefined
  }
}

const accountHash = async (accountRef: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(accountRef),
  )
  return Array.from(new Uint8Array(digest))
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
    .slice(0, 16)
}

const publicEntitlementProjection = (
  row: PrivacyEntitlementReceiptRow,
  generatedAt: string,
): PublicPrivacyReceiptProjection => ({
  authorityBoundary:
    'Public proof only. This privacy receipt proves an opt-out entitlement row was recorded; it grants no billing, refund, payout, settlement, provider, confidential-runtime, or public-claim authority.',
  caveatRefs: [
    'caveat.public.no_private_payment_material',
    'caveat.public.no_prompts_or_completions',
    'caveat.public.owner_signoff_required_for_green_claim',
  ],
  generatedAt,
  receipt: {
    schemaVersion: PRIVACY_ENTITLEMENT_RECEIPT_SCHEMA_VERSION,
    receiptRef: row.receipt_ref,
    entitlementRef: row.entitlement_ref,
    purchaseRef: row.purchase_ref,
    privacyTier: row.privacy_tier,
    captureExcluded: true,
    reasonRef: row.reason_ref,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  },
  sourceRefs: [
    `route:/api/public/inference/privacy-receipts/${row.receipt_ref}`,
    'table:inference_privacy_entitlement_receipts',
    'table:inference_privacy_entitlements',
  ],
  staleness: privacyReceiptStaleness,
})

const publicConfidentialProjection = (
  row: ConfidentialComputeReceiptRow,
  generatedAt: string,
): PublicPrivacyReceiptProjection => ({
  authorityBoundary:
    'Public proof only. This confidential-compute receipt proves a capture-excluded execution receipt was recorded; it grants no billing, refund, payout, settlement, provider, confidential-runtime, or public-claim authority.',
  caveatRefs: [
    'caveat.public.no_prompts_or_completions',
    'caveat.public.no_private_runtime_payload',
    'caveat.public.owner_signoff_required_for_green_claim',
  ],
  generatedAt,
  receipt: {
    schemaVersion: CONFIDENTIAL_COMPUTE_RECEIPT_SCHEMA_VERSION,
    receiptRef: row.receipt_ref,
    executionRef: row.execution_ref,
    requestRef: row.request_ref,
    captureExcluded: true,
    reasonRef: row.reason_ref,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  },
  sourceRefs: [
    `route:/api/public/inference/privacy-receipts/${row.receipt_ref}`,
    'table:inference_confidential_compute_execution_receipts',
  ],
  staleness: privacyReceiptStaleness,
})

export const readPublicPrivacyReceipt = async (
  db: D1Database,
  receiptRef: string,
  generatedAt: string,
): Promise<PublicPrivacyReceiptProjection | null> => {
  const entitlementRow = await db
    .prepare(
      `SELECT receipt_ref, entitlement_ref, account_ref, purchase_ref,
              privacy_tier, capture_excluded, reason_ref, created_at, updated_at
         FROM inference_privacy_entitlement_receipts
        WHERE receipt_ref = ?
        LIMIT 1`,
    )
    .bind(receiptRef)
    .first<PrivacyEntitlementReceiptRow>()

  if (entitlementRow !== null) {
    return publicEntitlementProjection(entitlementRow, generatedAt)
  }

  const confidentialRow = await db
    .prepare(
      `SELECT receipt_ref, execution_ref, account_ref, request_ref,
              capture_excluded, reason_ref, created_at, updated_at
         FROM inference_confidential_compute_execution_receipts
        WHERE receipt_ref = ?
        LIMIT 1`,
    )
    .bind(receiptRef)
    .first<ConfidentialComputeReceiptRow>()

  return confidentialRow === null
    ? null
    : publicConfidentialProjection(confidentialRow, generatedAt)
}

export const grantPaidPrivacyEntitlement = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    idempotencyKey: string
    nowIso: string
    purchaseRef: string
  }>,
): Promise<PrivacyEntitlementReceiptRow | null> => {
  const accountDigest = await accountHash(input.accountRef)
  const entitlementRef = `entitlement.inference.paid_privacy.${accountDigest}`
  const receiptRef = `receipt.inference.privacy_entitlement.${input.purchaseRef}`

  await db
    .prepare(
      `INSERT INTO inference_privacy_entitlement_receipts (
         receipt_ref, entitlement_ref, account_ref, purchase_ref,
         idempotency_key, privacy_tier, capture_excluded, reason_ref,
         created_at, updated_at
       ) VALUES (?, ?, ?, ?, ?, 'paid_privacy', 1, ?, ?, ?)
       ON CONFLICT(idempotency_key) DO NOTHING`,
    )
    .bind(
      receiptRef,
      entitlementRef,
      input.accountRef,
      input.purchaseRef,
      input.idempotencyKey,
      PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT,
      input.nowIso,
      input.nowIso,
    )
    .run()

  await db
    .prepare(
      `INSERT INTO inference_privacy_entitlements (
         account_ref, privacy_tier, note, created_at, updated_at
       ) VALUES (?, 'paid_privacy', ?, ?, ?)
       ON CONFLICT(account_ref) DO UPDATE SET
         privacy_tier = 'paid_privacy',
         note = excluded.note,
         updated_at = excluded.updated_at`,
    )
    .bind(
      input.accountRef,
      `receipt:${receiptRef}`,
      input.nowIso,
      input.nowIso,
    )
    .run()

  // Read back by key AND account: the idempotency_key column is globally
  // unique across purchase surfaces, so without the account guard a key
  // collision would return (and publicly attribute) another account's receipt.
  return await db
    .prepare(
      `SELECT receipt_ref, entitlement_ref, account_ref, purchase_ref,
              privacy_tier, capture_excluded, reason_ref, created_at, updated_at
         FROM inference_privacy_entitlement_receipts
        WHERE idempotency_key = ? AND account_ref = ?
        LIMIT 1`,
    )
    .bind(input.idempotencyKey, input.accountRef)
    .first<PrivacyEntitlementReceiptRow>()
}

export const recordConfidentialComputeExecutionReceipt = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    idempotencyKey: string
    nowIso: string
    requestRef: string
  }>,
): Promise<ConfidentialComputeReceiptRow | null> => {
  const executionRef = `execution.inference.confidential_compute.${input.requestRef}`
  const receiptRef = `receipt.inference.confidential_compute.${input.requestRef}`

  await db
    .prepare(
      `INSERT INTO inference_confidential_compute_execution_receipts (
         receipt_ref, execution_ref, account_ref, request_ref,
         idempotency_key, capture_excluded, reason_ref, created_at, updated_at
       ) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?)
       ON CONFLICT(idempotency_key) DO NOTHING`,
    )
    .bind(
      receiptRef,
      executionRef,
      input.accountRef,
      input.requestRef,
      input.idempotencyKey,
      PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE,
      input.nowIso,
      input.nowIso,
    )
    .run()

  // Same key-collision guard as the entitlement read-back: never return
  // another account's receipt row on an idempotency-key collision.
  return await db
    .prepare(
      `SELECT receipt_ref, execution_ref, account_ref, request_ref,
              capture_excluded, reason_ref, created_at, updated_at
         FROM inference_confidential_compute_execution_receipts
        WHERE idempotency_key = ? AND account_ref = ?
        LIMIT 1`,
    )
    .bind(input.idempotencyKey, input.accountRef)
    .first<ConfidentialComputeReceiptRow>()
}

const readBody = (request: Request) =>
  Effect.promise(() => request.text().catch(() => ''))

const authResponse = () => {
  const headers = new Headers({ 'www-authenticate': 'Bearer' })
  return noStoreJsonResponse({ error: 'unauthorized' }, { headers, status: 401 })
}

export const handlePaidPrivacyPurchase = (
  request: Request,
  deps: PrivacyReceiptRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      return authResponse()
    }

    const text = yield* readBody(request)
    const parsed = text === '' ? {} : safeJsonParse(text)
    if (parsed === null) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodePurchaseBody(parsed)
    if (body === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    const nowIso = deps.nowIso?.() ?? currentIsoTimestamp()
    const purchaseRef = compactRandomId('privacy_purchase')
    const idempotencyKey =
      boundedIdempotencyKey(body.idempotencyKey) ??
      `privacy-purchase:${session.accountRef}:${purchaseRef}`
    const row = yield* Effect.tryPromise(() =>
      grantPaidPrivacyEntitlement(deps.db, {
        accountRef: session.accountRef,
        idempotencyKey,
        nowIso,
        purchaseRef,
      }),
    ).pipe(Effect.orDie)

    if (row === null) {
      return noStoreJsonResponse(
        { error: 'privacy_entitlement_receipt_not_recorded' },
        { status: 500 },
      )
    }

    return noStoreJsonResponse(
      {
        ok: true,
        captureExcluded: true,
        entitlementRef: row.entitlement_ref,
        receiptRef: row.receipt_ref,
        receiptUrl: `/api/public/inference/privacy-receipts/${encodeURIComponent(row.receipt_ref)}`,
      },
      { status: 201 },
    )
  })

export const handleConfidentialComputeExecutionReceipt = (
  request: Request,
  deps: PrivacyReceiptRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    if (!deps.confidentialComputeEnabled) {
      return noStoreJsonResponse(
        { error: 'confidential_compute_disabled' },
        { status: 404 },
      )
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      return authResponse()
    }

    const text = yield* readBody(request)
    const parsed = text === '' ? {} : safeJsonParse(text)
    if (parsed === null) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodeConfidentialBody(parsed)
    if (body === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    const nowIso = deps.nowIso?.() ?? currentIsoTimestamp()
    const requestRef = compactRandomId('confidential_request')
    const idempotencyKey =
      boundedIdempotencyKey(body.idempotencyKey) ??
      `confidential-compute:${session.accountRef}:${requestRef}`
    const row = yield* Effect.tryPromise(() =>
      recordConfidentialComputeExecutionReceipt(deps.db, {
        accountRef: session.accountRef,
        idempotencyKey,
        nowIso,
        requestRef,
      }),
    ).pipe(Effect.orDie)

    if (row === null) {
      return noStoreJsonResponse(
        { error: 'confidential_compute_receipt_not_recorded' },
        { status: 500 },
      )
    }

    return noStoreJsonResponse(
      {
        ok: true,
        captureExcluded: true,
        executionRef: row.execution_ref,
        receiptRef: row.receipt_ref,
        receiptUrl: `/api/public/inference/privacy-receipts/${encodeURIComponent(row.receipt_ref)}`,
      },
      { status: 201 },
    )
  })

export const handlePublicPrivacyReceiptRead = (
  request: Request,
  deps: PrivacyReceiptRoutesDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const url = new URL(request.url)
    const match = url.pathname.match(
      /^\/api\/public\/inference\/privacy-receipts\/(.+)$/,
    )
    if (!match) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const receiptRef = decodeURIComponent(match[1] ?? '')
    const projection = yield* Effect.tryPromise(() =>
      readPublicPrivacyReceipt(
        deps.db,
        receiptRef,
        deps.nowIso?.() ?? currentIsoTimestamp(),
      ),
    ).pipe(Effect.orDie)

    return projection === null
      ? noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      : noStoreJsonResponse({ receipt: projection })
  })
