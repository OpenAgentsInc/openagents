import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { BILLING_CURRENCY, formatUsdCents } from './billing'
import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const ADJUTANT_USAGE_CATEGORIES = [
  'generation',
  'build',
  'hosting',
  'storage',
  'adjustment',
] as const

export const AdjutantUsageReceiptCategory = S.Literals([
  'generation',
  'build',
  'hosting',
  'storage',
  'adjustment',
])
export type AdjutantUsageReceiptCategory =
  typeof AdjutantUsageReceiptCategory.Type

export const AdjutantUsageReceiptBillingMode = S.Literals([
  'public_beta_free',
  'paid_credits',
])
export type AdjutantUsageReceiptBillingMode =
  typeof AdjutantUsageReceiptBillingMode.Type

export type AdjutantUsageReceiptRuntime = Readonly<{
  makeReceiptId: () => string
  nowIso: () => string
}>

export const systemAdjutantUsageReceiptRuntime: AdjutantUsageReceiptRuntime = {
  makeReceiptId: () => compactRandomId('adjutant_usage_receipt'),
  nowIso: currentIsoTimestamp,
}

export type RecordAdjutantUsageReceiptInput = Readonly<{
  adjustmentId?: string | null | undefined
  assignmentId: string
  billingLedgerEntryId?: string | null | undefined
  billingMode?: AdjutantUsageReceiptBillingMode | undefined
  category: AdjutantUsageReceiptCategory
  creditsChargedCents?: number | undefined
  idempotencyKey: string
  publicDetails?: Record<string, unknown> | undefined
  quantity: number
  runId?: string | null | undefined
  siteId?: string | null | undefined
  softwareOrderId?: string | null | undefined
  summary: string
  teamDetails?: Record<string, unknown> | undefined
  unit: string
  visibility: 'private' | 'team' | 'public'
}>

type AdjutantUsageReceiptRow = Readonly<{
  adjustment_id: string | null
  assignment_id: string
  billing_ledger_entry_id: string | null
  billing_mode: AdjutantUsageReceiptBillingMode
  category: AdjutantUsageReceiptCategory
  created_at: string
  credits_charged_cents: number
  currency: string
  id: string
  public_receipt_json: string
  quantity: number
  run_id: string | null
  site_id: string | null
  software_order_id: string | null
  summary: string
  team_receipt_json: string
  unit: string
  visibility: 'private' | 'team' | 'public'
}>

export type AdjutantUsageReceipt = Readonly<{
  adjustmentId: string | null
  assignmentId: string
  billingLedgerEntryId: string | null
  billingMode: AdjutantUsageReceiptBillingMode
  category: AdjutantUsageReceiptCategory
  createdAt: string
  creditsChargedCents: number
  creditsChargedFormatted: string
  currency: string
  id: string
  publicDetails: Record<string, unknown>
  quantity: number
  runId: string | null
  siteId: string | null
  softwareOrderId: string | null
  summary: string
  teamDetails: Record<string, unknown>
  unit: string
  visibility: 'private' | 'team' | 'public'
}>

export type CustomerAdjutantUsageReceipt = Readonly<{
  billingMode: AdjutantUsageReceiptBillingMode
  category: AdjutantUsageReceiptCategory
  createdAt: string
  creditsChargedCents: number
  creditsChargedFormatted: string
  details: Record<string, unknown>
  id: string
  quantity: number
  summary: string
  unit: string
}>

export type AdjutantUsageReceiptCategoryTotal = Readonly<{
  category: AdjutantUsageReceiptCategory
  creditsChargedCents: number
  creditsChargedFormatted: string
  quantity: number
  receiptCount: number
  unit: string | null
}>

export type AdjutantUsageReceiptSummary = Readonly<{
  billingMode: AdjutantUsageReceiptBillingMode
  categories: ReadonlyArray<AdjutantUsageReceiptCategoryTotal>
  totalCreditsChargedCents: number
  totalCreditsChargedFormatted: string
}>

