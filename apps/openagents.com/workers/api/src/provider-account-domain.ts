import {
  type OpenAiOAuthTokenResponse,
  assertNoProviderSecretMaterial,
  containsProviderSecretMaterial,
  providerAccountPublicMetadataJson,
  sanitizeProviderAccountText,
} from '@openagentsinc/provider-account-schema'

import { parseBase64UrlJsonRecord } from './json-boundary'
import {
  ProviderAccountCredentialMaterial,
  ProviderAccountInvalidVerificationUrl,
} from './provider-account-errors'
import {
  compactRandomId,
  currentDate,
  isoTimestampAfter,
  normalizeIsoTimestamp,
} from './runtime-primitives'

export const CHATGPT_CODEX_PROVIDER = 'chatgpt_codex' as const
export const GOOGLE_GEMINI_PROVIDER = 'google_gemini' as const
export const ANTHROPIC_CLAUDE_PROVIDER = 'anthropic_claude' as const
export const OPENROUTER_PROVIDER = 'openrouter' as const
export const CHATGPT_CODEX_VERIFICATION_URL =
  'https://auth.openai.com/codex/device'
export const PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS = {
  accounts: 'provider_accounts_public',
  attempts: 'provider_connection_attempts_public',
  events: 'provider_account_events_public',
  grants: 'provider_account_grants_public',
  runnerSessions: 'runner_sessions_public',
} as const
export type ProviderAccountPublicCollection =
  (typeof PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS)[keyof typeof PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS]

export const CODEX_CLIENT_ID = 'app_EMoamEEZ73f0CkXaXp7hrann'
export const DEVICE_LOGIN_TTL_MS = 1000 * 60 * 15
export const SESSION_GRANT_TTL_MS = 1000 * 60 * 60 * 2
export type OpenAiOAuthToken = OpenAiOAuthTokenResponse

export type IdFactory = (prefix: string) => string

export type ProviderAccountRuntime = Readonly<{
  makeId: IdFactory
  now: () => Date
}>

export const systemProviderAccountRuntime: ProviderAccountRuntime = {
  makeId: compactRandomId,
  now: currentDate,
}

export type ProviderAccountStatus =
  | 'pending'
  | 'connected'
  | 'expired'
  | 'denied'
  | 'disconnected'
  | 'unhealthy'

export type ProviderAccountHealth =
  | 'unknown'
  | 'healthy'
  | 'unhealthy'
  | 'requires_reauth'

export type ProviderAccountAuthMode =
  | 'chatgpt_device_code'
  | 'codex_device_auth'
  | 'manual_secret_ref'
  | 'api_key'
  | 'claude_local_auth'

export type ProviderAccountProvider =
  | typeof CHATGPT_CODEX_PROVIDER
  | typeof GOOGLE_GEMINI_PROVIDER
  | typeof ANTHROPIC_CLAUDE_PROVIDER
  | typeof OPENROUTER_PROVIDER

export const providerDisplayName = (
  provider: ProviderAccountProvider,
): string =>
  provider === CHATGPT_CODEX_PROVIDER
    ? 'ChatGPT/Codex'
    : provider === GOOGLE_GEMINI_PROVIDER
      ? 'Google Gemini'
      : provider === ANTHROPIC_CLAUDE_PROVIDER
        ? 'Anthropic Claude'
        : 'OpenRouter'

export type ProviderConnectionAttemptStatus =
  | 'pending'
  | 'connected'
  | 'expired'
  | 'denied'
  | 'failed'

export type ProviderAccountAuthGrantStatus =
  | 'issued'
  | 'used'
  | 'expired'
  | 'revoked'
  | 'failed'

export type ProviderConnectionAttemptSource =
  | 'shc_broker'
  | 'worker_device_code'
  | 'manual_placeholder'
  | 'browser_api_key'
  | 'pylon_local_codex_auth'
  | 'pylon_local_claude_auth'

export type ProviderConnectionAttemptMethod =
  | 'chatgpt_device_code'
  | 'codex_device_auth'
  | 'provider_api_key'
  | 'claude_local_auth'

