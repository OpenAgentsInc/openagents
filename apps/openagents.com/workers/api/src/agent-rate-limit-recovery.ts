import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Option, Schema as S } from 'effect'

import {
  type ProgrammaticAgentSession,
  sha256Hex,
} from './agent-registration'
import { parseJsonRecord } from './json-boundary'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  randomUuid,
} from './runtime-primitives'

export const AgentRateLimitRecoveryRouteKey = S.Literal(
  'public_agent_proposals',
)
export type AgentRateLimitRecoveryRouteKey =
  typeof AgentRateLimitRecoveryRouteKey.Type

export const AgentRateLimitMoneyAmount = S.Struct({
  amount: S.Number,
  asset: S.Literals(['bitcoin', 'credits', 'usd']),
  denomination: S.Literals(['sats', 'credits', 'cents']),
})
export type AgentRateLimitMoneyAmount = typeof AgentRateLimitMoneyAmount.Type

export const AgentRateLimitRecoveryGrant = S.Struct({
  expiresAt: S.NullOr(S.String),
  grantId: S.optionalKey(S.String),
  ownerUserId: S.String,
  routeKeys: S.Array(AgentRateLimitRecoveryRouteKey),
  spendCap: AgentRateLimitMoneyAmount,
  status: S.Literals(['active', 'revoked']),
})
export type AgentRateLimitRecoveryGrant =
  typeof AgentRateLimitRecoveryGrant.Type

export type AgentRateLimitChallengeRecord = Readonly<{
  actorRef: string
  archivedAt: string | null
  clientFingerprintHash: string
  createdAt: string
  entitlementKind: string
  expiresAt: string
  id: string
  idempotencyKeyHash: string
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  ownerUserId: string
  path: string
  price: AgentRateLimitMoneyAmount
  publicProjectionJson: string
  requestBodyDigest: string
  routeKey: AgentRateLimitRecoveryRouteKey
  spendCap: AgentRateLimitMoneyAmount
  submissionIdempotencyKeyHash: string
}>

export type AgentRateLimitReceiptRecord = Readonly<{
  actorRef: string
  amount: AgentRateLimitMoneyAmount
  archivedAt: string | null
  challengeId: string
  createdAt: string
  entitlementRef: string
  id: string
  ownerUserId: string
  publicProjectionJson: string
  receiptRef: string
  redactedPaymentRef: string
  routeKey: AgentRateLimitRecoveryRouteKey
}>

export type AgentRateLimitEntitlementRecord = Readonly<{
  actorRef: string
  archivedAt: string | null
  challengeId: string
  clientFingerprintHash: string
  consumedAt: string | null
  createdAt: string
  entitlementKind: string
  entitlementRef: string
  expiresAt: string
  id: string
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  ownerUserId: string
  path: string
  receiptRef: string
  requestBodyDigest: string
  routeKey: AgentRateLimitRecoveryRouteKey
  status: 'active' | 'consumed' | 'expired'
  submissionIdempotencyKeyHash: string
}>

export type AgentRateLimitRedemptionRecord = Readonly<{
  actorRef: string
  archivedAt: string | null
  challengeId: string
  createdAt: string
  entitlementRef: string
  id: string
  idempotencyKeyHash: string
  proofRef: string
  receiptRef: string
  replayed: number
}>

export type AgentRateLimitRecoveryStore = Readonly<{
  consumeEntitlement: (input: {
    actorRef: string
    clientFingerprintHash: string
    entitlementRef: string
    method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
    nowIso: string
    path: string
    requestBodyDigest: string
    routeKey: AgentRateLimitRecoveryRouteKey
    submissionIdempotencyKeyHash: string
  }) => Promise<AgentRateLimitEntitlementRecord | undefined>
  createChallenge: (record: AgentRateLimitChallengeRecord) => Promise<void>
  createRedemptionBundle: (input: {
    entitlement: AgentRateLimitEntitlementRecord
    receipt: AgentRateLimitReceiptRecord
    redemption: AgentRateLimitRedemptionRecord
  }) => Promise<void>
  readChallengeById: (
    challengeId: string,
  ) => Promise<AgentRateLimitChallengeRecord | undefined>
  readChallengeByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<AgentRateLimitChallengeRecord | undefined>
  readEntitlementByRef: (
    entitlementRef: string,
  ) => Promise<AgentRateLimitEntitlementRecord | undefined>
  readReceiptByRef: (
    receiptRef: string,
  ) => Promise<AgentRateLimitReceiptRecord | undefined>
  readRedemptionByChallengeId: (
    challengeId: string,
  ) => Promise<AgentRateLimitRedemptionRecord | undefined>
}>

