import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

const TARGETED_SITE_AGENT_TOOLKIT_SCOPES = [
  'campaign:discover',
  'campaign:capture',
  'campaign:audit',
  'campaign:preview',
  'campaign:outreach:request',
  'campaign:metric:record',
  'campaign:reward:propose',
] as const

export const TargetedSiteAgentToolkitScope = S.Literals(
  TARGETED_SITE_AGENT_TOOLKIT_SCOPES,
)
export type TargetedSiteAgentToolkitScope =
  typeof TargetedSiteAgentToolkitScope.Type

export const TargetedSiteAgentToolkitApprovalPolicy = S.Literals([
  'operator_approval',
  'owner_approval',
  'auto_dry_run_only',
])
export type TargetedSiteAgentToolkitApprovalPolicy =
  typeof TargetedSiteAgentToolkitApprovalPolicy.Type

export const TargetedSiteAgentToolkitGrantStatus = S.Literals([
  'active',
  'revoked',
  'expired',
])
export type TargetedSiteAgentToolkitGrantStatus =
  typeof TargetedSiteAgentToolkitGrantStatus.Type

export const TargetedSiteAgentToolkitActionKind = S.Literals([
  'discover_prospects',
  'capture_site',
  'audit_site',
  'generate_preview',
  'send_outreach_request',
  'record_metric',
  'propose_reward',
])
export type TargetedSiteAgentToolkitActionKind =
  typeof TargetedSiteAgentToolkitActionKind.Type

export const TargetedSiteAgentToolkitSuppressionState = S.Literals([
  'unknown',
  'clear',
  'suppressed',
  'manual_review',
])
export type TargetedSiteAgentToolkitSuppressionState =
  typeof TargetedSiteAgentToolkitSuppressionState.Type

export const TargetedSiteAgentToolkitApprovalState = S.Literals([
  'not_required',
  'requested',
  'approved',
  'rejected',
])
export type TargetedSiteAgentToolkitApprovalState =
  typeof TargetedSiteAgentToolkitApprovalState.Type

export const TargetedSiteAgentToolkitActionResultState = S.Literals([
  'accepted',
  'blocked',
  'rejected',
])
export type TargetedSiteAgentToolkitActionResultState =
  typeof TargetedSiteAgentToolkitActionResultState.Type

const ACTION_SCOPE_BY_KIND: Readonly<
  Record<TargetedSiteAgentToolkitActionKind, TargetedSiteAgentToolkitScope>
> = {
  audit_site: 'campaign:audit',
  capture_site: 'campaign:capture',
  discover_prospects: 'campaign:discover',
  generate_preview: 'campaign:preview',
  propose_reward: 'campaign:reward:propose',
  record_metric: 'campaign:metric:record',
  send_outreach_request: 'campaign:outreach:request',
}

export const TargetedSiteAgentToolkitGrantRecord = S.Struct({
  agentRef: S.String,
  approvalPolicy: TargetedSiteAgentToolkitApprovalPolicy,
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  createdAt: S.String,
  dailySendCap: S.Number,
  dryRunDefault: S.Boolean,
  expiresAt: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  ownerUserId: S.String,
  revokedAt: S.NullOr(S.String),
  scopes: S.Array(TargetedSiteAgentToolkitScope),
  spendCapCents: S.Number,
  status: TargetedSiteAgentToolkitGrantStatus,
  suppressionPolicyRef: S.NullOr(S.String),
  updatedAt: S.String,
})
export type TargetedSiteAgentToolkitGrantRecord =
  typeof TargetedSiteAgentToolkitGrantRecord.Type

export const TargetedSiteAgentToolkitActionRecord = S.Struct({
  actionKind: TargetedSiteAgentToolkitActionKind,
  agentRef: S.String,
  approvalState: TargetedSiteAgentToolkitApprovalState,
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  createdAt: S.String,
  dryRun: S.Boolean,
  grantId: S.String,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  reason: S.NullOr(S.String),
  receiptRef: S.String,
  requestedCostCents: S.Number,
  requestedSendCount: S.Number,
  resultState: TargetedSiteAgentToolkitActionResultState,
  suppressionState: TargetedSiteAgentToolkitSuppressionState,
})
export type TargetedSiteAgentToolkitActionRecord =
  typeof TargetedSiteAgentToolkitActionRecord.Type

export type TargetedSiteAgentToolkitRuntime = Readonly<{
  makeActionId: () => string
  makeGrantId: () => string
  nowIso: () => string
}>

export const systemTargetedSiteAgentToolkitRuntime: TargetedSiteAgentToolkitRuntime =
  {
    makeActionId: () => compactRandomId('targeted_site_agent_action'),
    makeGrantId: () => compactRandomId('targeted_site_agent_grant'),
    nowIso: currentIsoTimestamp,
  }