export type ProviderAccountEventKind =
  | 'login_connected'
  | 'login_denied'
  | 'login_expired'
  | 'login_failed'
  | 'login_started'
  | 'account_disconnected'
  | 'account_health_updated'
  | 'auth_grant_issued'
  | 'auth_grant_used'
  | 'auth_grant_revoked'
  | 'auth_grant_failed'

export type ProviderAccountRecord = Readonly<{
  id: string
  userId: string
  teamId: string | null
  provider: ProviderAccountProvider
  authMode: ProviderAccountAuthMode
  status: ProviderAccountStatus
  health: ProviderAccountHealth
  providerAccountRef: string
  secretRef: string | null
  accountLabel: string | null
  planType: string | null
  connectedAt: string | null
  disconnectedAt: string | null
  deniedAt: string | null
  lastStatusAt: string
  metadataJson: string | null
  createdAt: string
  updatedAt: string
  deletedAt: string | null
}>

export type ProviderConnectionAttemptRecord = Readonly<{
  id: string
  providerAccountId: string
  userId: string
  teamId: string | null
  provider: ProviderAccountProvider
  method: ProviderConnectionAttemptMethod
  source: ProviderConnectionAttemptSource
  loginRef: string | null
  verificationUrl: string | null
  userCode: string | null
  status: ProviderConnectionAttemptStatus
  expiresAt: string
  completedAt: string | null
  failedAt: string | null
  metadataJson: string | null
  createdAt: string
  updatedAt: string
}>

export type ProviderAccountEventRecord = Readonly<{
  id: string
  providerAccountId: string | null
  authGrantId: string | null
  userId: string
  teamId: string | null
  threadId: string | null
  workroomId: string | null
  runnerSessionId: string | null
  kind: ProviderAccountEventKind
  summary: string
  sourceRefsJson: string
  evidenceRefsJson: string
  targetRef: string | null
  metadataJson: string | null
  actorId: string | null
  createdAt: string
}>

export type ProviderAccountAuthGrantRecord = Readonly<{
  id: string
  providerAccountId: string
  userId: string
  teamId: string | null
  threadId: string | null
  workroomId: string | null
  runnerSessionId: string | null
  provider: ProviderAccountProvider
  providerAccountRef: string
  providerSecretRef: string
  grantRef: string
  status: ProviderAccountAuthGrantStatus
  requestedAction: string | null
  metadataJson: string | null
  createdAt: string
  updatedAt: string
  expiresAt: string
  usedAt: string | null
  revokedAt: string | null
  failedAt: string | null
}>

export type PublicProviderAccount = Readonly<{
  id: string
  provider: ProviderAccountProvider
  authMode: ProviderAccountAuthMode
  status: ProviderAccountStatus
  publicStatus: ProviderAccountStatus
  health: ProviderAccountHealth
  providerAccountRef: string
  hasSecretRef: boolean
  accountLabel?: string | undefined
  planType?: string | undefined
  connectedAt?: string | undefined
  disconnectedAt?: string | undefined
  lastStatusAt: string
  createdAt: string
  updatedAt: string
}>

export type PublicProviderConnectionAttempt = Readonly<{
  id: string
  providerAccountId: string
  providerAccountRef: string
  provider: ProviderAccountProvider
  method: ProviderConnectionAttemptMethod
  source: ProviderConnectionAttemptSource
  status: ProviderConnectionAttemptStatus
  loginRef?: string | undefined
  verificationUrl?: string | undefined
  userCode?: string | undefined
  expiresAt: string
  completedAt?: string | undefined
  failedAt?: string | undefined
  createdAt: string
  updatedAt: string
}>

export type ProviderAccountBundle = Readonly<{
  accounts: ReadonlyArray<PublicProviderAccount>
  attempts: ReadonlyArray<PublicProviderConnectionAttempt>
}>

export type PublicProviderAccountGrant = Readonly<{
  id: string
  providerAccountId: string
  provider: ProviderAccountProvider
  providerAccountRef: string
  grantRef: string
  status: ProviderAccountAuthGrantStatus
  requestedAction?: string | undefined
  threadId?: string | undefined
  workroomId?: string | undefined
  runnerSessionId?: string | undefined
  expiresAt: string
  createdAt: string
  updatedAt: string
}>