export type AgentRateLimitRecoveryRuntime = Readonly<{
  challengeTtlMs: number
  entitlementTtlMs: number
  makeChallengeId: () => string
  makeEntitlementId: () => string
  makeReceiptId: () => string
  makeRedemptionId: () => string
  nowIso: () => string
  nowMillis: () => number
}>

export const PublicAgentProposalRecoveryRoute = {
  entitlementKind: 'one_public_agent_proposal',
  method: 'POST',
  path: '/api/agents/proposals',
  price: {
    amount: 100,
    asset: 'bitcoin',
    denomination: 'sats',
  },
  previewPath: '/api/agents/proposals/rate-limit/preview',
  redeemPath: '/api/agents/proposals/rate-limit/redeem',
  routeKey: 'public_agent_proposals',
} as const

export const systemAgentRateLimitRecoveryRuntime: AgentRateLimitRecoveryRuntime =
  {
    challengeTtlMs: 10 * 60_000,
    entitlementTtlMs: 60 * 60_000,
    makeChallengeId: () => `agent_rate_limit_challenge_${randomUuid()}`,
    makeEntitlementId: () => `agent_rate_limit_entitlement_${randomUuid()}`,
    makeReceiptId: () => `agent_rate_limit_receipt_${randomUuid()}`,
    makeRedemptionId: () => `agent_rate_limit_redemption_${randomUuid()}`,
    nowIso: currentIsoTimestamp,
    nowMillis: currentEpochMillis,
  }

export class AgentRateLimitRecoveryError extends Error {
  readonly kind:
    | 'actor_mismatch'
    | 'binding_mismatch'
    | 'challenge_expired'
    | 'challenge_not_found'
    | 'entitlement_not_found'
    | 'grant_missing'
    | 'over_spend_cap'
    | 'storage_error'
    | 'unsafe_payment_ref'

  constructor(input: { kind: AgentRateLimitRecoveryError['kind']; reason: string }) {
    super(input.reason)
    this.kind = input.kind
  }
}

const decodeGrant = S.decodeUnknownOption(AgentRateLimitRecoveryGrant)

const prohibitedPaymentMaterialPattern =
  /(^|\b)(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|raw[_-]?invoice|payment_preimage|preimage|mdk_access_token|wallet_secret|private_key|webhook_secret)/i
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

const amountWithinCap = (
  price: AgentRateLimitMoneyAmount,
  spendCap: AgentRateLimitMoneyAmount,
): boolean =>
  price.asset === spendCap.asset &&
  price.denomination === spendCap.denomination &&
  price.amount <= spendCap.amount

const rowMoney = (input: {
  asset: 'bitcoin' | 'credits' | 'usd'
  denomination: 'sats' | 'credits' | 'cents'
  value: number
}): AgentRateLimitMoneyAmount => ({
  amount: input.value,
  asset: input.asset,
  denomination: input.denomination,
})

const challengeFromRow = (
  row: Readonly<{
    actor_ref: string
    archived_at: string | null
    client_fingerprint_hash: string
    created_at: string
    entitlement_kind: string
    expires_at: string
    id: string
    idempotency_key_hash: string
    method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
    owner_user_id: string
    path: string
    price_asset: 'bitcoin' | 'credits' | 'usd'
    price_denomination: 'sats' | 'credits' | 'cents'
    price_value: number
    public_projection_json: string
    request_body_digest: string
    route_key: AgentRateLimitRecoveryRouteKey
    spend_cap_asset: 'bitcoin' | 'credits' | 'usd'
    spend_cap_denomination: 'sats' | 'credits' | 'cents'
    spend_cap_value: number
    submission_idempotency_key_hash: string
  }>,
): AgentRateLimitChallengeRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  clientFingerprintHash: row.client_fingerprint_hash,
  createdAt: row.created_at,
  entitlementKind: row.entitlement_kind,
  expiresAt: row.expires_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  method: row.method,
  ownerUserId: row.owner_user_id,
  path: row.path,
  price: rowMoney({
    asset: row.price_asset,
    denomination: row.price_denomination,
    value: row.price_value,
  }),
  publicProjectionJson: row.public_projection_json,
  requestBodyDigest: row.request_body_digest,
  routeKey: row.route_key,
  spendCap: rowMoney({
    asset: row.spend_cap_asset,
    denomination: row.spend_cap_denomination,
    value: row.spend_cap_value,
  }),
  submissionIdempotencyKeyHash: row.submission_idempotency_key_hash,
})

