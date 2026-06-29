import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentLedgerAmount,
  assertBuyerPaymentLedgerRecordSafe,
  decodeBuyerPaymentLedgerAmount,
} from './buyer-payment-ledger'
import {
  OpenAgentsHostedMdkCheckoutProjection,
  OpenAgentsHostedMdkCheckoutStatus,
  OpenAgentsHostedMdkEnvironment,
  openAgentsHostedMdkPayloadHasPrivateMaterial,
} from './hosted-mdk-client'
import { parseJsonStringArray } from './json-boundary'

export const OpenAgentsSiteMdkCheckoutIntentRecord = S.Struct({
  amount: BuyerPaymentLedgerAmount,
  archivedAt: S.NullOr(S.String),
  cancelReturnPath: S.String,
  catalogRef: S.String,
  challengeRef: S.String,
  checkoutIntentRef: S.String,
  checkoutLaunchPath: S.NullOr(S.String),
  checkoutRef: S.String,
  checkoutUrlRef: S.String,
  createdAt: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  hostedCheckoutProjectionJson: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  productId: S.String,
  providerRef: S.String,
  publicProjectionJson: S.String,
  sandbox: S.Boolean,
  siteId: S.String,
  siteVersionId: S.String,
  status: OpenAgentsHostedMdkCheckoutStatus,
  successReturnPath: S.String,
  updatedAt: S.String,
})
export type OpenAgentsSiteMdkCheckoutIntentRecord =
  typeof OpenAgentsSiteMdkCheckoutIntentRecord.Type

export type OpenAgentsSiteMdkCheckoutIntentBundle = Readonly<{
  buyerPaymentChallenge: BuyerPaymentChallengeRecord
  checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord
}>

export type OpenAgentsSiteMdkCheckoutIntentStore = Readonly<{
  createCheckoutIntentBundle: (
    bundle: OpenAgentsSiteMdkCheckoutIntentBundle,
  ) => Promise<void>
  readCheckoutIntentByCheckoutRef: (
    checkoutRef: string,
  ) => Promise<OpenAgentsSiteMdkCheckoutIntentRecord | undefined>
  readCheckoutIntentByIntentRef: (
    checkoutIntentRef: string,
  ) => Promise<OpenAgentsSiteMdkCheckoutIntentRecord | undefined>
  updateCheckoutIntentStatus: (input: {
    checkoutRef: string
    hostedCheckoutProjectionJson: string
    publicProjectionJson: string
    status: OpenAgentsHostedMdkCheckoutStatus
    updatedAt: string
  }) => Promise<OpenAgentsSiteMdkCheckoutIntentRecord | undefined>
}>

export class OpenAgentsSiteMdkCheckoutIntentUnsafe extends S.TaggedErrorClass<OpenAgentsSiteMdkCheckoutIntentUnsafe>()(
  'OpenAgentsSiteMdkCheckoutIntentUnsafe',
  {
    reason: S.String,
  },
) {}

export class OpenAgentsSiteMdkCheckoutIntentStorageError extends S.TaggedErrorClass<OpenAgentsSiteMdkCheckoutIntentStorageError>()(
  'OpenAgentsSiteMdkCheckoutIntentStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

type CheckoutIntentRow = Readonly<{
  amount_asset: 'bitcoin' | 'usd'
  amount_denomination: 'bitcoin_millisatoshi' | 'usd_cent'
  amount_minor_units: number
  archived_at: string | null
  cancel_return_path: string
  catalog_ref: string
  challenge_ref: string
  checkout_intent_ref: string
  checkout_launch_path: string | null
  checkout_ref: string
  checkout_url_ref: string
  created_at: string
  environment: 'production' | 'sandbox'
  hosted_checkout_projection_json: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  product_id: string
  provider_ref: string
  public_projection_json: string
  sandbox: number
  site_id: string
  site_version_id: string
  status: 'created' | 'expired' | 'payment_received' | 'pending_payment'
  success_return_path: string
  updated_at: string
}>

const jsonStringArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...new Set(values)])

