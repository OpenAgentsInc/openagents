import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const OmniAcceptedOutcomeFundingMode = S.Literals([
  'free_beta',
  'credit_funded',
  'sats_funded',
  'internal_only',
])
export type OmniAcceptedOutcomeFundingMode =
  typeof OmniAcceptedOutcomeFundingMode.Type

export const OmniAcceptedOutcomeBuyerPriceAsset = S.Literals([
  'none',
  'usd',
  'credits',
  'sats',
])
export type OmniAcceptedOutcomeBuyerPriceAsset =
  typeof OmniAcceptedOutcomeBuyerPriceAsset.Type

export const OmniAcceptedOutcomeEconomicsRecord = S.Struct({
  acceptedOutcomeContractId: S.NullOr(S.String),
  acceptedValueCents: S.Number,
  archivedAt: S.NullOr(S.String),
  artifactCostCents: S.Number,
  buyerPriceAsset: OmniAcceptedOutcomeBuyerPriceAsset,
  buyerPriceCents: S.Number,
  createdAt: S.String,
  creditsCharged: S.Number,
  fundingMode: OmniAcceptedOutcomeFundingMode,
  grossMarginCents: S.Number,
  id: S.String,
  idempotencyKey: S.String,
  internalCaveatRef: S.NullOr(S.String),
  metadata: S.Record(S.String, S.Unknown),
  noSettlementImplication: S.Boolean,
  providerCostCents: S.Number,
  publicCaveatRef: S.String,
  retryCostCents: S.Number,
  reviewCostCents: S.Number,
  reviewMinutes: S.Number,
  runnerCostCents: S.Number,
  satsCharged: S.Number,
  totalCostCents: S.Number,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniAcceptedOutcomeEconomicsRecord =
  typeof OmniAcceptedOutcomeEconomicsRecord.Type

export type OmniAcceptedOutcomeEconomicsRuntime = Readonly<{
  makeEconomicsId: () => string
  nowIso: () => string
}>

export const systemOmniAcceptedOutcomeEconomicsRuntime: OmniAcceptedOutcomeEconomicsRuntime =
  {
    makeEconomicsId: () => compactRandomId('omni_outcome_economics'),
    nowIso: currentIsoTimestamp,
  }

export type RecordOmniAcceptedOutcomeEconomicsInput = Readonly<{
  acceptedOutcomeContractId?: string | undefined
  acceptedValueCents: number
  artifactCostCents?: number | undefined
  buyerPriceAsset: OmniAcceptedOutcomeBuyerPriceAsset
  buyerPriceCents?: number | undefined
  creditsCharged?: number | undefined
  fundingMode: OmniAcceptedOutcomeFundingMode
  id?: string | undefined
  idempotencyKey: string
  internalCaveatRef?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  providerCostCents?: number | undefined
  publicCaveatRef: string
  retryCostCents?: number | undefined
  reviewCostCents?: number | undefined
  reviewMinutes?: number | undefined
  runnerCostCents?: number | undefined
  satsCharged?: number | undefined
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

type WorkroomRefRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

type ContractRefRow = Readonly<{
  archived_at: string | null
  id: string
}>

type EconomicsRow = Readonly<{
  accepted_outcome_contract_id: string | null
  accepted_value_cents: number
  archived_at: string | null
  artifact_cost_cents: number
  buyer_price_asset: OmniAcceptedOutcomeBuyerPriceAsset
  buyer_price_cents: number
  created_at: string
  credits_charged: number
  funding_mode: OmniAcceptedOutcomeFundingMode
  gross_margin_cents: number
  id: string
  idempotency_key: string
  internal_caveat_ref: string | null
  metadata_json: string
  no_settlement_implication: number
  provider_cost_cents: number
  public_caveat_ref: string
  retry_cost_cents: number
  review_cost_cents: number
  review_minutes: number
  runner_cost_cents: number
  sats_charged: number
  total_cost_cents: number
  updated_at: string
  work_kind: OmniAcceptedOutcomeWorkKind
  workroom_id: string
}>

export class OmniAcceptedOutcomeEconomicsValidationError extends S.TaggedErrorClass<OmniAcceptedOutcomeEconomicsValidationError>()(
  'OmniAcceptedOutcomeEconomicsValidationError',
  { reason: S.String },
) {}

export class OmniAcceptedOutcomeEconomicsStorageError extends S.TaggedErrorClass<OmniAcceptedOutcomeEconomicsStorageError>()(
  'OmniAcceptedOutcomeEconomicsStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OmniAcceptedOutcomeEconomicsWorkroomNotFound extends S.TaggedErrorClass<OmniAcceptedOutcomeEconomicsWorkroomNotFound>()(
  'OmniAcceptedOutcomeEconomicsWorkroomNotFound',
  { workroomId: S.String },
) {}

export class OmniAcceptedOutcomeEconomicsContractNotFound extends S.TaggedErrorClass<OmniAcceptedOutcomeEconomicsContractNotFound>()(
  'OmniAcceptedOutcomeEconomicsContractNotFound',
  { acceptedOutcomeContractId: S.String },
) {}

export type OmniAcceptedOutcomeEconomicsError =
  | OmniAcceptedOutcomeEconomicsContractNotFound
  | OmniAcceptedOutcomeEconomicsStorageError
  | OmniAcceptedOutcomeEconomicsValidationError
  | OmniAcceptedOutcomeEconomicsWorkroomNotFound

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i
const PROHIBITED_FRAGMENTS = [
  'eligible_for_payout',
  'paid_out',
  'payment_settled',
  'payout',
  'settlement',
]

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_TEXT_PATTERN.test(value) &&
  !PROHIBITED_FRAGMENTS.some(fragment =>
    value.toLowerCase().includes(fragment),
  )

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason: `${field} must be an economics-safe ref without raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.`,
    })
  }
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): void => {
  if (metadata === undefined) {
    return
  }

  const json = JSON.stringify(metadata)

  if (
    containsProviderSecretMaterial(json) ||
    PROHIBITED_TEXT_PATTERN.test(json) ||
    PROHIBITED_FRAGMENTS.some(fragment => json.toLowerCase().includes(fragment))
  ) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.',
    })
  }
}

