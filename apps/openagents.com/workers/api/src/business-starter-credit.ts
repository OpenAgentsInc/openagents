import { Schema as S } from 'effect'

import {
  assertBusinessPipelinePublicSafeDescriptor,
  assertBusinessPipelinePublicSafeRef,
  businessPipelineSafeRefPart,
  type BusinessPipelineRuntime,
  type BusinessPipelineStore,
  BusinessPipelineValidationError,
  systemBusinessPipelineRuntime,
} from './business-pipeline-queue'
import { parseJsonStringArray } from './json-boundary'
import { runLedgerStatements } from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  usdCreditGrantReceiptRef,
  usdCreditGrantStatements,
} from './inference/usd-credit-bridge'
import { usdCentsToMsatFloor } from './inference/usd-msat-conversion'

export const SALES_STARTER_CREDIT_ATTRIBUTION_KIND = 'sales_starter_credit'
export const BUSINESS_STARTER_CREDIT_DEFAULT_AMOUNT_USD_CENTS = 10_000
export const BUSINESS_STARTER_CREDIT_DEFAULT_AMOUNT_CAP_USD_CENTS = 10_000
export const BUSINESS_STARTER_CREDIT_DEFAULT_WINDOW_GRANT_CAP = 25

export const BusinessStarterCreditGrant = S.Struct({
  accountRef: S.String,
  amountMsat: S.Number,
  amountUsdCents: S.Number,
  amountCapUsdCents: S.Number,
  attributionKind: S.Literal('sales_starter_credit'),
  createdAt: S.String,
  creditReceiptRef: S.String,
  engagementRef: S.String,
  grantRef: S.String,
  pipelineRef: S.String,
  redemptionReceiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  transferPolicy: S.Literal('non_transferable'),
  updatedAt: S.String,
  windowGrantCap: S.Number,
  windowRef: S.String,
})
export type BusinessStarterCreditGrant =
  typeof BusinessStarterCreditGrant.Type

export const BusinessStarterCreditGrantRefusalReason = S.Literals([
  'amount_invalid',
  'amount_cap_exceeded',
  'window_cap_exceeded',
  'zero_after_conversion',
])
export type BusinessStarterCreditGrantRefusalReason =
  typeof BusinessStarterCreditGrantRefusalReason.Type

export type BusinessStarterCreditGrantOutcome =
  | Readonly<{
      ok: true
      grant: BusinessStarterCreditGrant
      pipelineReceiptRefs: ReadonlyArray<string>
    }>
  | Readonly<{
      ok: false
      reason: BusinessStarterCreditGrantRefusalReason
      message: string
    }>

export type BusinessStarterCreditGrantInput = Readonly<{
  accountRef: string
  amountCapUsdCents?: number
  amountUsdCents?: number
  engagementRef?: string
  grantRef?: string
  sourceRefs?: ReadonlyArray<string>
  windowGrantCap?: number
  windowRef?: string
}>

export type BusinessStarterCreditRedemptionInput = Readonly<{
  grantRef: string
  redemptionReceiptRef: string
}>

export class BusinessStarterCreditStoreError extends S.TaggedErrorClass<BusinessStarterCreditStoreError>()(
  'BusinessStarterCreditStoreError',
  {
    kind: S.Literals(['conflict', 'not_found', 'storage_error', 'validation_error']),
    reason: S.String,
  },
) {}

export type BusinessStarterCreditRuntime = BusinessPipelineRuntime &
  Readonly<{
    usdCentsToMsat: (amountUsdCents: number) => number
  }>

export const systemBusinessStarterCreditRuntime: BusinessStarterCreditRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
  usdCentsToMsat: usdCentsToMsatFloor,
}

type BusinessStarterCreditGrantRow = Readonly<{
  account_ref: string
  amount_cap_usd_cents: number
  amount_msat: number
  amount_usd_cents: number
  attribution_kind: typeof SALES_STARTER_CREDIT_ATTRIBUTION_KIND
  created_at: string
  credit_receipt_ref: string
  engagement_ref: string
  grant_ref: string
  pipeline_ref: string
  redemption_receipt_refs_json: string
  source_refs_json: string
  transfer_policy: 'non_transferable'
  updated_at: string
  window_grant_cap: number
  window_ref: string
}>