export type PublicProviderAccountEvent = Readonly<{
  id: string
  providerAccountId?: string | undefined
  authGrantId?: string | undefined
  kind: ProviderAccountEventKind
  summary: string
  targetRef?: string | undefined
  createdAt: string
}>

export type PublicRunnerSession = Readonly<{
  id: string
  runnerId: string
  lane: string
  backend: string
  status: string
  teamId?: string | undefined
  threadId?: string | undefined
  workroomId?: string | undefined
  providerAccountRef?: string | undefined
  activeAuthGrantRef?: string | undefined
  opencodeServerUrl?: string | undefined
  opencodeServerAuthRef?: string | undefined
  createdAt: string
  updatedAt: string
  startedAt?: string | undefined
  completedAt?: string | undefined
  failedAt?: string | undefined
}>

export type ProviderAccountPublicProjection =
  | PublicProviderAccount
  | PublicProviderConnectionAttempt
  | PublicProviderAccountGrant
  | PublicProviderAccountEvent
  | PublicRunnerSession

export type RedactedOpenCodeMaterializationPlan = Readonly<{
  provider: 'openai'
  authRef: string
  authContentEnv: 'OPENCODE_AUTH_CONTENT'
  homeIsolation: 'per-run-opencode-home'
  serverPassword: 'runner-generated'
  scrubAfterCloseout: true
}>

export type RedactedGeminiApiKeyMaterializationPlan = Readonly<{
  kind: 'probe_gemini_api_key'
  provider: typeof GOOGLE_GEMINI_PROVIDER
  providerSecretRef: string
  target: Readonly<{
    kind: 'env'
    name: 'GOOGLE_GENERATIVE_AI_API_KEY'
  }>
  homeIsolation: 'per_run'
  scrubAfterCloseout: true
}>

export type RedactedAnthropicApiKeyMaterializationPlan = Readonly<{
  kind: 'claude_agent_anthropic_api_key'
  provider: typeof ANTHROPIC_CLAUDE_PROVIDER
  providerSecretRef: string
  target: Readonly<{
    kind: 'env'
    name: 'ANTHROPIC_API_KEY'
  }>
  homeIsolation: 'per_run'
  scrubAfterCloseout: true
}>

export type ResolvedProviderAccountGrant = Readonly<{
  grantRef: string
  ownerUserId: string
  provider: ProviderAccountProvider
  providerAccountRef: string
  providerSecretRef: string
  requestedAction?: string | undefined
  runnerSessionId?: string | undefined
  expiresAt: string
  status: 'used'
  materialization:
    | RedactedOpenCodeMaterializationPlan
    | RedactedGeminiApiKeyMaterializationPlan
    | RedactedAnthropicApiKeyMaterializationPlan
}>

export type StartedCodexDeviceLogin = Readonly<{
  deviceAuthId: string
  verificationUrl: string
  userCode: string
  expiresAt: string
  intervalSeconds: number
}>

export type StartCodexDeviceLogin = () => Promise<StartedCodexDeviceLogin>

export type StartedCodexDeviceLoginSecret = Readonly<{
  deviceAuthId: string
  userCode: string
}>

export type CodexOAuthAuth = Readonly<{
  type: 'oauth'
  refresh: string
  access: string
  expires: number
  accountId?: string | undefined
  idToken?: string | undefined
}>

export type CodexDeviceLoginPollResult =
  | Readonly<{ status: 'pending' }>
  | Readonly<{ status: 'failed'; reason?: string | undefined }>
  | Readonly<{
      status: 'connected'
      auth: CodexOAuthAuth
      accountLabel?: string | undefined
      planType?: string | undefined
    }>

export type StoreStartedCodexDeviceLogin = (
  input: Readonly<{
    attemptId: string
    deviceAuthId: string
    userCode: string
    expiresAt: string
  }>,
) => Promise<void>

export type ReadStartedCodexDeviceLogin = (
  attemptId: string,
) => Promise<StartedCodexDeviceLoginSecret | undefined>