const receiptFromRow = (
  row: Readonly<{
    actor_ref: string
    amount_asset: 'bitcoin' | 'credits' | 'usd'
    amount_denomination: 'sats' | 'credits' | 'cents'
    amount_value: number
    archived_at: string | null
    challenge_id: string
    created_at: string
    entitlement_ref: string
    id: string
    owner_user_id: string
    public_projection_json: string
    receipt_ref: string
    redacted_payment_ref: string
    route_key: AgentRateLimitRecoveryRouteKey
  }>,
): AgentRateLimitReceiptRecord => ({
  actorRef: row.actor_ref,
  amount: rowMoney({
    asset: row.amount_asset,
    denomination: row.amount_denomination,
    value: row.amount_value,
  }),
  archivedAt: row.archived_at,
  challengeId: row.challenge_id,
  createdAt: row.created_at,
  entitlementRef: row.entitlement_ref,
  id: row.id,
  ownerUserId: row.owner_user_id,
  publicProjectionJson: row.public_projection_json,
  receiptRef: row.receipt_ref,
  redactedPaymentRef: row.redacted_payment_ref,
  routeKey: row.route_key,
})

const entitlementFromRow = (
  row: Readonly<{
    actor_ref: string
    archived_at: string | null
    challenge_id: string
    client_fingerprint_hash: string
    consumed_at: string | null
    created_at: string
    entitlement_kind: string
    entitlement_ref: string
    expires_at: string
    id: string
    method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
    owner_user_id: string
    path: string
    receipt_ref: string
    request_body_digest: string
    route_key: AgentRateLimitRecoveryRouteKey
    status: 'active' | 'consumed' | 'expired'
    submission_idempotency_key_hash: string
  }>,
): AgentRateLimitEntitlementRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeId: row.challenge_id,
  clientFingerprintHash: row.client_fingerprint_hash,
  consumedAt: row.consumed_at,
  createdAt: row.created_at,
  entitlementKind: row.entitlement_kind,
  entitlementRef: row.entitlement_ref,
  expiresAt: row.expires_at,
  id: row.id,
  method: row.method,
  ownerUserId: row.owner_user_id,
  path: row.path,
  receiptRef: row.receipt_ref,
  requestBodyDigest: row.request_body_digest,
  routeKey: row.route_key,
  status: row.status,
  submissionIdempotencyKeyHash: row.submission_idempotency_key_hash,
})

const redemptionFromRow = (
  row: Readonly<{
    actor_ref: string
    archived_at: string | null
    challenge_id: string
    created_at: string
    entitlement_ref: string
    id: string
    idempotency_key_hash: string
    proof_ref: string
    receipt_ref: string
    replayed: number
  }>,
): AgentRateLimitRedemptionRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeId: row.challenge_id,
  createdAt: row.created_at,
  entitlementRef: row.entitlement_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  proofRef: row.proof_ref,
  receiptRef: row.receipt_ref,
  replayed: row.replayed,
})

const storageError = (
  operation: string,
  error: unknown,
): AgentRateLimitRecoveryError =>
  new AgentRateLimitRecoveryError({
    kind: 'storage_error',
    reason:
      error instanceof Error
        ? `${operation}: ${error.message}`
        : `${operation}: ${String(error)}`,
  })