export class AdjutantUsageReceiptStorageError extends S.TaggedErrorClass<AdjutantUsageReceiptStorageError>()(
  'AdjutantUsageReceiptStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantUsageReceiptUnsafe extends S.TaggedErrorClass<AdjutantUsageReceiptUnsafe>()(
  'AdjutantUsageReceiptUnsafe',
  {
    reason: S.String,
  },
) {}

export type AdjutantUsageReceiptError =
  | AdjutantUsageReceiptStorageError
  | AdjutantUsageReceiptUnsafe

const RECEIPT_DETAILS_LIMIT_BYTES = 4096

const nullableText = (value: string | null | undefined): string | null => {
  const text = value?.trim()

  return text === undefined || text === '' ? null : text
}

const compactSummary = (
  value: string,
): Effect.Effect<string, AdjutantUsageReceiptUnsafe> =>
  Effect.gen(function* () {
    const summary = value.replace(/\s+/g, ' ').trim()

    if (summary === '') {
      return yield* new AdjutantUsageReceiptUnsafe({
        reason: 'Usage receipt summary is required.',
      })
    }

    if (containsProviderSecretMaterial(summary)) {
      return yield* new AdjutantUsageReceiptUnsafe({
        reason: 'Usage receipt summary contains secret-shaped material.',
      })
    }

    return summary.length <= 240 ? summary : `${summary.slice(0, 237)}...`
  })

const safeDetailsJson = (
  value: Record<string, unknown> | undefined,
): Effect.Effect<string, AdjutantUsageReceiptUnsafe> =>
  Effect.gen(function* () {
    const json = yield* Effect.try({
      catch: error =>
        new AdjutantUsageReceiptUnsafe({
          reason:
            error instanceof Error
              ? error.message
              : 'Usage receipt details are not serializable.',
        }),
      try: () => JSON.stringify(value ?? {}),
    })

    if (json.length > RECEIPT_DETAILS_LIMIT_BYTES) {
      return yield* new AdjutantUsageReceiptUnsafe({
        reason: 'Usage receipt details are too large.',
      })
    }

    if (containsProviderSecretMaterial(json)) {
      return yield* new AdjutantUsageReceiptUnsafe({
        reason: 'Usage receipt details contain secret-shaped material.',
      })
    }

    return json
  })

const validateBillingPolicy = (
  input: RecordAdjutantUsageReceiptInput,
): Effect.Effect<
  Readonly<{
    billingLedgerEntryId: string | null
    billingMode: AdjutantUsageReceiptBillingMode
    creditsChargedCents: number
  }>,
  AdjutantUsageReceiptUnsafe
> =>
  Effect.gen(function* () {
    const billingMode = input.billingMode ?? 'public_beta_free'
    const creditsChargedCents = Math.max(
      0,
      Math.trunc(input.creditsChargedCents ?? 0),
    )
    const billingLedgerEntryId = nullableText(input.billingLedgerEntryId)

    if (billingMode === 'public_beta_free') {
      if (creditsChargedCents !== 0 || billingLedgerEntryId !== null) {
        return yield* new AdjutantUsageReceiptUnsafe({
          reason:
            'Public beta Autopilot Site receipts must not charge credits or link billing debits.',
        })
      }
    }

    if (billingMode === 'paid_credits' && billingLedgerEntryId === null) {
      return yield* new AdjutantUsageReceiptUnsafe({
        reason:
          'Paid Autopilot Site receipts must link an existing billing ledger entry.',
      })
    }

    return {
      billingLedgerEntryId,
      billingMode,
      creditsChargedCents,
    }
  })

