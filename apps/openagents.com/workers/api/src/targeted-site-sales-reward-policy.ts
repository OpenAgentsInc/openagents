import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const TargetedSiteSalesRewardOutcomeKind = S.Literals([
  'lead_proposed',
  'meeting_accepted',
  'customer_accepted',
  'reward_eligible',
  'payout_intent_created',
  'reward_held',
  'reward_disputed',
  'reward_reversed',
  'refund_recorded',
  'complaint_recorded',
  'settlement_caveat_recorded',
])
export type TargetedSiteSalesRewardOutcomeKind =
  typeof TargetedSiteSalesRewardOutcomeKind.Type

export const TargetedSiteSalesRewardPolicyState = S.Literals([
  'proposed',
  'accepted',
  'held',
  'disputed',
  'reversed',
  'eligible',
])
export type TargetedSiteSalesRewardPolicyState =
  typeof TargetedSiteSalesRewardPolicyState.Type

export const TargetedSiteSalesRewardAsset = S.Literals([
  'credits',
  'sats',
  'internal_payable',
])
export type TargetedSiteSalesRewardAsset =
  typeof TargetedSiteSalesRewardAsset.Type

export const TargetedSiteSalesRewardSettlementPosture = S.Literals([
  'no_settlement_claim',
  'eligible_not_settled',
  'payout_intent_not_settled',
  'blocked_or_reversed',
])
export type TargetedSiteSalesRewardSettlementPosture =
  typeof TargetedSiteSalesRewardSettlementPosture.Type

export const TargetedSiteSalesRewardPolicyEventRecord = S.Struct({
  acceptedWorkRef: S.NullOr(S.String),
  agentRef: S.String,
  archivedAt: S.NullOr(S.String),
  buyerPaymentRef: S.NullOr(S.String),
  campaignId: S.String,
  createdAt: S.String,
  disputeRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  occurredAt: S.String,
  outcomeKind: TargetedSiteSalesRewardOutcomeKind,
  payoutIntentRef: S.NullOr(S.String),
  policyState: TargetedSiteSalesRewardPolicyState,
  prospectId: S.NullOr(S.String),
  publicReceiptRef: S.String,
  referralAttributionRef: S.NullOr(S.String),
  relatedEventId: S.NullOr(S.String),
  rewardAmount: S.Number,
  rewardAsset: TargetedSiteSalesRewardAsset,
  settlementCaveatRef: S.NullOr(S.String),
})
export type TargetedSiteSalesRewardPolicyEventRecord =
  typeof TargetedSiteSalesRewardPolicyEventRecord.Type

export const TargetedSiteSalesRewardPolicyProjection = S.Struct({
  acceptedWorkRef: S.NullOr(S.String),
  agentRef: S.String,
  buyerPaymentRef: S.NullOr(S.String),
  campaignId: S.String,
  disputeRef: S.NullOr(S.String),
  eventCount: S.Number,
  latestEventAt: S.NullOr(S.String),
  latestOutcomeKind: S.NullOr(TargetedSiteSalesRewardOutcomeKind),
  payoutIntentRef: S.NullOr(S.String),
  policyState: TargetedSiteSalesRewardPolicyState,
  prospectId: S.NullOr(S.String),
  publicReceiptRef: S.NullOr(S.String),
  referralAttributionRef: S.NullOr(S.String),
  rewardAmount: S.Number,
  rewardAsset: TargetedSiteSalesRewardAsset,
  settlementCaveatRef: S.NullOr(S.String),
  settlementPosture: TargetedSiteSalesRewardSettlementPosture,
})
export type TargetedSiteSalesRewardPolicyProjection =
  typeof TargetedSiteSalesRewardPolicyProjection.Type

export type TargetedSiteSalesRewardPolicyRuntime = Readonly<{
  makeEventId: () => string
  nowIso: () => string
}>

export const systemTargetedSiteSalesRewardPolicyRuntime: TargetedSiteSalesRewardPolicyRuntime =
  {
    makeEventId: () => compactRandomId('targeted_site_sales_reward_policy'),
    nowIso: currentIsoTimestamp,
  }

export type RecordTargetedSiteSalesRewardPolicyEventInput = Readonly<{
  acceptedWorkRef?: string | undefined
  agentRef: string
  buyerPaymentRef?: string | undefined
  campaignId: string
  disputeRef?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  occurredAt?: string | undefined
  outcomeKind: TargetedSiteSalesRewardOutcomeKind
  payoutIntentRef?: string | undefined
  prospectId?: string | undefined
  publicReceiptRef?: string | undefined
  referralAttributionRef?: string | undefined
  relatedEventId?: string | undefined
  rewardAmount?: number | undefined
  rewardAsset?: TargetedSiteSalesRewardAsset | undefined
  settlementCaveatRef?: string | undefined
}>