export const makeD1AgentRateLimitRecoveryStore = (
  db: D1Database,
): AgentRateLimitRecoveryStore => ({
  consumeEntitlement: async input => {
    try {
      const result = await db
        .prepare(
          `UPDATE agent_rate_limit_entitlements
           SET status = 'consumed',
               consumed_at = ?
           WHERE entitlement_ref = ?
             AND actor_ref = ?
             AND route_key = ?
             AND method = ?
             AND path = ?
             AND request_body_digest = ?
             AND submission_idempotency_key_hash = ?
             AND client_fingerprint_hash = ?
             AND status = 'active'
             AND expires_at > ?
             AND archived_at IS NULL`,
        )
        .bind(
          input.nowIso,
          input.entitlementRef,
          input.actorRef,
          input.routeKey,
          input.method,
          input.path,
          input.requestBodyDigest,
          input.submissionIdempotencyKeyHash,
          input.clientFingerprintHash,
          input.nowIso,
        )
        .run()

      if (Number(result.meta?.changes ?? 0) < 1) {
        return undefined
      }

      const row = await db
        .prepare(
          `SELECT *
           FROM agent_rate_limit_entitlements
           WHERE entitlement_ref = ?
             AND archived_at IS NULL
           LIMIT 1`,
        )
        .bind(input.entitlementRef)
        .first()

      return row === null ? undefined : entitlementFromRow(row as never)
    } catch (error) {
      throw storageError('agentRateLimitRecovery.consumeEntitlement', error)
    }
  },
  createChallenge: async record => {
    try {
      await db
        .prepare(
          `INSERT OR IGNORE INTO agent_rate_limit_challenges
           (id, idempotency_key_hash, actor_ref, owner_user_id, route_key,
            method, path, submission_idempotency_key_hash,
            client_fingerprint_hash, request_body_digest, price_asset,
            price_denomination, price_value, spend_cap_asset,
            spend_cap_denomination, spend_cap_value, entitlement_kind,
            expires_at, public_projection_json, created_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.idempotencyKeyHash,
          record.actorRef,
          record.ownerUserId,
          record.routeKey,
          record.method,
          record.path,
          record.submissionIdempotencyKeyHash,
          record.clientFingerprintHash,
          record.requestBodyDigest,
          record.price.asset,
          record.price.denomination,
          record.price.amount,
          record.spendCap.asset,
          record.spendCap.denomination,
          record.spendCap.amount,
          record.entitlementKind,
          record.expiresAt,
          record.publicProjectionJson,
          record.createdAt,
          record.archivedAt,
        )
        .run()
    } catch (error) {
      throw storageError('agentRateLimitRecovery.createChallenge', error)
    }
  },
  createRedemptionBundle: async input => {
    try {
      await db.batch([
        db
          .prepare(
            `INSERT OR IGNORE INTO agent_rate_limit_receipts
             (id, receipt_ref, challenge_id, actor_ref, owner_user_id,
              route_key, amount_asset, amount_denomination, amount_value,
              entitlement_ref, redacted_payment_ref, public_projection_json,
              created_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.receipt.id,
            input.receipt.receiptRef,
            input.receipt.challengeId,
            input.receipt.actorRef,
            input.receipt.ownerUserId,
            input.receipt.routeKey,
            input.receipt.amount.asset,
            input.receipt.amount.denomination,
            input.receipt.amount.amount,
            input.receipt.entitlementRef,
            input.receipt.redactedPaymentRef,
            input.receipt.publicProjectionJson,
            input.receipt.createdAt,
            input.receipt.archivedAt,
          ),
        db
          .prepare(
            `INSERT OR IGNORE INTO agent_rate_limit_entitlements
             (id, entitlement_ref, challenge_id, receipt_ref, actor_ref,
              owner_user_id, route_key, method, path,
              submission_idempotency_key_hash, client_fingerprint_hash,
              request_body_digest, entitlement_kind, status, expires_at,
              created_at, consumed_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.entitlement.id,
            input.entitlement.entitlementRef,
            input.entitlement.challengeId,
            input.entitlement.receiptRef,
            input.entitlement.actorRef,
            input.entitlement.ownerUserId,
            input.entitlement.routeKey,
            input.entitlement.method,
            input.entitlement.path,
            input.entitlement.submissionIdempotencyKeyHash,
            input.entitlement.clientFingerprintHash,
            input.entitlement.requestBodyDigest,
            input.entitlement.entitlementKind,
            input.entitlement.status,
            input.entitlement.expiresAt,
            input.entitlement.createdAt,
            input.entitlement.consumedAt,
            input.entitlement.archivedAt,
          ),
        db
          .prepare(
            `INSERT OR IGNORE INTO agent_rate_limit_redemptions
             (id, idempotency_key_hash, challenge_id, actor_ref, proof_ref,
              entitlement_ref, receipt_ref, replayed, public_projection_json,
              created_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.redemption.id,
            input.redemption.idempotencyKeyHash,
            input.redemption.challengeId,
            input.redemption.actorRef,
            input.redemption.proofRef,
            input.redemption.entitlementRef,
            input.redemption.receiptRef,
            input.redemption.replayed,
            input.receipt.publicProjectionJson,
            input.redemption.createdAt,
            input.redemption.archivedAt,
          ),
      ])
    } catch (error) {
      throw storageError('agentRateLimitRecovery.createRedemptionBundle', error)
    }
  },
  readChallengeById: async challengeId => {
    try {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_rate_limit_challenges
           WHERE id = ?
             AND archived_at IS NULL
           LIMIT 1`,
        )
        .bind(challengeId)
        .first()

      return row === null ? undefined : challengeFromRow(row as never)
    } catch (error) {
      throw storageError('agentRateLimitRecovery.readChallengeById', error)
    }
  },
  readChallengeByIdempotencyKeyHash: async idempotencyKeyHash => {
    try {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_rate_limit_challenges
           WHERE idempotency_key_hash = ?
             AND archived_at IS NULL
           LIMIT 1`,
        )
        .bind(idempotencyKeyHash)
        .first()

      return row === null ? undefined : challengeFromRow(row as never)
    } catch (error) {
      throw storageError(
        'agentRateLimitRecovery.readChallengeByIdempotencyKeyHash',
        error,
      )
    }
  },
  readEntitlementByRef: async entitlementRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_rate_limit_entitlements
           WHERE entitlement_ref = ?
             AND archived_at IS NULL
           LIMIT 1`,
        )
        .bind(entitlementRef)
        .first()

      return row === null ? undefined : entitlementFromRow(row as never)
    } catch (error) {
      throw storageError('agentRateLimitRecovery.readEntitlementByRef', error)
    }
  },
  readReceiptByRef: async receiptRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_rate_limit_receipts
           WHERE receipt_ref = ?
             AND archived_at IS NULL
           LIMIT 1`,
        )
        .bind(receiptRef)
        .first()

      return row === null ? undefined : receiptFromRow(row as never)
    } catch (error) {
      throw storageError('agentRateLimitRecovery.readReceiptByRef', error)
    }
  },
  readRedemptionByChallengeId: async challengeId => {
    try {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_rate_limit_redemptions
           WHERE challenge_id = ?
             AND archived_at IS NULL
           LIMIT 1`,
        )
        .bind(challengeId)
        .first()

      return row === null ? undefined : redemptionFromRow(row as never)
    } catch (error) {
      throw storageError(
        'agentRateLimitRecovery.readRedemptionByChallengeId',
        error,
      )
    }
  },
})

const stableJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value)
  }

  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`
  }

  const record = value as Record<string, unknown>

  return `{${Object.keys(record)
    .sort()
    .map(key => `${JSON.stringify(key)}:${stableJson(record[key])}`)
    .join(',')}}`
}