const receiptFromRow = (
  row: AdjutantUsageReceiptRow,
): AdjutantUsageReceipt => ({
  adjustmentId: row.adjustment_id,
  assignmentId: row.assignment_id,
  billingLedgerEntryId: row.billing_ledger_entry_id,
  billingMode: row.billing_mode,
  category: row.category,
  createdAt: row.created_at,
  creditsChargedCents: row.credits_charged_cents,
  creditsChargedFormatted: formatUsdCents(row.credits_charged_cents),
  currency: row.currency,
  id: row.id,
  publicDetails: parseJsonRecord(row.public_receipt_json) ?? {},
  quantity: row.quantity,
  runId: row.run_id,
  siteId: row.site_id,
  softwareOrderId: row.software_order_id,
  summary: row.summary,
  teamDetails: parseJsonRecord(row.team_receipt_json) ?? {},
  unit: row.unit,
  visibility: row.visibility,
})

const customerReceiptFromReceipt = (
  receipt: AdjutantUsageReceipt,
): CustomerAdjutantUsageReceipt => ({
  billingMode: receipt.billingMode,
  category: receipt.category,
  createdAt: receipt.createdAt,
  creditsChargedCents: receipt.creditsChargedCents,
  creditsChargedFormatted: receipt.creditsChargedFormatted,
  details: receipt.publicDetails,
  id: receipt.id,
  quantity: receipt.quantity,
  summary: receipt.summary,
  unit: receipt.unit,
})

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantUsageReceiptStorageError> =>
  Effect.tryPromise({
    catch: error => new AdjutantUsageReceiptStorageError({ error, operation }),
    try: run,
  })

const readReceiptByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  AdjutantUsageReceipt | null,
  AdjutantUsageReceiptStorageError
> =>
  d1Effect('adjutantUsageReceipts.readByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                software_order_id,
                site_id,
                adjustment_id,
                run_id,
                category,
                visibility,
                billing_mode,
                summary,
                quantity,
                unit,
                credits_charged_cents,
                currency,
                billing_ledger_entry_id,
                public_receipt_json,
                team_receipt_json,
                created_at
           FROM adjutant_usage_receipts
          WHERE idempotency_key = ?
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<AdjutantUsageReceiptRow>(),
  ).pipe(Effect.map(row => (row === null ? null : receiptFromRow(row))))

export const recordAdjutantUsageReceipt = (
  db: D1Database,
  input: RecordAdjutantUsageReceiptInput,
  runtime: AdjutantUsageReceiptRuntime = systemAdjutantUsageReceiptRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<AdjutantUsageReceipt, AdjutantUsageReceiptError> =>
  Effect.gen(function* () {
    const unit = yield* compactSummary(input.unit)
    const summary = yield* compactSummary(input.summary)
    const publicReceiptJson = yield* safeDetailsJson(input.publicDetails)
    const teamReceiptJson = yield* safeDetailsJson(input.teamDetails)
    const billing = yield* validateBillingPolicy(input)
    const now = runtime.nowIso()
    const quantity = Math.max(0, Math.trunc(input.quantity))

    yield* d1Effect('adjutantUsageReceipts.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO adjutant_usage_receipts
             (id,
              assignment_id,
              software_order_id,
              site_id,
              adjustment_id,
              run_id,
              category,
              visibility,
              billing_mode,
              summary,
              quantity,
              unit,
              credits_charged_cents,
              currency,
              billing_ledger_entry_id,
              public_receipt_json,
              team_receipt_json,
              idempotency_key,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runtime.makeReceiptId(),
          input.assignmentId,
          nullableText(input.softwareOrderId),
          nullableText(input.siteId),
          nullableText(input.adjustmentId),
          nullableText(input.runId),
          input.category,
          input.visibility,
          billing.billingMode,
          summary,
          quantity,
          unit,
          billing.creditsChargedCents,
          BILLING_CURRENCY,
          billing.billingLedgerEntryId,
          publicReceiptJson,
          teamReceiptJson,
          input.idempotencyKey,
          now,
        )
        .run(),
    )

    const receipt = yield* readReceiptByIdempotencyKey(db, input.idempotencyKey)

    if (receipt === null) {
      return yield* new AdjutantUsageReceiptStorageError({
        error: 'Usage receipt insert did not return a row.',
        operation: 'adjutantUsageReceipts.insert.readback',
      })
    }

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('adjutant_usage_receipts', [[receipt.id]]),
      )
    }

    return receipt
  })