export type StoreConnectedCodexAuth = (
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    auth: CodexOAuthAuth
  }>,
) => Promise<string>

/**
 * CX-5 (#8549): local-auth/import custody write for a `CLAUDE_CODE_OAUTH_TOKEN`
 * obtained by the owner running `claude setup-token` on their own machine —
 * the Claude equivalent of `StoreConnectedCodexAuth`. Unlike Codex's
 * access/refresh/expires triple, Claude Code's long-lived OAuth token is a
 * single opaque bearer string; there is nothing else to normalize or refresh.
 */
export type StoreConnectedClaudeAuth = (
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    authContentValue: string
  }>,
) => Promise<string>

export type DeleteStartedCodexDeviceLogin = (attemptId: string) => Promise<void>

export type PollCodexDeviceLogin = (
  secret: StartedCodexDeviceLoginSecret,
) => Promise<CodexDeviceLoginPollResult>

export type StartDeviceLoginInput = Readonly<{
  userId: string
  accountLabel?: string | undefined
  createNew?: boolean | undefined
  providerAccountRef?: string | undefined
}>

export type RecordConnectedInput = Readonly<{
  actorId: string
  attemptId: string
  providerAccountRef?: string | undefined
  accountLabel?: string | undefined
  planType?: string | undefined
  secretRef?: string | undefined
}>

export type RecordFailedInput = Readonly<{
  actorId: string
  attemptId: string
  providerAccountRef?: string | undefined
  status?: 'denied' | 'expired' | 'failed' | undefined
  reason?: string | undefined
}>

export type RecordHealthInput = Readonly<{
  actorId: string
  providerAccountRef: string
  health: 'healthy' | 'unhealthy' | 'requires_reauth'
  reason?: string | undefined
}>

export type IssueProviderAccountGrantInput = Readonly<{
  userId: string
  providerAccountRef: string
  requestedAction?: string | undefined
  threadId?: string | undefined
  workroomId?: string | undefined
  runnerSessionId?: string | undefined
}>

export type ResolveProviderAccountGrantInput = Readonly<{
  actorId: string
  grantRef: string
  providerAccountRef?: string | undefined
  runnerSessionId?: string | undefined
}>

export type ProviderAccountRepository = Readonly<{
  findAccountByRef: (
    userId: string,
    providerAccountRef: string,
  ) => Promise<ProviderAccountRecord | undefined>
  findAccountByProviderAccountRef: (
    providerAccountRef: string,
  ) => Promise<ProviderAccountRecord | undefined>
  findReusableAccount: (
    userId: string,
  ) => Promise<ProviderAccountRecord | undefined>
  listAccountsForUser: (
    userId: string,
  ) => Promise<ReadonlyArray<ProviderAccountRecord>>
  listPendingAttemptsForUser: (
    userId: string,
  ) => Promise<ReadonlyArray<ProviderConnectionAttemptRecord>>
  findAttemptForUser: (
    userId: string,
    attemptId: string,
  ) => Promise<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined
  >
  findAttemptById: (attemptId: string) => Promise<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined
  >
  saveStartedDeviceLogin: (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
    accountAlreadyExists: boolean,
  ) => Promise<void>
  recordConnectedAttempt: (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ) => Promise<ProviderAccountRecord>
  recordFailedAttempt: (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ) => Promise<ProviderAccountRecord>
  recordAccountHealth: (
    providerAccountRef: string,
    account: ProviderAccountRecord,
    event: ProviderAccountEventRecord,
  ) => Promise<ProviderAccountRecord | undefined>
  createAuthGrant: (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ) => Promise<ProviderAccountAuthGrantRecord>
  findGrantByRef: (
    grantRef: string,
  ) => Promise<ProviderAccountAuthGrantRecord | undefined>
  markGrantUsed: (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ) => Promise<ProviderAccountAuthGrantRecord>
  disconnectAccount: (
    userId: string,
    providerAccountRef: string,
    now: string,
    metadataJson: string,
    event: ProviderAccountEventRecord,
  ) => Promise<ProviderAccountRecord | undefined>
}>