export const agentRateLimitRecoveryGrantsFromSession = (
  session: ProgrammaticAgentSession,
): ReadonlyArray<AgentRateLimitRecoveryGrant> => {
  const metadata = parseJsonRecord(session.credential.profileMetadataJson)
  const grants = metadata?.agentRateLimitRecoveryGrants

  return Array.isArray(grants)
    ? grants.flatMap(grant => {
        const decoded = Option.getOrUndefined(decodeGrant(grant))

        return decoded === undefined ? [] : [decoded]
      })
    : []
}

export const agentRateLimitActorRef = (
  session: ProgrammaticAgentSession,
): string => `agent:${session.user.id}`

export const computeAgentRateLimitRequestBodyDigest = async (
  routeKey: AgentRateLimitRecoveryRouteKey,
  method: string,
  path: string,
  body: unknown,
): Promise<string> =>
  `sha256:${await sha256Hex(
    `${routeKey}:${method.toUpperCase()}:${path}:${stableJson(body)}`,
  )}`

export const agentRateLimitSubmissionIdempotencyKeyHash = async (
  actorRef: string,
  routeKey: AgentRateLimitRecoveryRouteKey,
  idempotencyKey: string,
): Promise<string> =>
  sha256Hex(`${actorRef}:${routeKey}:${idempotencyKey}`)