export const listAdjutantUsageReceiptsForAssignment = (
  db: D1Database,
  assignmentId: string,
  limit = 50,
): Effect.Effect<
  ReadonlyArray<AdjutantUsageReceipt>,
  AdjutantUsageReceiptStorageError
> =>
  d1Effect('adjutantUsageReceipts.assignment.list', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                software_order_id,
                site_id,
                adjustment_id,
                run_id,
                category,
                visibility,
                billing_mode,
                summary,
                quantity,
                unit,
                credits_charged_cents,
                currency,
                billing_ledger_entry_id,
                public_receipt_json,
                team_receipt_json,
                created_at
           FROM adjutant_usage_receipts
          WHERE assignment_id = ?
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(assignmentId, Math.max(1, Math.min(100, Math.trunc(limit))))
      .all<AdjutantUsageReceiptRow>(),
  ).pipe(Effect.map(result => result.results.map(receiptFromRow)))

export const listCustomerAdjutantUsageReceiptsForOrder = (
  db: D1Database,
  softwareOrderId: string,
  limit = 50,
): Effect.Effect<
  ReadonlyArray<CustomerAdjutantUsageReceipt>,
  AdjutantUsageReceiptStorageError
> =>
  d1Effect('adjutantUsageReceipts.customerOrder.list', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                software_order_id,
                site_id,
                adjustment_id,
                run_id,
                category,
                visibility,
                billing_mode,
                summary,
                quantity,
                unit,
                credits_charged_cents,
                currency,
                billing_ledger_entry_id,
                public_receipt_json,
                team_receipt_json,
                created_at
           FROM adjutant_usage_receipts
          WHERE software_order_id = ?
            AND visibility = 'public'
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(softwareOrderId, Math.max(1, Math.min(100, Math.trunc(limit))))
      .all<AdjutantUsageReceiptRow>(),
  ).pipe(
    Effect.map(result =>
      result.results.map(receiptFromRow).map(customerReceiptFromReceipt),
    ),
  )

export const summarizeAdjutantUsageReceipts = (
  receipts: ReadonlyArray<
    Pick<
      CustomerAdjutantUsageReceipt,
      'billingMode' | 'category' | 'creditsChargedCents' | 'quantity' | 'unit'
    >
  >,
): AdjutantUsageReceiptSummary => {
  const totalCreditsChargedCents = receipts.reduce(
    (total, receipt) => total + receipt.creditsChargedCents,
    0,
  )
  const categories = ADJUTANT_USAGE_CATEGORIES.flatMap(category => {
    const categoryReceipts = receipts.filter(
      receipt => receipt.category === category,
    )

    if (categoryReceipts.length === 0) {
      return []
    }

    const units = new Set(categoryReceipts.map(receipt => receipt.unit))
    const creditsChargedCents = categoryReceipts.reduce(
      (total, receipt) => total + receipt.creditsChargedCents,
      0,
    )

    return [
      {
        category,
        creditsChargedCents,
        creditsChargedFormatted: formatUsdCents(creditsChargedCents),
        quantity: categoryReceipts.reduce(
          (total, receipt) => total + receipt.quantity,
          0,
        ),
        receiptCount: categoryReceipts.length,
        unit: units.size === 1 ? (categoryReceipts[0]?.unit ?? null) : null,
      },
    ]
  })

  return {
    billingMode: receipts.some(
      receipt => receipt.billingMode === 'paid_credits',
    )
      ? 'paid_credits'
      : 'public_beta_free',
    categories,
    totalCreditsChargedCents,
    totalCreditsChargedFormatted: formatUsdCents(totalCreditsChargedCents),
  }
}