export type ProviderAccountRow = Readonly<{
  id: string
  user_id: string
  team_id: string | null
  provider: ProviderAccountProvider
  auth_mode: ProviderAccountAuthMode
  status: ProviderAccountStatus
  health: ProviderAccountHealth
  provider_account_ref: string
  secret_ref: string | null
  account_label: string | null
  plan_type: string | null
  connected_at: string | null
  disconnected_at: string | null
  denied_at: string | null
  last_status_at: string
  metadata_json: string | null
  created_at: string
  updated_at: string
  deleted_at: string | null
}>

export type ProviderConnectionAttemptRow = Readonly<{
  id: string
  provider_account_id: string
  user_id: string
  team_id: string | null
  provider: ProviderAccountProvider
  method: ProviderConnectionAttemptMethod
  source: ProviderConnectionAttemptSource
  login_ref: string | null
  verification_url: string | null
  user_code: string | null
  status: ProviderConnectionAttemptStatus
  expires_at: string
  completed_at: string | null
  failed_at: string | null
  metadata_json: string | null
  created_at: string
  updated_at: string
}>

export type ProviderAccountAuthGrantRow = Readonly<{
  id: string
  provider_account_id: string
  user_id: string
  team_id: string | null
  thread_id: string | null
  workroom_id: string | null
  runner_session_id: string | null
  provider: ProviderAccountProvider
  provider_account_ref: string
  provider_secret_ref: string
  grant_ref: string
  status: ProviderAccountAuthGrantStatus
  requested_action: string | null
  metadata_json: string | null
  created_at: string
  updated_at: string
  expires_at: string
  used_at: string | null
  revoked_at: string | null
  failed_at: string | null
}>

const compact = <T extends Record<string, unknown>>(value: T): T =>
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
  Object.fromEntries(
    Object.entries(value).filter(([, entry]) => entry !== undefined),
  ) as T

export const toAccountRecord = (
  row: ProviderAccountRow,
): ProviderAccountRecord => ({
  id: row.id,
  userId: row.user_id,
  teamId: row.team_id,
  provider: row.provider,
  authMode: row.auth_mode,
  status: row.status,
  health: row.health,
  providerAccountRef: row.provider_account_ref,
  secretRef: row.secret_ref,
  accountLabel: row.account_label,
  planType: row.plan_type,
  connectedAt: row.connected_at,
  disconnectedAt: row.disconnected_at,
  deniedAt: row.denied_at,
  lastStatusAt: row.last_status_at,
  metadataJson: row.metadata_json,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  deletedAt: row.deleted_at,
})

export const toAttemptRecord = (
  row: ProviderConnectionAttemptRow,
): ProviderConnectionAttemptRecord => ({
  id: row.id,
  providerAccountId: row.provider_account_id,
  userId: row.user_id,
  teamId: row.team_id,
  provider: row.provider,
  method: row.method,
  source: row.source,
  loginRef: row.login_ref,
  verificationUrl: row.verification_url,
  userCode: row.user_code,
  status: row.status,
  expiresAt: row.expires_at,
  completedAt: row.completed_at,
  failedAt: row.failed_at,
  metadataJson: row.metadata_json,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

export const toGrantRecord = (
  row: ProviderAccountAuthGrantRow,
): ProviderAccountAuthGrantRecord => ({
  id: row.id,
  providerAccountId: row.provider_account_id,
  userId: row.user_id,
  teamId: row.team_id,
  threadId: row.thread_id,
  workroomId: row.workroom_id,
  runnerSessionId: row.runner_session_id,
  provider: row.provider,
  providerAccountRef: row.provider_account_ref,
  providerSecretRef: row.provider_secret_ref,
  grantRef: row.grant_ref,
  status: row.status,
  requestedAction: row.requested_action,
  metadataJson: row.metadata_json,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  expiresAt: row.expires_at,
  usedAt: row.used_at,
  revokedAt: row.revoked_at,
  failedAt: row.failed_at,
})

export const textOrUndefined = (value: string | null): string | undefined =>
  value === null || value === '' ? undefined : value

export const addMilliseconds = (date: Date, milliseconds: number): string =>
  isoTimestampAfter(date, milliseconds)

export const parseExpiresInSeconds = (
  value: string | number | undefined,
): number => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(1, Math.floor(value))
  }

  if (typeof value === 'string') {
    const parsed = Number.parseInt(value, 10)

    if (Number.isFinite(parsed)) {
      return Math.max(1, parsed)
    }
  }

  return Math.floor(DEVICE_LOGIN_TTL_MS / 1000)
}