const integer = (field: string, value: number | undefined): number => {
  const normalized = value ?? 0

  if (!Number.isInteger(normalized) || normalized < 0) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason: `${field} must be a non-negative integer.`,
    })
  }

  return normalized
}

const assertFundingMode = (
  input: RecordOmniAcceptedOutcomeEconomicsInput,
): void => {
  const buyerPriceCents = integer('buyerPriceCents', input.buyerPriceCents)
  const creditsCharged = integer('creditsCharged', input.creditsCharged)
  const satsCharged = integer('satsCharged', input.satsCharged)

  if (
    input.fundingMode === 'free_beta' &&
    (buyerPriceCents !== 0 ||
      creditsCharged !== 0 ||
      satsCharged !== 0 ||
      input.buyerPriceAsset !== 'none')
  ) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason: 'free_beta economics cannot record buyer charges.',
    })
  }

  if (
    input.fundingMode === 'credit_funded' &&
    (creditsCharged <= 0 || satsCharged !== 0)
  ) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason: 'credit_funded economics require credits and no sats charge.',
    })
  }

  if (
    input.fundingMode === 'sats_funded' &&
    (satsCharged <= 0 || creditsCharged !== 0)
  ) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason: 'sats_funded economics require sats and no credits charge.',
    })
  }

  if (
    input.fundingMode === 'internal_only' &&
    (buyerPriceCents !== 0 || creditsCharged !== 0 || satsCharged !== 0)
  ) {
    throw new OmniAcceptedOutcomeEconomicsValidationError({
      reason: 'internal_only economics cannot record buyer charges.',
    })
  }
}

const assertValidInput = (
  input: RecordOmniAcceptedOutcomeEconomicsInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef(
    'acceptedOutcomeContractId',
    input.acceptedOutcomeContractId,
  )
  assertSafeRef('publicCaveatRef', input.publicCaveatRef)
  assertSafeRef('internalCaveatRef', input.internalCaveatRef)
  assertSafeMetadata(input.metadata)
  assertFundingMode(input)

  integer('acceptedValueCents', input.acceptedValueCents)
  integer('artifactCostCents', input.artifactCostCents)
  integer('providerCostCents', input.providerCostCents)
  integer('retryCostCents', input.retryCostCents)
  integer('reviewCostCents', input.reviewCostCents)
  integer('reviewMinutes', input.reviewMinutes)
  integer('runnerCostCents', input.runnerCostCents)
}