const storageError = (
  operation: string,
  error: unknown,
): OpenAgentsSiteMdkCheckoutIntentStorageError =>
  new OpenAgentsSiteMdkCheckoutIntentStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const assertCheckoutIntentSafe = (
  record: OpenAgentsSiteMdkCheckoutIntentRecord,
): void => {
  S.decodeUnknownSync(OpenAgentsSiteMdkCheckoutIntentRecord)(record)
  decodeBuyerPaymentLedgerAmount(record.amount)

  if (openAgentsHostedMdkPayloadHasPrivateMaterial(record)) {
    throw new OpenAgentsSiteMdkCheckoutIntentUnsafe({
      reason:
        'Site MDK checkout intents must not contain MDK credentials, raw invoices, payment hashes, preimages, wallet state, customer private data, provider grants, payout claims, or secrets.',
    })
  }
}

const checkoutIntentFromRow = (
  row: CheckoutIntentRow,
): OpenAgentsSiteMdkCheckoutIntentRecord => ({
  amount: {
    amountMinorUnits: row.amount_minor_units,
    asset: row.amount_asset,
    denomination: row.amount_denomination,
  },
  archivedAt: row.archived_at,
  cancelReturnPath: row.cancel_return_path,
  catalogRef: row.catalog_ref,
  challengeRef: row.challenge_ref,
  checkoutIntentRef: row.checkout_intent_ref,
  checkoutLaunchPath: row.checkout_launch_path,
  checkoutRef: row.checkout_ref,
  checkoutUrlRef: row.checkout_url_ref,
  createdAt: row.created_at,
  environment: row.environment,
  hostedCheckoutProjectionJson: row.hosted_checkout_projection_json,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: [...parseJsonStringArray(row.metadata_refs_json)],
  productId: row.product_id,
  providerRef: row.provider_ref,
  publicProjectionJson: row.public_projection_json,
  sandbox: row.sandbox === 1,
  siteId: row.site_id,
  siteVersionId: row.site_version_id,
  status: row.status,
  successReturnPath: row.success_return_path,
  updatedAt: row.updated_at,
})

