import { Schema as S } from 'effect'

import {
  AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS,
  AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS,
} from './agent-claim-reward-policy'
import { parseJsonStringArray } from './json-boundary'

export const AGENT_CLAIM_X_REWARD_CAMPAIGN_REF =
  'campaign.agent_claim.x_tweet_1000_sats.v1'
export const AGENT_CLAIM_X_REWARD_AMOUNT_SATS = 1000

export const AgentClaimRewardState = S.Literals([
  'pending',
  'verified',
  'approved',
  'payout_intent_created',
  'dispatched',
  'settled',
  'rejected',
  'reversed',
  'expired',
])
export type AgentClaimRewardState = typeof AgentClaimRewardState.Type

export const AgentClaimRewardDestinationKind = S.Literals([
  'lightning_address',
  'lnurl',
  'bolt12',
  'bolt11_invoice',
  'unknown',
])
export type AgentClaimRewardDestinationKind =
  typeof AgentClaimRewardDestinationKind.Type

export type AgentClaimRewardLedgerRecord = Readonly<{
  id: string
  idempotencyKey: string
  campaignRef: string
  agentClaimRef: string
  ownerRef: string
  xAccountRef: string
  tweetRef: string
  state: AgentClaimRewardState
  amountSats: number
  destinationKind: AgentClaimRewardDestinationKind
  redactedDestinationRef: string | null
  payoutIntentRef: string | null
  dispatchAttemptRef: string | null
  settlementRef: string | null
  rejectionReason: string | null
  policyRefsJson: string
  caveatRefsJson: string
  createdAt: string
  updatedAt: string
}>

export type AgentClaimRewardLedgerRow = Readonly<{
  id: string
  idempotency_key: string
  campaign_ref: string
  agent_claim_ref: string
  owner_ref: string
  x_account_ref: string
  tweet_ref: string
  state: AgentClaimRewardState
  amount_sats: number
  destination_kind: AgentClaimRewardDestinationKind
  redacted_destination_ref: string | null
  payout_intent_ref: string | null
  dispatch_attempt_ref: string | null
  settlement_ref: string | null
  rejection_reason: string | null
  policy_refs_json: string
  caveat_refs_json: string
  created_at: string
  updated_at: string
}>

export type CreateAgentClaimRewardInput = Readonly<{
  agentClaimRef: string
  id: string
  idempotencyKey: string
  now: string
  ownerRef: string
  redactedDestinationRef?: string | null
  destinationKind?: AgentClaimRewardDestinationKind
  tweetRef: string
  xAccountRef: string
}>

export type AgentClaimRewardPublicReceipt = Readonly<{
  receiptRef: string
  campaignRef: string
  agentClaimRef: string
  ownerRef: string
  xAccountRef: string
  tweetRef: string
  state: AgentClaimRewardState
  amountSats: 1000
  destinationKind: AgentClaimRewardDestinationKind
  redactedDestinationRef: string | null
  payoutIntentRef: string | null
  dispatchAttemptRef: string | null
  settlementRef: string | null
  caveatRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
}>

export type AgentClaimRewardLedgerStore = Readonly<{
  createPendingReward: (
    input: CreateAgentClaimRewardInput,
  ) => Promise<AgentClaimRewardLedgerRecord>
  readRewardByIdempotencyKey: (
    idempotencyKey: string,
  ) => Promise<AgentClaimRewardLedgerRecord | undefined>
  readRewardByReceiptRef: (
    receiptRef: string,
  ) => Promise<AgentClaimRewardLedgerRecord | undefined>
}>

export const rowToAgentClaimRewardLedgerRecord = (
  row: AgentClaimRewardLedgerRow,
): AgentClaimRewardLedgerRecord => ({
  agentClaimRef: row.agent_claim_ref,
  amountSats: row.amount_sats,
  campaignRef: row.campaign_ref,
  caveatRefsJson: row.caveat_refs_json,
  createdAt: row.created_at,
  destinationKind: row.destination_kind,
  dispatchAttemptRef: row.dispatch_attempt_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  ownerRef: row.owner_ref,
  payoutIntentRef: row.payout_intent_ref,
  policyRefsJson: row.policy_refs_json,
  redactedDestinationRef: row.redacted_destination_ref,
  rejectionReason: row.rejection_reason,
  settlementRef: row.settlement_ref,
  state: row.state,
  tweetRef: row.tweet_ref,
  updatedAt: row.updated_at,
  xAccountRef: row.x_account_ref,
})

