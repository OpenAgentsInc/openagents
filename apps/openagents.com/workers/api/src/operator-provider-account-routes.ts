import { type AuthKvStore, authKvStoreForEnv } from './auth/auth-kv'
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  identityAuthMirrorFromEnv,
  makeProviderAccountRepositoryForEnv,
  type IdentityAuthMirror,
} from './identity-auth-domain-store'
import {
  isRecord,
  optionalBoolean,
  optionalString,
  readJsonObject,
  safeJsonRecord,
} from './json-boundary'
import {
  logWorkerRouteError,
  observedPromise,
  unwrapEffectTryPromiseCause,
} from './observability'
import type { OperatorTargetUser } from './operator-targets'
import {
  pollOpenAiCodexDeviceLogin,
  startOpenAiCodexDeviceLogin,
} from './provider-account-client'
import {
  type DeleteStartedCodexDeviceLogin,
  type PollCodexDeviceLogin,
  type ProviderAccountProvider,
  type ProviderAccountRecord,
  type ProviderAccountRepository,
  type PublicProviderAccount,
  type PublicProviderConnectionAttempt,
  type ReadStartedCodexDeviceLogin,
  type ResolvedProviderAccountGrant,
  type StartCodexDeviceLogin,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
} from './provider-account-domain'
import {
  type ProviderAccountFailoverFailureClass,
  classifyProviderAccountHealthEvent,
} from './provider-account-failover-policy'
import {
  makeProviderAccountLeaseService,
  type ProviderAccountLeaseService,
} from './provider-account-lease-service'
import { PROVIDER_ACCOUNT_LEASE_POLICY_VERSION } from './provider-account-lease-policy'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'
import {
  issueProviderAccountGrant,
  recordProviderAccountHealth,
  refreshChatGptCodexDeviceLoginForUser,
  resolveProviderAccountGrant,
  startChatGptCodexDeviceLogin,
} from './provider-accounts'
import { identityDbForEnv, type IdentityDb } from './identity-db'
import { openAgentsDatabase } from './runtime'
import {
  compactRandomId,
  currentIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

type OperatorProviderAccountEnv = Readonly<{
  AUTH_KV?: AuthKvStore | undefined
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>
type HttpResponse = globalThis.Response

type ProviderAccountProbeCollisionClass =
  | 'none'
  | 'wrong_account_identity'
  | 'auth_material_overwrite'
  | 'grant_account_mismatch'
  | 'lease_isolation_failed'
  | 'hidden_global_lock_detected'

const providerAccountProvider = (
  value: unknown,
): ProviderAccountProvider | undefined =>
  value === 'chatgpt_codex' ||
  value === 'anthropic_claude' ||
  value === 'google_gemini'
    ? value
    : undefined

export type ProviderAccountSanityProbeResult =
  | ProviderAccountSanityClassification
  | Readonly<{
      classification: ProviderAccountSanityClassification
      collisionClass?: ProviderAccountProbeCollisionClass | undefined
      observedProviderAccountRef?: string | undefined
      providerFailureClass?: ProviderAccountFailoverFailureClass | undefined
      providerStatus?: number | undefined
      serializedThroughGlobalLock?: boolean | undefined
    }>

type OperatorDeviceLoginTargetUser = Readonly<{
  userId: string
  displayName: string
  email: string | null
  githubUsername: string | null
}>

type OperatorProviderAccountDependencies<
  Bindings extends OperatorProviderAccountEnv,
> = Readonly<{
  deleteStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => DeleteStartedCodexDeviceLogin
  makeProviderAccountRepository?: (db: D1Database) => ProviderAccountRepository
  pollDeviceLogin?: PollCodexDeviceLogin
  probeResolvedGrant?: (
    input: Readonly<{
      account: ProviderAccountRecord
      authMaterialAvailable: boolean
      grant: ResolvedProviderAccountGrant
      leaseId: string | null
      probeId: string | null
    }>,
  ) => Promise<ProviderAccountSanityProbeResult>
  readConnectedCodexAuthMaterial: (
    env: Bindings,
    ownerUserId: string,
    providerAccountRef: string,
  ) => Promise<unknown | undefined>
  readSelectedOperatorTargetUser: (
    identityDb: IdentityDb,
    selector: Record<string, unknown>,
  ) => Promise<OperatorTargetUser | undefined>
  readStartedCodexDeviceLogin: (kv: AuthKvStore) => ReadStartedCodexDeviceLogin
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  startDeviceLogin?: StartCodexDeviceLogin
  storeConnectedCodexAuth: (env: Bindings) => StoreConnectedCodexAuth
  storeStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => StoreStartedCodexDeviceLogin
}>

export type ProviderAccountSanityClassification =
  | 'healthy'
  | 'requires_reauth'
  | 'low_credit'
  | 'rate_limited'
  | 'quota_exhausted'
  | 'provider_outage'
  | 'grant_resolution_failed'
  | 'launch_probe_failed'
  | 'unknown_failure'

type ProviderAccountSanityCheck = Readonly<{
  providerAccountId: string
  providerAccountRef: string
  accountLabel: string | null
  classification: ProviderAccountSanityClassification
  health: PublicProviderAccount['health']
  status: PublicProviderAccount['status']
  summary: string
  checkedAt: string
  probeId: string | null
  leaseId: string | null
  startedAt: string
  finishedAt: string
  terminalStatus: 'passed' | 'failed'
  collisionClass: ProviderAccountProbeCollisionClass
  failureClass: ProviderAccountFailoverFailureClass | null
}>

type ProviderAccountSanityResponse = Readonly<{
  checks: ReadonlyArray<ProviderAccountSanityCheck>
  summary: Readonly<{
    total: number
    healthy: number
    requiresAttention: number
    collisionCount: number
  }>
  probeRunId: string | null
  parallel: number
}>

type ProviderAccountLeaseResponse = Readonly<{
  leaseId: string
  leaseRef: string
  providerAccountId: string
  providerAccountRef: string
  accountLabel: string | null
  requestedAction: string
  runId: string | null
  assignmentId: string | null
  orderId: string | null
  selectedByPolicyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION
  selectionReason: string
  selectedByActor: string
  activeLeaseCountBeforeSelection: number
  operatorPriority: number
  startedAt: string
  expiresAt: string
  lastTouchedAt: string
  status: 'active'
}>

type ProviderAccountLeaseGrantResponse = Readonly<{
  leaseRef: string
  providerAccountRef: string
  requestedAction: string | null
  runId: string | null
  assignmentId: string | null
  orderId: string | null
  grant: Readonly<{
    grantRef: string
    status: string
    expiresAt: string
    requestedAction?: string | undefined
    runnerSessionId?: string | undefined
    workroomId?: string | undefined
    threadId?: string | undefined
  }>
}>

type ProviderAccountFailoverResponse = Readonly<{
  receiptId: string
  outcome: 'retrying' | 'blocked'
  failureClass: ProviderAccountFailoverFailureClass
  accountStateAction: string
  previousLeaseRef: string | null
  previousProviderAccountRef: string | null
  nextLease: ProviderAccountLeaseResponse | null
  attemptNumber: number
  maxAttempts: number
  customerSafeStatus: string
}>

type ProviderAccountLeaseListItem = Readonly<{
  leaseRef: string
  providerAccountRef: string
  accountLabel: string | null
  requestedAction: string
  runId: string | null
  assignmentId: string | null
  orderId: string | null
  startedAt: string
  expiresAt: string
  lastTouchedAt: string | null
  status: string
}>

type ProviderAccountLeaseExplainResponse = Readonly<{
  status: 'selected' | 'none'
  providerAccountRef: string | null
  accountLabel: string | null
  selectedByPolicyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION
  selectionReason: string
  activeLeaseCount: number | null
  leaseLimit: number | null
  operatorPriority: number | null
}>

type ProviderAccountFleetDashboardAccount = Readonly<{
  providerAccountRef: string
  accountLabel: string | null
  operatorLabel: string | null
  status: string
  health: string
  eligibility: 'eligible' | 'ineligible'
  eligibilityReasons: ReadonlyArray<string>
  operatorPriority: number
  activeLeaseCount: number
  leaseLimit: number
  lastSanityCheckAt: string | null
  lastSanityCheckResult: string | null
  lastParallelProbeAt: string | null
  lastParallelProbeResult: string | null
  lastSelectedAt: string | null
  lastSuccessfulLaunchAt: string | null
  lastFailedLaunchAt: string | null
  recentFailureClass: string | null
  cooldownUntil: string | null
  lowCredit: boolean
  reauthRequiredReason: string | null
  refillNote: string | null
  operatorNote: string | null
  connectedAt: string | null
  reconnectCommand: string
  sanityCommand: string
}>

type ProviderAccountFleetDashboardResponse = Readonly<{
  accounts: ReadonlyArray<ProviderAccountFleetDashboardAccount>
  activeLeases: ReadonlyArray<ProviderAccountLeaseListItem>
  selector: ProviderAccountLeaseExplainResponse
  summary: Readonly<{
    total: number
    eligible: number
    activeLeaseCount: number
    lowCredit: number
    requiresReauth: number
    cooldown: number
    unhealthy: number
  }>
}>

type OperatorProviderAccountResetResponse = Readonly<{
  ok: true
  providerAccountRef: string
  resetAt: string
}>

type OperatorDeviceLoginStartResponse = Readonly<{
  status: 'pending'
  targetUser: OperatorDeviceLoginTargetUser
  attemptId: string
  providerAccountRef: string
  accountLabel: string | null
  verificationUrl: string
  userCode: string
  expiresAt: string
  intervalSeconds: number
  nextPollCommand: string
}>

type OperatorDeviceLoginStatusResponse = Readonly<{
  status: PublicProviderConnectionAttempt['status']
  failureReason: string | null
  attemptId: string
  providerAccountRef: string
  providerAccountStatus: PublicProviderAccount['status']
  providerAccountHealth: PublicProviderAccount['health']
  accountLabel: string | null
  expiresAt: string
  completedAt: string | null
  failedAt: string | null
}>

const targetUserProjection = (
  targetUser: OperatorTargetUser,
): OperatorDeviceLoginTargetUser => ({
  userId: targetUser.userId,
  displayName: targetUser.displayName,
  email: targetUser.email,
  githubUsername: targetUser.githubUsername,
})

const accountLabel = (account: PublicProviderAccount): string | null =>
  account.accountLabel ?? null

const optionalParallelNumber = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const optionalPositiveInteger = (
  value: unknown,
  fallback: number,
  maximum: number,
): number =>
  Math.max(
    1,
    Math.min(
      maximum,
      Math.trunc(
        typeof value === 'number' && Number.isFinite(value) ? value : fallback,
      ),
    ),
  )

const optionalLimit = (value: unknown, fallback: number, maximum: number) =>
  optionalPositiveInteger(value, fallback, maximum)

const optionalFailureClass = (
  value: unknown,
): ProviderAccountFailoverFailureClass | undefined =>
  value === 'token_invalidated' ||
  value === 'low_credits' ||
  value === 'rate_limited' ||
  value === 'quota_exhausted' ||
  value === 'provider_outage' ||
  value === 'launch_timeout' ||
  value === 'grant_resolution_failed' ||
  value === 'runner_failure' ||
  value === 'unknown_provider_failure'
    ? value
    : undefined

const nextPollCommand = (attemptId: string): string =>
  `node scripts/provider-chatgpt-device-login.mjs poll ${attemptId}`

const operatorStartResponse = (
  targetUser: OperatorTargetUser,
  result: Readonly<{
    account: PublicProviderAccount
    attempt: PublicProviderConnectionAttempt
    expiresAt: string
    intervalSeconds: number
    providerAccountRef: string
    verificationUrl: string
    userCode: string
  }>,
): OperatorDeviceLoginStartResponse => ({
  status: 'pending',
  targetUser: targetUserProjection(targetUser),
  attemptId: result.attempt.id,
  providerAccountRef: result.providerAccountRef,
  accountLabel: accountLabel(result.account),
  verificationUrl: result.verificationUrl,
  userCode: result.userCode,
  expiresAt: result.expiresAt,
  intervalSeconds: result.intervalSeconds,
  nextPollCommand: nextPollCommand(result.attempt.id),
})

const failureReason = (
  attempt: PublicProviderConnectionAttempt,
): string | null =>
  attempt.status === 'expired'
    ? 'device_login_expired'
    : attempt.status === 'denied'
      ? 'device_login_denied'
      : attempt.status === 'failed'
        ? 'device_login_failed'
        : null

const operatorStatusResponse = (
  result: Readonly<{
    account: PublicProviderAccount
    attempt: PublicProviderConnectionAttempt
  }>,
): OperatorDeviceLoginStatusResponse => ({
  status: result.attempt.status,
  failureReason: failureReason(result.attempt),
  attemptId: result.attempt.id,
  providerAccountRef: result.account.providerAccountRef,
  providerAccountStatus: result.account.status,
  providerAccountHealth: result.account.health,
  accountLabel: accountLabel(result.account),
  expiresAt: result.attempt.expiresAt,
  completedAt: result.attempt.completedAt ?? null,
  failedAt: result.attempt.failedAt ?? null,
})

const providerHealthForSanity = (
  classification: ProviderAccountSanityClassification,
): 'healthy' | 'unhealthy' | 'requires_reauth' =>
  classification === 'healthy'
    ? 'healthy'
    : classification === 'requires_reauth'
      ? 'requires_reauth'
      : 'unhealthy'

const summaryForClassification = (
  classification: ProviderAccountSanityClassification,
): string =>
  classification === 'healthy'
    ? 'ChatGPT/Codex sanity check passed.'
    : classification === 'requires_reauth'
      ? 'ChatGPT/Codex auth material is unavailable or invalid; reconnect the account.'
      : classification === 'low_credit'
        ? 'ChatGPT/Codex account appears to have low credits.'
        : classification === 'rate_limited'
          ? 'ChatGPT/Codex account is currently rate limited.'
          : classification === 'quota_exhausted'
            ? 'ChatGPT/Codex account quota appears exhausted.'
            : classification === 'provider_outage'
              ? 'ChatGPT/Codex provider appears unavailable.'
              : classification === 'grant_resolution_failed'
                ? 'OpenAgents could not resolve a scoped provider-account grant.'
                : classification === 'launch_probe_failed'
                  ? 'Minimal launch/auth probe failed.'
                  : 'ChatGPT/Codex sanity check failed for an unknown reason.'

const sanityClassificationFromHealthClassification = (
  classification: ReturnType<
    typeof classifyProviderAccountHealthEvent
  >['classification'],
): ProviderAccountSanityClassification =>
  classification === 'healthy'
    ? 'healthy'
    : classification === 'token_invalidated' ||
        classification === 'requires_reauth'
      ? 'requires_reauth'
      : classification === 'low_credits'
        ? 'low_credit'
        : classification === 'rate_limited'
          ? 'rate_limited'
          : classification === 'quota_exhausted'
            ? 'quota_exhausted'
            : classification === 'provider_outage'
              ? 'provider_outage'
              : classification === 'grant_resolution_failed'
                ? 'grant_resolution_failed'
                : classification === 'launch_timeout'
                  ? 'launch_probe_failed'
                  : 'unknown_failure'

const normalizeProbeResult = (
  result: ProviderAccountSanityProbeResult,
  expectedProviderAccountRef: string,
): Readonly<{
  classification: ProviderAccountSanityClassification
  collisionClass: ProviderAccountProbeCollisionClass
  failureClass: ProviderAccountFailoverFailureClass | null
}> => {
  if (typeof result === 'string') {
    return {
      classification: result,
      collisionClass: 'none',
      failureClass: null,
    }
  }

  const collisionClass =
    result.collisionClass ??
    (result.serializedThroughGlobalLock === true
      ? 'hidden_global_lock_detected'
      : result.observedProviderAccountRef !== undefined &&
          result.observedProviderAccountRef !== expectedProviderAccountRef
        ? 'wrong_account_identity'
        : 'none')
  const failureClass =
    result.providerFailureClass ??
    (result.providerStatus !== undefined && result.providerStatus >= 500
      ? 'provider_outage'
      : undefined)
  const healthEvent =
    failureClass === undefined && collisionClass === 'none'
      ? undefined
      : classifyProviderAccountHealthEvent(
          {
            ...(failureClass === undefined ? {} : { code: failureClass }),
            ...(collisionClass === 'none' ? {} : { collisionClass }),
            ...(result.providerStatus === undefined
              ? {}
              : { providerStatus: result.providerStatus }),
          },
          currentIsoTimestamp(),
        )

  return {
    classification:
      healthEvent === undefined
        ? result.classification
        : sanityClassificationFromHealthClassification(
            healthEvent.classification,
          ),
    collisionClass,
    failureClass: failureClass ?? null,
  }
}

const authMaterialContentJson = (value: unknown): string | undefined =>
  isRecord(value) ? optionalString(value.authContentJson) : undefined

const hasShortLivedCodexAccessMaterial = (value: unknown): boolean => {
  const parsed = safeJsonRecord(authMaterialContentJson(value))
  const openai = isRecord(parsed?.openai) ? parsed.openai : undefined

  if (openai === undefined) {
    return false
  }

  const access = optionalString(openai?.access)

  return optionalString(openai?.type) === 'oauth' && access !== undefined
}

const defaultResolvedGrantProbe = async <
  Bindings extends OperatorProviderAccountEnv,
>(
  env: Bindings,
  dependencies: OperatorProviderAccountDependencies<Bindings>,
  input: Readonly<{
    account: ProviderAccountRecord
    authMaterial: unknown
  }>,
): Promise<ProviderAccountSanityProbeResult> => {
  if (!hasShortLivedCodexAccessMaterial(input.authMaterial)) {
    return {
      classification: 'requires_reauth',
      providerFailureClass: 'token_invalidated',
    }
  }

  return 'healthy'
}

const recordSanityCheck = async (
  db: D1Database,
  input: Readonly<{
    account: ProviderAccountRecord
    classification: ProviderAccountSanityClassification
    failureClass: ProviderAccountFailoverFailureClass | null
    grantRef: string | null
    summary: string
    checkedAt: string
  }>,
  // KS-8.18 follow-up (#8362): fail-soft identity/auth mirror handle.
  mirror?: IdentityAuthMirror | undefined,
): Promise<void> => {
  const sanityCheckId = compactRandomId('provider_sanity_check')
  await db.batch([
    db
      .prepare(
        `INSERT INTO provider_account_sanity_checks
          (id,
           provider_account_id,
           user_id,
           team_id,
           provider,
           provider_account_ref,
           classification,
           summary,
           grant_ref,
           created_at,
           metadata_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        sanityCheckId,
        input.account.id,
        input.account.userId,
        input.account.teamId,
        input.account.provider,
        input.account.providerAccountRef,
        input.classification,
        input.summary,
        input.grantRef,
        input.checkedAt,
        JSON.stringify({
          classification: input.classification,
          failureClass: input.failureClass,
          providerAccountRef: input.account.providerAccountRef,
          source: 'operator_sanity_check',
        }),
      ),
    db
      .prepare(
        `UPDATE provider_accounts
            SET last_sanity_check_at = ?,
                last_sanity_check_result = ?,
                updated_at = ?
          WHERE id = ?`,
      )
      .bind(
        input.checkedAt,
        input.classification,
        input.checkedAt,
        input.account.id,
      ),
  ])

  if (mirror !== undefined) {
    await mirror.mirrorRowsByKey('provider_account_sanity_checks', [
      [sanityCheckId],
    ])
    await mirror.mirrorRowsByKey('provider_accounts', [[input.account.id]])
  }
}

const recordParallelProbeReceipt = async (
  db: D1Database,
  input: Readonly<{
    account: ProviderAccountRecord
    check: ProviderAccountSanityCheck
    probeRunId: string
  }>,
  // KS-8.18 follow-up (#8362): fail-soft identity/auth mirror handle.
  mirror?: IdentityAuthMirror | undefined,
): Promise<void> => {
  const receiptId = compactRandomId('provider_parallel_probe_receipt')
  await db.batch([
    db
      .prepare(
        `INSERT INTO provider_account_parallel_probe_receipts
        (id,
         probe_run_id,
         probe_id,
         lease_id,
         provider_account_id,
         user_id,
         team_id,
         provider_account_ref,
         started_at,
         finished_at,
         terminal_status,
         classification,
         collision_class,
         metadata_json)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        receiptId,
        input.probeRunId,
        input.check.probeId,
        input.check.leaseId,
        input.account.id,
        input.account.userId,
        input.account.teamId,
        input.account.providerAccountRef,
        input.check.startedAt,
        input.check.finishedAt,
        input.check.terminalStatus,
        input.check.classification,
        input.check.collisionClass,
        JSON.stringify({
          classification: input.check.classification,
          collisionClass: input.check.collisionClass,
          providerAccountRef: input.account.providerAccountRef,
          source: 'operator_parallel_sanity_probe',
        }),
      ),
    db
      .prepare(
        `UPDATE provider_accounts
            SET last_parallel_probe_at = ?,
                last_parallel_probe_result = ?,
                updated_at = ?
          WHERE id = ?`,
      )
      .bind(
        input.check.finishedAt,
        input.check.collisionClass === 'none'
          ? input.check.classification
          : input.check.collisionClass,
        input.check.finishedAt,
        input.account.id,
      ),
  ])

  if (mirror !== undefined) {
    await mirror.mirrorRowsByKey('provider_account_parallel_probe_receipts', [
      [receiptId],
    ])
    await mirror.mirrorRowsByKey('provider_accounts', [[input.account.id]])
  }
}

const allSummary = (
  checks: ReadonlyArray<ProviderAccountSanityCheck>,
): ProviderAccountSanityResponse['summary'] => ({
  total: checks.length,
  healthy: checks.filter(check => check.classification === 'healthy').length,
  requiresAttention: checks.filter(check => check.classification !== 'healthy')
    .length,
  collisionCount: checks.filter(check => check.collisionClass !== 'none')
    .length,
})

const clampParallelism = (value: number | undefined): number =>
  Math.max(1, Math.min(5, Math.trunc(value ?? 1)))

const clampLeaseTtlSeconds = (value: number | undefined): number =>
  Math.max(60, Math.min(3_600, Math.trunc(value ?? 900)))

// Bounded-concurrency fan-out over independent operator bulk-action items
// (e.g. per-provider-account sanity checks). Uses Effect structured
// concurrency (`Effect.forEach` with a fixed concurrency) instead of a bare
// chunked `Promise.all`: one item's uncaught failure no longer rejects its
// whole chunk and aborts every chunk not yet reached. Each item's outcome is
// isolated with `Effect.result`; a failure is logged and dropped from the
// result set rather than discarding already-succeeded results (in the same
// chunk or earlier chunks) and skipping every account never reached.
const mapWithConcurrency = async <Input, Output>(
  values: ReadonlyArray<Input>,
  concurrency: number,
  mapper: (value: Input) => Promise<Output>,
): Promise<Array<Output>> => {
  const outcomes = await Effect.runPromise(
    Effect.forEach(
      values,
      value => Effect.result(Effect.tryPromise(() => mapper(value))),
      { concurrency },
    ),
  )

  const results: Array<Output> = []

  for (const outcome of outcomes) {
    if (outcome._tag === 'Success') {
      results.push(outcome.success)
      continue
    }

    logWorkerRouteError(
      'operator_bulk_action_item_failed',
      unwrapEffectTryPromiseCause(outcome.failure),
      {},
    )
  }

  return results
}

const fleetEligibilityReasons = (
  row: Readonly<{
    status: string
    health: string
    active_lease_count: number
    lease_limit: number
    low_credit_flag: number
    cooldown_until: string | null
    reauth_required_reason: string | null
    deleted_at: string | null
    has_secret_ref: number
  }>,
  now: string,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []

  if (row.deleted_at !== null) {
    reasons.push('deleted')
  }
  if (row.status !== 'connected') {
    reasons.push(`status:${row.status}`)
  }
  if (row.health !== 'healthy') {
    reasons.push(`health:${row.health}`)
  }
  if (row.has_secret_ref === 0) {
    reasons.push('missing_server_auth_material')
  }
  if (row.low_credit_flag !== 0) {
    reasons.push('low_credit')
  }
  if (row.reauth_required_reason !== null) {
    reasons.push(`reauth_required:${row.reauth_required_reason}`)
  }
  if (row.cooldown_until !== null && row.cooldown_until > now) {
    reasons.push('cooldown')
  }
  if (row.active_lease_count >= row.lease_limit) {
    reasons.push('lease_limit_reached')
  }

  return reasons
}

const reconnectCommandFor = (providerAccountRef: string): string =>
  `node scripts/provider-chatgpt-device-login.mjs start --providerAccountRef ${providerAccountRef}`

const sanityCommandFor = (providerAccountRef: string): string =>
  `node scripts/provider-chatgpt-device-login.mjs sanity ${providerAccountRef}`

const providerAccountFleetDashboard = async (
  db: D1Database,
  leaseService: ProviderAccountLeaseService,
  userId: string,
  now: string,
): Promise<ProviderAccountFleetDashboardResponse> => {
  const [accountRows, activeLeases, selector] = await Promise.all([
    db
      .prepare(
        `SELECT pa.provider_account_ref,
                pa.account_label,
                pa.operator_label,
                pa.status,
                pa.health,
                pa.operator_priority,
                COALESCE(pa.lease_limit, 1) AS lease_limit,
                COALESCE(pa.low_credit_flag, 0) AS low_credit_flag,
                pa.cooldown_until,
                pa.recent_failure_class,
                pa.last_sanity_check_at,
                pa.last_sanity_check_result,
                pa.last_parallel_probe_at,
                pa.last_parallel_probe_result,
                pa.last_selected_at,
                pa.last_successful_launch_at,
                pa.last_failed_launch_at,
                pa.reauth_required_reason,
                pa.refill_note,
                pa.operator_note,
                pa.connected_at,
                pa.deleted_at,
                CASE WHEN pa.secret_ref IS NULL THEN 0 ELSE 1 END AS has_secret_ref,
                COUNT(active_leases.id) AS active_lease_count
           FROM provider_accounts pa
           LEFT JOIN provider_account_leases active_leases
             ON active_leases.provider_account_id = pa.id
            AND active_leases.status = 'active'
            AND active_leases.expires_at > ?
          WHERE pa.user_id = ?
            AND pa.provider = 'chatgpt_codex'
          GROUP BY pa.id
          ORDER BY
            CASE WHEN pa.status = 'connected' AND pa.health = 'healthy' THEN 0 ELSE 1 END,
            COALESCE(pa.low_credit_flag, 0) ASC,
            pa.operator_priority ASC,
            COALESCE(pa.operator_label, pa.account_label, pa.provider_account_ref) ASC
          LIMIT 200`,
      )
      .bind(now, userId)
      .all<
        Readonly<{
          provider_account_ref: string
          account_label: string | null
          operator_label: string | null
          status: string
          health: string
          operator_priority: number
          lease_limit: number
          low_credit_flag: number
          cooldown_until: string | null
          recent_failure_class: string | null
          last_sanity_check_at: string | null
          last_sanity_check_result: string | null
          last_parallel_probe_at: string | null
          last_parallel_probe_result: string | null
          last_selected_at: string | null
          last_successful_launch_at: string | null
          last_failed_launch_at: string | null
          reauth_required_reason: string | null
          refill_note: string | null
          operator_note: string | null
          connected_at: string | null
          deleted_at: string | null
          has_secret_ref: number
          active_lease_count: number
        }>
      >(),
    leaseService.listActive(userId, now),
    leaseService.explainSelection(userId, now),
  ])

  const accounts = accountRows.results.map(row => {
    const eligibilityReasons = fleetEligibilityReasons(row, now)

    return {
      providerAccountRef: row.provider_account_ref,
      accountLabel: row.account_label,
      operatorLabel: row.operator_label,
      status: row.status,
      health: row.health,
      eligibility: eligibilityReasons.length === 0 ? 'eligible' : 'ineligible',
      eligibilityReasons,
      operatorPriority: row.operator_priority,
      activeLeaseCount: row.active_lease_count,
      leaseLimit: row.lease_limit,
      lastSanityCheckAt: row.last_sanity_check_at,
      lastSanityCheckResult: row.last_sanity_check_result,
      lastParallelProbeAt: row.last_parallel_probe_at,
      lastParallelProbeResult: row.last_parallel_probe_result,
      lastSelectedAt: row.last_selected_at,
      lastSuccessfulLaunchAt: row.last_successful_launch_at,
      lastFailedLaunchAt: row.last_failed_launch_at,
      recentFailureClass: row.recent_failure_class,
      cooldownUntil: row.cooldown_until,
      lowCredit: row.low_credit_flag !== 0,
      reauthRequiredReason: row.reauth_required_reason,
      refillNote: row.refill_note,
      operatorNote: row.operator_note,
      connectedAt: row.connected_at,
      reconnectCommand: reconnectCommandFor(row.provider_account_ref),
      sanityCommand: sanityCommandFor(row.provider_account_ref),
    } satisfies ProviderAccountFleetDashboardAccount
  })

  return {
    accounts,
    activeLeases,
    selector,
    summary: {
      total: accounts.length,
      eligible: accounts.filter(account => account.eligibility === 'eligible')
        .length,
      activeLeaseCount: activeLeases.length,
      lowCredit: accounts.filter(account => account.lowCredit).length,
      requiresReauth: accounts.filter(
        account =>
          account.health === 'requires_reauth' ||
          account.eligibilityReasons.includes('health:requires_reauth') ||
          account.eligibilityReasons.some(reason =>
            reason.startsWith('reauth_required:'),
          ),
      ).length,
      cooldown: accounts.filter(account =>
        account.eligibilityReasons.includes('cooldown'),
      ).length,
      unhealthy: accounts.filter(account => account.health === 'unhealthy')
        .length,
    },
  }
}

const resetOperatorProviderAccount = async (
  db: D1Database,
  input: Readonly<{
    providerAccountRef: string
    resetAt: string
    userId: string
  }>,
  // KS-8.18 follow-up (#8362): fail-soft identity/auth mirror handle.
  mirror?: IdentityAuthMirror | undefined,
): Promise<boolean> => {
  const result = await db
    .prepare(
      `UPDATE provider_accounts
          SET health = CASE
                WHEN status = 'connected'
                 AND reauth_required_reason IS NULL
                THEN 'healthy'
                ELSE health
              END,
              low_credit_flag = 0,
              cooldown_until = NULL,
              recent_failure_class = NULL,
              refill_note = NULL,
              last_status_at = ?,
              updated_at = ?
        WHERE user_id = ?
          AND provider_account_ref = ?
          AND deleted_at IS NULL`,
    )
    .bind(input.resetAt, input.resetAt, input.userId, input.providerAccountRef)
    .run()

  const changed = (result.meta?.changes ?? 0) > 0
  if (changed && mirror !== undefined) {
    // No `id` in scope — the WHERE clause resolves by (user_id,
    // provider_account_ref), so scan-mirror on that composite predicate
    // (neither column is custody-bearing).
    await mirror.mirrorRowsWhere(
      'provider_accounts',
      ['user_id', 'provider_account_ref'],
      [input.userId, input.providerAccountRef],
    )
  }
  return changed
}

 // KS-8.18 follow-up (#8362): default to the identity-auth-mirrored
// factory so operator-driven provider-account writes (health, grants,
// events) converge to Postgres; the injectable override stays available
// for tests that pass their own in-memory/fake repository.
const repositoryFor = <Bindings extends OperatorProviderAccountEnv>(
  env: Bindings,
  dependencies: OperatorProviderAccountDependencies<Bindings>,
): ProviderAccountRepository =>
  dependencies.makeProviderAccountRepository !== undefined
    ? dependencies.makeProviderAccountRepository(openAgentsDatabase(env))
    : makeProviderAccountRepositoryForEnv(env)

const leaseServiceFor = <Bindings extends OperatorProviderAccountEnv>(
  env: Bindings,
): ProviderAccountLeaseService =>
  makeProviderAccountLeaseService({
    db: openAgentsDatabase(env),
    mirror: identityAuthMirrorFromEnv(env),
  })

export const makeOperatorProviderAccountRoutes = <
  Bindings extends OperatorProviderAccountEnv,
>(
  dependencies: OperatorProviderAccountDependencies<Bindings>,
) => {
  const rejectUnlessOperatorAdminRoute = async (
    request: Request,
    env: Bindings,
    allowedMethod: string,
  ): Promise<HttpResponse | undefined> => {
    if (request.method !== allowedMethod) {
      return methodNotAllowed([allowedMethod])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    return undefined
  }

  const startDeviceLogin = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    try {
      const result = await observedPromise(
        'OperatorProviderAccount.startChatGptCodexDeviceLogin',
        () =>
          startChatGptCodexDeviceLogin(
            repositoryFor(env, dependencies),
            {
              userId: targetUser.userId,
              ...(optionalString(body.accountLabel) === undefined
                ? {}
                : { accountLabel: optionalString(body.accountLabel) }),
              ...(optionalBoolean(body.createNew) === undefined
                ? {}
                : { createNew: optionalBoolean(body.createNew) }),
              ...(optionalString(body.providerAccountRef) === undefined
                ? {}
                : {
                    providerAccountRef: optionalString(body.providerAccountRef),
                  }),
            },
            dependencies.startDeviceLogin ?? startOpenAiCodexDeviceLogin,
            {
              storeStartedDeviceLogin:
                dependencies.storeStartedCodexDeviceLogin(authKvStoreForEnv(env)),
            },
          ),
      )

      return noStoreJsonResponse(operatorStartResponse(targetUser, result), {
        status: 201,
      })
    } catch (error) {
      logWorkerRouteError(
        'operator_provider_device_login_start_failed',
        error,
        {
          errorName: providerAccountRouteErrorName(error),
          targetUserId: targetUser.userId,
        },
      )

      return noStoreJsonResponse(
        {
          error: 'operator_provider_device_login_start_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 502) },
      )
    }
  }

  const pollDeviceLogin = async (
    request: Request,
    env: Bindings,
    attemptId: string,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'GET')

    if (rejected !== undefined) {
      return rejected
    }

    const repository = repositoryFor(env, dependencies)
    const record = await repository.findAttemptById(attemptId)

    if (record === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    try {
      const result = await observedPromise(
        'OperatorProviderAccount.refreshChatGptCodexDeviceLoginForUser',
        () =>
          refreshChatGptCodexDeviceLoginForUser(
            repository,
            {
              attemptId,
              userId: record.attempt.userId,
            },
            dependencies.readStartedCodexDeviceLogin(authKvStoreForEnv(env)),
            dependencies.storeConnectedCodexAuth(env),
            dependencies.pollDeviceLogin ?? pollOpenAiCodexDeviceLogin,
            dependencies.deleteStartedCodexDeviceLogin(authKvStoreForEnv(env)),
          ),
      )

      if (result === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return noStoreJsonResponse(operatorStatusResponse(result))
    } catch (error) {
      logWorkerRouteError('operator_provider_device_login_poll_failed', error, {
        attemptId,
        errorName: providerAccountRouteErrorName(error),
      })

      return noStoreJsonResponse(
        {
          error: 'operator_provider_device_login_poll_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 502) },
      )
    }
  }

  const runSanityCheckForAccount = async (
    env: Bindings,
    repository: ProviderAccountRepository,
    account: ProviderAccountRecord,
    probeContext: Readonly<{
      leaseId: string
      probeId: string
    }> | null = null,
  ): Promise<ProviderAccountSanityCheck> => {
    const startedAt = currentIsoTimestamp()
    const finish = async (
      classification: ProviderAccountSanityClassification,
      grantRef: string | null,
      collisionClass: ProviderAccountProbeCollisionClass = 'none',
      failureClass: ProviderAccountFailoverFailureClass | null = null,
    ): Promise<ProviderAccountSanityCheck> => {
      const finishedAt = currentIsoTimestamp()
      const health = providerHealthForSanity(classification)
      const summary = summaryForClassification(classification)

      await recordProviderAccountHealth(repository, {
        actorId: 'operator_sanity_check',
        providerAccountRef: account.providerAccountRef,
        health,
        reason: `sanity_check:${failureClass ?? classification}`,
      })
      await recordSanityCheck(
        openAgentsDatabase(env),
        {
          account,
          checkedAt: finishedAt,
          classification,
          failureClass,
          grantRef,
          summary,
        },
        identityAuthMirrorFromEnv(env),
      )

      return {
        providerAccountId: account.id,
        providerAccountRef: account.providerAccountRef,
        accountLabel: account.accountLabel,
        classification,
        health,
        status: classification === 'healthy' ? 'connected' : account.status,
        summary,
        checkedAt: finishedAt,
        probeId: probeContext?.probeId ?? null,
        leaseId: probeContext?.leaseId ?? null,
        startedAt,
        finishedAt,
        terminalStatus: classification === 'healthy' ? 'passed' : 'failed',
        collisionClass,
        failureClass,
      }
    }

    if (
      account.status !== 'connected' ||
      account.health === 'requires_reauth' ||
      account.secretRef === null
    ) {
      return finish('requires_reauth', null)
    }

    try {
      const grant = await issueProviderAccountGrant(repository, {
        providerAccountRef: account.providerAccountRef,
        requestedAction: 'operator_sanity_check',
        runnerSessionId:
          probeContext === null
            ? `operator_sanity_${account.id}`
            : `operator_sanity_${account.id}_${probeContext.probeId}`,
        userId: account.userId,
      })

      if (grant === undefined) {
        return finish('grant_resolution_failed', null)
      }

      const resolved = await resolveProviderAccountGrant(repository, {
        actorId: 'operator_sanity_check',
        grantRef: grant.grantRef,
        providerAccountRef: account.providerAccountRef,
        runnerSessionId:
          probeContext === null
            ? `operator_sanity_${account.id}`
            : `operator_sanity_${account.id}_${probeContext.probeId}`,
      })

      if (resolved === undefined) {
        return finish('grant_resolution_failed', grant.grantRef)
      }

      const authMaterial = await dependencies.readConnectedCodexAuthMaterial(
        env,
        account.userId,
        account.providerAccountRef,
      )

      if (authMaterial === undefined) {
        return finish('requires_reauth', resolved.grantRef)
      }

      const classification =
        dependencies.probeResolvedGrant === undefined
          ? normalizeProbeResult(
              await defaultResolvedGrantProbe(env, dependencies, {
                account,
                authMaterial,
              }),
              account.providerAccountRef,
            )
          : normalizeProbeResult(
              await dependencies.probeResolvedGrant({
                account,
                authMaterialAvailable: true,
                grant: resolved,
                leaseId: probeContext?.leaseId ?? null,
                probeId: probeContext?.probeId ?? null,
              }),
              account.providerAccountRef,
            )

      if (resolved.providerAccountRef !== account.providerAccountRef) {
        return finish(
          'unknown_failure',
          resolved.grantRef,
          'grant_account_mismatch',
        )
      }

      return finish(
        classification.classification,
        resolved.grantRef,
        classification.collisionClass,
        classification.failureClass,
      )
    } catch (error) {
      logWorkerRouteError('operator_provider_account_sanity_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef: account.providerAccountRef,
      })

      return finish('grant_resolution_failed', null)
    }
  }

  const sanityCheck = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const repository = repositoryFor(env, dependencies)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const runAll = optionalBoolean(body.all) === true
    const parallel = clampParallelism(optionalParallelNumber(body.parallel))
    const probeRunId =
      runAll && parallel > 1
        ? compactRandomId('provider_parallel_probe_run')
        : null
    const targetAccounts =
      providerAccountRef !== undefined
        ? [
            await repository.findAccountByProviderAccountRef(
              providerAccountRef,
            ),
          ].filter(
            (account): account is ProviderAccountRecord =>
              account !== undefined,
          )
        : runAll
          ? await (async () => {
              const targetUser =
                await dependencies.readSelectedOperatorTargetUser(
                  identityDbForEnv(env),
                  body,
                )

              if (targetUser === undefined) {
                return undefined
              }

              return (
                await repository.listAccountsForUser(targetUser.userId)
              ).filter(
                account =>
                  account.status === 'connected' && account.deletedAt === null,
              )
            })()
          : undefined

    if (targetAccounts === undefined) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason:
            'providerAccountRef or all=true with a target user is required',
        },
        { status: 400 },
      )
    }

    if (targetAccounts.length === 0) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const checks =
      probeRunId === null
        ? await mapWithConcurrency(targetAccounts, 1, account =>
            runSanityCheckForAccount(env, repository, account),
          )
        : await mapWithConcurrency(targetAccounts, parallel, async account => {
            const check = await runSanityCheckForAccount(
              env,
              repository,
              account,
              {
                leaseId: compactRandomId('provider_probe_lease'),
                probeId: compactRandomId('provider_probe'),
              },
            )

            await recordParallelProbeReceipt(
              openAgentsDatabase(env),
              {
                account,
                check,
                probeRunId,
              },
              identityAuthMirrorFromEnv(env),
            )

            return check
          })

    return noStoreJsonResponse({
      checks,
      summary: allSummary(checks),
      probeRunId,
      parallel: probeRunId === null ? 1 : parallel,
    } satisfies ProviderAccountSanityResponse)
  }

  const acquireLease = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const requestedAction = optionalString(body.requestedAction)
    const requiredProviderValue = optionalString(body.requiredProvider)
    const requiredProvider =
      requiredProviderValue === undefined
        ? undefined
        : providerAccountProvider(requiredProviderValue)
    const now = currentIsoTimestamp()
    const leaseTtlSeconds = clampLeaseTtlSeconds(
      optionalParallelNumber(body.ttlSeconds),
    )
    const expiresAt = isoTimestampAfterIso(now, leaseTtlSeconds * 1_000)

    if (requestedAction === undefined) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason: 'requestedAction is required',
        },
        { status: 400 },
      )
    }

    if (requiredProviderValue !== undefined && requiredProvider === undefined) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason:
            'requiredProvider must be chatgpt_codex, anthropic_claude, or google_gemini.',
        },
        { status: 400 },
      )
    }

    try {
      const lease = await leaseServiceFor(env).acquire({
        assignmentId: optionalString(body.assignmentId) ?? null,
        expiresAt,
        now,
        orderId: optionalString(body.orderId) ?? null,
        requiredProvider: requiredProvider ?? null,
        requestedAction,
        runId: optionalString(body.runId) ?? null,
        selectedByActor: 'operator_provider_account_routes',
        source: 'operator_lease_acquire',
        userId: targetUser.userId,
      })

      if (lease === undefined) {
        return noStoreJsonResponse(
          {
            error: 'no_eligible_provider_account',
            reason:
              requiredProvider === undefined
                ? 'No connected healthy provider account is eligible for lease.'
                : `No connected healthy ${requiredProvider} account is eligible for lease.`,
            selectedByPolicyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
          },
          { status: 409 },
        )
      }

      return noStoreJsonResponse(lease satisfies ProviderAccountLeaseResponse, {
        status: 201,
      })
    } catch (error) {
      logWorkerRouteError('operator_provider_account_lease_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        targetUserId: targetUser.userId,
      })

      return noStoreJsonResponse(
        {
          error: 'operator_provider_account_lease_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 502) },
      )
    }
  }

  const issueLeaseGrant = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const leaseRef = optionalString(body.leaseRef)

    if (leaseRef === undefined) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: 'leaseRef is required' },
        { status: 400 },
      )
    }

    const now = currentIsoTimestamp()
    const lease = await leaseServiceFor(env).findActive({
      leaseRef,
      now,
      userId: targetUser.userId,
    })

    if (lease === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const requestedAction =
      optionalString(body.requestedAction) ?? lease.requestedAction
    const runnerSessionId =
      optionalString(body.runnerSessionId) ??
      optionalString(body.runId) ??
      lease.runId ??
      lease.leaseRef
    const threadId = optionalString(body.threadId)
    const workroomId = optionalString(body.workroomId)

    try {
      const grant = await issueProviderAccountGrant(
        repositoryFor(env, dependencies),
        {
          providerAccountRef: lease.providerAccountRef,
          requestedAction,
          runnerSessionId,
          userId: targetUser.userId,
          ...(threadId === undefined ? {} : { threadId }),
          ...(workroomId === undefined ? {} : { workroomId }),
        },
      )

      if (grant === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return noStoreJsonResponse(
        {
          leaseRef: lease.leaseRef,
          providerAccountRef: lease.providerAccountRef,
          requestedAction,
          runId: lease.runId,
          assignmentId: lease.assignmentId,
          orderId: lease.orderId,
          grant: {
            grantRef: grant.grantRef,
            status: grant.status,
            expiresAt: grant.expiresAt,
            ...(grant.requestedAction === undefined
              ? {}
              : { requestedAction: grant.requestedAction }),
            ...(grant.runnerSessionId === undefined
              ? {}
              : { runnerSessionId: grant.runnerSessionId }),
            ...(grant.workroomId === undefined
              ? {}
              : { workroomId: grant.workroomId }),
            ...(grant.threadId === undefined
              ? {}
              : { threadId: grant.threadId }),
          },
        } satisfies ProviderAccountLeaseGrantResponse,
        { status: 201 },
      )
    } catch (error) {
      logWorkerRouteError(
        'operator_provider_account_lease_grant_failed',
        error,
        {
          errorName: providerAccountRouteErrorName(error),
          leaseRef,
          targetUserId: targetUser.userId,
        },
      )

      return noStoreJsonResponse(
        {
          error: 'operator_provider_account_lease_grant_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 502) },
      )
    }
  }

  const failoverLease = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const previousLeaseRef = optionalString(body.previousLeaseRef)
    const failureClass = optionalFailureClass(body.failureClass)
    const requestedAction = optionalString(body.requestedAction)
    const attemptNumber = optionalPositiveInteger(body.attemptNumber, 1, 20)
    const maxAttempts = optionalPositiveInteger(body.maxAttempts, 3, 20)
    const now = currentIsoTimestamp()

    if (
      previousLeaseRef === undefined ||
      failureClass === undefined ||
      requestedAction === undefined
    ) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason:
            'previousLeaseRef, failureClass, and requestedAction are required',
        },
        { status: 400 },
      )
    }

    try {
      const result = await leaseServiceFor(env).failover({
        assignmentId: optionalString(body.assignmentId) ?? null,
        attemptNumber,
        expiresAt: isoTimestampAfterIso(now, 15 * 60 * 1_000),
        failureClass,
        maxAttempts,
        now,
        orderId: optionalString(body.orderId) ?? null,
        previousLeaseRef,
        requestedAction,
        runId: optionalString(body.runId) ?? null,
        selectedByActor: 'operator_provider_account_routes',
        source: 'operator_provider_account_failover',
        userId: targetUser.userId,
      })

      if (result === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return noStoreJsonResponse(
        {
          receiptId: result.receipt.receiptId,
          accountStateAction: result.action.accountStateAction,
          attemptNumber,
          customerSafeStatus: result.receipt.customerSafeStatus,
          failureClass,
          maxAttempts,
          nextLease: result.nextLease,
          outcome: result.outcome,
          previousLeaseRef: result.previousLease.leaseRef,
          previousProviderAccountRef:
            result.previousLease.providerAccountRef,
        } satisfies ProviderAccountFailoverResponse,
        { status: result.outcome === 'blocked' ? 409 : 201 },
      )
    } catch (error) {
      logWorkerRouteError('operator_provider_account_failover_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        targetUserId: targetUser.userId,
      })

      return noStoreJsonResponse(
        {
          error: 'operator_provider_account_failover_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 502) },
      )
    }
  }

  const failoverHistory = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const receipts = await leaseServiceFor(env).listFailoverReceipts({
      assignmentId: optionalString(body.assignmentId) ?? null,
      limit: optionalLimit(body.limit, 25, 100),
      orderId: optionalString(body.orderId) ?? null,
      runId: optionalString(body.runId) ?? null,
      userId: targetUser.userId,
    })

    return noStoreJsonResponse({ receipts, total: receipts.length })
  }

  const fleetDashboard = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    return noStoreJsonResponse(
      await providerAccountFleetDashboard(
        openAgentsDatabase(env),
        leaseServiceFor(env),
        targetUser.userId,
        currentIsoTimestamp(),
      ),
    )
  }

  const activeLeases = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const leases = await leaseServiceFor(env).listActive(
      targetUser.userId,
      currentIsoTimestamp(),
    )

    return noStoreJsonResponse({ leases, total: leases.length })
  }

  const explainLease = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    return noStoreJsonResponse(
      await leaseServiceFor(env).explainSelection(
        targetUser.userId,
        currentIsoTimestamp(),
      ),
    )
  }

  const touchLease = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const leaseRef = optionalString(body.leaseRef)

    if (leaseRef === undefined) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: 'leaseRef is required' },
        { status: 400 },
      )
    }

    const now = currentIsoTimestamp()
    const touched = await leaseServiceFor(env).touch({
      expiresAt: isoTimestampAfterIso(
        now,
        clampLeaseTtlSeconds(optionalParallelNumber(body.ttlSeconds)) * 1_000,
      ),
      leaseRef,
      now,
      userId: targetUser.userId,
    })

    return touched
      ? noStoreJsonResponse({ leaseRef, status: 'touched' })
      : noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  }

  const releaseLease = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const leaseRef = optionalString(body.leaseRef)
    const status = optionalString(body.status)
    const releaseStatus =
      status === 'succeeded' || status === 'failed' || status === 'released'
        ? status
        : 'released'

    if (leaseRef === undefined) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: 'leaseRef is required' },
        { status: 400 },
      )
    }

    const released = await leaseServiceFor(env).release({
      failureClass: optionalString(body.failureClass) ?? null,
      leaseRef,
      now: currentIsoTimestamp(),
      status: releaseStatus,
      terminalOutcome: optionalString(body.terminalOutcome) ?? releaseStatus,
      userId: targetUser.userId,
    })

    return released
      ? noStoreJsonResponse({ leaseRef, status: releaseStatus })
      : noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  }

  const resetAccount = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    const rejected = await rejectUnlessOperatorAdminRoute(request, env, 'POST')

    if (rejected !== undefined) {
      return rejected
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      identityDbForEnv(env),
      body,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const providerAccountRef =
      optionalString(body.providerAccountRef) ?? optionalString(body.accountRefHash)

    if (providerAccountRef === undefined) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason: 'providerAccountRef or accountRefHash is required',
        },
        { status: 400 },
      )
    }

    const resetAt = currentIsoTimestamp()
    const reset = await resetOperatorProviderAccount(
      openAgentsDatabase(env),
      {
        providerAccountRef,
        resetAt,
        userId: targetUser.userId,
      },
      identityAuthMirrorFromEnv(env),
    )

    if (!reset) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse({
      ok: true,
      providerAccountRef,
      resetAt,
    } satisfies OperatorProviderAccountResetResponse)
  }

  return {
    routeOperatorProviderAccountRequest: (
      request: Request,
      env: Bindings,
    ): Promise<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/operator/accounts/reset') {
        return resetAccount(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/device-login/start'
      ) {
        return startDeviceLogin(request, env)
      }

      const pollMatch =
        /^\/api\/operator\/provider-accounts\/chatgpt-codex\/device-login\/([^/]+)$/.exec(
          url.pathname,
        )

      if (pollMatch !== null) {
        return pollDeviceLogin(request, env, pollMatch[1] ?? '')
      }

      if (
        url.pathname === '/api/operator/provider-accounts/chatgpt-codex/sanity'
      ) {
        return sanityCheck(request, env)
      }

      if (
        url.pathname === '/api/operator/provider-accounts/chatgpt-codex/leases'
      ) {
        return acquireLease(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/grant'
      ) {
        return issueLeaseGrant(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/failover'
      ) {
        return failoverLease(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/failover-history'
      ) {
        return failoverHistory(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard'
      ) {
        return fleetDashboard(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/active'
      ) {
        return activeLeases(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/explain'
      ) {
        return explainLease(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/touch'
      ) {
        return touchLease(request, env)
      }

      if (
        url.pathname ===
        '/api/operator/provider-accounts/chatgpt-codex/leases/release'
      ) {
        return releaseLease(request, env)
      }

      return undefined
    },
  }
}
