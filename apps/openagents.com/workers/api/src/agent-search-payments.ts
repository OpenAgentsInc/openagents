import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import { sha256Hex } from './agent-registration'
import {
  AGENT_SEARCH_BASIC_RECOVERY_PRICE,
  AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
  AGENT_SEARCH_BASIC_RECOVERY_SCOPE_REF,
  AGENT_SEARCH_ENDPOINT,
  AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT,
  type AgentSearchSession,
  AgentSearchValidationError,
  agentSearchPolicyFromConfig,
  validateAgentSearchRequest,
} from './agent-search'
import { type OpenAgentsWorkerConfigShape } from './config'
import type { InferenceEntitlementsMirror } from './inference-entitlements-store'
import { recordFromUnknown } from './json-boundary'
import {
  compactRandomId,
  currentIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

export type AgentSearchPaymentRuntime = Readonly<{
  makeChallengeId: () => string
  makeEntitlementId: () => string
  makeReceiptId: () => string
  makeRedemptionId: () => string
  nowIso: () => string
}>

export const systemAgentSearchPaymentRuntime: AgentSearchPaymentRuntime = {
  makeChallengeId: () => compactRandomId('agent_search_payment_challenge'),
  makeEntitlementId: () => compactRandomId('agent_search_entitlement'),
  makeReceiptId: () => compactRandomId('agent_search_payment_receipt'),
  makeRedemptionId: () => compactRandomId('agent_search_payment_redemption'),
  nowIso: currentIsoTimestamp,
}

export type AgentSearchPaymentAmount = Readonly<{
  amountMinorUnits: number
  asset: 'credits'
  denomination: 'credit'
}>

export type AgentSearchPaymentChallengeRecord = Readonly<{
  actorRef: string
  agentUserId: string
  archivedAt: string | null
  createdAt: string
  credentialId: string
  expiresAt: string
  id: string
  idempotencyKeyHash: string
  method: 'POST'
  mode: 'basic'
  path: typeof AGENT_SEARCH_ENDPOINT
  price: AgentSearchPaymentAmount
  productId: string
  publicProjectionJson: string
  requestBodyDigest: string
  spendCap: AgentSearchPaymentAmount
  tokenPrefix: string
}>

export type AgentSearchPaymentReceiptRecord = Readonly<{
  actorRef: string
  agentUserId: string
  archivedAt: string | null
  amount: AgentSearchPaymentAmount
  challengeId: string
  createdAt: string
  credentialId: string
  entitlementRef: string
  id: string
  productId: string
  publicProjectionJson: string
  receiptRef: string
  redactedPaymentRef: string
}>

export type AgentSearchEntitlementRecord = Readonly<{
  actorRef: string
  agentUserId: string
  archivedAt: string | null
  challengeId: string
  consumedAt: string | null
  createdAt: string
  credentialId: string
  entitlementRef: string
  expiresAt: string
  id: string
  method: 'POST'
  mode: 'basic'
  path: typeof AGENT_SEARCH_ENDPOINT
  productId: string
  receiptRef: string
  requestBodyDigest: string
  scopeRef: string
  status: 'active' | 'consumed' | 'expired'
}>

export type AgentSearchPaymentRedemptionRecord = Readonly<{
  actorRef: string
  archivedAt: string | null
  challengeId: string
  createdAt: string
  credentialId: string
  entitlementRef: string
  id: string
  idempotencyKeyHash: string
  proofRef: string
  publicProjectionJson: string
  receiptRef: string
}>

export type AgentSearchPaymentStore = Readonly<{
  createChallenge: (
    challenge: AgentSearchPaymentChallengeRecord,
  ) => Promise<void>
  createRedemptionBundle: (input: {
    entitlement: AgentSearchEntitlementRecord
    receipt: AgentSearchPaymentReceiptRecord
    redemption: AgentSearchPaymentRedemptionRecord
  }) => Promise<void>
  readChallengeById: (
    challengeId: string,
  ) => Promise<AgentSearchPaymentChallengeRecord | undefined>
  readChallengeByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<AgentSearchPaymentChallengeRecord | undefined>
  readRedemptionByChallengeId: (
    challengeId: string,
  ) => Promise<AgentSearchPaymentRedemptionRecord | undefined>
}>

export type AgentSearchPaymentPreview = Readonly<{
  challenge: Readonly<{
    expiresAt: string
    id: string
    method: 'POST'
    path: typeof AGENT_SEARCH_ENDPOINT
    productId: string
    requestBodyDigest: string
  }>
  endpoints: Readonly<{
    redeem: typeof AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT
    search: typeof AGENT_SEARCH_ENDPOINT
  }>
  payment: Readonly<{
    price: AgentSearchPaymentAmount
    proofRefSemantics: 'redacted_mdk_l402_ref'
    spendCap: AgentSearchPaymentAmount
  }>
}>

export type AgentSearchPaymentRedemption = Readonly<{
  entitlement: Readonly<{
    entitlementRef: string
    expiresAt: string
    productId: string
    scopeRef: string
  }>
  receipt: Readonly<{
    receiptRef: string
  }>
  replayed: boolean
  search: Readonly<{
    entitlementHeader: string
    href: typeof AGENT_SEARCH_ENDPOINT
  }>
}>

const rawPaymentMaterialPattern =
  /(lnbc|lntb|lnbcrt|payment_hash|payment_preimage|preimage|raw_invoice|wallet|mnemonic|secret|bearer\s+)/i

const amountFromUnknown = (
  value: unknown,
  field: string,
): AgentSearchPaymentAmount => {
  const record = recordFromUnknown(value)

  if (record === undefined) {
    throw new AgentSearchValidationError(`${field} is required.`)
  }

  if (record.asset !== 'credits' || record.denomination !== 'credit') {
    throw new AgentSearchValidationError(
      `${field} must use credits/credit for hosted search.`,
    )
  }

  if (
    typeof record.amountMinorUnits !== 'number' ||
    !Number.isInteger(record.amountMinorUnits) ||
    record.amountMinorUnits < 0
  ) {
    throw new AgentSearchValidationError(
      `${field}.amountMinorUnits must be a non-negative integer.`,
    )
  }

  return {
    amountMinorUnits: record.amountMinorUnits,
    asset: 'credits',
    denomination: 'credit',
  }
}

const spendCapAllowsPrice = (
  spendCap: AgentSearchPaymentAmount,
  price: AgentSearchPaymentAmount,
): boolean =>
  spendCap.asset === price.asset &&
  spendCap.denomination === price.denomination &&
  spendCap.amountMinorUnits >= price.amountMinorUnits

const proofRefFromUnknown = (value: unknown): string => {
  if (typeof value !== 'string' || value.trim().length < 8) {
    throw new AgentSearchValidationError('l402ProofRef is required.')
  }

  const proofRef = value.trim()

  if (
    containsProviderSecretMaterial(proofRef) ||
    rawPaymentMaterialPattern.test(proofRef)
  ) {
    throw new AgentSearchValidationError(
      'l402ProofRef must be a redacted public-safe proof reference, not raw payment material.',
    )
  }

  return proofRef
}

const previewProjection = (
  challenge: AgentSearchPaymentChallengeRecord,
): AgentSearchPaymentPreview => ({
  challenge: {
    expiresAt: challenge.expiresAt,
    id: challenge.id,
    method: challenge.method,
    path: challenge.path,
    productId: challenge.productId,
    requestBodyDigest: challenge.requestBodyDigest,
  },
  endpoints: {
    redeem: AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT,
    search: AGENT_SEARCH_ENDPOINT,
  },
  payment: {
    price: challenge.price,
    proofRefSemantics: 'redacted_mdk_l402_ref',
    spendCap: challenge.spendCap,
  },
})

const redemptionProjection = (
  input: Readonly<{
    entitlement: AgentSearchEntitlementRecord
    receipt: AgentSearchPaymentReceiptRecord
    replayed: boolean
  }>,
): AgentSearchPaymentRedemption => ({
  entitlement: {
    entitlementRef: input.entitlement.entitlementRef,
    expiresAt: input.entitlement.expiresAt,
    productId: input.entitlement.productId,
    scopeRef: input.entitlement.scopeRef,
  },
  receipt: {
    receiptRef: input.receipt.receiptRef,
  },
  replayed: input.replayed,
  search: {
    entitlementHeader: 'X-OpenAgents-Agent-Search-Entitlement',
    href: AGENT_SEARCH_ENDPOINT,
  },
})

const challengePublicProjection = (
  challenge: AgentSearchPaymentChallengeRecord,
): string => JSON.stringify(previewProjection(challenge))

const redemptionPublicProjection = (
  redemption: AgentSearchPaymentRedemption,
): string => JSON.stringify(redemption)

export const previewAgentSearchPayment = async (
  store: AgentSearchPaymentStore,
  input: Readonly<{
    body: Record<string, unknown>
    config: OpenAgentsWorkerConfigShape
    idempotencyKey: string
    session: AgentSearchSession
  }>,
  runtime: AgentSearchPaymentRuntime = systemAgentSearchPaymentRuntime,
): Promise<AgentSearchPaymentPreview> => {
  const policy = agentSearchPolicyFromConfig(input.config.exa)
  const searchBody = recordFromUnknown(input.body.search)

  if (searchBody === undefined) {
    throw new AgentSearchValidationError('search request body is required.')
  }

  const search = validateAgentSearchRequest(searchBody, policy)
  const spendCap = amountFromUnknown(input.body.spendCap, 'spendCap')
  const price = AGENT_SEARCH_BASIC_RECOVERY_PRICE

  if (!spendCapAllowsPrice(spendCap, price)) {
    throw new AgentSearchValidationError(
      'spendCap must cover the hosted search recovery price.',
    )
  }

  const actorRef = `agent:${input.session.user.id}`
  const idempotencyKeyHash = await sha256Hex(
    `${actorRef}:agent_search_payment_preview:${input.idempotencyKey}`,
  )
  const existing =
    await store.readChallengeByIdempotencyKeyHash(idempotencyKeyHash)

  if (existing !== undefined) {
    return previewProjection(existing)
  }

  const nowIso = runtime.nowIso()
  const challenge: AgentSearchPaymentChallengeRecord = {
    actorRef,
    agentUserId: input.session.user.id,
    archivedAt: null,
    createdAt: nowIso,
    credentialId: input.session.credential.id,
    expiresAt: isoTimestampAfterIso(nowIso, 15 * 60 * 1000),
    id: runtime.makeChallengeId(),
    idempotencyKeyHash,
    method: 'POST',
    mode: search.mode,
    path: AGENT_SEARCH_ENDPOINT,
    price,
    productId: AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
    publicProjectionJson: '{}',
    requestBodyDigest: await sha256Hex(JSON.stringify(search)),
    spendCap,
    tokenPrefix: input.session.credential.tokenPrefix,
  }
  const storedChallenge = {
    ...challenge,
    publicProjectionJson: challengePublicProjection(challenge),
  }

  await store.createChallenge(storedChallenge)

  return previewProjection(storedChallenge)
}

export const redeemAgentSearchPayment = async (
  store: AgentSearchPaymentStore,
  input: Readonly<{
    body: Record<string, unknown>
    idempotencyKey: string
    session: AgentSearchSession
  }>,
  runtime: AgentSearchPaymentRuntime = systemAgentSearchPaymentRuntime,
): Promise<AgentSearchPaymentRedemption> => {
  const challengeId =
    typeof input.body.challengeId === 'string'
      ? input.body.challengeId.trim()
      : ''
  const proofRef = proofRefFromUnknown(input.body.l402ProofRef)

  if (challengeId === '') {
    throw new AgentSearchValidationError('challengeId is required.')
  }

  const challenge = await store.readChallengeById(challengeId)
  const actorRef = `agent:${input.session.user.id}`

  if (
    challenge === undefined ||
    challenge.actorRef !== actorRef ||
    challenge.credentialId !== input.session.credential.id
  ) {
    throw new AgentSearchValidationError(
      'Agent search payment challenge was not found.',
    )
  }

  const nowIso = runtime.nowIso()

  if (challenge.expiresAt <= nowIso) {
    throw new AgentSearchValidationError(
      'Agent search payment challenge has expired.',
    )
  }

  const existing = await store.readRedemptionByChallengeId(challenge.id)

  if (existing !== undefined) {
    return {
      entitlement: {
        entitlementRef: existing.entitlementRef,
        expiresAt: challenge.expiresAt,
        productId: challenge.productId,
        scopeRef: AGENT_SEARCH_BASIC_RECOVERY_SCOPE_REF,
      },
      receipt: {
        receiptRef: existing.receiptRef,
      },
      replayed: true,
      search: {
        entitlementHeader: 'X-OpenAgents-Agent-Search-Entitlement',
        href: AGENT_SEARCH_ENDPOINT,
      },
    }
  }

  const entitlementRef = `entitlement.agent_search.${challenge.id}`
  const receiptRef = `receipt.agent_search_payment.${challenge.id}`
  const receipt: AgentSearchPaymentReceiptRecord = {
    actorRef,
    agentUserId: challenge.agentUserId,
    archivedAt: null,
    amount: challenge.price,
    challengeId: challenge.id,
    createdAt: nowIso,
    credentialId: challenge.credentialId,
    entitlementRef,
    id: runtime.makeReceiptId(),
    productId: challenge.productId,
    publicProjectionJson: '{}',
    receiptRef,
    redactedPaymentRef: proofRef,
  }
  const entitlement: AgentSearchEntitlementRecord = {
    actorRef,
    agentUserId: challenge.agentUserId,
    archivedAt: null,
    challengeId: challenge.id,
    consumedAt: null,
    createdAt: nowIso,
    credentialId: challenge.credentialId,
    entitlementRef,
    expiresAt: isoTimestampAfterIso(nowIso, 10 * 60 * 1000),
    id: runtime.makeEntitlementId(),
    method: 'POST',
    mode: challenge.mode,
    path: challenge.path,
    productId: challenge.productId,
    receiptRef,
    requestBodyDigest: challenge.requestBodyDigest,
    scopeRef: AGENT_SEARCH_BASIC_RECOVERY_SCOPE_REF,
    status: 'active',
  }
  const projection = redemptionProjection({
    entitlement,
    receipt,
    replayed: false,
  })
  const redemption: AgentSearchPaymentRedemptionRecord = {
    actorRef,
    archivedAt: null,
    challengeId: challenge.id,
    createdAt: nowIso,
    credentialId: challenge.credentialId,
    entitlementRef,
    id: runtime.makeRedemptionId(),
    idempotencyKeyHash: await sha256Hex(
      `${actorRef}:agent_search_payment_redeem:${input.idempotencyKey}`,
    ),
    proofRef,
    publicProjectionJson: redemptionPublicProjection(projection),
    receiptRef,
  }

  await store.createRedemptionBundle({
    entitlement,
    receipt: {
      ...receipt,
      publicProjectionJson: redemptionPublicProjection(projection),
    },
    redemption,
  })

  return projection
}

type ChallengeRow = Readonly<{
  actor_ref: string
  agent_user_id: string
  archived_at: string | null
  created_at: string
  credential_id: string
  expires_at: string
  id: string
  idempotency_key_hash: string
  mode: 'basic'
  price_value: number
  product_id: string
  public_projection_json: string
  request_body_digest: string
  spend_cap_value: number
  token_prefix: string
}>

type RedemptionRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_id: string
  created_at: string
  credential_id: string
  entitlement_ref: string
  id: string
  idempotency_key_hash: string
  proof_ref: string
  public_projection_json: string
  receipt_ref: string
}>