export type CreateTargetedSiteAgentToolkitGrantInput = Readonly<{
  agentRef: string
  approvalPolicy?: TargetedSiteAgentToolkitApprovalPolicy | undefined
  campaignId: string
  dailySendCap?: number | undefined
  dryRunDefault?: boolean | undefined
  expiresAt?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  isAdmin?: boolean | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  ownerUserId: string
  scopes: ReadonlyArray<TargetedSiteAgentToolkitScope>
  spendCapCents?: number | undefined
  suppressionPolicyRef?: string | undefined
}>

export type RecordTargetedSiteAgentToolkitActionInput = Readonly<{
  actionKind: TargetedSiteAgentToolkitActionKind
  approvalState?: TargetedSiteAgentToolkitApprovalState | undefined
  dryRun?: boolean | undefined
  grantId: string
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  receiptRef?: string | undefined
  requestedCostCents?: number | undefined
  requestedSendCount?: number | undefined
  suppressionState?: TargetedSiteAgentToolkitSuppressionState | undefined
}>

type CampaignOwnerRow = Readonly<{
  id: string
  owner_user_id: string
}>

type GrantRow = Readonly<{
  agent_ref: string
  approval_policy: TargetedSiteAgentToolkitApprovalPolicy
  archived_at: string | null
  campaign_id: string
  created_at: string
  daily_send_cap: number
  dry_run_default: number
  expires_at: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  owner_user_id: string
  revoked_at: string | null
  scopes_json: string
  spend_cap_cents: number
  status: TargetedSiteAgentToolkitGrantStatus
  suppression_policy_ref: string | null
  updated_at: string
}>

const isTargetedSiteAgentToolkitScope = (
  value: string,
): value is TargetedSiteAgentToolkitScope =>
  TARGETED_SITE_AGENT_TOOLKIT_SCOPES.includes(
    value as TargetedSiteAgentToolkitScope,
  )

type ActionRow = Readonly<{
  action_kind: TargetedSiteAgentToolkitActionKind
  agent_ref: string
  approval_state: TargetedSiteAgentToolkitApprovalState
  archived_at: string | null
  campaign_id: string
  created_at: string
  dry_run: number
  grant_id: string
  id: string
  idempotency_key: string
  metadata_json: string
  reason: string | null
  receipt_ref: string
  requested_cost_cents: number
  requested_send_count: number
  result_state: TargetedSiteAgentToolkitActionResultState
  suppression_state: TargetedSiteAgentToolkitSuppressionState
}>

export class TargetedSiteAgentToolkitValidationError extends S.TaggedErrorClass<TargetedSiteAgentToolkitValidationError>()(
  'TargetedSiteAgentToolkitValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteAgentToolkitStorageError extends S.TaggedErrorClass<TargetedSiteAgentToolkitStorageError>()(
  'TargetedSiteAgentToolkitStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class TargetedSiteAgentToolkitCampaignNotFound extends S.TaggedErrorClass<TargetedSiteAgentToolkitCampaignNotFound>()(
  'TargetedSiteAgentToolkitCampaignNotFound',
  {
    campaignId: S.String,
  },
) {}

export class TargetedSiteAgentToolkitForbidden extends S.TaggedErrorClass<TargetedSiteAgentToolkitForbidden>()(
  'TargetedSiteAgentToolkitForbidden',
  {
    campaignId: S.String,
  },
) {}

export class TargetedSiteAgentToolkitGrantNotFound extends S.TaggedErrorClass<TargetedSiteAgentToolkitGrantNotFound>()(
  'TargetedSiteAgentToolkitGrantNotFound',
  {
    grantId: S.String,
  },
) {}