export const parseExpiresAt = (
  value: string | undefined,
  expiresIn: string | number | undefined,
  now: Date,
): string => {
  if (value !== undefined) {
    const timestamp = Date.parse(value)

    if (Number.isFinite(timestamp)) {
      return normalizeIsoTimestamp(value)
    }
  }

  return addMilliseconds(now, parseExpiresInSeconds(expiresIn) * 1000)
}

const decodeJwtClaims = (
  token: string | undefined,
): Record<string, unknown> | undefined => {
  const payload = token?.split('.')[1]

  if (payload === undefined) {
    return undefined
  }

  return parseBase64UrlJsonRecord(payload)
}

export const extractCodexAccountId = (
  tokens: OpenAiOAuthToken,
): string | undefined => {
  const claims =
    decodeJwtClaims(tokens.id_token) ?? decodeJwtClaims(tokens.access_token)
  const authClaim = claims?.['https://api.openai.com/auth']
  const organizations = claims?.organizations
  const firstOrganization =
    Array.isArray(organizations) &&
    typeof organizations[0] === 'object' &&
    organizations[0] !== null
      ? Object.fromEntries(Object.entries(organizations[0]))
      : undefined
  const directAccountId = claims?.chatgpt_account_id
  const nestedAccountId =
    typeof authClaim === 'object' &&
    authClaim !== null &&
    !Array.isArray(authClaim)
      ? Object.fromEntries(Object.entries(authClaim)).chatgpt_account_id
      : undefined
  const organizationId = firstOrganization?.id

  return typeof directAccountId === 'string'
    ? directAccountId
    : typeof nestedAccountId === 'string'
      ? nestedAccountId
      : typeof organizationId === 'string'
        ? organizationId
        : undefined
}

export const extractCodexAccountLabel = (
  tokens: OpenAiOAuthToken,
): string | undefined => {
  const claims =
    decodeJwtClaims(tokens.id_token) ?? decodeJwtClaims(tokens.access_token)
  const email = claims?.email

  return typeof email === 'string'
    ? sanitizeProviderAccountText(email, 120)
    : undefined
}

export const isAttemptExpired = (
  attempt: ProviderConnectionAttemptRecord,
  now: Date,
): boolean =>
  attempt.status === 'pending' && Date.parse(attempt.expiresAt) <= now.getTime()

const publicAttemptStatus = (
  attempt: ProviderConnectionAttemptRecord,
  now: Date,
): ProviderConnectionAttemptStatus =>
  isAttemptExpired(attempt, now) ? 'expired' : attempt.status

const publicAccountStatus = (
  account: ProviderAccountRecord,
  attempts: ReadonlyArray<ProviderConnectionAttemptRecord>,
  now: Date,
): ProviderAccountStatus => {
  if (account.status !== 'pending') {
    return account.status
  }

  if (attempts.length === 0) {
    return 'pending'
  }

  return attempts.some(attempt => !isAttemptExpired(attempt, now))
    ? 'pending'
    : 'expired'
}

export const assertProviderAccountPublicProjection = <
  T extends ProviderAccountPublicProjection,
>(
  collection: ProviderAccountPublicCollection,
  value: T,
): T => {
  assertNoProviderSecretMaterial(value, collection)

  return value
}

export const toPublicProviderAccount = (
  account: ProviderAccountRecord,
  attempts: ReadonlyArray<ProviderConnectionAttemptRecord>,
  now: Date,
): PublicProviderAccount =>
  assertProviderAccountPublicProjection(
    PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.accounts,
    compact({
      id: account.id,
      provider: account.provider,
      authMode: account.authMode,
      status: account.status,
      publicStatus: publicAccountStatus(account, attempts, now),
      health: account.health,
      providerAccountRef: account.providerAccountRef,
      hasSecretRef: account.secretRef !== null,
      accountLabel: textOrUndefined(account.accountLabel),
      planType: textOrUndefined(account.planType),
      connectedAt: textOrUndefined(account.connectedAt),
      disconnectedAt: textOrUndefined(account.disconnectedAt),
      lastStatusAt: account.lastStatusAt,
      createdAt: account.createdAt,
      updatedAt: account.updatedAt,
    }),
  )