const rowToChallenge = (
  row: ChallengeRow,
): AgentSearchPaymentChallengeRecord => ({
  actorRef: row.actor_ref,
  agentUserId: row.agent_user_id,
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  credentialId: row.credential_id,
  expiresAt: row.expires_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  method: 'POST',
  mode: row.mode,
  path: AGENT_SEARCH_ENDPOINT,
  price: {
    amountMinorUnits: row.price_value,
    asset: 'credits',
    denomination: 'credit',
  },
  productId: row.product_id,
  publicProjectionJson: row.public_projection_json,
  requestBodyDigest: row.request_body_digest,
  spendCap: {
    amountMinorUnits: row.spend_cap_value,
    asset: 'credits',
    denomination: 'credit',
  },
  tokenPrefix: row.token_prefix,
})

const rowToRedemption = (
  row: RedemptionRow,
): AgentSearchPaymentRedemptionRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeId: row.challenge_id,
  createdAt: row.created_at,
  credentialId: row.credential_id,
  entitlementRef: row.entitlement_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  proofRef: row.proof_ref,
  publicProjectionJson: row.public_projection_json,
  receiptRef: row.receipt_ref,
})

// KS-8.9 (#8320): optional fire-safe Postgres dual-write mirror; absent =>
// byte-identical D1-only behavior.
export const makeD1AgentSearchPaymentStore = (
  db: D1Database,
  mirror?: InferenceEntitlementsMirror | undefined,
): AgentSearchPaymentStore => ({
  createChallenge: async challenge => {
    await db
      .prepare(
        `INSERT OR IGNORE INTO agent_search_payment_challenges
           (id,
            idempotency_key_hash,
            actor_ref,
            agent_user_id,
            credential_id,
            token_prefix,
            method,
            path,
            mode,
            request_body_digest,
            product_id,
            price_asset,
            price_denomination,
            price_value,
            spend_cap_asset,
            spend_cap_denomination,
            spend_cap_value,
            expires_at,
            public_projection_json,
            created_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        challenge.id,
        challenge.idempotencyKeyHash,
        challenge.actorRef,
        challenge.agentUserId,
        challenge.credentialId,
        challenge.tokenPrefix,
        challenge.method,
        challenge.path,
        challenge.mode,
        challenge.requestBodyDigest,
        challenge.productId,
        challenge.price.asset,
        challenge.price.denomination,
        challenge.price.amountMinorUnits,
        challenge.spendCap.asset,
        challenge.spendCap.denomination,
        challenge.spendCap.amountMinorUnits,
        challenge.expiresAt,
        challenge.publicProjectionJson,
        challenge.createdAt,
        challenge.archivedAt,
      )
      .run()
    mirror?.([
      {
        kind: 'write',
        row: {
          actor_ref: challenge.actorRef,
          agent_user_id: challenge.agentUserId,
          archived_at: challenge.archivedAt,
          created_at: challenge.createdAt,
          credential_id: challenge.credentialId,
          expires_at: challenge.expiresAt,
          id: challenge.id,
          idempotency_key_hash: challenge.idempotencyKeyHash,
          method: challenge.method,
          mode: challenge.mode,
          path: challenge.path,
          price_asset: challenge.price.asset,
          price_denomination: challenge.price.denomination,
          price_value: challenge.price.amountMinorUnits,
          product_id: challenge.productId,
          public_projection_json: challenge.publicProjectionJson,
          request_body_digest: challenge.requestBodyDigest,
          spend_cap_asset: challenge.spendCap.asset,
          spend_cap_denomination: challenge.spendCap.denomination,
          spend_cap_value: challenge.spendCap.amountMinorUnits,
          token_prefix: challenge.tokenPrefix,
        },
        table: 'agent_search_payment_challenges',
      },
    ])
  },
  createRedemptionBundle: async input => {
    await db.batch([
      db
        .prepare(
          `INSERT OR IGNORE INTO agent_search_payment_receipts
             (id,
              receipt_ref,
              challenge_id,
              actor_ref,
              agent_user_id,
              credential_id,
              product_id,
              amount_asset,
              amount_denomination,
              amount_value,
              entitlement_ref,
              redacted_payment_ref,
              public_projection_json,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.receipt.id,
          input.receipt.receiptRef,
          input.receipt.challengeId,
          input.receipt.actorRef,
          input.receipt.agentUserId,
          input.receipt.credentialId,
          input.receipt.productId,
          input.receipt.amount.asset,
          input.receipt.amount.denomination,
          input.receipt.amount.amountMinorUnits,
          input.receipt.entitlementRef,
          input.receipt.redactedPaymentRef,
          input.receipt.publicProjectionJson,
          input.receipt.createdAt,
          input.receipt.archivedAt,
        ),
      db
        .prepare(
          `INSERT OR IGNORE INTO agent_search_entitlements
             (id,
              entitlement_ref,
              challenge_id,
              receipt_ref,
              actor_ref,
              agent_user_id,
              credential_id,
              product_id,
              scope_ref,
              method,
              path,
              mode,
              request_body_digest,
              status,
              expires_at,
              created_at,
              consumed_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.entitlement.id,
          input.entitlement.entitlementRef,
          input.entitlement.challengeId,
          input.entitlement.receiptRef,
          input.entitlement.actorRef,
          input.entitlement.agentUserId,
          input.entitlement.credentialId,
          input.entitlement.productId,
          input.entitlement.scopeRef,
          input.entitlement.method,
          input.entitlement.path,
          input.entitlement.mode,
          input.entitlement.requestBodyDigest,
          input.entitlement.status,
          input.entitlement.expiresAt,
          input.entitlement.createdAt,
          input.entitlement.consumedAt,
          input.entitlement.archivedAt,
        ),
      db
        .prepare(
          `INSERT OR IGNORE INTO agent_search_payment_redemptions
             (id,
              idempotency_key_hash,
              challenge_id,
              actor_ref,
              credential_id,
              proof_ref,
              entitlement_ref,
              receipt_ref,
              public_projection_json,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.redemption.id,
          input.redemption.idempotencyKeyHash,
          input.redemption.challengeId,
          input.redemption.actorRef,
          input.redemption.credentialId,
          input.redemption.proofRef,
          input.redemption.entitlementRef,
          input.redemption.receiptRef,
          input.redemption.publicProjectionJson,
          input.redemption.createdAt,
          input.redemption.archivedAt,
        ),
    ])
    mirror?.([
      {
        kind: 'write',
        row: {
          actor_ref: input.receipt.actorRef,
          agent_user_id: input.receipt.agentUserId,
          amount_asset: input.receipt.amount.asset,
          amount_denomination: input.receipt.amount.denomination,
          amount_value: input.receipt.amount.amountMinorUnits,
          archived_at: input.receipt.archivedAt,
          challenge_id: input.receipt.challengeId,
          created_at: input.receipt.createdAt,
          credential_id: input.receipt.credentialId,
          entitlement_ref: input.receipt.entitlementRef,
          id: input.receipt.id,
          product_id: input.receipt.productId,
          public_projection_json: input.receipt.publicProjectionJson,
          receipt_ref: input.receipt.receiptRef,
          redacted_payment_ref: input.receipt.redactedPaymentRef,
        },
        table: 'agent_search_payment_receipts',
      },
      {
        kind: 'write',
        row: {
          actor_ref: input.entitlement.actorRef,
          agent_user_id: input.entitlement.agentUserId,
          archived_at: input.entitlement.archivedAt,
          challenge_id: input.entitlement.challengeId,
          consumed_at: input.entitlement.consumedAt,
          created_at: input.entitlement.createdAt,
          credential_id: input.entitlement.credentialId,
          entitlement_ref: input.entitlement.entitlementRef,
          expires_at: input.entitlement.expiresAt,
          id: input.entitlement.id,
          method: input.entitlement.method,
          mode: input.entitlement.mode,
          path: input.entitlement.path,
          product_id: input.entitlement.productId,
          receipt_ref: input.entitlement.receiptRef,
          request_body_digest: input.entitlement.requestBodyDigest,
          scope_ref: input.entitlement.scopeRef,
          status: input.entitlement.status,
        },
        table: 'agent_search_entitlements',
      },
      {
        kind: 'write',
        row: {
          actor_ref: input.redemption.actorRef,
          archived_at: input.redemption.archivedAt,
          challenge_id: input.redemption.challengeId,
          created_at: input.redemption.createdAt,
          credential_id: input.redemption.credentialId,
          entitlement_ref: input.redemption.entitlementRef,
          id: input.redemption.id,
          idempotency_key_hash: input.redemption.idempotencyKeyHash,
          proof_ref: input.redemption.proofRef,
          public_projection_json: input.redemption.publicProjectionJson,
          receipt_ref: input.redemption.receiptRef,
        },
        table: 'agent_search_payment_redemptions',
      },
    ])
  },
  readChallengeById: async challengeId => {
    const row = await db
      .prepare(
        `SELECT id,
                idempotency_key_hash,
                actor_ref,
                agent_user_id,
                credential_id,
                token_prefix,
                mode,
                request_body_digest,
                product_id,
                price_value,
                spend_cap_value,
                expires_at,
                public_projection_json,
                created_at,
                archived_at
           FROM agent_search_payment_challenges
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(challengeId)
      .first<ChallengeRow>()

    return row === null ? undefined : rowToChallenge(row)
  },
  readChallengeByIdempotencyKeyHash: async idempotencyKeyHash => {
    const row = await db
      .prepare(
        `SELECT id,
                idempotency_key_hash,
                actor_ref,
                agent_user_id,
                credential_id,
                token_prefix,
                mode,
                request_body_digest,
                product_id,
                price_value,
                spend_cap_value,
                expires_at,
                public_projection_json,
                created_at,
                archived_at
           FROM agent_search_payment_challenges
          WHERE idempotency_key_hash = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKeyHash)
      .first<ChallengeRow>()

    return row === null ? undefined : rowToChallenge(row)
  },
  readRedemptionByChallengeId: async challengeId => {
    const row = await db
      .prepare(
        `SELECT id,
                idempotency_key_hash,
                challenge_id,
                actor_ref,
                credential_id,
                proof_ref,
                entitlement_ref,
                receipt_ref,
                public_projection_json,
                created_at,
                archived_at
           FROM agent_search_payment_redemptions
          WHERE challenge_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(challengeId)
      .first<RedemptionRow>()

    return row === null ? undefined : rowToRedemption(row)
  },
})