export const agentRateLimitChallengeIdempotencyKeyHash = async (
  actorRef: string,
  routeKey: AgentRateLimitRecoveryRouteKey,
  idempotencyKey: string,
): Promise<string> =>
  sha256Hex(`challenge:${actorRef}:${routeKey}:${idempotencyKey}`)

export const agentRateLimitRedemptionIdempotencyKeyHash = async (
  actorRef: string,
  challengeId: string,
  idempotencyKey: string,
): Promise<string> => sha256Hex(`redeem:${actorRef}:${challengeId}:${idempotencyKey}`)

export const validateAgentRateLimitPaymentProofRef = (
  proofRef: string,
): void => {
  if (
    !safeRefPattern.test(proofRef) ||
    containsProviderSecretMaterial(proofRef) ||
    prohibitedPaymentMaterialPattern.test(proofRef)
  ) {
    throw new AgentRateLimitRecoveryError({
      kind: 'unsafe_payment_ref',
      reason:
        'Rate-limit recovery payment proof must be a public-safe redacted proof ref.',
    })
  }
}

export const activeAgentRateLimitRecoveryGrant = (
  session: ProgrammaticAgentSession,
  routeKey: AgentRateLimitRecoveryRouteKey,
  price: AgentRateLimitMoneyAmount,
  nowIso: string,
): AgentRateLimitRecoveryGrant | undefined =>
  agentRateLimitRecoveryGrantsFromSession(session).find(
    grant =>
      grant.status === 'active' &&
      grant.routeKeys.includes(routeKey) &&
      (grant.expiresAt === null || grant.expiresAt > nowIso) &&
      amountWithinCap(price, grant.spendCap),
  )

export const assertAmountWithinSpendCap = (
  price: AgentRateLimitMoneyAmount,
  spendCap: AgentRateLimitMoneyAmount,
): void => {
  if (!amountWithinCap(price, spendCap)) {
    throw new AgentRateLimitRecoveryError({
      kind: 'over_spend_cap',
      reason: 'Rate-limit recovery price must be within the declared spend cap.',
    })
  }
}

export const recoveryPublicProjection = (
  input: Readonly<{
    actorRef: string
    ownerUserId: string
    routeKey: AgentRateLimitRecoveryRouteKey
  }>,
): Record<string, unknown> => ({
  actorRef: input.actorRef,
  authority:
    'owner-approved route spend cap required before any paid retry is honored',
  customerSafe: true,
  dataClassification: 'public',
  ownerUserId: input.ownerUserId,
  publicSafe: true,
  redactionPolicyRef: 'openagents.agent_rate_limit_recovery.redaction.v1',
  routeKey: input.routeKey,
})

export const previewAgentRateLimitRecovery = async (
  store: AgentRateLimitRecoveryStore,
  input: Readonly<{
    actorRef: string
    clientFingerprintHash: string
    idempotencyKey: string
    ownerUserId: string
    requestBodyDigest: string
    routeKey: AgentRateLimitRecoveryRouteKey
    spendCap: AgentRateLimitMoneyAmount
    submissionIdempotencyKeyHash: string
  }>,
  runtime: AgentRateLimitRecoveryRuntime = systemAgentRateLimitRecoveryRuntime,
) => {
  assertAmountWithinSpendCap(
    PublicAgentProposalRecoveryRoute.price,
    input.spendCap,
  )

  const idempotencyKeyHash = await agentRateLimitChallengeIdempotencyKeyHash(
    input.actorRef,
    input.routeKey,
    input.idempotencyKey,
  )
  const existing = await store.readChallengeByIdempotencyKeyHash(
    idempotencyKeyHash,
  )
  const nowIso = runtime.nowIso()
  const challenge =
    existing ??
    ({
      actorRef: input.actorRef,
      archivedAt: null,
      clientFingerprintHash: input.clientFingerprintHash,
      createdAt: nowIso,
      entitlementKind: PublicAgentProposalRecoveryRoute.entitlementKind,
      expiresAt: epochMillisToIsoTimestamp(
        runtime.nowMillis() + runtime.challengeTtlMs,
      ),
      id: runtime.makeChallengeId(),
      idempotencyKeyHash,
      method: PublicAgentProposalRecoveryRoute.method,
      ownerUserId: input.ownerUserId,
      path: PublicAgentProposalRecoveryRoute.path,
      price: PublicAgentProposalRecoveryRoute.price,
      publicProjectionJson: JSON.stringify(
        recoveryPublicProjection({
          actorRef: input.actorRef,
          ownerUserId: input.ownerUserId,
          routeKey: input.routeKey,
        }),
      ),
      requestBodyDigest: input.requestBodyDigest,
      routeKey: input.routeKey,
      spendCap: input.spendCap,
      submissionIdempotencyKeyHash: input.submissionIdempotencyKeyHash,
    } satisfies AgentRateLimitChallengeRecord)

  if (existing === undefined) {
    await store.createChallenge(challenge)
  }

  return challenge
}