type CountRow = Readonly<{ count: number }>

const grantSelect = `SELECT
  grant_ref,
  pipeline_ref,
  account_ref,
  engagement_ref,
  attribution_kind,
  transfer_policy,
  amount_usd_cents,
  amount_msat,
  amount_cap_usd_cents,
  window_ref,
  window_grant_cap,
  credit_receipt_ref,
  redemption_receipt_refs_json,
  source_refs_json,
  created_at,
  updated_at
 FROM business_starter_credit_grants`

const normalizePositiveInteger = (
  field: string,
  value: number,
): number => {
  const normalized = Math.trunc(value)
  if (!Number.isFinite(normalized) || normalized <= 0) {
    throw new BusinessStarterCreditStoreError({
      kind: 'validation_error',
      reason: `${field} must be a positive integer`,
    })
  }
  return normalized
}

const defaultWindowRef = (nowIso: string): string =>
  `${SALES_STARTER_CREDIT_ATTRIBUTION_KIND}.${nowIso.slice(0, 7)}`

const defaultEngagementRef = (pipelineRef: string): string =>
  `business.engagement.pipeline_${businessPipelineSafeRefPart(pipelineRef)}`

const defaultGrantRef = (
  pipelineRef: string,
  runtime: BusinessStarterCreditRuntime,
): string =>
  `sales-starter:${businessPipelineSafeRefPart(pipelineRef)}:${runtime.makeId('grant')}`

const normalizeRefs = (
  field: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = [...new Set((refs ?? []).map(ref => ref.trim()).filter(Boolean))]
  normalized.forEach(ref => assertBusinessPipelinePublicSafeRef(field, ref))
  return normalized
}

const grantFromRow = (
  row: BusinessStarterCreditGrantRow,
): BusinessStarterCreditGrant => {
  const grant: BusinessStarterCreditGrant = {
    accountRef: row.account_ref,
    amountCapUsdCents: Number(row.amount_cap_usd_cents),
    amountMsat: Number(row.amount_msat),
    amountUsdCents: Number(row.amount_usd_cents),
    attributionKind: row.attribution_kind,
    createdAt: row.created_at,
    creditReceiptRef: row.credit_receipt_ref,
    engagementRef: row.engagement_ref,
    grantRef: row.grant_ref,
    pipelineRef: row.pipeline_ref,
    redemptionReceiptRefs: parseJsonStringArray(row.redemption_receipt_refs_json),
    sourceRefs: parseJsonStringArray(row.source_refs_json),
    transferPolicy: row.transfer_policy,
    updatedAt: row.updated_at,
    windowGrantCap: Number(row.window_grant_cap),
    windowRef: row.window_ref,
  }

  assertBusinessPipelinePublicSafeRef('grantRef', grant.grantRef)
  assertBusinessPipelinePublicSafeRef('pipelineRef', grant.pipelineRef)
  assertBusinessPipelinePublicSafeRef('accountRef', grant.accountRef)
  assertBusinessPipelinePublicSafeRef('engagementRef', grant.engagementRef)
  assertBusinessPipelinePublicSafeRef('creditReceiptRef', grant.creditReceiptRef)
  assertBusinessPipelinePublicSafeRef('windowRef', grant.windowRef)
  grant.redemptionReceiptRefs.forEach(ref =>
    assertBusinessPipelinePublicSafeRef('redemptionReceiptRefs', ref),
  )
  grant.sourceRefs.forEach(ref =>
    assertBusinessPipelinePublicSafeRef('sourceRefs', ref),
  )

  return S.decodeUnknownSync(BusinessStarterCreditGrant)(grant)
}

const storageError = (error: unknown): BusinessStarterCreditStoreError =>
  error instanceof BusinessStarterCreditStoreError
    ? error
    : error instanceof BusinessPipelineValidationError
      ? new BusinessStarterCreditStoreError({
          kind: 'validation_error',
          reason: error.reason,
        })
    : new BusinessStarterCreditStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