export const toPublicProviderConnectionAttempt = (
  attempt: ProviderConnectionAttemptRecord,
  account: ProviderAccountRecord,
  now: Date,
): PublicProviderConnectionAttempt =>
  assertProviderAccountPublicProjection(
    PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.attempts,
    compact({
      id: attempt.id,
      providerAccountId: attempt.providerAccountId,
      providerAccountRef: account.providerAccountRef,
      provider: attempt.provider,
      method: attempt.method,
      source: attempt.source,
      status: publicAttemptStatus(attempt, now),
      loginRef: textOrUndefined(attempt.loginRef),
      verificationUrl: textOrUndefined(attempt.verificationUrl),
      userCode: textOrUndefined(attempt.userCode),
      expiresAt: attempt.expiresAt,
      completedAt: textOrUndefined(attempt.completedAt),
      failedAt: textOrUndefined(attempt.failedAt),
      createdAt: attempt.createdAt,
      updatedAt: attempt.updatedAt,
    }),
  )

const publicGrantStatus = (
  grant: ProviderAccountAuthGrantRecord,
  now: Date,
): ProviderAccountAuthGrantStatus =>
  grant.status === 'issued' && Date.parse(grant.expiresAt) <= now.getTime()
    ? 'expired'
    : grant.status

export const toPublicProviderAccountGrant = (
  grant: ProviderAccountAuthGrantRecord,
  now: Date,
): PublicProviderAccountGrant =>
  assertProviderAccountPublicProjection(
    PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.grants,
    compact({
      id: grant.id,
      providerAccountId: grant.providerAccountId,
      provider: grant.provider,
      providerAccountRef: grant.providerAccountRef,
      grantRef: grant.grantRef,
      status: publicGrantStatus(grant, now),
      requestedAction: textOrUndefined(grant.requestedAction),
      threadId: textOrUndefined(grant.threadId),
      workroomId: textOrUndefined(grant.workroomId),
      runnerSessionId: textOrUndefined(grant.runnerSessionId),
      expiresAt: grant.expiresAt,
      createdAt: grant.createdAt,
      updatedAt: grant.updatedAt,
    }),
  )

export const toPublicProviderAccountEvent = (
  event: ProviderAccountEventRecord,
): PublicProviderAccountEvent => {
  assertNoProviderSecretMaterial(
    event.metadataJson ?? '',
    `${PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.events}.metadataJson`,
  )
  assertNoProviderSecretMaterial(
    event.sourceRefsJson,
    `${PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.events}.sourceRefsJson`,
  )
  assertNoProviderSecretMaterial(
    event.evidenceRefsJson,
    `${PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.events}.evidenceRefsJson`,
  )

  return assertProviderAccountPublicProjection(
    PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.events,
    compact({
      id: event.id,
      providerAccountId: textOrUndefined(event.providerAccountId),
      authGrantId: textOrUndefined(event.authGrantId),
      kind: event.kind,
      summary: event.summary,
      targetRef: textOrUndefined(event.targetRef),
      createdAt: event.createdAt,
    }),
  )
}

export const buildRedactedOpenCodeMaterializationPlan = (
  providerSecretRef: string,
): RedactedOpenCodeMaterializationPlan => ({
  provider: 'openai',
  authRef: providerSecretRef,
  authContentEnv: 'OPENCODE_AUTH_CONTENT',
  homeIsolation: 'per-run-opencode-home',
  serverPassword: 'runner-generated',
  scrubAfterCloseout: true,
})

