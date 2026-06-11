import { readFileSync } from 'node:fs'

import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeFakeOpenAgentsHostedMdkClient } from '../hosted-mdk-client'
import {
  ForumPaidActionError,
  type ForumPaidActionKindType,
  type ForumPaidActionPreviewInput,
  type ForumPaidActionRuntime,
  ForumPaidActionTarget as ForumPaidActionTargetSchema,
  ForumPublicProjection,
  ForumPublicProjectionUnsafe,
  ForumTipPreviewRateLimit,
  lookupForumDirectTip,
  lookupForumPaidActionReceipt,
  previewForumPaidAction,
  redeemForumPaidAction,
  reconcileForumDirectTipWebhook,
  submitForumDirectTip,
} from './index'

type ChallengeRow = Readonly<{
  action_kind: ForumPaidActionKindType
  actor_ref: string
  archived_at: string | null
  created_at: string
  expires_at: string
  id: string
  idempotency_key: string
  method: 'POST'
  path: string
  price_asset: 'sats'
  price_value: number
  public_projection_json: string
  recipient_actor_ref: string | null
  recipient_readiness_ref: string | null
  request_body_digest: string
  route_params_json: string
  spend_cap_asset: 'sats'
  spend_cap_value: number
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
  mdk_provider_ref: string | null
  mdk_environment: 'production' | 'sandbox' | null
  mdk_sandbox: number | null
  mdk_implementation_state:
    | 'fake_provider_contract'
    | 'live_provider_configured'
    | 'missing_configuration'
    | null
  mdk_checkout_ref: string | null
  mdk_checkout_url_ref: string | null
  mdk_checkout_launch_path: string | null
  mdk_invoice_ref: string | null
  mdk_payment_hash_ref: string | null
  l402_credential_ref: string | null
  l402_replay_nonce_ref: string | null
  l402_endpoint_ref: string | null
  l402_entitlement_scope_refs_json: string | null
  l402_www_authenticate: string | null
}>

type RedemptionRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_id: string
  created_at: string
  entitlement_ref: string
  id: string
  idempotency_key: string
  proof_ref: string
  receipt_id: string | null
  replayed: number
}>