// Business-funnel table update (D1) — never part of the credits ledger batch.
const pipelineReceiptUpdateStatement = (
  db: D1Database,
  pipelineRef: string,
  receiptRefs: ReadonlyArray<string>,
  nowIso: string,
): D1PreparedStatement =>
  db
    .prepare(
      `UPDATE business_pipeline_rows
        SET receipt_refs_json = ?,
            updated_at = ?
        WHERE pipeline_ref = ?`,
    )
    .bind(JSON.stringify(receiptRefs), nowIso, pipelineRef)

export type BusinessStarterCreditStore = Readonly<{
  createGrant: (
    pipelineRef: string,
    input: BusinessStarterCreditGrantInput,
    runtime?: BusinessStarterCreditRuntime,
  ) => Promise<BusinessStarterCreditGrantOutcome>
  linkRedemption: (
    pipelineRef: string,
    input: BusinessStarterCreditRedemptionInput,
    runtime?: BusinessPipelineRuntime,
  ) => Promise<BusinessStarterCreditGrant>
  readGrant: (grantRef: string) => Promise<BusinessStarterCreditGrant | null>
}>

export const makeD1BusinessStarterCreditStore = (
  db: D1Database,
  /** CFG-4 (#8519): the Postgres-authoritative credits ledger — the
   * `pay_ins`/`pay_in_legs`/`agent_balances` rows `usdCreditGrantStatements`
   * creates run here; `business_starter_credit_grants` and
   * `business_pipeline_rows` stay on D1 (business-funnel domain). */
  ledgerDb: PaymentsLedgerDb,
  pipelineStore: BusinessPipelineStore,
  defaultRuntime: BusinessStarterCreditRuntime = systemBusinessStarterCreditRuntime,
): BusinessStarterCreditStore => {
  const readGrant = async (
    grantRef: string,
  ): Promise<BusinessStarterCreditGrant | null> => {
    assertBusinessPipelinePublicSafeRef('grantRef', grantRef)
    const row = await db
      .prepare(`${grantSelect} WHERE grant_ref = ?`)
      .bind(grantRef)
      .first<BusinessStarterCreditGrantRow>()

    return row === null ? null : grantFromRow(row)
  }

  const createGrant = async (
    pipelineRefInput: string,
    input: BusinessStarterCreditGrantInput,
    runtime: BusinessStarterCreditRuntime = defaultRuntime,
  ): Promise<BusinessStarterCreditGrantOutcome> => {
    try {
      const pipelineRef = pipelineRefInput.trim()
      assertBusinessPipelinePublicSafeRef('pipelineRef', pipelineRef)

      const pipeline = await pipelineStore.readPipelineRow(pipelineRef)
      if (pipeline === null) {
        throw new BusinessStarterCreditStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }

      const nowIso = runtime.nowIso()
      const grantRef = (input.grantRef ?? defaultGrantRef(pipelineRef, runtime)).trim()
      const accountRef = input.accountRef.trim()
      const engagementRef = (input.engagementRef ?? defaultEngagementRef(pipelineRef)).trim()
      const windowRef = (input.windowRef ?? defaultWindowRef(nowIso)).trim()
      const amountUsdCents = normalizePositiveInteger(
        'amountUsdCents',
        input.amountUsdCents ?? BUSINESS_STARTER_CREDIT_DEFAULT_AMOUNT_USD_CENTS,
      )
      const amountCapUsdCents = normalizePositiveInteger(
        'amountCapUsdCents',
        input.amountCapUsdCents ??
          BUSINESS_STARTER_CREDIT_DEFAULT_AMOUNT_CAP_USD_CENTS,
      )
      const windowGrantCap = normalizePositiveInteger(
        'windowGrantCap',
        input.windowGrantCap ?? BUSINESS_STARTER_CREDIT_DEFAULT_WINDOW_GRANT_CAP,
      )
      const sourceRefs = normalizeRefs('sourceRefs', [
        'github:OpenAgentsInc/openagents#8264',
        ...(input.sourceRefs ?? []),
      ])

      assertBusinessPipelinePublicSafeRef('grantRef', grantRef)
      assertBusinessPipelinePublicSafeRef('accountRef', accountRef)
      assertBusinessPipelinePublicSafeRef('engagementRef', engagementRef)
      assertBusinessPipelinePublicSafeRef('windowRef', windowRef)
      assertBusinessPipelinePublicSafeDescriptor('attributionKind', SALES_STARTER_CREDIT_ATTRIBUTION_KIND)

      const existing = await readGrant(grantRef)
      if (existing !== null) {
        if (existing.pipelineRef !== pipelineRef) {
          throw new BusinessStarterCreditStoreError({
            kind: 'conflict',
            reason: `starter credit grant belongs to another pipeline: ${grantRef}`,
          })
        }
        // CFG-4 (#8519): the grants row (D1) commits before the credits
        // transaction (Postgres) — see the seam comment below. If an earlier
        // attempt crashed between the two, this replay heals it by re-running
        // the idempotent credits statements (pay_ins UNIQUE idempotency key +
        // replay-guarded balance credit make an already-granted credit a
        // no-op — never a double grant).
        await runLedgerStatements(
          ledgerDb,
          usdCreditGrantStatements(
            {
              accountRef: existing.accountRef,
              contextRef: `${SALES_STARTER_CREDIT_ATTRIBUTION_KIND}:${pipelineRef}:${grantRef}`,
              grantMsat: existing.amountMsat,
              grantRef,
            },
            nowIso,
          ),
        )
        return {
          grant: existing,
          ok: true,
          pipelineReceiptRefs: pipeline.receiptRefs,
        }
      }

      if (amountUsdCents > amountCapUsdCents) {
        return {
          message: `Starter credit amount exceeds the ${amountCapUsdCents} cent cap.`,
          ok: false,
          reason: 'amount_cap_exceeded',
        }
      }

      const windowCount = await db
        .prepare(
          `SELECT COUNT(*) AS count
             FROM business_starter_credit_grants
            WHERE window_ref = ?`,
        )
        .bind(windowRef)
        .first<CountRow>()

      if (Number(windowCount?.count ?? 0) >= windowGrantCap) {
        return {
          message: `Starter credit window cap exceeded for ${windowRef}.`,
          ok: false,
          reason: 'window_cap_exceeded',
        }
      }

      const amountMsat = runtime.usdCentsToMsat(amountUsdCents)
      if (!Number.isInteger(amountMsat) || amountMsat <= 0) {
        return {
          message: 'Starter credit amount converts below one millisatoshi.',
          ok: false,
          reason: 'zero_after_conversion',
        }
      }

      const creditReceiptRef = usdCreditGrantReceiptRef(grantRef)
      assertBusinessPipelinePublicSafeRef('creditReceiptRef', creditReceiptRef)
      const pipelineReceiptRefs = [
        ...new Set([...pipeline.receiptRefs, creditReceiptRef]),
      ]

      // CFG-4 (#8519) NON-ATOMIC SEAM: the credits side (`pay_ins`/
      // `pay_in_legs`/`agent_balances`, Postgres) and the business-funnel
      // side (`business_starter_credit_grants` + `business_pipeline_rows`,
      // D1) no longer share one atomic batch — the credits domain is
      // Postgres-authoritative now. Ordering: the D1 grants row commits
      // FIRST, so the `business_starter_credit_window_cap` trigger
      // (migration 0295) can still refuse a raced over-cap grant BEFORE any
      // credit is minted (credits-first would let a raced cap refusal orphan
      // an already-minted credit). A crash between the two heals on retry:
      // the replayed call hits the existing-grant early return above, which
      // re-runs the idempotent credits statements and never double-grants.
      await db.batch([
        db
          .prepare(
            `INSERT INTO business_starter_credit_grants (
                  grant_ref,
                  pipeline_ref,
                  account_ref,
                  engagement_ref,
                  attribution_kind,
                  transfer_policy,
                  amount_usd_cents,
                  amount_msat,
                  amount_cap_usd_cents,
                  window_ref,
                  window_grant_cap,
                  credit_receipt_ref,
                  redemption_receipt_refs_json,
                  source_refs_json,
                  created_at,
                  updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            grantRef,
            pipelineRef,
            accountRef,
            engagementRef,
            SALES_STARTER_CREDIT_ATTRIBUTION_KIND,
            'non_transferable',
            amountUsdCents,
            amountMsat,
            amountCapUsdCents,
            windowRef,
            windowGrantCap,
            creditReceiptRef,
            JSON.stringify([]),
            JSON.stringify(sourceRefs),
            nowIso,
            nowIso,
          ),
        pipelineReceiptUpdateStatement(db, pipelineRef, pipelineReceiptRefs, nowIso),
      ])

      // Second half of the seam: the idempotent credits transaction.
      await runLedgerStatements(
        ledgerDb,
        usdCreditGrantStatements(
          {
            accountRef,
            contextRef: `${SALES_STARTER_CREDIT_ATTRIBUTION_KIND}:${pipelineRef}:${grantRef}`,
            grantMsat: amountMsat,
            grantRef,
          },
          nowIso,
        ),
      )

      const grant = await readGrant(grantRef)
      if (grant === null) {
        throw new BusinessStarterCreditStoreError({
          kind: 'storage_error',
          reason: `starter credit grant was not readable after create: ${grantRef}`,
        })
      }

      return { grant, ok: true, pipelineReceiptRefs }
    } catch (error) {
      if (
        error instanceof Error &&
        error.message.includes('business_starter_credit_window_cap_exceeded')
      ) {
        return {
          message: 'Starter credit window cap exceeded.',
          ok: false,
          reason: 'window_cap_exceeded',
        }
      }
      throw storageError(error)
    }
  }

  const linkRedemption = async (
    pipelineRefInput: string,
    input: BusinessStarterCreditRedemptionInput,
    runtime: BusinessPipelineRuntime = systemBusinessPipelineRuntime,
  ): Promise<BusinessStarterCreditGrant> => {
    try {
      const pipelineRef = pipelineRefInput.trim()
      const grantRef = input.grantRef.trim()
      const redemptionReceiptRef = input.redemptionReceiptRef.trim()
      assertBusinessPipelinePublicSafeRef('pipelineRef', pipelineRef)
      assertBusinessPipelinePublicSafeRef('grantRef', grantRef)
      assertBusinessPipelinePublicSafeRef('redemptionReceiptRef', redemptionReceiptRef)

      const grant = await readGrant(grantRef)
      if (grant === null || grant.pipelineRef !== pipelineRef) {
        throw new BusinessStarterCreditStoreError({
          kind: 'not_found',
          reason: `starter credit grant not found for pipeline: ${grantRef}`,
        })
      }

      const pipeline = await pipelineStore.readPipelineRow(pipelineRef)
      if (pipeline === null) {
        throw new BusinessStarterCreditStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }

      const nowIso = runtime.nowIso()
      const redemptionReceiptRefs = [
        ...new Set([...grant.redemptionReceiptRefs, redemptionReceiptRef]),
      ]
      const pipelineReceiptRefs = [
        ...new Set([...pipeline.receiptRefs, grant.creditReceiptRef, redemptionReceiptRef]),
      ]

      // Business-funnel tables only (no credits rows) — a plain D1 batch.
      await db.batch([
        db
          .prepare(
            `UPDATE business_starter_credit_grants
                SET redemption_receipt_refs_json = ?,
                    updated_at = ?
                WHERE grant_ref = ?`,
          )
          .bind(JSON.stringify(redemptionReceiptRefs), nowIso, grantRef),
        pipelineReceiptUpdateStatement(db, pipelineRef, pipelineReceiptRefs, nowIso),
      ])

      const updated = await readGrant(grantRef)
      if (updated === null) {
        throw new BusinessStarterCreditStoreError({
          kind: 'storage_error',
          reason: `starter credit grant was not readable after redemption link: ${grantRef}`,
        })
      }
      return updated
    } catch (error) {
      throw storageError(error)
    }
  }

  return { createGrant, linkRedemption, readGrant }
}