const bindChallengeInsert = (
  db: D1Database,
  record: BuyerPaymentChallengeRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT OR IGNORE INTO buyer_payment_challenges
       (id, challenge_ref, idempotency_key_hash, actor_ref, owner_user_id,
        product_id, surface, method, path, request_body_digest,
        price_asset, price_denomination, price_amount_minor_units,
        spend_cap_asset, spend_cap_denomination, spend_cap_amount_minor_units,
        status, expires_at, metadata_refs_json, public_projection_json,
        created_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      record.id,
      record.challengeRef,
      record.idempotencyKeyHash,
      record.actorRef,
      record.ownerUserId,
      record.productId,
      record.surface,
      record.method,
      record.path,
      record.requestBodyDigest,
      record.price.asset,
      record.price.denomination,
      record.price.amountMinorUnits,
      record.spendCap.asset,
      record.spendCap.denomination,
      record.spendCap.amountMinorUnits,
      record.status,
      record.expiresAt,
      jsonStringArray(record.metadataRefs),
      record.publicProjectionJson,
      record.createdAt,
      record.archivedAt,
    )

const bindCheckoutIntentInsert = (
  db: D1Database,
  record: OpenAgentsSiteMdkCheckoutIntentRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT OR IGNORE INTO site_mdk_checkout_intents
       (id, checkout_intent_ref, idempotency_key_hash, site_id,
        site_version_id, catalog_ref, product_id, challenge_ref, checkout_ref,
        checkout_url_ref, checkout_launch_path, provider_ref, status,
        environment, sandbox, amount_asset, amount_denomination,
        amount_minor_units, success_return_path, cancel_return_path,
        metadata_refs_json, hosted_checkout_projection_json,
        public_projection_json, created_at, updated_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      record.id,
      record.checkoutIntentRef,
      record.idempotencyKeyHash,
      record.siteId,
      record.siteVersionId,
      record.catalogRef,
      record.productId,
      record.challengeRef,
      record.checkoutRef,
      record.checkoutUrlRef,
      record.checkoutLaunchPath,
      record.providerRef,
      record.status,
      record.environment,
      record.sandbox ? 1 : 0,
      record.amount.asset,
      record.amount.denomination,
      record.amount.amountMinorUnits,
      record.successReturnPath,
      record.cancelReturnPath,
      jsonStringArray(record.metadataRefs),
      record.hostedCheckoutProjectionJson,
      record.publicProjectionJson,
      record.createdAt,
      record.updatedAt,
      record.archivedAt,
    )

export const makeD1SiteMdkCheckoutIntentStore = (
  db: D1Database,
): OpenAgentsSiteMdkCheckoutIntentStore => ({
  createCheckoutIntentBundle: async bundle => {
    assertBuyerPaymentLedgerRecordSafe(
      'Site MDK checkout challenge',
      bundle.buyerPaymentChallenge,
    )
    assertCheckoutIntentSafe(bundle.checkoutIntent)

    try {
      await db.batch([
        bindChallengeInsert(db, bundle.buyerPaymentChallenge),
        bindCheckoutIntentInsert(db, bundle.checkoutIntent),
      ])
    } catch (error) {
      throw storageError('siteMdkCheckoutIntent.createBundle', error)
    }
  },
  readCheckoutIntentByCheckoutRef: async checkoutRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM site_mdk_checkout_intents
            WHERE checkout_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(checkoutRef)
        .first<CheckoutIntentRow>()

      return row === null ? undefined : checkoutIntentFromRow(row)
    } catch (error) {
      throw storageError('siteMdkCheckoutIntent.readByCheckoutRef', error)
    }
  },
  readCheckoutIntentByIntentRef: async checkoutIntentRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM site_mdk_checkout_intents
            WHERE checkout_intent_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(checkoutIntentRef)
        .first<CheckoutIntentRow>()

      return row === null ? undefined : checkoutIntentFromRow(row)
    } catch (error) {
      throw storageError('siteMdkCheckoutIntent.readByIntentRef', error)
    }
  },
  updateCheckoutIntentStatus: async input => {
    try {
      await db
        .prepare(
          `UPDATE site_mdk_checkout_intents
              SET status = ?,
                  hosted_checkout_projection_json = ?,
                  public_projection_json = ?,
                  updated_at = ?
            WHERE checkout_ref = ?
              AND archived_at IS NULL`,
        )
        .bind(
          input.status,
          input.hostedCheckoutProjectionJson,
          input.publicProjectionJson,
          input.updatedAt,
          input.checkoutRef,
        )
        .run()

      const row = await db
        .prepare(
          `SELECT *
             FROM site_mdk_checkout_intents
            WHERE checkout_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(input.checkoutRef)
        .first<CheckoutIntentRow>()

      return row === null ? undefined : checkoutIntentFromRow(row)
    } catch (error) {
      throw storageError('siteMdkCheckoutIntent.updateStatus', error)
    }
  },
})

export const projectSiteMdkCheckoutIntentPublicJson = (
  input: Readonly<{
    checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord
    hostedCheckout: OpenAgentsHostedMdkCheckoutProjection
  }>,
): string =>
  JSON.stringify({
    catalogRef: input.checkoutIntent.catalogRef,
    checkoutIntentRef: input.checkoutIntent.checkoutIntentRef,
    checkoutRef: input.checkoutIntent.checkoutRef,
    checkoutStatus: input.checkoutIntent.status,
    checkoutUrlRef: input.checkoutIntent.checkoutUrlRef,
    hostedCheckout: input.hostedCheckout,
    productId: input.checkoutIntent.productId,
    siteId: input.checkoutIntent.siteId,
    siteVersionId: input.checkoutIntent.siteVersionId,
  })