type CampaignRow = Readonly<{ id: string }>
type ProspectRow = Readonly<{ id: string }>
type EventRow = Readonly<{
  accepted_work_ref: string | null
  agent_ref: string
  archived_at: string | null
  buyer_payment_ref: string | null
  campaign_id: string
  created_at: string
  dispute_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  occurred_at: string
  outcome_kind: TargetedSiteSalesRewardOutcomeKind
  payout_intent_ref: string | null
  policy_state: TargetedSiteSalesRewardPolicyState
  prospect_id: string | null
  public_receipt_ref: string
  referral_attribution_ref: string | null
  related_event_id: string | null
  reward_amount: number
  reward_asset: TargetedSiteSalesRewardAsset
  settlement_caveat_ref: string | null
}>

export class TargetedSiteSalesRewardPolicyValidationError extends S.TaggedErrorClass<TargetedSiteSalesRewardPolicyValidationError>()(
  'TargetedSiteSalesRewardPolicyValidationError',
  { reason: S.String },
) {}

export class TargetedSiteSalesRewardPolicyStorageError extends S.TaggedErrorClass<TargetedSiteSalesRewardPolicyStorageError>()(
  'TargetedSiteSalesRewardPolicyStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class TargetedSiteSalesRewardPolicyCampaignNotFound extends S.TaggedErrorClass<TargetedSiteSalesRewardPolicyCampaignNotFound>()(
  'TargetedSiteSalesRewardPolicyCampaignNotFound',
  { campaignId: S.String },
) {}

export class TargetedSiteSalesRewardPolicyProspectNotFound extends S.TaggedErrorClass<TargetedSiteSalesRewardPolicyProspectNotFound>()(
  'TargetedSiteSalesRewardPolicyProspectNotFound',
  { prospectId: S.String },
) {}

export class TargetedSiteSalesRewardPolicyRelatedEventNotFound extends S.TaggedErrorClass<TargetedSiteSalesRewardPolicyRelatedEventNotFound>()(
  'TargetedSiteSalesRewardPolicyRelatedEventNotFound',
  { relatedEventId: S.String },
) {}

export type TargetedSiteSalesRewardPolicyError =
  | TargetedSiteSalesRewardPolicyCampaignNotFound
  | TargetedSiteSalesRewardPolicyProspectNotFound
  | TargetedSiteSalesRewardPolicyRelatedEventNotFound
  | TargetedSiteSalesRewardPolicyStorageError
  | TargetedSiteSalesRewardPolicyValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: `${field} must be a public-safe ref without raw provider, email, payment, wallet, or private customer material.`,
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
    PROHIBITED_TEXT_PATTERN.test(json)
  ) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason:
        'metadata must not contain raw provider, email, payment, wallet, or private customer material.',
    })
  }
}

const assertNonNegativeInteger = (
  field: string,
  value: number | undefined,
): void => {
  if (value === undefined) {
    return
  }

  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: `${field} must be a non-negative integer.`,
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): TargetedSiteSalesRewardPolicyStorageError =>
  new TargetedSiteSalesRewardPolicyStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TargetedSiteSalesRewardPolicyStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const eventFromRow = (
  row: EventRow,
): TargetedSiteSalesRewardPolicyEventRecord => ({
  acceptedWorkRef: row.accepted_work_ref,
  agentRef: row.agent_ref,
  archivedAt: row.archived_at,
  buyerPaymentRef: row.buyer_payment_ref,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  disputeRef: row.dispute_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  occurredAt: row.occurred_at,
  outcomeKind: row.outcome_kind,
  payoutIntentRef: row.payout_intent_ref,
  policyState: row.policy_state,
  prospectId: row.prospect_id,
  publicReceiptRef: row.public_receipt_ref,
  referralAttributionRef: row.referral_attribution_ref,
  relatedEventId: row.related_event_id,
  rewardAmount: row.reward_amount,
  rewardAsset: row.reward_asset,
  settlementCaveatRef: row.settlement_caveat_ref,
})

const POLICY_STATE_BY_OUTCOME_KIND: Readonly<
  Record<
    TargetedSiteSalesRewardOutcomeKind,
    TargetedSiteSalesRewardPolicyState
  >