export type TargetedSiteAgentToolkitError =
  | TargetedSiteAgentToolkitCampaignNotFound
  | TargetedSiteAgentToolkitForbidden
  | TargetedSiteAgentToolkitGrantNotFound
  | TargetedSiteAgentToolkitStorageError
  | TargetedSiteAgentToolkitValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)\b|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteAgentToolkitValidationError({
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

  if (containsProviderSecretMaterial(json) || PROHIBITED_TEXT_PATTERN.test(json)) {
    throw new TargetedSiteAgentToolkitValidationError({
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
    throw new TargetedSiteAgentToolkitValidationError({
      reason: `${field} must be a non-negative integer.`,
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): TargetedSiteAgentToolkitStorageError =>
  new TargetedSiteAgentToolkitStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TargetedSiteAgentToolkitStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const grantFromRow = (row: GrantRow): TargetedSiteAgentToolkitGrantRecord => ({
  agentRef: row.agent_ref,
  approvalPolicy: row.approval_policy,
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  dailySendCap: row.daily_send_cap,
  dryRunDefault: row.dry_run_default === 1,
  expiresAt: row.expires_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  ownerUserId: row.owner_user_id,
  revokedAt: row.revoked_at,
  scopes: parseJsonStringArray(row.scopes_json).filter(
    isTargetedSiteAgentToolkitScope,
  ),
  spendCapCents: row.spend_cap_cents,
  status: row.status,
  suppressionPolicyRef: row.suppression_policy_ref,
  updatedAt: row.updated_at,
})

const actionFromRow = (
  row: ActionRow,
): TargetedSiteAgentToolkitActionRecord => ({
  actionKind: row.action_kind,
  agentRef: row.agent_ref,
  approvalState: row.approval_state,
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  dryRun: row.dry_run === 1,
  grantId: row.grant_id,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  reason: row.reason,
  receiptRef: row.receipt_ref,
  requestedCostCents: row.requested_cost_cents,
  requestedSendCount: row.requested_send_count,
  resultState: row.result_state,
  suppressionState: row.suppression_state,
})

const readCampaignOwner = (
  db: D1Database,
  campaignId: string,
): Effect.Effect<CampaignOwnerRow | null, TargetedSiteAgentToolkitStorageError> =>
  d1Effect('targetedSiteAgentToolkit.campaignOwner', () =>
    db
      .prepare(
        `SELECT id, owner_user_id
           FROM targeted_site_campaigns
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(campaignId)
      .first<CampaignOwnerRow>(),
  )

const readGrantByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  TargetedSiteAgentToolkitGrantRecord | null,
  TargetedSiteAgentToolkitStorageError
> =>
  d1Effect('targetedSiteAgentToolkit.grantByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM targeted_site_agent_toolkit_grants
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<GrantRow>(),
  ).pipe(Effect.map(row => (row === null ? null : grantFromRow(row))))

const readActiveGrant = (
  db: D1Database,
  grantId: string,
): Effect.Effect<
  TargetedSiteAgentToolkitGrantRecord | null,
  TargetedSiteAgentToolkitStorageError
> =>
  d1Effect('targetedSiteAgentToolkit.activeGrant', () =>
    db
      .prepare(
        `SELECT *
           FROM targeted_site_agent_toolkit_grants
          WHERE id = ?
            AND status = 'active'
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(grantId)
      .first<GrantRow>(),
  ).pipe(Effect.map(row => (row === null ? null : grantFromRow(row))))

const readActionByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  TargetedSiteAgentToolkitActionRecord | null,
  TargetedSiteAgentToolkitStorageError
> =>
  d1Effect('targetedSiteAgentToolkit.actionByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM targeted_site_agent_toolkit_actions
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ActionRow>(),
  ).pipe(Effect.map(row => (row === null ? null : actionFromRow(row))))

const usedSendCountToday = (
  db: D1Database,
  grantId: string,
  todayPrefix: string,
): Effect.Effect<number, TargetedSiteAgentToolkitStorageError> =>
  d1Effect('targetedSiteAgentToolkit.usedSendCountToday', () =>
    db
      .prepare(
        `SELECT COALESCE(SUM(requested_send_count), 0) AS used_send_count
           FROM targeted_site_agent_toolkit_actions
          WHERE grant_id = ?
            AND result_state = 'accepted'
            AND created_at >= ?
            AND archived_at IS NULL`,
      )
      .bind(grantId, `${todayPrefix}T00:00:00.000Z`)
      .first<{ used_send_count: number }>(),
  ).pipe(Effect.map(row => row?.used_send_count ?? 0))

const requiredScopeForAction = (
  actionKind: TargetedSiteAgentToolkitActionKind,
): TargetedSiteAgentToolkitScope => ACTION_SCOPE_BY_KIND[actionKind]

const actionReceiptRef = (
  actionKind: TargetedSiteAgentToolkitActionKind,
  idempotencyKey: string,
): string => `targeted_site_agent_toolkit:${actionKind}:${idempotencyKey}`

export const createTargetedSiteAgentToolkitGrant = (
  db: D1Database,
  input: CreateTargetedSiteAgentToolkitGrantInput,
  runtime: TargetedSiteAgentToolkitRuntime =
    systemTargetedSiteAgentToolkitRuntime,
): Effect.Effect<TargetedSiteAgentToolkitGrantRecord, TargetedSiteAgentToolkitError> =>
  Effect.gen(function* () {
    assertSafeRef('id', input.id)
    assertSafeRef('idempotencyKey', input.idempotencyKey)
    assertSafeRef('campaignId', input.campaignId)
    assertSafeRef('ownerUserId', input.ownerUserId)
    assertSafeRef('agentRef', input.agentRef)
    assertSafeRef('suppressionPolicyRef', input.suppressionPolicyRef)
    assertSafeMetadata(input.metadata)
    assertNonNegativeInteger('spendCapCents', input.spendCapCents)
    assertNonNegativeInteger('dailySendCap', input.dailySendCap)

    if (input.scopes.length === 0) {
      return yield* new TargetedSiteAgentToolkitValidationError({
        reason: 'scopes must include at least one campaign tool scope.',
      })
    }

    const existing = yield* readGrantByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const campaign = yield* readCampaignOwner(db, input.campaignId)

    if (campaign === null) {
      return yield* new TargetedSiteAgentToolkitCampaignNotFound({
        campaignId: input.campaignId,
      })
    }

    if (campaign.owner_user_id !== input.ownerUserId && input.isAdmin !== true) {
      return yield* new TargetedSiteAgentToolkitForbidden({
        campaignId: input.campaignId,
      })
    }

    const now = runtime.nowIso()
    const record: TargetedSiteAgentToolkitGrantRecord = {
      agentRef: input.agentRef,
      approvalPolicy: input.approvalPolicy ?? 'auto_dry_run_only',
      archivedAt: null,
      campaignId: input.campaignId,
      createdAt: now,
      dailySendCap: input.dailySendCap ?? 0,
      dryRunDefault: input.dryRunDefault ?? true,
      expiresAt: input.expiresAt ?? null,
      id: input.id ?? runtime.makeGrantId(),
      idempotencyKey: input.idempotencyKey,
      metadata: input.metadata ?? {},
      ownerUserId: campaign.owner_user_id,
      revokedAt: null,
      scopes: [...input.scopes],
      spendCapCents: input.spendCapCents ?? 0,
      status: 'active',
      suppressionPolicyRef: input.suppressionPolicyRef ?? null,
      updatedAt: now,
    }

    yield* d1Effect('targetedSiteAgentToolkit.grants.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO targeted_site_agent_toolkit_grants
             (id,
              idempotency_key,
              campaign_id,
              owner_user_id,
              agent_ref,
              scopes_json,
              dry_run_default,
              spend_cap_cents,
              daily_send_cap,
              suppression_policy_ref,
              approval_policy,
              status,
              metadata_json,
              created_at,
              updated_at,
              expires_at,
              revoked_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, NULL, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.campaignId,
          record.ownerUserId,
          record.agentRef,
          JSON.stringify(record.scopes),
          record.dryRunDefault ? 1 : 0,
          record.spendCapCents,
          record.dailySendCap,
          record.suppressionPolicyRef,
          record.approvalPolicy,
          JSON.stringify(record.metadata),
          record.createdAt,
          record.updatedAt,
          record.expiresAt,
        )
        .run()
        .then(() => undefined),
    )

    return (yield* readGrantByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

const actionDecision = (
  grant: TargetedSiteAgentToolkitGrantRecord,
  input: RecordTargetedSiteAgentToolkitActionInput,
  dryRun: boolean,
  usedSendCount: number,
): Readonly<{
  approvalState: TargetedSiteAgentToolkitApprovalState
  reason: string | null
  resultState: TargetedSiteAgentToolkitActionResultState
  suppressionState: TargetedSiteAgentToolkitSuppressionState
}> => {
  const requestedCostCents = input.requestedCostCents ?? 0
  const requestedSendCount = input.requestedSendCount ?? 0
  const suppressionState = input.suppressionState ?? 'clear'
  const approvalState =
    dryRun || grant.approvalPolicy === 'auto_dry_run_only'
      ? 'not_required'
      : (input.approvalState ?? 'requested')
  const requiredScope = requiredScopeForAction(input.actionKind)

  if (!grant.scopes.includes(requiredScope)) {
    return {
      approvalState,
      reason: `missing required scope ${requiredScope}`,
      resultState: 'rejected',
      suppressionState,
    }
  }

  if (grant.approvalPolicy === 'auto_dry_run_only' && !dryRun) {
    return {
      approvalState,
      reason: 'grant only permits dry-run actions',
      resultState: 'blocked',
      suppressionState,
    }
  }

  if (requestedCostCents > grant.spendCapCents) {
    return {
      approvalState,
      reason: 'requested cost exceeds spend cap',
      resultState: 'blocked',
      suppressionState,
    }
  }

  if (usedSendCount + requestedSendCount > grant.dailySendCap) {
    return {
      approvalState,
      reason: 'requested sends exceed daily send cap',
      resultState: 'blocked',
      suppressionState,
    }
  }

  if (suppressionState !== 'clear') {
    return {
      approvalState,
      reason: `suppression state is ${suppressionState}`,
      resultState: 'blocked',
      suppressionState,
    }
  }

  if (
    !dryRun &&
    grant.approvalPolicy !== 'auto_dry_run_only' &&
    approvalState !== 'approved'
  ) {
    return {
      approvalState,
      reason: 'non-dry-run action requires approval',
      resultState: 'blocked',
      suppressionState,
    }
  }

  return {
    approvalState,
    reason: null,
    resultState: 'accepted',
    suppressionState,
  }
}

export const recordTargetedSiteAgentToolkitAction = (
  db: D1Database,
  input: RecordTargetedSiteAgentToolkitActionInput,
  runtime: TargetedSiteAgentToolkitRuntime =
    systemTargetedSiteAgentToolkitRuntime,
): Effect.Effect<TargetedSiteAgentToolkitActionRecord, TargetedSiteAgentToolkitError> =>
  Effect.gen(function* () {
    assertSafeRef('id', input.id)
    assertSafeRef('grantId', input.grantId)
    assertSafeRef('idempotencyKey', input.idempotencyKey)
    assertSafeRef('receiptRef', input.receiptRef)
    assertSafeMetadata(input.metadata)
    assertNonNegativeInteger('requestedCostCents', input.requestedCostCents)
    assertNonNegativeInteger('requestedSendCount', input.requestedSendCount)

    const existing = yield* readActionByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const grant = yield* readActiveGrant(db, input.grantId)

    if (grant === null) {
      return yield* new TargetedSiteAgentToolkitGrantNotFound({
        grantId: input.grantId,
      })
    }

    const now = runtime.nowIso()
    const usedSendCount = yield* usedSendCountToday(
      db,
      grant.id,
      now.slice(0, 10),
    )
    const dryRun = input.dryRun ?? grant.dryRunDefault
    const decision = actionDecision(grant, input, dryRun, usedSendCount)
    const record: TargetedSiteAgentToolkitActionRecord = {
      actionKind: input.actionKind,
      agentRef: grant.agentRef,
      approvalState: decision.approvalState,
      archivedAt: null,
      campaignId: grant.campaignId,
      createdAt: now,
      dryRun,
      grantId: grant.id,
      id: input.id ?? runtime.makeActionId(),
      idempotencyKey: input.idempotencyKey,
      metadata: input.metadata ?? {},
      reason: decision.reason,
      receiptRef:
        input.receiptRef ??
        actionReceiptRef(input.actionKind, input.idempotencyKey),
      requestedCostCents: input.requestedCostCents ?? 0,
      requestedSendCount: input.requestedSendCount ?? 0,
      resultState: decision.resultState,
      suppressionState: decision.suppressionState,
    }

    yield* d1Effect('targetedSiteAgentToolkit.actions.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO targeted_site_agent_toolkit_actions
             (id,
              idempotency_key,
              grant_id,
              campaign_id,
              agent_ref,
              action_kind,
              dry_run,
              requested_cost_cents,
              requested_send_count,
              suppression_state,
              approval_state,
              result_state,
              receipt_ref,
              reason,
              metadata_json,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.grantId,
          record.campaignId,
          record.agentRef,
          record.actionKind,
          record.dryRun ? 1 : 0,
          record.requestedCostCents,
          record.requestedSendCount,
          record.suppressionState,
          record.approvalState,
          record.resultState,
          record.receiptRef,
          record.reason,
          JSON.stringify(record.metadata),
          record.createdAt,
        )
        .run()
        .then(() => undefined),
    )

    return (yield* readActionByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

export const agentToolkitActionContract = (
  grant: TargetedSiteAgentToolkitGrantRecord,
) => ({
  agentRef: grant.agentRef,
  approvalPolicy: grant.approvalPolicy,
  campaignId: grant.campaignId,
  dailySendCap: grant.dailySendCap,
  dryRunDefault: grant.dryRunDefault,
  grantId: grant.id,
  scopes: grant.scopes,
  spendCapCents: grant.spendCapCents,
  status: grant.status,
})

export const publicTargetedSiteAgentToolkitActionProjection = (
  action: TargetedSiteAgentToolkitActionRecord,
) => ({
  actionKind: action.actionKind,
  approvalState: action.approvalState,
  campaignId: action.campaignId,
  createdAt: action.createdAt,
  dryRun: action.dryRun,
  receiptRef: action.receiptRef,
  resultState: action.resultState,
})