export const buildRedactedGeminiApiKeyMaterializationPlan = (
  providerSecretRef: string,
): RedactedGeminiApiKeyMaterializationPlan => ({
  kind: 'probe_gemini_api_key',
  provider: GOOGLE_GEMINI_PROVIDER,
  providerSecretRef,
  target: {
    kind: 'env',
    name: 'GOOGLE_GENERATIVE_AI_API_KEY',
  },
  homeIsolation: 'per_run',
  scrubAfterCloseout: true,
})

export const buildRedactedAnthropicApiKeyMaterializationPlan = (
  providerSecretRef: string,
): RedactedAnthropicApiKeyMaterializationPlan => ({
  kind: 'claude_agent_anthropic_api_key',
  provider: ANTHROPIC_CLAUDE_PROVIDER,
  providerSecretRef,
  target: {
    kind: 'env',
    name: 'ANTHROPIC_API_KEY',
  },
  homeIsolation: 'per_run',
  scrubAfterCloseout: true,
})

export const makeProviderAccountBundle = (
  accounts: ReadonlyArray<ProviderAccountRecord>,
  attempts: ReadonlyArray<ProviderConnectionAttemptRecord>,
  now: Date,
): ProviderAccountBundle => {
  const accountsById = new Map(accounts.map(account => [account.id, account]))

  return {
    accounts: accounts.map(account =>
      toPublicProviderAccount(
        account,
        attempts.filter(attempt => attempt.providerAccountId === account.id),
        now,
      ),
    ),
    attempts: attempts.flatMap(attempt => {
      const account = accountsById.get(attempt.providerAccountId)

      return account === undefined
        ? []
        : [toPublicProviderConnectionAttempt(attempt, account, now)]
    }),
  }
}

export const normalizeAccountLabel = (
  value: string | undefined,
): string | null => sanitizeProviderAccountText(value, 120) ?? null

export const normalizeProviderAccountRef = (
  value: string | undefined,
): string | undefined => sanitizeProviderAccountText(value, 180)

export const sanitizeOrRejectSecretText = (
  value: string | undefined,
  maxLength: number,
  fieldName: string,
): string | undefined => {
  if (value === undefined) {
    return undefined
  }

  if (containsProviderSecretMaterial(value)) {
    throw new ProviderAccountCredentialMaterial({
      fieldName,
      message: `${fieldName} contains credential-shaped material.`,
    })
  }

  return sanitizeProviderAccountText(value, maxLength)
}

export const normalizeVerificationUrl = (value: string): string => {
  let url: URL

  try {
    url = new URL(value)
  } catch {
    throw new ProviderAccountInvalidVerificationUrl({
      message:
        'Codex verification URL must be https://auth.openai.com/codex/device.',
    })
  }

  if (
    url.protocol !== 'https:' ||
    url.hostname !== 'auth.openai.com' ||
    url.pathname !== '/codex/device'
  ) {
    throw new ProviderAccountInvalidVerificationUrl({
      message:
        'Codex verification URL must be https://auth.openai.com/codex/device.',
    })
  }

  return url.toString()
}

export const makeEvent = (
  input: Readonly<{
    id: string
    kind: ProviderAccountEventKind
    providerAccountId: string | null
    userId: string
    teamId: string | null
    summary: string
    targetRef: string | null
    metadata: Record<string, unknown>
    actorId?: string | undefined
    sourceRefs?: ReadonlyArray<string> | undefined
    evidenceRefs?: ReadonlyArray<string> | undefined
    createdAt: string
  }>,
): ProviderAccountEventRecord => ({
  id: input.id,
  providerAccountId: input.providerAccountId,
  authGrantId: null,
  userId: input.userId,
  teamId: input.teamId,
  threadId: null,
  workroomId: null,
  runnerSessionId: null,
  kind: input.kind,
  summary: input.summary,
  sourceRefsJson: JSON.stringify(input.sourceRefs ?? []),
  evidenceRefsJson: JSON.stringify(input.evidenceRefs ?? []),
  targetRef: input.targetRef,
  metadataJson: providerAccountPublicMetadataJson({
    providerAccountRef: input.targetRef ?? 'provider-account:none',
    status: String(input.metadata.status ?? input.kind),
    source: String(input.metadata.source ?? 'worker'),
  }),
  actorId: input.actorId ?? input.userId,
  createdAt: input.createdAt,
})