> = {
  complaint_recorded: 'disputed',
  customer_accepted: 'accepted',
  lead_proposed: 'proposed',
  meeting_accepted: 'accepted',
  payout_intent_created: 'eligible',
  refund_recorded: 'reversed',
  reward_disputed: 'disputed',
  reward_eligible: 'eligible',
  reward_held: 'held',
  reward_reversed: 'reversed',
  settlement_caveat_recorded: 'held',
}

const policyStateForOutcomeKind = (
  outcomeKind: TargetedSiteSalesRewardOutcomeKind,
): TargetedSiteSalesRewardPolicyState =>
  POLICY_STATE_BY_OUTCOME_KIND[outcomeKind]

const settlementPostureForEvent = (
  event: TargetedSiteSalesRewardPolicyEventRecord,
): TargetedSiteSalesRewardSettlementPosture => {
  if (event.policyState === 'reversed' || event.policyState === 'disputed') {
    return 'blocked_or_reversed'
  }

  if (event.payoutIntentRef !== null) {
    return 'payout_intent_not_settled'
  }

  if (event.policyState === 'eligible') {
    return 'eligible_not_settled'
  }

  return 'no_settlement_claim'
}

const publicReceiptRef = (
  outcomeKind: TargetedSiteSalesRewardOutcomeKind,
  idempotencyKey: string,
): string => `targeted_site_sales_reward:${outcomeKind}:${idempotencyKey}`

const assertTransitionPreconditions = (
  input: RecordTargetedSiteSalesRewardPolicyEventInput,
): void => {
  if (
    [
      'payout_intent_created',
      'reward_disputed',
      'reward_reversed',
      'refund_recorded',
      'complaint_recorded',
      'settlement_caveat_recorded',
    ].includes(input.outcomeKind) &&
    input.relatedEventId === undefined
  ) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: `${input.outcomeKind} must link to a related reward policy event.`,
    })
  }

  if (
    input.outcomeKind === 'reward_eligible' &&
    input.acceptedWorkRef === undefined
  ) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: 'reward_eligible must include acceptedWorkRef.',
    })
  }

  if (
    input.outcomeKind === 'payout_intent_created' &&
    input.payoutIntentRef === undefined
  ) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: 'payout_intent_created must include payoutIntentRef.',
    })
  }

  if (
    (input.outcomeKind === 'reward_disputed' ||
      input.outcomeKind === 'complaint_recorded') &&
    input.disputeRef === undefined
  ) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: `${input.outcomeKind} must include disputeRef.`,
    })
  }

  if (
    input.outcomeKind === 'settlement_caveat_recorded' &&
    input.settlementCaveatRef === undefined
  ) {
    throw new TargetedSiteSalesRewardPolicyValidationError({
      reason: 'settlement_caveat_recorded must include settlementCaveatRef.',
    })
  }
}

const assertValidInput = (
  input: RecordTargetedSiteSalesRewardPolicyEventInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', input.campaignId)
  assertSafeRef('agentRef', input.agentRef)
  assertSafeRef('prospectId', input.prospectId)
  assertSafeRef('buyerPaymentRef', input.buyerPaymentRef)
  assertSafeRef('referralAttributionRef', input.referralAttributionRef)
  assertSafeRef('acceptedWorkRef', input.acceptedWorkRef)
  assertSafeRef('payoutIntentRef', input.payoutIntentRef)
  assertSafeRef('settlementCaveatRef', input.settlementCaveatRef)
  assertSafeRef('disputeRef', input.disputeRef)
  assertSafeRef('publicReceiptRef', input.publicReceiptRef)
  assertSafeRef('relatedEventId', input.relatedEventId)
  assertSafeMetadata(input.metadata)
  assertNonNegativeInteger('rewardAmount', input.rewardAmount)
  assertTransitionPreconditions(input)
}