const storageError = (
  operation: string,
  error: unknown,
): OmniAcceptedOutcomeEconomicsStorageError =>
  new OmniAcceptedOutcomeEconomicsStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniAcceptedOutcomeEconomicsStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const rowToRecord = (
  row: EconomicsRow,
): OmniAcceptedOutcomeEconomicsRecord => ({
  acceptedOutcomeContractId: row.accepted_outcome_contract_id,
  acceptedValueCents: row.accepted_value_cents,
  archivedAt: row.archived_at,
  artifactCostCents: row.artifact_cost_cents,
  buyerPriceAsset: row.buyer_price_asset,
  buyerPriceCents: row.buyer_price_cents,
  createdAt: row.created_at,
  creditsCharged: row.credits_charged,
  fundingMode: row.funding_mode,
  grossMarginCents: row.gross_margin_cents,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  internalCaveatRef: row.internal_caveat_ref,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  noSettlementImplication: row.no_settlement_implication === 1,
  providerCostCents: row.provider_cost_cents,
  publicCaveatRef: row.public_caveat_ref,
  retryCostCents: row.retry_cost_cents,
  reviewCostCents: row.review_cost_cents,
  reviewMinutes: row.review_minutes,
  runnerCostCents: row.runner_cost_cents,
  satsCharged: row.sats_charged,
  totalCostCents: row.total_cost_cents,
  updatedAt: row.updated_at,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  OmniAcceptedOutcomeEconomicsRecord | null,
  OmniAcceptedOutcomeEconomicsStorageError
> =>
  d1Effect('omniAcceptedOutcomeEconomics.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_accepted_outcome_economics
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<EconomicsRow>(),
  ).pipe(Effect.map(row => (row === null ? null : rowToRecord(row))))

/**
 * Read one stored accepted-outcome economics record by its accepted-outcome id.
 *
 * Mirrors readByIdempotencyKey but keys on the record id, which is the identity a
 * downstream view (gross-margin receipt, contributor accrual bundle) dereferences
 * by. Returns null for an unknown or archived id rather than failing, so callers
 * can distinguish "no such outcome" from a storage fault. Read-only.
 */
export const readOmniAcceptedOutcomeEconomicsById = (
  db: D1Database,
  id: string,
): Effect.Effect<
  OmniAcceptedOutcomeEconomicsRecord | null,
  OmniAcceptedOutcomeEconomicsStorageError
> =>
  d1Effect('omniAcceptedOutcomeEconomics.byId', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_accepted_outcome_economics
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(id)
      .first<EconomicsRow>(),
  ).pipe(Effect.map(row => (row === null ? null : rowToRecord(row))))

const readWorkroom = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<WorkroomRefRow | null, OmniAcceptedOutcomeEconomicsStorageError> =>
  d1Effect('omniAcceptedOutcomeEconomics.workroom', () =>
    db
      .prepare(
        `SELECT id, work_kind, archived_at
           FROM omni_workrooms
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workroomId)
      .first<WorkroomRefRow>(),
  )

const readContract = (
  db: D1Database,
  acceptedOutcomeContractId: string,
): Effect.Effect<ContractRefRow | null, OmniAcceptedOutcomeEconomicsStorageError> =>
  d1Effect('omniAcceptedOutcomeEconomics.contract', () =>
    db
      .prepare(
        `SELECT id, archived_at
           FROM omni_accepted_outcome_contracts
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(acceptedOutcomeContractId)
      .first<ContractRefRow>(),
  )

const totalCostCents = (
  input: RecordOmniAcceptedOutcomeEconomicsInput,
): number =>
  integer('runnerCostCents', input.runnerCostCents) +
  integer('providerCostCents', input.providerCostCents) +
  integer('retryCostCents', input.retryCostCents) +
  integer('reviewCostCents', input.reviewCostCents) +
  integer('artifactCostCents', input.artifactCostCents)

export const recordOmniAcceptedOutcomeEconomics = (
  db: D1Database,
  input: RecordOmniAcceptedOutcomeEconomicsInput,
  runtime: OmniAcceptedOutcomeEconomicsRuntime =
    systemOmniAcceptedOutcomeEconomicsRuntime,
): Effect.Effect<
  OmniAcceptedOutcomeEconomicsRecord,
  OmniAcceptedOutcomeEconomicsError
> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const workroom = yield* readWorkroom(db, input.workroomId)

    if (workroom === null) {
      return yield* new OmniAcceptedOutcomeEconomicsWorkroomNotFound({
        workroomId: input.workroomId,
      })
    }

    if (workroom.work_kind !== input.workKind) {
      return yield* new OmniAcceptedOutcomeEconomicsValidationError({
        reason: 'economics workKind must match the workroom workKind.',
      })
    }

    if (input.acceptedOutcomeContractId !== undefined) {
      const contract = yield* readContract(db, input.acceptedOutcomeContractId)

      if (contract === null) {
        return yield* new OmniAcceptedOutcomeEconomicsContractNotFound({
          acceptedOutcomeContractId: input.acceptedOutcomeContractId,
        })
      }
    }

    const now = runtime.nowIso()
    const totalCost = totalCostCents(input)
    const acceptedValueCents = integer(
      'acceptedValueCents',
      input.acceptedValueCents,
    )
    const record: OmniAcceptedOutcomeEconomicsRecord = {
      acceptedOutcomeContractId: input.acceptedOutcomeContractId ?? null,
      acceptedValueCents,
      archivedAt: null,
      artifactCostCents: integer(
        'artifactCostCents',
        input.artifactCostCents,
      ),
      buyerPriceAsset: input.buyerPriceAsset,
      buyerPriceCents: integer('buyerPriceCents', input.buyerPriceCents),
      createdAt: now,
      creditsCharged: integer('creditsCharged', input.creditsCharged),
      fundingMode: input.fundingMode,
      grossMarginCents: acceptedValueCents - totalCost,
      id: input.id ?? runtime.makeEconomicsId(),
      idempotencyKey: input.idempotencyKey,
      internalCaveatRef: input.internalCaveatRef ?? null,
      metadata: input.metadata ?? {},
      noSettlementImplication: true,
      providerCostCents: integer('providerCostCents', input.providerCostCents),
      publicCaveatRef: input.publicCaveatRef,
      retryCostCents: integer('retryCostCents', input.retryCostCents),
      reviewCostCents: integer('reviewCostCents', input.reviewCostCents),
      reviewMinutes: integer('reviewMinutes', input.reviewMinutes),
      runnerCostCents: integer('runnerCostCents', input.runnerCostCents),
      satsCharged: integer('satsCharged', input.satsCharged),
      totalCostCents: totalCost,
      updatedAt: now,
      workKind: input.workKind,
      workroomId: input.workroomId,
    }

    yield* d1Effect('omniAcceptedOutcomeEconomics.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_accepted_outcome_economics
             (id,
              idempotency_key,
              workroom_id,
              accepted_outcome_contract_id,
              work_kind,
              funding_mode,
              buyer_price_asset,
              buyer_price_cents,
              credits_charged,
              sats_charged,
              runner_cost_cents,
              provider_cost_cents,
              retry_cost_cents,
              review_minutes,
              review_cost_cents,
              artifact_cost_cents,
              total_cost_cents,
              accepted_value_cents,
              gross_margin_cents,
              public_caveat_ref,
              internal_caveat_ref,
              no_settlement_implication,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.workroomId,
          record.acceptedOutcomeContractId,
          record.workKind,
          record.fundingMode,
          record.buyerPriceAsset,
          record.buyerPriceCents,
          record.creditsCharged,
          record.satsCharged,
          record.runnerCostCents,
          record.providerCostCents,
          record.retryCostCents,
          record.reviewMinutes,
          record.reviewCostCents,
          record.artifactCostCents,
          record.totalCostCents,
          record.acceptedValueCents,
          record.grossMarginCents,
          record.publicCaveatRef,
          record.internalCaveatRef,
          record.noSettlementImplication ? 1 : 0,
          JSON.stringify(record.metadata),
          record.createdAt,
          record.updatedAt,
        )
        .run()
        .then(() => undefined),
    )

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const publicOmniAcceptedOutcomeEconomicsProjection = (
  economics: OmniAcceptedOutcomeEconomicsRecord,
) => ({
  fundingMode: economics.fundingMode,
  noSettlementImplication: economics.noSettlementImplication,
  publicCaveatRef: economics.publicCaveatRef,
  workKind: economics.workKind,
  workroomId: economics.workroomId,
})

export const operatorOmniAcceptedOutcomeEconomicsProjection = (
  economics: OmniAcceptedOutcomeEconomicsRecord,
) => economics