export const redeemAgentRateLimitRecovery = async (
  store: AgentRateLimitRecoveryStore,
  input: Readonly<{
    actorRef: string
    challengeId: string
    idempotencyKey: string
    l402ProofRef: string
  }>,
  runtime: AgentRateLimitRecoveryRuntime = systemAgentRateLimitRecoveryRuntime,
) => {
  validateAgentRateLimitPaymentProofRef(input.l402ProofRef)

  const challenge = await store.readChallengeById(input.challengeId)

  if (challenge === undefined) {
    throw new AgentRateLimitRecoveryError({
      kind: 'challenge_not_found',
      reason: 'Rate-limit recovery challenge was not found.',
    })
  }

  if (challenge.actorRef !== input.actorRef) {
    throw new AgentRateLimitRecoveryError({
      kind: 'actor_mismatch',
      reason: 'Rate-limit recovery challenge actor does not match.',
    })
  }

  if (Date.parse(challenge.expiresAt) <= runtime.nowMillis()) {
    throw new AgentRateLimitRecoveryError({
      kind: 'challenge_expired',
      reason: 'Rate-limit recovery challenge expired.',
    })
  }

  const existingRedemption = await store.readRedemptionByChallengeId(
    challenge.id,
  )

  if (existingRedemption !== undefined) {
    return {
      entitlementRef: existingRedemption.entitlementRef,
      originalReceiptRef: existingRedemption.receiptRef,
      receiptRef: existingRedemption.receiptRef,
      replayed: true,
    }
  }

  const nowIso = runtime.nowIso()
  const entitlementRef = `agent_rate_limit_entitlement:${challenge.id}`
  const receiptRef = `receipt.agent_rate_limit.${challenge.id}`
  const receipt: AgentRateLimitReceiptRecord = {
    actorRef: challenge.actorRef,
    amount: challenge.price,
    archivedAt: null,
    challengeId: challenge.id,
    createdAt: nowIso,
    entitlementRef,
    id: runtime.makeReceiptId(),
    ownerUserId: challenge.ownerUserId,
    publicProjectionJson: challenge.publicProjectionJson,
    receiptRef,
    redactedPaymentRef: input.l402ProofRef,
    routeKey: challenge.routeKey,
  }
  const entitlement: AgentRateLimitEntitlementRecord = {
    actorRef: challenge.actorRef,
    archivedAt: null,
    challengeId: challenge.id,
    clientFingerprintHash: challenge.clientFingerprintHash,
    consumedAt: null,
    createdAt: nowIso,
    entitlementKind: challenge.entitlementKind,
    entitlementRef,
    expiresAt: epochMillisToIsoTimestamp(
      runtime.nowMillis() + runtime.entitlementTtlMs,
    ),
    id: runtime.makeEntitlementId(),
    method: challenge.method,
    ownerUserId: challenge.ownerUserId,
    path: challenge.path,
    receiptRef,
    requestBodyDigest: challenge.requestBodyDigest,
    routeKey: challenge.routeKey,
    status: 'active',
    submissionIdempotencyKeyHash: challenge.submissionIdempotencyKeyHash,
  }
  const redemption: AgentRateLimitRedemptionRecord = {
    actorRef: challenge.actorRef,
    archivedAt: null,
    challengeId: challenge.id,
    createdAt: nowIso,
    entitlementRef,
    id: runtime.makeRedemptionId(),
    idempotencyKeyHash: await agentRateLimitRedemptionIdempotencyKeyHash(
      input.actorRef,
      input.challengeId,
      input.idempotencyKey,
    ),
    proofRef: input.l402ProofRef,
    receiptRef,
    replayed: 0,
  }

  await store.createRedemptionBundle({ entitlement, receipt, redemption })

  return {
    entitlementRef,
    originalReceiptRef: null,
    receiptRef,
    replayed: false,
  }
}