const readCampaign = (
  db: D1Database,
  campaignId: string,
): Effect.Effect<CampaignRow | null, TargetedSiteSalesRewardPolicyStorageError> =>
  d1Effect('targetedSiteSalesRewardPolicy.campaign', () =>
    db
      .prepare(
        `SELECT id
           FROM targeted_site_campaigns
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(campaignId)
      .first<CampaignRow>(),
  )

const readProspect = (
  db: D1Database,
  campaignId: string,
  prospectId: string,
): Effect.Effect<ProspectRow | null, TargetedSiteSalesRewardPolicyStorageError> =>
  d1Effect('targetedSiteSalesRewardPolicy.prospect', () =>
    db
      .prepare(
        `SELECT id
           FROM targeted_site_prospects
          WHERE id = ?
            AND campaign_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(prospectId, campaignId)
      .first<ProspectRow>(),
  )

const readEventById = (
  db: D1Database,
  eventId: string,
): Effect.Effect<
  TargetedSiteSalesRewardPolicyEventRecord | null,
  TargetedSiteSalesRewardPolicyStorageError
> =>
  d1Effect('targetedSiteSalesRewardPolicy.eventById', () =>
    db
      .prepare(
        `SELECT *
           FROM targeted_site_sales_reward_policy_events
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(eventId)
      .first<EventRow>(),
  ).pipe(Effect.map(row => (row === null ? null : eventFromRow(row))))

const readEventByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  TargetedSiteSalesRewardPolicyEventRecord | null,
  TargetedSiteSalesRewardPolicyStorageError
> =>
  d1Effect('targetedSiteSalesRewardPolicy.eventByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM targeted_site_sales_reward_policy_events
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<EventRow>(),
  ).pipe(Effect.map(row => (row === null ? null : eventFromRow(row))))

const listEventsForProjection = (
  db: D1Database,
  campaignId: string,
  agentRef: string,
  prospectId: string | undefined,
): Effect.Effect<
  ReadonlyArray<TargetedSiteSalesRewardPolicyEventRecord>,
  TargetedSiteSalesRewardPolicyStorageError
> =>
  d1Effect('targetedSiteSalesRewardPolicy.eventsForProjection', () => {
    const statement =
      prospectId === undefined
        ? db
            .prepare(
              `SELECT *
                 FROM targeted_site_sales_reward_policy_events
                WHERE campaign_id = ?
                  AND agent_ref = ?
                  AND archived_at IS NULL
                ORDER BY occurred_at ASC, created_at ASC`,
            )
            .bind(campaignId, agentRef)
        : db
            .prepare(
              `SELECT *
                 FROM targeted_site_sales_reward_policy_events
                WHERE campaign_id = ?
                  AND agent_ref = ?
                  AND prospect_id = ?
                  AND archived_at IS NULL
                ORDER BY occurred_at ASC, created_at ASC`,
            )
            .bind(campaignId, agentRef, prospectId)

    return statement.all<EventRow>()
  }).pipe(Effect.map(result => (result.results ?? []).map(eventFromRow)))

export const recordTargetedSiteSalesRewardPolicyEvent = (
  db: D1Database,
  input: RecordTargetedSiteSalesRewardPolicyEventInput,
  runtime: TargetedSiteSalesRewardPolicyRuntime =
    systemTargetedSiteSalesRewardPolicyRuntime,
): Effect.Effect<
  TargetedSiteSalesRewardPolicyEventRecord,
  TargetedSiteSalesRewardPolicyError
> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readEventByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existing !== null) {
      return existing
    }

    const campaign = yield* readCampaign(db, input.campaignId)

    if (campaign === null) {
      return yield* new TargetedSiteSalesRewardPolicyCampaignNotFound({
        campaignId: input.campaignId,
      })
    }

    if (input.prospectId !== undefined) {
      const prospect = yield* readProspect(
        db,
        input.campaignId,
        input.prospectId,
      )

      if (prospect === null) {
        return yield* new TargetedSiteSalesRewardPolicyProspectNotFound({
          prospectId: input.prospectId,
        })
      }
    }

    if (input.relatedEventId !== undefined) {
      const related = yield* readEventById(db, input.relatedEventId)

      if (related === null || related.campaignId !== input.campaignId) {
        return yield* new TargetedSiteSalesRewardPolicyRelatedEventNotFound({
          relatedEventId: input.relatedEventId,
        })
      }
    }

    const now = runtime.nowIso()
    const record: TargetedSiteSalesRewardPolicyEventRecord = {
      acceptedWorkRef: input.acceptedWorkRef ?? null,
      agentRef: input.agentRef,
      archivedAt: null,
      buyerPaymentRef: input.buyerPaymentRef ?? null,
      campaignId: input.campaignId,
      createdAt: now,
      disputeRef: input.disputeRef ?? null,
      id: input.id ?? runtime.makeEventId(),
      idempotencyKey: input.idempotencyKey,
      metadata: input.metadata ?? {},
      occurredAt: input.occurredAt ?? now,
      outcomeKind: input.outcomeKind,
      payoutIntentRef: input.payoutIntentRef ?? null,
      policyState: policyStateForOutcomeKind(input.outcomeKind),
      prospectId: input.prospectId ?? null,
      publicReceiptRef:
        input.publicReceiptRef ??
        publicReceiptRef(input.outcomeKind, input.idempotencyKey),
      referralAttributionRef: input.referralAttributionRef ?? null,
      relatedEventId: input.relatedEventId ?? null,
      rewardAmount: input.rewardAmount ?? 0,
      rewardAsset: input.rewardAsset ?? 'credits',
      settlementCaveatRef: input.settlementCaveatRef ?? null,
    }

    yield* d1Effect('targetedSiteSalesRewardPolicy.events.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO targeted_site_sales_reward_policy_events
             (id,
              idempotency_key,
              campaign_id,
              agent_ref,
              prospect_id,
              outcome_kind,
              policy_state,
              reward_asset,
              reward_amount,
              buyer_payment_ref,
              referral_attribution_ref,
              accepted_work_ref,
              payout_intent_ref,
              settlement_caveat_ref,
              dispute_ref,
              public_receipt_ref,
              related_event_id,
              metadata_json,
              occurred_at,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.campaignId,
          record.agentRef,
          record.prospectId,
          record.outcomeKind,
          record.policyState,
          record.rewardAsset,
          record.rewardAmount,
          record.buyerPaymentRef,
          record.referralAttributionRef,
          record.acceptedWorkRef,
          record.payoutIntentRef,
          record.settlementCaveatRef,
          record.disputeRef,
          record.publicReceiptRef,
          record.relatedEventId,
          JSON.stringify(record.metadata),
          record.occurredAt,
          record.createdAt,
        )
        .run()
        .then(() => undefined),
    )

    return (yield* readEventByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const projectTargetedSiteSalesRewardPolicy = (
  db: D1Database,
  input: Readonly<{
    agentRef: string
    campaignId: string
    prospectId?: string | undefined
  }>,
): Effect.Effect<
  TargetedSiteSalesRewardPolicyProjection,
  TargetedSiteSalesRewardPolicyError
> =>
  Effect.gen(function* () {
    assertSafeRef('campaignId', input.campaignId)
    assertSafeRef('agentRef', input.agentRef)
    assertSafeRef('prospectId', input.prospectId)

    const campaign = yield* readCampaign(db, input.campaignId)

    if (campaign === null) {
      return yield* new TargetedSiteSalesRewardPolicyCampaignNotFound({
        campaignId: input.campaignId,
      })
    }

    const events = yield* listEventsForProjection(
      db,
      input.campaignId,
      input.agentRef,
      input.prospectId,
    )
    const latest = events.at(-1)

    if (latest === undefined) {
      return {
        acceptedWorkRef: null,
        agentRef: input.agentRef,
        buyerPaymentRef: null,
        campaignId: input.campaignId,
        disputeRef: null,
        eventCount: 0,
        latestEventAt: null,
        latestOutcomeKind: null,
        payoutIntentRef: null,
        policyState: 'proposed',
        prospectId: input.prospectId ?? null,
        publicReceiptRef: null,
        referralAttributionRef: null,
        rewardAmount: 0,
        rewardAsset: 'credits',
        settlementCaveatRef: null,
        settlementPosture: 'no_settlement_claim',
      }
    }

    return {
      acceptedWorkRef: latest.acceptedWorkRef,
      agentRef: latest.agentRef,
      buyerPaymentRef: latest.buyerPaymentRef,
      campaignId: latest.campaignId,
      disputeRef: latest.disputeRef,
      eventCount: events.length,
      latestEventAt: latest.occurredAt,
      latestOutcomeKind: latest.outcomeKind,
      payoutIntentRef: latest.payoutIntentRef,
      policyState: latest.policyState,
      prospectId: latest.prospectId,
      publicReceiptRef: latest.publicReceiptRef,
      referralAttributionRef: latest.referralAttributionRef,
      rewardAmount: latest.rewardAmount,
      rewardAsset: latest.rewardAsset,
      settlementCaveatRef: latest.settlementCaveatRef,
      settlementPosture: settlementPostureForEvent(latest),
    }
  })

export const publicTargetedSiteSalesRewardPolicyProjection = (
  projection: TargetedSiteSalesRewardPolicyProjection,
) => ({
  agentRef: projection.agentRef,
  campaignId: projection.campaignId,
  eventCount: projection.eventCount,
  latestEventAt: projection.latestEventAt,
  latestOutcomeKind: projection.latestOutcomeKind,
  policyState: projection.policyState,
  prospectId: projection.prospectId,
  publicReceiptRef: projection.publicReceiptRef,
  rewardAmount: projection.rewardAmount,
  rewardAsset: projection.rewardAsset,
  settlementPosture: projection.settlementPosture,
})