export const createPendingAgentClaimRewardRecord = (
  input: CreateAgentClaimRewardInput,
): AgentClaimRewardLedgerRecord => ({
  agentClaimRef: input.agentClaimRef,
  amountSats: AGENT_CLAIM_X_REWARD_AMOUNT_SATS,
  campaignRef: AGENT_CLAIM_X_REWARD_CAMPAIGN_REF,
  caveatRefsJson: JSON.stringify(AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS),
  createdAt: input.now,
  destinationKind: input.destinationKind ?? 'unknown',
  dispatchAttemptRef: null,
  id: input.id,
  idempotencyKey: input.idempotencyKey,
  ownerRef: input.ownerRef,
  payoutIntentRef: null,
  policyRefsJson: JSON.stringify(AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS),
  redactedDestinationRef: input.redactedDestinationRef ?? null,
  rejectionReason: null,
  settlementRef: null,
  state: 'verified',
  tweetRef: input.tweetRef,
  updatedAt: input.now,
  xAccountRef: input.xAccountRef,
})

export const projectAgentClaimRewardPublicReceipt = (
  record: AgentClaimRewardLedgerRecord,
): AgentClaimRewardPublicReceipt => ({
  agentClaimRef: record.agentClaimRef,
  amountSats: AGENT_CLAIM_X_REWARD_AMOUNT_SATS,
  campaignRef: record.campaignRef,
  caveatRefs: parseJsonStringArray(record.caveatRefsJson),
  destinationKind: record.destinationKind,
  dispatchAttemptRef: record.dispatchAttemptRef,
  ownerRef: record.ownerRef,
  payoutIntentRef: record.payoutIntentRef,
  receiptRef: record.id,
  redactedDestinationRef: record.redactedDestinationRef,
  policyRefs: parseJsonStringArray(record.policyRefsJson),
  settlementRef: record.settlementRef,
  state:
    record.state === 'settled' && record.settlementRef === null
      ? 'dispatched'
      : record.state,
  tweetRef: record.tweetRef,
  xAccountRef: record.xAccountRef,
})

export const agentClaimRewardReceiptHasPrivateMaterial = (
  receipt: AgentClaimRewardPublicReceipt,
): boolean => {
  const serialized = JSON.stringify(receipt).toLowerCase()
  const forbiddenFragments = [
    'oauth',
    'bearer ',
    'mnemonic',
    'preimage',
    'payment_hash',
    'lnbc',
    'bolt11:',
    'invoice:',
    'wallet_state',
    'fingerprint',
    '@example.com',
  ]

  return forbiddenFragments.some(fragment => serialized.includes(fragment))
}

export const makeD1AgentClaimRewardLedgerStore = (
  db: D1Database,
): AgentClaimRewardLedgerStore => {
  const readRewardByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<AgentClaimRewardLedgerRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
         FROM agent_claim_reward_ledger
         WHERE idempotency_key = ?`,
      )
      .bind(idempotencyKey)
      .first<AgentClaimRewardLedgerRow>()

    return row === null ? undefined : rowToAgentClaimRewardLedgerRecord(row)
  }

  const readRewardByReceiptRef = async (
    receiptRef: string,
  ): Promise<AgentClaimRewardLedgerRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
         FROM agent_claim_reward_ledger
         WHERE id = ?`,
      )
      .bind(receiptRef)
      .first<AgentClaimRewardLedgerRow>()

    return row === null ? undefined : rowToAgentClaimRewardLedgerRecord(row)
  }

  return {
    createPendingReward: async input => {
      const existing = await readRewardByIdempotencyKey(input.idempotencyKey)

      if (existing !== undefined) {
        return existing
      }

      const record = createPendingAgentClaimRewardRecord(input)

      await db
        .prepare(
          `INSERT INTO agent_claim_reward_ledger
          (id, idempotency_key, campaign_ref, agent_claim_ref, owner_ref,
           x_account_ref, tweet_ref, state, amount_sats, destination_kind,
           redacted_destination_ref, payout_intent_ref, dispatch_attempt_ref,
           settlement_ref, rejection_reason, policy_refs_json, caveat_refs_json,
           created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.campaignRef,
          record.agentClaimRef,
          record.ownerRef,
          record.xAccountRef,
          record.tweetRef,
          record.state,
          record.amountSats,
          record.destinationKind,
          record.redactedDestinationRef,
          record.payoutIntentRef,
          record.dispatchAttemptRef,
          record.settlementRef,
          record.rejectionReason,
          record.policyRefsJson,
          record.caveatRefsJson,
          record.createdAt,
          record.updatedAt,
        )
        .run()

      return (await readRewardByReceiptRef(record.id)) ?? record
    },
    readRewardByIdempotencyKey,
    readRewardByReceiptRef,
  }
}