type ReceiptRow = Readonly<{
  action_kind: ForumPaidActionKindType
  amount_asset: 'sats'
  amount_value: number
  archived_at: string | null
  created_at: string
  id: string
  public_projection_json: string
  receipt_ref: string
  recipient_actor_ref: string | null
  redacted_payment_ref: string
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

type MoneyActionRow = Readonly<{
  action_kind: ForumPaidActionKindType
  amount_asset: 'sats'
  amount_value: number
  earning_actor_ref: string | null
  id: string
  payment_event_id: string | null
  public_projection_json: string
  receipt_id: string | null
}>

type PaymentEventRow = Readonly<{
  amount_asset: 'sats'
  amount_value: number
  archived_at: string | null
  created_at: string
  external_ref: string
  id: string
  money_action_id: string
  provider_ref: string
  public_projection_json: string
  redacted_evidence_ref: string
}>

type DirectTipAttemptRow = Readonly<{
  amount_sats: number
  archived_at: string | null
  created_at: string
  external_ref: string
  id: string
  idempotency_key: string
  payer_actor_ref: string
  payment_event_id: string | null
  payment_event_status:
    | 'confirmed'
    | 'failed'
    | 'observed'
    | 'refunded'
    | 'replayed'
    | 'reversed'
  payment_mode: 'live' | 'sandbox' | 'signet' | 'unknown'
  provider_ref: string
  receipt_ref: string | null
  recipient_actor_ref: string
  redacted_evidence_ref: string
  status: 'settled' | 'failed' | 'recovery_pending'
  target_post_id: string
  target_post_permalink: string | null
  target_topic_id: string
  updated_at: string
}>

type DirectTipWebhookEventRow = Readonly<{
  amount_sats: number
  archived_at: string | null
  delivery_count: number
  direct_tip_attempt_id: string
  event_body_digest_ref: string
  external_ref: string
  first_seen_at: string
  id: string
  last_seen_at: string
  payment_event_status:
    | 'confirmed'
    | 'failed'
    | 'observed'
    | 'refunded'
    | 'replayed'
    | 'reversed'
  provider_event_ref: string
  provider_ref: string
  reconciliation_result: string
  reconciliation_status: 'settled' | 'failed' | 'recovery_pending'
  redacted_evidence_ref: string
  signature_binding_ref: string
}>

class ForumPaidActionStore {
  challenges: Array<ChallengeRow> = []
  directTipAttempts: Array<DirectTipAttemptRow> = []
  directTipWebhookEvents: Array<DirectTipWebhookEventRow> = []
  redemptions: Array<RedemptionRow> = []
  receipts: Array<ReceiptRow> = []
  moneyActions: Array<MoneyActionRow> = []
  paymentEvents: Array<PaymentEventRow> = []
}

class ForumPaidActionStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ForumPaidActionStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (
      this.query.includes('COUNT(*) AS count') &&
      this.query.includes('FROM forum_l402_challenges')
    ) {
      const actorRef = String(this.values[0])
      const actionKind = String(this.values[1])
      const sinceIso = String(this.values[2])
      const count = this.store.challenges.filter(
        item =>
          item.actor_ref === actorRef &&
          item.action_kind === actionKind &&
          item.created_at >= sinceIso &&
          item.archived_at === null,
      ).length

      return Promise.resolve({ count } as T)
    }

    if (
      this.query.includes('FROM forum_l402_challenges') &&
      this.query.includes('idempotency_key = ?')
    ) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.challenges.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_direct_tip_attempts') &&
      this.query.includes('idempotency_key = ?')
    ) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.directTipAttempts.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_direct_tip_attempts') &&
      this.query.includes('provider_ref = ?') &&
      this.query.includes('external_ref = ?')
    ) {
      const providerRef = String(this.values[0])
      const externalRef = String(this.values[1])
      const row =
        this.store.directTipAttempts.find(
          item =>
            item.provider_ref === providerRef &&
            item.external_ref === externalRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_direct_tip_attempts') &&
      this.query.includes('id = ?')
    ) {
      const attemptId = String(this.values[0])
      const row =
        this.store.directTipAttempts.find(
          item => item.id === attemptId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_direct_tip_webhook_events')) {
      const providerEventRef = String(this.values[0])
      const row =
        this.store.directTipWebhookEvents.find(
          item =>
            item.provider_event_ref === providerEventRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_money_actions')) {
      const paymentEventId = String(this.values[0])
      const row =
        this.store.moneyActions.find(
          item => item.payment_event_id === paymentEventId,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_l402_challenges') &&
      this.query.includes('id = ?')
    ) {
      const challengeId = String(this.values[0])
      const row =
        this.store.challenges.find(
          item => item.id === challengeId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_l402_redemptions')) {
      const challengeId = String(this.values[0])
      const row =
        this.store.redemptions.find(
          item =>
            item.challenge_id === challengeId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_receipts') &&
      this.query.includes('receipt_ref = ?')
    ) {
      const receiptRef = String(this.values[0])
      const receipt =
        this.store.receipts.find(
          item => item.receipt_ref === receiptRef && item.archived_at === null,
        ) ?? null
      const moneyAction =
        receipt === null
          ? null
          : (this.store.moneyActions.find(
              item => item.receipt_id === receipt.id,
            ) ?? null)
      const paymentEvent =
        moneyAction?.payment_event_id === null ||
        moneyAction?.payment_event_id === undefined
          ? null
          : (this.store.paymentEvents.find(
              item =>
                item.id === moneyAction.payment_event_id &&
                item.archived_at === null,
            ) ?? null)
      const row =
        receipt === null
          ? null
          : {
              ...receipt,
              payment_event_projection_json:
                paymentEvent?.public_projection_json ?? null,
              settlement_claim_projection_json: null,
            }

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_payment_events')) {
      const providerRef = String(this.values[0])
      const externalRef = String(this.values[1])
      const row =
        this.store.paymentEvents.find(
          item =>
            item.provider_ref === providerRef &&
            item.external_ref === externalRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_receipts') &&
      this.query.includes('id = ?')
    ) {
      const receiptId = String(this.values[0])
      const row =
        this.store.receipts.find(
          item => item.id === receiptId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO forum_l402_challenges')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.challenges.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.challenges.push({
          action_kind: this.values[3] as ForumPaidActionKindType,
          actor_ref: String(this.values[2]),
          archived_at: null,
          created_at: String(this.values[33]),
          expires_at: String(this.values[17]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          method: this.values[4] as 'POST',
          path: String(this.values[5]),
          price_asset: this.values[13] as 'sats',
          price_value: Number(this.values[14]),
          public_projection_json: String(this.values[32]),
          recipient_actor_ref:
            this.values[11] === null ? null : String(this.values[11]),
          recipient_readiness_ref:
            this.values[12] === null ? null : String(this.values[12]),
          request_body_digest: String(this.values[7]),
          route_params_json: String(this.values[6]),
          spend_cap_asset: this.values[15] as 'sats',
          spend_cap_value: Number(this.values[16]),
          target_forum_id:
            this.values[8] === null ? null : String(this.values[8]),
          target_post_id:
            this.values[10] === null ? null : String(this.values[10]),
          target_topic_id:
            this.values[9] === null ? null : String(this.values[9]),
          mdk_provider_ref:
            this.values[18] === null ? null : String(this.values[18]),
          mdk_environment:
            this.values[19] === null
              ? null
              : (this.values[19] as 'production' | 'sandbox'),
          mdk_sandbox:
            this.values[20] === null ? null : Number(this.values[20]),
          mdk_implementation_state:
            this.values[21] === null
              ? null
              : (this.values[21] as ChallengeRow['mdk_implementation_state']),
          mdk_checkout_ref:
            this.values[22] === null ? null : String(this.values[22]),
          mdk_checkout_url_ref:
            this.values[23] === null ? null : String(this.values[23]),
          mdk_checkout_launch_path:
            this.values[24] === null ? null : String(this.values[24]),
          mdk_invoice_ref:
            this.values[25] === null ? null : String(this.values[25]),
          mdk_payment_hash_ref:
            this.values[26] === null ? null : String(this.values[26]),
          l402_credential_ref:
            this.values[27] === null ? null : String(this.values[27]),
          l402_replay_nonce_ref:
            this.values[28] === null ? null : String(this.values[28]),
          l402_endpoint_ref:
            this.values[29] === null ? null : String(this.values[29]),
          l402_entitlement_scope_refs_json:
            this.values[30] === null ? null : String(this.values[30]),
          l402_www_authenticate:
            this.values[31] === null ? null : String(this.values[31]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT INTO forum_receipts') &&
      this.query.includes("VALUES (?, ?, 'post_reward', NULL")
    ) {
      this.store.receipts.push({
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: Number(this.values[4]),
        archived_at: null,
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        public_projection_json: String(this.values[7]),
        receipt_ref: String(this.values[1]),
        recipient_actor_ref: String(this.values[5]),
        redacted_payment_ref: String(this.values[6]),
        target_forum_id: null,
        target_post_id: String(this.values[3]),
        target_topic_id: String(this.values[2]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_receipts')) {
      this.store.receipts.push({
        action_kind: this.values[2] as ForumPaidActionKindType,
        amount_asset: this.values[6] as 'sats',
        amount_value: Number(this.values[7]),
        archived_at: null,
        created_at: String(this.values[11]),
        id: String(this.values[0]),
        public_projection_json: String(this.values[10]),
        receipt_ref: String(this.values[1]),
        recipient_actor_ref:
          this.values[8] === null ? null : String(this.values[8]),
        redacted_payment_ref: String(this.values[9]),
        target_forum_id:
          this.values[3] === null ? null : String(this.values[3]),
        target_post_id: this.values[5] === null ? null : String(this.values[5]),
        target_topic_id:
          this.values[4] === null ? null : String(this.values[4]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_direct_tip_attempts')) {
      this.store.directTipAttempts.push({
        amount_sats: Number(this.values[7]),
        archived_at: null,
        created_at: String(this.values[16]),
        external_ref: String(this.values[9]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        payer_actor_ref: String(this.values[2]),
        payment_event_id:
          this.values[15] === null ? null : String(this.values[15]),
        payment_event_status:
          this.values[12] as DirectTipAttemptRow['payment_event_status'],
        payment_mode: this.values[11] as DirectTipAttemptRow['payment_mode'],
        provider_ref: String(this.values[8]),
        receipt_ref: this.values[14] === null ? null : String(this.values[14]),
        recipient_actor_ref: String(this.values[3]),
        redacted_evidence_ref: String(this.values[10]),
        status: this.values[13] as DirectTipAttemptRow['status'],
        target_post_id: String(this.values[5]),
        target_post_permalink:
          this.values[6] === null ? null : String(this.values[6]),
        target_topic_id: String(this.values[4]),
        updated_at: String(this.values[17]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT OR IGNORE INTO forum_money_actions') &&
      this.query.includes("VALUES (?, ?, ?, 'post_reward', NULL")
    ) {
      this.store.moneyActions.push({
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: Number(this.values[5]),
        earning_actor_ref: String(this.values[8]),
        id: String(this.values[0]),
        payment_event_id:
          this.values[6] === null ? null : String(this.values[6]),
        public_projection_json: String(this.values[9]),
        receipt_id: this.values[7] === null ? null : String(this.values[7]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_money_actions')) {
      this.store.moneyActions.push({
        action_kind: this.values[3] as ForumPaidActionKindType,
        amount_asset: this.values[7] as 'sats',
        amount_value: Number(this.values[8]),
        earning_actor_ref:
          this.values[11] === null ? null : String(this.values[11]),
        id: String(this.values[0]),
        payment_event_id:
          this.values[9] === null ? null : String(this.values[9]),
        public_projection_json: String(this.values[12]),
        receipt_id: this.values[10] === null ? null : String(this.values[10]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT INTO forum_payment_events') &&
      this.query.includes("VALUES (?, ?, ?, ?, 'sats'")
    ) {
      this.store.paymentEvents.push({
        amount_asset: 'sats',
        amount_value: Number(this.values[4]),
        archived_at: null,
        created_at: String(this.values[7]),
        external_ref: String(this.values[3]),
        id: String(this.values[0]),
        money_action_id: String(this.values[1]),
        provider_ref: String(this.values[2]),
        public_projection_json: String(this.values[6]),
        redacted_evidence_ref: String(this.values[5]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_payment_events')) {
      this.store.paymentEvents.push({
        amount_asset: this.values[4] as 'sats',
        amount_value: Number(this.values[5]),
        archived_at: null,
        created_at: String(this.values[8]),
        external_ref: String(this.values[3]),
        id: String(this.values[0]),
        money_action_id: String(this.values[1]),
        provider_ref: String(this.values[2]),
        public_projection_json: String(this.values[7]),
        redacted_evidence_ref: String(this.values[6]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_l402_redemptions')) {
      this.store.redemptions.push({
        actor_ref: String(this.values[3]),
        archived_at: null,
        challenge_id: String(this.values[2]),
        created_at: String(this.values[8]),
        entitlement_ref: String(this.values[5]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        proof_ref: String(this.values[4]),
        receipt_id: this.values[6] === null ? null : String(this.values[6]),
        replayed: 0,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_direct_tip_webhook_events')) {
      this.store.directTipWebhookEvents.push({
        amount_sats: Number(this.values[5]),
        archived_at: null,
        delivery_count: 1,
        direct_tip_attempt_id: String(this.values[2]),
        event_body_digest_ref: String(this.values[8]),
        external_ref: String(this.values[4]),
        first_seen_at: String(this.values[12]),
        id: String(this.values[0]),
        last_seen_at: String(this.values[13]),
        payment_event_status:
          this.values[6] as DirectTipWebhookEventRow['payment_event_status'],
        provider_event_ref: String(this.values[1]),
        provider_ref: String(this.values[3]),
        reconciliation_result: String(this.values[11]),
        reconciliation_status:
          this.values[10] as DirectTipWebhookEventRow['reconciliation_status'],
        redacted_evidence_ref: String(this.values[7]),
        signature_binding_ref: String(this.values[9]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_direct_tip_webhook_events')) {
      const providerEventRef = String(this.values[1])
      const row = this.store.directTipWebhookEvents.find(
        item => item.provider_event_ref === providerEventRef,
      )

      if (row !== undefined) {
        const index = this.store.directTipWebhookEvents.indexOf(row)
        this.store.directTipWebhookEvents[index] = {
          ...row,
          delivery_count: row.delivery_count + 1,
          last_seen_at: String(this.values[0]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_money_actions')) {
      const receiptId = String(this.values[0])
      const moneyActionId = String(this.values[1])
      const row = this.store.moneyActions.find(item => item.id === moneyActionId)

      if (row !== undefined) {
        const index = this.store.moneyActions.indexOf(row)
        this.store.moneyActions[index] = { ...row, receipt_id: receiptId }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_payment_events')) {
      const paymentEventId = String(this.values[4])
      const row = this.store.paymentEvents.find(
        item => item.id === paymentEventId,
      )

      if (row !== undefined) {
        const index = this.store.paymentEvents.indexOf(row)
        this.store.paymentEvents[index] = {
          ...row,
          external_ref: String(this.values[1]),
          provider_ref: String(this.values[0]),
          public_projection_json: String(this.values[3]),
          redacted_evidence_ref: String(this.values[2]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_direct_tip_attempts')) {
      const attemptId = String(this.values[8])
      const row = this.store.directTipAttempts.find(
        item => item.id === attemptId,
      )

      if (row !== undefined) {
        const index = this.store.directTipAttempts.indexOf(row)
        this.store.directTipAttempts[index] = {
          ...row,
          external_ref: String(this.values[1]),
          payment_event_status:
            this.values[4] as DirectTipAttemptRow['payment_event_status'],
          payment_mode: this.values[3] as DirectTipAttemptRow['payment_mode'],
          provider_ref: String(this.values[0]),
          receipt_ref:
            row.receipt_ref ??
            (this.values[6] === null ? null : String(this.values[6])),
          redacted_evidence_ref: String(this.values[2]),
          status: this.values[5] as DirectTipAttemptRow['status'],
          updated_at: String(this.values[7]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const paidActionDb = (store: ForumPaidActionStore): D1Database => ({
  // redeemWriteBatch commits its receipt/money-action/payment-event
  // writes in ONE db.batch; the fixture applies the statements in order.
  batch: async (statements: ReadonlyArray<D1PreparedStatement>) => {
    const results: Array<D1Result<unknown>> = []
    for (const statement of statements) {
      results.push(await statement.run())
    }

    return results as never
  },
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ForumPaidActionStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime: ForumPaidActionRuntime = {
  challengeTtlMs: 10 * 60_000,
  makeChallengeId: () => '77777777-7777-4777-8777-777777777777',
  makeEntitlementRef: challengeId => `forum_entitlement:${challengeId}`,
  makeMoneyActionId: () => '99999999-9999-4999-8999-999999999999',
  makePaymentEventId: () => '66666666-6666-4666-8666-666666666666',
  makeReceiptId: () => '88888888-8888-4888-8888-888888888888',
  makeReceiptRef: challengeId => `receipt.forum.${challengeId}`,
  makeRedemptionId: () => 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  nowIso: () => '2026-06-05T20:00:00.000Z',
  nowMillis: () => Date.parse('2026-06-05T20:00:00.000Z'),
}

const expiredRuntime: ForumPaidActionRuntime = {
  ...runtime,
  nowIso: () => '2026-06-05T20:20:00.000Z',
  nowMillis: () => Date.parse('2026-06-05T20:20:00.000Z'),
}

const hostedMdkConfig = {
  configRef: 'config.forum.mdk.sandbox',
  credentialBindingRef: 'binding.forum.mdk.sandbox',
  environment: 'sandbox',
  providerRef: 'provider.forum.mdk.sandbox',
  webhookBindingRef: null,
} as const

const forumHostedMdkClient = (nowIso = runtime.nowIso()) =>
  makeFakeOpenAgentsHostedMdkClient(hostedMdkConfig, { nowIso })

const countingForumHostedMdkClient = () => {
  const state = { checkoutCount: 0 }
  const client = forumHostedMdkClient()

  return {
    checkoutCount: () => state.checkoutCount,
    client: {
      ...client,
      createCheckout: request => {
        state.checkoutCount += 1

        return client.createCheckout(request)
      },
      createCheckoutPromise: request => {
        state.checkoutCount += 1

        return client.createCheckoutPromise(request)
      },
    } satisfies typeof client,
  }
}

const publicProjection = S.decodeUnknownSync(ForumPublicProjection)({
  classificationCaveatRef: 'classification.public_forum_payment_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: ['payment_private.invoice_redacted'],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.payment_public.v1',
  safeArtifactRefs: ['artifact.public_forum_payment'],
  safeReceiptRefs: ['receipt.forum.reward.public_1'],
  trustTier: 'reviewed',
})

const paidActionTarget = S.decodeUnknownSync(ForumPaidActionTargetSchema)({
  forumId: '33333333-3333-4333-8333-333333333333',
  postId: '55555555-5555-4555-8555-555555555555',
  topicId: '44444444-4444-4444-8444-444444444444',
})

const previewInput = (
  overrides: Partial<ForumPaidActionPreviewInput> = {},
): ForumPaidActionPreviewInput => ({
  actionKind: 'post_boost',
  actorRef: 'actor.alice',
  hostedMdkClient: forumHostedMdkClient(),
  idempotencyKey: 'forum:boost:post:1:actor.alice',
  method: 'POST',
  nonPayableDenial: null,
  path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/boosts',
  price: { amount: 100, asset: 'sats' },
  publicProjection,
  recipientActorRef: 'actor.ben',
  recipientReadinessRef: 'readiness.public.forum_tip_recipient.ben',
  requestBodyDigest: 'sha256:reward-body',
  routeParams: {
    postId: '55555555-5555-4555-8555-555555555555',
  },
  spendCap: { amount: 100, asset: 'sats' },
  target: paidActionTarget,
  ...overrides,
})

const redeemInput = (
  challengeId: string,
  overrides: Partial<Parameters<typeof redeemForumPaidAction>[1]> = {},
) => ({
  actorRef: 'actor.alice',
  challengeId,
  idempotencyKey: 'forum:reward:post:1:actor.alice:redeem',
  l402ProofRef: 'mdk_payment_proof_forum_reward_1',
  method: 'POST' as const,
  path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/boosts',
  recipientActorRef: 'actor.ben',
  recipientReadinessRef: 'readiness.public.forum_tip_recipient.ben',
  requestBodyDigest: 'sha256:reward-body',
  routeParams: {
    postId: '55555555-5555-4555-8555-555555555555',
  },
  ...overrides,
})

const verifiedPaymentEvent = (
  overrides: Partial<
    NonNullable<Parameters<typeof redeemForumPaidAction>[1]['paymentEvent']>
  > = {},
) => ({
  externalRef: 'external.payment.redacted.forum_reward_1',
  paymentMode: 'signet' as const,
  providerRef: 'provider.mdk_l402.redacted',
  redactedEvidenceRef: 'evidence.payment.redacted.forum_reward_1',
  status: 'confirmed' as const,
  ...overrides,
})

const directTipInput = (
  overrides: Partial<Parameters<typeof submitForumDirectTip>[1]> = {},
): Parameters<typeof submitForumDirectTip>[1] => ({
  amount: { amount: 15, asset: 'sats' },
  idempotencyKey: 'forum:direct-tip:post:1:actor.alice',
  payerActorRef: 'actor.alice',
  paymentEvidence: {
    externalRef: 'external.payment.redacted.direct_tip_1',
    paymentMode: 'live',
    providerRef: 'provider.mdk_agent_wallet.redacted',
    redactedEvidenceRef: 'evidence.payment.redacted.direct_tip_1',
    status: 'confirmed',
  },
  post: {
    authorActorRef: 'actor.ben',
    postId: '55555555-5555-4555-8555-555555555555',
    publicProjection,
    targetPostPermalink:
      'https://openagents.com/forum/t/44444444-4444-4444-8444-444444444444#post-55555555-5555-4555-8555-555555555555',
    topicId: '44444444-4444-4444-8444-444444444444',
  },
  recipientReadiness: {
    directPayment: {
      bolt12Offer:
        'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
      kind: 'bolt12_offer',
      settlementAuthority: 'recipient_wallet_direct',
    },
    tippingAvailable: true,
  },
  ...overrides,
})

const storedChallenge = (
  overrides: Partial<ChallengeRow> = {},
): ChallengeRow => ({
  action_kind: 'post_boost',
  actor_ref: 'actor.alice',
  archived_at: null,
  created_at: '2026-06-05T20:00:00.000Z',
  expires_at: '2026-06-05T20:10:00.000Z',
  id: 'prior-tip-challenge',
  idempotency_key: 'prior-tip-challenge',
  l402_credential_ref: 'credential.forum_l402.prior',
  l402_endpoint_ref: 'endpoint.forum_paid_action.post_boost',
  l402_entitlement_scope_refs_json: '["entitlement.forum.post_boost.single"]',
  l402_replay_nonce_ref: 'replay_nonce.forum_l402.prior',
  l402_www_authenticate: 'L402 challenge_ref="challenge.forum_l402.prior"',
  mdk_checkout_launch_path: null,
  mdk_checkout_ref: 'mdk_checkout.prior',
  mdk_checkout_url_ref: null,
  mdk_environment: 'sandbox',
  mdk_implementation_state: 'fake_provider_contract',
  mdk_invoice_ref: 'mdk_invoice.redacted.prior',
  mdk_payment_hash_ref: 'payment_hash.redacted.prior',
  mdk_provider_ref: 'provider.mdk_l402.redacted',
  mdk_sandbox: 1,
  method: 'POST',
  path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/boosts',
  price_asset: 'sats',
  price_value: 100,
  public_projection_json: JSON.stringify(publicProjection),
  recipient_actor_ref: 'actor.ben',
  recipient_readiness_ref: 'readiness.public.forum_tip_recipient.ben',
  request_body_digest: 'sha256:reward-body',
  route_params_json: '{"postId":"55555555-5555-4555-8555-555555555555"}',
  spend_cap_asset: 'sats',
  spend_cap_value: 100,
  target_forum_id: '33333333-3333-4333-8333-333333333333',
  target_post_id: '55555555-5555-4555-8555-555555555555',
  target_topic_id: '44444444-4444-4444-8444-444444444444',
  ...overrides,
})

const createChallenge = async (store: ForumPaidActionStore) =>
  Effect.runPromise(
    previewForumPaidAction(paidActionDb(store), previewInput(), runtime),
  )

describe('Forum paid actions', () => {
  test('direct tip attempts migration defines idempotency, provider, receipt, and status indexes', () => {
    const migration = readFileSync(
      new URL(
        '../../migrations/0146_forum_direct_tip_attempts.sql',
        import.meta.url,
      ),
      'utf8',
    )

    expect(migration).toContain(
      'CREATE TABLE IF NOT EXISTS forum_direct_tip_attempts',
    )
    expect(migration).toContain('idempotency_key TEXT NOT NULL UNIQUE')
    expect(migration).toContain('UNIQUE (provider_ref, external_ref)')
    expect(migration).toContain(
      'receipt_ref TEXT REFERENCES forum_receipts(receipt_ref) ON DELETE SET NULL',
    )
    expect(migration).toContain('idx_forum_direct_tip_attempts_status')
    expect(migration).toContain('idx_forum_direct_tip_attempts_target')
  })

  test('direct tip webhook migration defines provider-event replay storage', () => {
    const migration = readFileSync(
      new URL(
        '../../migrations/0147_forum_direct_tip_webhook_events.sql',
        import.meta.url,
      ),
      'utf8',
    )

    expect(migration).toContain(
      'CREATE TABLE IF NOT EXISTS forum_direct_tip_webhook_events',
    )
    expect(migration).toContain('provider_event_ref TEXT NOT NULL UNIQUE')
    expect(migration).toContain(
      'direct_tip_attempt_id TEXT NOT NULL REFERENCES forum_direct_tip_attempts(id)',
    )
    expect(migration).toContain(
      'idx_forum_direct_tip_webhook_events_attempt',
    )
  })

  test('blocks ordinary post rewards from the hosted L402 challenge path', async () => {
    const store = new ForumPaidActionStore()
    const preview = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          idempotencyKey: 'forum:reward:post:1:actor.alice',
          path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/rewards',
        }),
        runtime,
      ),
    )

    expect(preview).toStrictEqual({
      challenge: null,
      entitlementRef: null,
      paymentRequired: false,
      writeDenial: {
        actorRef: 'actor.alice',
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
        requiredPermission: null,
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('records a confirmed BOLT 12 direct tip as settled recipient-wallet evidence', async () => {
    const store = new ForumPaidActionStore()
    const response = await Effect.runPromise(
      submitForumDirectTip(paidActionDb(store), directTipInput(), runtime),
    )

    expect(response.status).toBe('settled')
    expect(response.receipt?.receiptRef).toBe(
      'receipt.forum.direct_tip.77777777-7777-4777-8777-777777777777',
    )
    expect(response.receipt?.tipSettlement).toMatchObject({
      creatorReceivedSpendableValue: true,
      settlementAuthority: 'recipient_wallet_direct',
      state: 'settled',
    })
    expect(response.receipt?.paymentEvent).toMatchObject({
      actionKind: 'post_reward',
      amount: { amount: 15, asset: 'sats' },
      externalRef: 'external.payment.redacted.direct_tip_1',
      payerActorRef: 'actor.alice',
      recipientActorRef: 'actor.ben',
      settlementAuthority: 'recipient_wallet_direct',
      status: 'confirmed',
    })
    expect(store.challenges).toHaveLength(0)
    expect(store.receipts).toHaveLength(1)
    expect(store.paymentEvents).toHaveLength(1)
  })

  test('returns direct tips idempotently for the same payer, post, amount, and provider ref', async () => {
    const store = new ForumPaidActionStore()
    const first = await Effect.runPromise(
      submitForumDirectTip(paidActionDb(store), directTipInput(), runtime),
    )
    const second = await Effect.runPromise(
      submitForumDirectTip(paidActionDb(store), directTipInput(), runtime),
    )

    expect(second.idempotent).toBe(true)
    expect(second.attemptId).toBe(first.attemptId)
    expect(second.receipt?.receiptRef).toBe(first.receipt?.receiptRef)
    expect(store.receipts).toHaveLength(1)
    expect(store.paymentEvents).toHaveLength(1)
  })

  test('records failed direct-tip send evidence without a public receipt', async () => {
    const store = new ForumPaidActionStore()
    const response = await Effect.runPromise(
      submitForumDirectTip(
        paidActionDb(store),
        directTipInput({
          paymentEvidence: {
            externalRef: 'external.payment.redacted.direct_tip_failed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef: 'evidence.payment.redacted.direct_tip_failed',
            status: 'failed',
          },
        }),
        runtime,
      ),
    )

    expect(response.status).toBe('failed')
    expect(response.receipt).toBe(null)
    expect(store.directTipAttempts).toHaveLength(1)
    expect(store.moneyActions).toHaveLength(1)
    expect(store.moneyActions[0]?.receipt_id).toBe(null)
    expect(store.paymentEvents).toHaveLength(1)
  })

  test('looks up a direct-tip attempt status without requiring an L402 challenge', async () => {
    const store = new ForumPaidActionStore()
    const response = await Effect.runPromise(
      submitForumDirectTip(
        paidActionDb(store),
        directTipInput({
          paymentEvidence: {
            externalRef: 'external.payment.redacted.direct_tip_observed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.direct_tip_observed',
            status: 'observed',
          },
        }),
        runtime,
      ),
    )
    const lookup = await Effect.runPromise(
      lookupForumDirectTip(paidActionDb(store), response.attemptId),
    )

    expect(lookup).toMatchObject({
      attemptId: response.attemptId,
      receipt: null,
      status: 'recovery_pending',
    })
  })

  test('promotes a recovery-pending direct tip from a verified MDK webhook', async () => {
    const store = new ForumPaidActionStore()
    const pending = await Effect.runPromise(
      submitForumDirectTip(
        paidActionDb(store),
        directTipInput({
          paymentEvidence: {
            externalRef: 'external.payment.redacted.direct_tip_observed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.direct_tip_observed',
            status: 'observed',
          },
        }),
        runtime,
      ),
    )
    const reconciled = await Effect.runPromise(
      reconcileForumDirectTipWebhook(
        paidActionDb(store),
        {
          amount: { amount: 15, asset: 'sats' },
          attemptId: pending.attemptId,
          eventBodyDigestRef: 'sha256:forum_mdk_webhook.test.digest',
          paymentEvidence: {
            externalRef: 'external.payment.mdk_webhook.test.evt_1',
            paymentMode: 'live',
            providerRef: 'provider.mdk_webhook.test',
            redactedEvidenceRef: 'evidence.payment.mdk_webhook.test.evt_1',
            status: 'confirmed',
          },
          providerEventRef: 'provider_event.mdk.test.evt_1',
          signatureBindingRef: 'binding.forum.mdk.test',
        },
        runtime,
      ),
    )

    expect(reconciled).toMatchObject({
      attemptId: pending.attemptId,
      idempotent: false,
      receipt: {
        amount: { amount: 15, asset: 'sats' },
        paymentEvent: {
          externalRef: 'external.payment.mdk_webhook.test.evt_1',
          providerRef: 'provider.mdk_webhook.test',
          settlementAuthority: 'recipient_wallet_direct',
          status: 'confirmed',
        },
        tipSettlement: {
          creatorReceivedSpendableValue: true,
          settlementAuthority: 'recipient_wallet_direct',
          state: 'settled',
        },
      },
      status: 'settled',
    })
    expect(store.directTipAttempts[0]?.status).toBe('settled')
    expect(store.directTipWebhookEvents).toHaveLength(1)
    expect(store.receipts).toHaveLength(1)
    expect(store.moneyActions[0]?.receipt_id).toBe(
      '88888888-8888-4888-8888-888888888888',
    )
  })

  test('replays duplicate direct-tip webhooks without duplicating receipts', async () => {
    const store = new ForumPaidActionStore()
    const pending = await Effect.runPromise(
      submitForumDirectTip(
        paidActionDb(store),
        directTipInput({
          paymentEvidence: {
            externalRef: 'external.payment.redacted.direct_tip_observed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.direct_tip_observed',
            status: 'observed',
          },
        }),
        runtime,
      ),
    )
    const input = {
      amount: { amount: 15, asset: 'sats' } as const,
      attemptId: pending.attemptId,
      eventBodyDigestRef: 'sha256:forum_mdk_webhook.test.digest',
      paymentEvidence: {
        externalRef: 'external.payment.mdk_webhook.test.evt_2',
        paymentMode: 'live' as const,
        providerRef: 'provider.mdk_webhook.test',
        redactedEvidenceRef: 'evidence.payment.mdk_webhook.test.evt_2',
        status: 'confirmed' as const,
      },
      providerEventRef: 'provider_event.mdk.test.evt_2',
      signatureBindingRef: 'binding.forum.mdk.test',
    }
    const first = await Effect.runPromise(
      reconcileForumDirectTipWebhook(paidActionDb(store), input, runtime),
    )
    const second = await Effect.runPromise(
      reconcileForumDirectTipWebhook(paidActionDb(store), input, runtime),
    )

    expect(first.idempotent).toBe(false)
    expect(second.idempotent).toBe(true)
    expect(second.receipt?.receiptRef).toBe(first.receipt?.receiptRef)
    expect(store.receipts).toHaveLength(1)
    expect(store.directTipWebhookEvents[0]?.delivery_count).toBe(2)
  })

  test('returns the existing receipt when payer retry arrives after webhook settlement', async () => {
    const store = new ForumPaidActionStore()
    const originalInput = directTipInput({
      paymentEvidence: {
        externalRef: 'external.payment.redacted.direct_tip_observed',
        paymentMode: 'live',
        providerRef: 'provider.mdk_agent_wallet.redacted',
        redactedEvidenceRef: 'evidence.payment.redacted.direct_tip_observed',
        status: 'observed',
      },
    })
    const pending = await Effect.runPromise(
      submitForumDirectTip(paidActionDb(store), originalInput, runtime),
    )

    await Effect.runPromise(
      reconcileForumDirectTipWebhook(
        paidActionDb(store),
        {
          amount: { amount: 15, asset: 'sats' },
          attemptId: pending.attemptId,
          eventBodyDigestRef: 'sha256:forum_mdk_webhook.test.digest',
          paymentEvidence: {
            externalRef: 'external.payment.mdk_webhook.test.evt_retry',
            paymentMode: 'live',
            providerRef: 'provider.mdk_webhook.test',
            redactedEvidenceRef: 'evidence.payment.mdk_webhook.test.evt_retry',
            status: 'confirmed',
          },
          providerEventRef: 'provider_event.mdk.test.evt_retry',
          signatureBindingRef: 'binding.forum.mdk.test',
        },
        runtime,
      ),
    )

    const retry = await Effect.runPromise(
      submitForumDirectTip(paidActionDb(store), originalInput, runtime),
    )

    expect(retry).toMatchObject({
      attemptId: pending.attemptId,
      idempotent: true,
      receipt: {
        paymentEvent: {
          externalRef: 'external.payment.mdk_webhook.test.evt_retry',
          providerRef: 'provider.mdk_webhook.test',
          status: 'confirmed',
        },
      },
      status: 'settled',
    })
    expect(store.receipts).toHaveLength(1)
    expect(store.paymentEvents).toHaveLength(1)
  })

  test('rejects direct-tip webhook amount mismatch before settlement', async () => {
    const store = new ForumPaidActionStore()
    const pending = await Effect.runPromise(
      submitForumDirectTip(
        paidActionDb(store),
        directTipInput({
          paymentEvidence: {
            externalRef: 'external.payment.redacted.direct_tip_observed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.direct_tip_observed',
            status: 'observed',
          },
        }),
        runtime,
      ),
    )

    await expect(
      Effect.runPromise(
        reconcileForumDirectTipWebhook(
          paidActionDb(store),
          {
            amount: { amount: 16, asset: 'sats' },
            attemptId: pending.attemptId,
            eventBodyDigestRef: 'sha256:forum_mdk_webhook.test.digest',
            paymentEvidence: {
              externalRef: 'external.payment.mdk_webhook.test.evt_3',
              paymentMode: 'live',
              providerRef: 'provider.mdk_webhook.test',
              redactedEvidenceRef: 'evidence.payment.mdk_webhook.test.evt_3',
              status: 'confirmed',
            },
            providerEventRef: 'provider_event.mdk.test.evt_3',
            signatureBindingRef: 'binding.forum.mdk.test',
          },
          runtime,
        ),
      ),
    ).rejects.toMatchObject({ kind: 'binding_mismatch' })
    expect(store.receipts).toHaveLength(0)
  })

  test('blocks duplicate direct-tip provider events and self tips', async () => {
    const store = new ForumPaidActionStore()

    await Effect.runPromise(
      submitForumDirectTip(paidActionDb(store), directTipInput(), runtime),
    )
    await expect(
      Effect.runPromise(
        submitForumDirectTip(
          paidActionDb(store),
          directTipInput({
            idempotencyKey: 'forum:direct-tip:post:1:actor.alice:duplicate',
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({ kind: 'payment_event_replayed' })
    await expect(
      Effect.runPromise(
        submitForumDirectTip(
          paidActionDb(new ForumPaidActionStore()),
          directTipInput({
            payerActorRef: 'actor.ben',
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({ kind: 'self_tip_blocked' })
  })

  test('creates an unpaid L402 challenge for a configured non-tip paid action', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)

    expect(preview).toMatchObject({
      entitlementRef: null,
      paymentRequired: true,
      writeDenial: {
        denialKind: 'payment_required',
        payable: true,
      },
    })
    expect(preview.challenge).toMatchObject({
      actionKind: 'post_boost',
      actorRef: 'actor.alice',
      challengeId: '77777777-7777-4777-8777-777777777777',
      expiresAt: '2026-06-05T20:10:00.000Z',
      l402: {
        checkoutRef: expect.stringContaining(
          'product_forum_post_boost_single',
        ),
        credentialRef:
          'credential.forum_l402.77777777-7777-4777-8777-777777777777',
        endpointRef: 'endpoint.forum_paid_action.post_boost',
        environment: 'sandbox',
        implementationState: 'fake_provider_contract',
        invoiceRef: expect.stringContaining(
          'product_forum_post_boost_single',
        ),
        provider: 'mdk_hosted',
        providerMode: 'hosted_mdk',
        sandbox: true,
        settlementAuthority: 'buyer_payment_evidence_only',
      },
      path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/boosts',
      price: { amount: 100, asset: 'sats' },
      recipientActorRef: 'actor.ben',
      recipientReadinessRef: 'readiness.public.forum_tip_recipient.ben',
      requestBodyDigest: 'sha256:reward-body',
      routeParams: {
        postId: '55555555-5555-4555-8555-555555555555',
      },
      spendCap: { amount: 100, asset: 'sats' },
    })
    expect(preview.challenge?.l402?.wwwAuthenticate).toContain('L402')
    expect(JSON.stringify(preview)).not.toContain('lnbc')
    expect(JSON.stringify(preview)).not.toContain('preimage')
    expect(store.challenges).toHaveLength(1)
  })

  test('replays preview idempotently without issuing another hosted checkout', async () => {
    const store = new ForumPaidActionStore()
    const tracked = countingForumHostedMdkClient()
    const input = previewInput({ hostedMdkClient: tracked.client })
    const first = await Effect.runPromise(
      previewForumPaidAction(paidActionDb(store), input, runtime),
    )
    const replay = await Effect.runPromise(
      previewForumPaidAction(paidActionDb(store), input, runtime),
    )

    expect(tracked.checkoutCount()).toBe(1)
    expect(replay.challenge?.challengeId).toBe(first.challenge?.challengeId)
    expect(replay.challenge?.l402?.checkoutRef).toBe(
      first.challenge?.l402?.checkoutRef,
    )
    expect(store.challenges).toHaveLength(1)
  })

  test('redeems a paid retry into one receipt and earning money action', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''
    const redemption = await Effect.runPromise(
      redeemForumPaidAction(
        paidActionDb(store),
        redeemInput(challengeId),
        runtime,
      ),
    )
    const lookup = await Effect.runPromise(
      lookupForumPaidActionReceipt(paidActionDb(store), redemption.receiptRef),
    )

    expect(redemption).toStrictEqual({
      entitlementRef: 'forum_entitlement:77777777-7777-4777-8777-777777777777',
      originalReceiptRef: null,
      receiptRef: 'receipt.forum.77777777-7777-4777-8777-777777777777',
      replayed: false,
    })
    expect(store.receipts).toHaveLength(1)
    expect(store.moneyActions).toStrictEqual([
      {
        action_kind: 'post_boost',
        amount_asset: 'sats',
        amount_value: 100,
        earning_actor_ref: 'actor.ben',
        id: '99999999-9999-4999-8999-999999999999',
        payment_event_id: null,
        public_projection_json: JSON.stringify(publicProjection),
        receipt_id: '88888888-8888-4888-8888-888888888888',
      },
    ])
    expect(lookup).toMatchObject({
      actionKind: 'post_boost',
      amount: { amount: 100, asset: 'sats' },
      paymentEvent: null,
      receiptRef: redemption.receiptRef,
      recipientActorRef: 'actor.ben',
      targetPostPermalink:
        'https://openagents.com/forum/t/44444444-4444-4444-8444-444444444444#post-55555555-5555-4555-8555-555555555555',
      tipSettlement: {
        acceptedWorkPayoutEvidence: false,
        contentRewardEvidence: true,
        creatorReceivedSpendableValue: false,
        recipientSettlementEvidence: false,
        settlementAuthority: 'content_reward_evidence_only',
        state: 'evidence_only',
        treasuryAcceptedWorkClaimAllowed: false,
      },
    })
    expect(JSON.stringify(lookup)).not.toContain('lnbc')
    expect(JSON.stringify(lookup)).not.toContain('preimage')
  })

  test('links a verified payment event to the reward money action and receipt lookup', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''
    const redemption = await Effect.runPromise(
      redeemForumPaidAction(
        paidActionDb(store),
        redeemInput(challengeId, {
          paymentEvent: verifiedPaymentEvent(),
        }),
        runtime,
      ),
    )
    const lookup = await Effect.runPromise(
      lookupForumPaidActionReceipt(paidActionDb(store), redemption.receiptRef),
    )

    expect(store.paymentEvents).toStrictEqual([
      expect.objectContaining({
        amount_asset: 'sats',
        amount_value: 100,
        external_ref: 'external.payment.redacted.forum_reward_1',
        id: '66666666-6666-4666-8666-666666666666',
        money_action_id: '99999999-9999-4999-8999-999999999999',
        provider_ref: 'provider.mdk_l402.redacted',
        redacted_evidence_ref: 'evidence.payment.redacted.forum_reward_1',
      }),
    ])
    expect(store.moneyActions[0]?.payment_event_id).toBe(
      '66666666-6666-4666-8666-666666666666',
    )
    expect(lookup?.paymentEvent).toMatchObject({
      actionKind: 'post_boost',
      amount: { amount: 100, asset: 'sats' },
      challengeId,
      externalRef: 'external.payment.redacted.forum_reward_1',
      payerActorRef: 'actor.alice',
      paymentEventRef: '66666666-6666-4666-8666-666666666666',
      paymentMode: 'signet',
      providerRef: 'provider.mdk_l402.redacted',
      receiptRef: redemption.receiptRef,
      recipientActorRef: 'actor.ben',
      redactedEvidenceRef: 'evidence.payment.redacted.forum_reward_1',
      settlementAuthority: 'buyer_payment_evidence_only',
      status: 'confirmed',
    })
    expect(lookup?.targetPostPermalink).toBe(
      'https://openagents.com/forum/t/44444444-4444-4444-8444-444444444444#post-55555555-5555-4555-8555-555555555555',
    )
    expect(lookup?.tipSettlement).toMatchObject({
      acceptedWorkPayoutEvidence: false,
      contentRewardEvidence: true,
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'buyer_payment_evidence_only',
      state: 'paid',
      treasuryAcceptedWorkClaimAllowed: false,
    })
    expect(JSON.stringify(lookup)).not.toContain('lnbc')
    expect(JSON.stringify(lookup)).not.toContain('preimage')
    expect(JSON.stringify(lookup)).not.toContain('raw_payment_hash')
  })

  test('safely rejects duplicate provider payment events before receipt creation', async () => {
    const store = new ForumPaidActionStore()
    const paymentEvent = verifiedPaymentEvent()
    store.paymentEvents.push({
      amount_asset: 'sats',
      amount_value: 100,
      archived_at: null,
      created_at: '2026-06-05T19:55:00.000Z',
      external_ref: paymentEvent.externalRef,
      id: 'existing-payment-event',
      money_action_id: 'existing-money-action',
      provider_ref: paymentEvent.providerRef,
      public_projection_json: '{}',
      redacted_evidence_ref: paymentEvent.redactedEvidenceRef,
    })
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            paymentEvent,
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'payment_event_replayed',
    })
    expect(store.receipts).toHaveLength(0)
    expect(store.moneyActions).toHaveLength(0)
  })

  test('rejects failed payment verification before receipt creation', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            paymentEvent: verifiedPaymentEvent({ status: 'failed' }),
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'payment_verification_failed',
    })
    expect(store.receipts).toHaveLength(0)
    expect(store.moneyActions).toHaveLength(0)
    expect(store.paymentEvents).toHaveLength(0)
  })

  test('rejects unsafe payment event material before receipt creation', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            paymentEvent: verifiedPaymentEvent({
              externalRef:
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            }),
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'unsafe_payment_ref',
    })
    expect(store.receipts).toHaveLength(0)
    expect(store.moneyActions).toHaveLength(0)
    expect(store.paymentEvents).toHaveLength(0)
  })

  test('does not let post rewards use L402 even when both recipients are ready', async () => {
    const store = new ForumPaidActionStore()
    const aliceToBenTarget = S.decodeUnknownSync(ForumPaidActionTargetSchema)({
      forumId: '33333333-3333-4333-8333-333333333333',
      postId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
      topicId: '44444444-4444-4444-8444-444444444444',
    })
    const benToAliceTarget = S.decodeUnknownSync(ForumPaidActionTargetSchema)({
      forumId: '33333333-3333-4333-8333-333333333333',
      postId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      topicId: '44444444-4444-4444-8444-444444444444',
    })
    const aliceToBenPath =
      '/api/forum/posts/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb/rewards'
    const benToAlicePath =
      '/api/forum/posts/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa/rewards'
    const aliceToBenPreview = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          actorRef: 'agent:alice',
          idempotencyKey: 'simulation:forum:post_reward:alice_to_ben:preview',
          path: aliceToBenPath,
          recipientActorRef: 'agent:ben',
          recipientReadinessRef: 'readiness.public.forum_tip_recipient.ben',
          requestBodyDigest: 'sha256:simulation-alice-to-ben',
          routeParams: { postId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb' },
          target: aliceToBenTarget,
        }),
        runtime,
      ),
    )
    const benToAlicePreview = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          actorRef: 'agent:ben',
          idempotencyKey: 'simulation:forum:post_reward:ben_to_alice:preview',
          path: benToAlicePath,
          recipientActorRef: 'agent:alice',
          recipientReadinessRef: 'readiness.public.forum_tip_recipient.alice',
          requestBodyDigest: 'sha256:simulation-ben-to-alice',
          routeParams: { postId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa' },
          target: benToAliceTarget,
        }),
        runtime,
      ),
    )

    expect(aliceToBenPreview).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        actorRef: 'agent:alice',
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(benToAlicePreview).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        actorRef: 'agent:ben',
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(0)
    expect(store.receipts).toHaveLength(0)
    expect(store.moneyActions).toHaveLength(0)
    expect(JSON.stringify(store)).not.toContain('lnbc')
    expect(JSON.stringify(store)).not.toContain('preimage')
    expect(JSON.stringify(store)).not.toContain('mnemonic')
    expect(JSON.stringify(store)).not.toContain('private_key')
  })

  test('returns the original receipt on replay without creating another receipt', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''
    const first = await Effect.runPromise(
      redeemForumPaidAction(
        paidActionDb(store),
        redeemInput(challengeId),
        runtime,
      ),
    )
    const replay = await Effect.runPromise(
      redeemForumPaidAction(
        paidActionDb(store),
        redeemInput(challengeId, {
          idempotencyKey: 'forum:reward:post:1:actor.alice:redeem:again',
        }),
        runtime,
      ),
    )

    expect(replay).toStrictEqual({
      entitlementRef: first.entitlementRef,
      originalReceiptRef: first.receiptRef,
      receiptRef: first.receiptRef,
      replayed: true,
    })
    expect(store.receipts).toHaveLength(1)
    expect(store.redemptions).toHaveLength(1)
  })

  test('rejects expired challenge, route/body mismatch, and actor mismatch', async () => {
    const store = new ForumPaidActionStore()
    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId),
          expiredRuntime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'challenge_expired',
    })

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/down-signals',
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'binding_mismatch',
    })

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            requestBodyDigest: 'sha256:different-body',
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'binding_mismatch',
    })

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            actorRef: 'actor.mallory',
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'actor_mismatch',
    })
  })

  test('refuses over-cap payment previews', async () => {
    const store = new ForumPaidActionStore()

    await expect(
      Effect.runPromise(
        previewForumPaidAction(
          paidActionDb(store),
          previewInput({
            spendCap: { amount: 99, asset: 'sats' },
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'over_spend_cap',
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('refuses reward challenge issuance without recipient readiness', async () => {
    const store = new ForumPaidActionStore()

    await expect(
      Effect.runPromise(
        previewForumPaidAction(
          paidActionDb(store),
          previewInput({
            actionKind: 'post_reward',
            path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/rewards',
            recipientReadinessRef: null,
          }),
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumPaidActionError',
      kind: 'recipient_not_ready',
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('blocks self-tipping before issuing a payment challenge', async () => {
    const store = new ForumPaidActionStore()
    const preview = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          actorRef: 'actor.ben',
          path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/rewards',
          recipientActorRef: 'actor.ben',
          recipientReadinessRef: 'readiness.public.forum_tip_recipient.ben',
        }),
        runtime,
      ),
    )

    expect(preview).toStrictEqual({
      challenge: null,
      entitlementRef: null,
      paymentRequired: false,
      writeDenial: {
        actorRef: 'actor.ben',
        denialKind: 'safety_denied',
        denialRef: 'policy.public.forum_tip.self_tipping_blocked',
        payable: false,
        requiredPermission: null,
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('does not replay prior post reward L402 challenges', async () => {
    const store = new ForumPaidActionStore()
    store.challenges.push(
      ...Array.from({ length: ForumTipPreviewRateLimit.limit }, (_, index) =>
        storedChallenge({
          id: `prior-tip-challenge-${index}`,
          idempotency_key: `prior-tip-challenge-${index}`,
        }),
      ),
    )
    const blocked = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          idempotencyKey: 'forum:reward:post:rate-limited',
          path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/rewards',
        }),
        runtime,
      ),
    )
    const replay = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          idempotencyKey: 'prior-tip-challenge-0',
          path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/rewards',
        }),
        runtime,
      ),
    )

    expect(blocked).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(replay).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(ForumTipPreviewRateLimit.limit)
  })

  test('does not require hosted MDK configuration for the blocked post reward L402 path', async () => {
    const store = new ForumPaidActionStore()

    const preview = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          actionKind: 'post_reward',
          hostedMdkClient: undefined,
          path: '/api/forum/posts/55555555-5555-4555-8555-555555555555/rewards',
        }),
        runtime,
      ),
    )

    expect(preview).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('does not let payment buy non-payable permissions', async () => {
    const store = new ForumPaidActionStore()
    const preview = await Effect.runPromise(
      previewForumPaidAction(
        paidActionDb(store),
        previewInput({
          nonPayableDenial: {
            denialKind: 'privacy_denied',
            denialRef: 'forum.policy.private_room',
            requiredPermission: 'f_reply',
          },
        }),
        runtime,
      ),
    )

    expect(preview).toStrictEqual({
      challenge: null,
      entitlementRef: null,
      paymentRequired: false,
      writeDenial: {
        actorRef: 'actor.alice',
        denialKind: 'privacy_denied',
        denialRef: 'forum.policy.private_room',
        payable: false,
        requiredPermission: 'f_reply',
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('rejects raw payment material in proof refs and public projections', async () => {
    const store = new ForumPaidActionStore()

    await expect(
      Effect.runPromise(
        previewForumPaidAction(
          paidActionDb(store),
          previewInput({
            publicProjection: {
              ...publicProjection,
              safeArtifactRefs: ['provider_payload.raw'],
            },
          }),
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(ForumPublicProjectionUnsafe)

    const preview = await createChallenge(store)
    const challengeId = preview.challenge?.challengeId ?? ''

    await expect(
      Effect.runPromise(
        redeemForumPaidAction(
          paidActionDb(store),
          redeemInput(challengeId, {
            l402ProofRef: 'lnbc1rawinvoice',
          }),
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(ForumPaidActionError)
    expect(store.receipts).toHaveLength(0)
  })
})
