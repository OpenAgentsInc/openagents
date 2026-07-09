import {
  providerAccountPublicMetadataJson,
  requirePublicSecretReference,
} from '@openagentsinc/provider-account-schema'
import { Context, Effect, Layer } from 'effect'

import {
  ANTHROPIC_CLAUDE_PROVIDER,
  CHATGPT_CODEX_PROVIDER,
  SESSION_GRANT_TTL_MS,
  type CodexOAuthAuth,
  type DeleteStartedCodexDeviceLogin,
  type IdFactory,
  type IssueProviderAccountGrantInput,
  type PollCodexDeviceLogin,
  type ProviderAccountAuthGrantRecord,
  type ProviderAccountBundle,
  type ProviderAccountEventKind,
  type ProviderAccountRecord,
  type ProviderAccountRepository,
  type ProviderAccountStatus,
  type ProviderConnectionAttemptRecord,
  type PublicProviderAccount,
  type PublicProviderAccountGrant,
  type PublicProviderConnectionAttempt,
  type ReadStartedCodexDeviceLogin,
  type RecordConnectedInput,
  type RecordFailedInput,
  type RecordHealthInput,
  type ResolveProviderAccountGrantInput,
  type ResolvedProviderAccountGrant,
  type StartCodexDeviceLogin,
  type StartDeviceLoginInput,
  type StoreConnectedClaudeAuth,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
  addMilliseconds,
  buildRedactedAnthropicApiKeyMaterializationPlan,
  buildRedactedGeminiApiKeyMaterializationPlan,
  buildRedactedOpenCodeMaterializationPlan,
  isAttemptExpired,
  makeEvent,
  makeProviderAccountBundle,
  normalizeAccountLabel,
  normalizeProviderAccountRef,
  providerDisplayName,
  sanitizeOrRejectSecretText,
  systemProviderAccountRuntime,
  textOrUndefined,
  toPublicProviderAccount,
  toPublicProviderAccountGrant,
  toPublicProviderConnectionAttempt,
} from './provider-account-domain'
import {
  ProviderAccountCredentialMaterial,
  ProviderAccountNotConnectedHealthy,
  type ProviderAccountError,
  ProviderAccountNotFound,
  ProviderAccountRefMismatch,
  ProviderDeviceLoginAttemptAlreadyConnected,
  ProviderDeviceLoginAttemptExpired,
  ProviderDeviceLoginAttemptNotPending,
  ProviderGrantAccountMismatch,
  ProviderGrantExpired,
  ProviderGrantNotIssued,
  ProviderGrantRunnerSessionMismatch,
  providerAccountErrorFromUnknown,
} from './provider-account-errors'

export const listProviderAccountsForUser = async (
  repository: ProviderAccountRepository,
  userId: string,
  now = systemProviderAccountRuntime.now(),
): Promise<ProviderAccountBundle> => {
  const [accounts, attempts] = await Promise.all([
    repository.listAccountsForUser(userId),
    repository.listPendingAttemptsForUser(userId),
  ])

  return makeProviderAccountBundle(accounts, attempts, now)
}

export const startChatGptCodexDeviceLogin = async (
  repository: ProviderAccountRepository,
  input: StartDeviceLoginInput,
  startDeviceLogin: StartCodexDeviceLogin,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
    storeStartedDeviceLogin?: StoreStartedCodexDeviceLogin | undefined
  }> = {},
): Promise<
  Readonly<{
    account: PublicProviderAccount
    attempt: PublicProviderConnectionAttempt
    expiresAt: string
    intervalSeconds: number
    providerAccountRef: string
    verificationUrl: string
    userCode: string
  }>
> => {
  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )
  const explicitAccount =
    requestedProviderAccountRef === undefined
      ? undefined
      : await repository.findAccountByRef(
          input.userId,
          requestedProviderAccountRef,
        )

  if (
    requestedProviderAccountRef !== undefined &&
    explicitAccount === undefined
  ) {
    throw new ProviderAccountNotFound({
      message: 'Provider account not found.',
    })
  }

  const reusableAccount =
    requestedProviderAccountRef !== undefined || input.createNew === true
      ? undefined
      : await repository.findReusableAccount(input.userId)
  const previous = explicitAccount ?? reusableAccount
  const deviceLogin = await startDeviceLogin()
  const providerAccountRef =
    previous?.providerAccountRef ?? `provider-account_${makeId('ref')}`
  const accountLabel = normalizeAccountLabel(input.accountLabel)
  const accountAlreadyExists = previous !== undefined
  const nextStatus =
    previous?.status === 'connected' ? previous.status : 'pending'
  const nextHealth =
    previous?.status === 'connected' ? previous.health : 'unknown'
  const metadataAccountLabel =
    accountLabel ?? previous?.accountLabel ?? undefined
  const metadataPlanType = previous?.planType ?? undefined
  const account: ProviderAccountRecord = {
    id: previous?.id ?? makeId('provider_account'),
    userId: input.userId,
    teamId: previous?.teamId ?? null,
    provider: CHATGPT_CODEX_PROVIDER,
    authMode: 'chatgpt_device_code',
    status: nextStatus,
    health: nextHealth,
    providerAccountRef,
    secretRef: previous?.secretRef ?? null,
    accountLabel: accountLabel ?? previous?.accountLabel ?? null,
    planType: previous?.planType ?? null,
    connectedAt: previous?.connectedAt ?? null,
    disconnectedAt:
      nextStatus === 'pending' ? null : (previous?.disconnectedAt ?? null),
    deniedAt: nextStatus === 'pending' ? null : (previous?.deniedAt ?? null),
    lastStatusAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'worker_device_code',
      status: nextStatus,
      ...(metadataAccountLabel === undefined
        ? {}
        : { accountLabel: metadataAccountLabel }),
      ...(metadataPlanType === undefined ? {} : { planType: metadataPlanType }),
    }),
    createdAt: previous?.createdAt ?? now,
    updatedAt: now,
    deletedAt: null,
  }
  const attempt: ProviderConnectionAttemptRecord = {
    id: makeId('provider_attempt'),
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    provider: CHATGPT_CODEX_PROVIDER,
    method: 'chatgpt_device_code',
    source: 'worker_device_code',
    loginRef: makeId('codex_login'),
    verificationUrl: deviceLogin.verificationUrl,
    userCode: deviceLogin.userCode,
    status: 'pending',
    expiresAt: deviceLogin.expiresAt,
    completedAt: null,
    failedAt: null,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'worker_device_code',
      status: 'pending',
    }),
    createdAt: now,
    updatedAt: now,
  }
  const event = makeEvent({
    id: makeId('provider_event'),
    kind: 'login_started',
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    summary: 'ChatGPT/Codex device login started.',
    targetRef: providerAccountRef,
    metadata: {
      source: 'worker_device_code',
      status: 'pending',
    },
    createdAt: now,
  })

  await options.storeStartedDeviceLogin?.({
    attemptId: attempt.id,
    deviceAuthId: deviceLogin.deviceAuthId,
    userCode: deviceLogin.userCode,
    expiresAt: deviceLogin.expiresAt,
  })

  await repository.saveStartedDeviceLogin(
    account,
    attempt,
    event,
    accountAlreadyExists,
  )

  return {
    account: toPublicProviderAccount(account, [attempt], nowDate),
    attempt: toPublicProviderConnectionAttempt(attempt, account, nowDate),
    expiresAt: deviceLogin.expiresAt,
    intervalSeconds: deviceLogin.intervalSeconds,
    providerAccountRef,
    verificationUrl: deviceLogin.verificationUrl,
    userCode: deviceLogin.userCode,
  }
}

export const getDeviceLoginAttemptForUser = async (
  repository: ProviderAccountRepository,
  userId: string,
  attemptId: string,
  now = systemProviderAccountRuntime.now(),
): Promise<
  | Readonly<{
      account: PublicProviderAccount
      attempt: PublicProviderConnectionAttempt
    }>
  | undefined
> => {
  const record = await repository.findAttemptForUser(userId, attemptId)

  if (record === undefined) {
    return undefined
  }

  return {
    account: toPublicProviderAccount(record.account, [record.attempt], now),
    attempt: toPublicProviderConnectionAttempt(
      record.attempt,
      record.account,
      now,
    ),
  }
}

export const refreshChatGptCodexDeviceLoginForUser = async (
  repository: ProviderAccountRepository,
  input: Readonly<{
    userId: string
    attemptId: string
  }>,
  readStartedDeviceLogin: ReadStartedCodexDeviceLogin,
  storeConnectedAuth: StoreConnectedCodexAuth,
  pollDeviceLogin: PollCodexDeviceLogin,
  deleteStartedDeviceLogin: DeleteStartedCodexDeviceLogin = () =>
    Promise.resolve(),
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<
  | Readonly<{
      account: PublicProviderAccount
      attempt: PublicProviderConnectionAttempt
    }>
  | undefined
> => {
  const record = await repository.findAttemptForUser(
    input.userId,
    input.attemptId,
  )

  if (record === undefined) {
    return undefined
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()

  if (record.attempt.status !== 'pending') {
    return {
      account: toPublicProviderAccount(
        record.account,
        [record.attempt],
        nowDate,
      ),
      attempt: toPublicProviderConnectionAttempt(
        record.attempt,
        record.account,
        nowDate,
      ),
    }
  }

  if (isAttemptExpired(record.attempt, nowDate)) {
    return recordDeviceLoginFailed(
      repository,
      {
        actorId: input.userId,
        attemptId: input.attemptId,
        reason: 'Device login expired before completion.',
        status: 'expired',
      },
      options,
    )
  }

  const secret = await readStartedDeviceLogin(input.attemptId)

  if (secret === undefined) {
    return recordDeviceLoginFailed(
      repository,
      {
        actorId: input.userId,
        attemptId: input.attemptId,
        reason: 'Device login transient state missing; start a new connection.',
        status: 'failed',
      },
      options,
    )
  }

  const pollResult = await pollDeviceLogin(secret)

  if (pollResult.status === 'pending') {
    return {
      account: toPublicProviderAccount(
        record.account,
        [record.attempt],
        nowDate,
      ),
      attempt: toPublicProviderConnectionAttempt(
        record.attempt,
        record.account,
        nowDate,
      ),
    }
  }

  if (pollResult.status === 'failed') {
    return recordDeviceLoginFailed(
      repository,
      {
        actorId: input.userId,
        attemptId: input.attemptId,
        reason: pollResult.reason ?? 'Device login polling failed.',
        status: 'failed',
      },
      options,
    )
  }

  const secretRef = await storeConnectedAuth({
    ownerUserId: record.account.userId,
    providerAccountRef: record.account.providerAccountRef,
    auth: pollResult.auth,
  })
  const connected = await recordDeviceLoginConnected(
    repository,
    {
      accountLabel: pollResult.accountLabel,
      actorId: input.userId,
      attemptId: input.attemptId,
      planType: pollResult.planType,
      providerAccountRef: record.account.providerAccountRef,
      secretRef,
    },
    options,
  )

  await deleteStartedDeviceLogin(input.attemptId)

  return connected
}

export const recordDeviceLoginConnected = async (
  repository: ProviderAccountRepository,
  input: RecordConnectedInput,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<
  | Readonly<{
      account: PublicProviderAccount
      attempt: PublicProviderConnectionAttempt
    }>
  | undefined
> => {
  const record = await repository.findAttemptById(input.attemptId)

  if (record === undefined) {
    return undefined
  }

  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )

  if (
    requestedProviderAccountRef !== undefined &&
    requestedProviderAccountRef !== record.account.providerAccountRef
  ) {
    throw new ProviderAccountRefMismatch({
      message: 'Provider account ref does not match device login attempt.',
    })
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()

  if (record.attempt.status === 'connected') {
    return {
      account: toPublicProviderAccount(
        record.account,
        [record.attempt],
        nowDate,
      ),
      attempt: toPublicProviderConnectionAttempt(
        record.attempt,
        record.account,
        nowDate,
      ),
    }
  }

  if (record.attempt.status !== 'pending') {
    throw new ProviderDeviceLoginAttemptNotPending({
      message: 'Device login attempt is not pending.',
    })
  }

  if (isAttemptExpired(record.attempt, nowDate)) {
    throw new ProviderDeviceLoginAttemptExpired({
      message: 'Device login attempt is expired.',
    })
  }

  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const accountLabel =
    sanitizeOrRejectSecretText(input.accountLabel, 120, 'accountLabel') ??
    record.account.accountLabel
  const planType =
    sanitizeOrRejectSecretText(input.planType, 80, 'planType') ??
    record.account.planType
  const secretRef = requirePublicSecretReference(
    input.secretRef ?? `codex-auth://${record.account.providerAccountRef}`,
  )
  const account: ProviderAccountRecord = {
    ...record.account,
    accountLabel,
    connectedAt: now,
    deniedAt: null,
    disconnectedAt: null,
    health: 'healthy',
    lastStatusAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef: record.account.providerAccountRef,
      status: 'connected',
      source: 'broker_callback',
      ...(accountLabel === null ? {} : { accountLabel }),
      ...(planType === null ? {} : { planType }),
    }),
    planType,
    secretRef,
    status: 'connected',
    updatedAt: now,
  }
  const attempt: ProviderConnectionAttemptRecord = {
    ...record.attempt,
    completedAt: now,
    failedAt: null,
    loginRef: null,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef: record.account.providerAccountRef,
      source: 'broker_callback',
      status: 'connected',
    }),
    status: 'connected',
    updatedAt: now,
    userCode: null,
    verificationUrl: null,
  }
  const event = makeEvent({
    id: makeId('provider_event'),
    kind: 'login_connected',
    providerAccountId: account.id,
    userId: account.userId,
    teamId: account.teamId,
    summary: 'ChatGPT/Codex account connection was verified by a broker.',
    targetRef: account.providerAccountRef,
    metadata: {
      source: 'broker_callback',
      status: 'connected',
    },
    actorId: input.actorId,
    sourceRefs: [`actor:${input.actorId}`, `attempt:${attempt.id}`],
    createdAt: now,
  })
  const updated = await repository.recordConnectedAttempt(
    account,
    attempt,
    event,
  )

  return {
    account: toPublicProviderAccount(updated, [attempt], nowDate),
    attempt: toPublicProviderConnectionAttempt(attempt, updated, nowDate),
  }
}

export const recordDeviceLoginFailed = async (
  repository: ProviderAccountRepository,
  input: RecordFailedInput,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<
  | Readonly<{
      account: PublicProviderAccount
      attempt: PublicProviderConnectionAttempt
    }>
  | undefined
> => {
  const record = await repository.findAttemptById(input.attemptId)

  if (record === undefined) {
    return undefined
  }

  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )

  if (
    requestedProviderAccountRef !== undefined &&
    requestedProviderAccountRef !== record.account.providerAccountRef
  ) {
    throw new ProviderAccountRefMismatch({
      message: 'Provider account ref does not match device login attempt.',
    })
  }

  if (record.attempt.status === 'connected') {
    throw new ProviderDeviceLoginAttemptAlreadyConnected({
      message: 'Connected device login attempts cannot be marked failed.',
    })
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const requestedStatus = input.status ?? 'failed'
  const attemptStatus =
    requestedStatus === 'failed'
      ? 'failed'
      : requestedStatus === 'denied'
        ? 'denied'
        : 'expired'
  const accountStatus: ProviderAccountStatus =
    attemptStatus === 'expired' ? 'expired' : 'denied'
  const eventKind: ProviderAccountEventKind =
    attemptStatus === 'expired'
      ? 'login_expired'
      : attemptStatus === 'denied'
        ? 'login_denied'
        : 'login_failed'
  const now = nowDate.toISOString()
  const reason = sanitizeOrRejectSecretText(input.reason, 240, 'reason')
  const account: ProviderAccountRecord = {
    ...record.account,
    deniedAt: accountStatus === 'denied' ? now : record.account.deniedAt,
    health: 'requires_reauth',
    lastStatusAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef: record.account.providerAccountRef,
      source: 'broker_callback',
      status: accountStatus,
    }),
    status: accountStatus,
    updatedAt: now,
  }
  const attempt: ProviderConnectionAttemptRecord = {
    ...record.attempt,
    failedAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef: record.account.providerAccountRef,
      source: 'broker_callback',
      status: attemptStatus,
    }),
    status: attemptStatus,
    updatedAt: now,
  }
  const event = makeEvent({
    id: runtime.makeId('provider_event'),
    kind: eventKind,
    providerAccountId: account.id,
    userId: account.userId,
    teamId: account.teamId,
    summary:
      reason === undefined
        ? 'ChatGPT/Codex account connection was not completed.'
        : `ChatGPT/Codex account connection was not completed: ${reason}`,
    targetRef: account.providerAccountRef,
    metadata: {
      source: 'broker_callback',
      status: attemptStatus,
    },
    actorId: input.actorId,
    sourceRefs: [`actor:${input.actorId}`, `attempt:${attempt.id}`],
    createdAt: now,
  })
  const updated = await repository.recordFailedAttempt(account, attempt, event)

  return {
    account: toPublicProviderAccount(updated, [attempt], nowDate),
    attempt: toPublicProviderConnectionAttempt(attempt, updated, nowDate),
  }
}

const requireCodexOAuthText = (
  value: string | undefined,
  fieldName: string,
): string => {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new ProviderAccountCredentialMaterial({
      fieldName,
      message: 'Codex local auth material is missing a required field.',
    })
  }

  return value
}

const normalizeImportedCodexOAuthAuth = (
  auth: CodexOAuthAuth,
  now: Date,
): CodexOAuthAuth => {
  const access = requireCodexOAuthText(auth.access, 'auth.access')
  const refresh = requireCodexOAuthText(auth.refresh, 'auth.refresh')
  const expires =
    Number.isFinite(auth.expires) && auth.expires > now.getTime()
      ? auth.expires
      : now.getTime() + 1000 * 60 * 60

  return {
    type: 'oauth',
    access,
    refresh,
    expires,
    ...(auth.accountId === undefined
      ? {}
      : { accountId: requireCodexOAuthText(auth.accountId, 'auth.accountId') }),
    ...(auth.idToken === undefined
      ? {}
      : { idToken: requireCodexOAuthText(auth.idToken, 'auth.idToken') }),
  }
}

export const connectChatGptCodexLocalAuthForUser = async (
  repository: ProviderAccountRepository,
  input: Readonly<{
    userId: string
    auth: CodexOAuthAuth
    accountLabel?: string | undefined
    createNew?: boolean | undefined
    providerAccountRef?: string | undefined
  }>,
  storeConnectedAuth: StoreConnectedCodexAuth,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<
  Readonly<{
    account: PublicProviderAccount
    attempt: PublicProviderConnectionAttempt
    providerAccountRef: string
    generatedAt: string
  }>
> => {
  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )
  const explicitAccount =
    requestedProviderAccountRef === undefined
      ? undefined
      : await repository.findAccountByRef(
          input.userId,
          requestedProviderAccountRef,
        )

  if (
    requestedProviderAccountRef !== undefined &&
    explicitAccount === undefined
  ) {
    throw new ProviderAccountNotFound({
      message: 'Provider account not found.',
    })
  }

  if (
    explicitAccount !== undefined &&
    explicitAccount.provider !== CHATGPT_CODEX_PROVIDER
  ) {
    throw new ProviderAccountRefMismatch({
      message: 'Provider account ref belongs to a different provider.',
    })
  }

  const reusableAccount =
    requestedProviderAccountRef !== undefined || input.createNew === true
      ? undefined
      : await repository.findReusableAccount(input.userId)
  const previous = explicitAccount ?? reusableAccount
  const providerAccountRef =
    previous?.providerAccountRef ?? `provider-account_${makeId('ref')}`
  const normalizedAuth = normalizeImportedCodexOAuthAuth(input.auth, nowDate)
  const secretRef = await storeConnectedAuth({
    ownerUserId: input.userId,
    providerAccountRef,
    auth: normalizedAuth,
  })
  const accountLabel =
    normalizeAccountLabel(input.accountLabel) ??
    normalizeAccountLabel(normalizedAuth.accountId) ??
    previous?.accountLabel ??
    null
  const account: ProviderAccountRecord = {
    id: previous?.id ?? makeId('provider_account'),
    userId: input.userId,
    teamId: previous?.teamId ?? null,
    provider: CHATGPT_CODEX_PROVIDER,
    authMode: 'codex_device_auth',
    status: 'connected',
    health: 'healthy',
    providerAccountRef,
    secretRef,
    accountLabel,
    planType: previous?.planType ?? null,
    connectedAt: now,
    disconnectedAt: null,
    deniedAt: null,
    lastStatusAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'pylon_local_codex_auth',
      status: 'connected',
      ...(accountLabel === null ? {} : { accountLabel }),
      ...(previous?.planType === null || previous?.planType === undefined
        ? {}
        : { planType: previous.planType }),
    }),
    createdAt: previous?.createdAt ?? now,
    updatedAt: now,
    deletedAt: null,
  }
  const attempt: ProviderConnectionAttemptRecord = {
    id: makeId('provider_attempt'),
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    provider: CHATGPT_CODEX_PROVIDER,
    method: 'codex_device_auth',
    source: 'pylon_local_codex_auth',
    loginRef: null,
    verificationUrl: null,
    userCode: null,
    status: 'connected',
    expiresAt: now,
    completedAt: now,
    failedAt: null,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'pylon_local_codex_auth',
      status: 'connected',
    }),
    createdAt: now,
    updatedAt: now,
  }
  const event = makeEvent({
    id: makeId('provider_event'),
    kind: 'login_connected',
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    summary:
      'ChatGPT/Codex account connected from a linked Pylon local Codex login.',
    targetRef: providerAccountRef,
    metadata: {
      source: 'pylon_local_codex_auth',
      status: 'connected',
    },
    actorId: input.userId,
    sourceRefs: [`actor:${input.userId}`, `attempt:${attempt.id}`],
    createdAt: now,
  })

  await repository.saveStartedDeviceLogin(account, attempt, event, previous !== undefined)

  return {
    account: toPublicProviderAccount(account, [attempt], nowDate),
    attempt: toPublicProviderConnectionAttempt(attempt, account, nowDate),
    providerAccountRef,
    generatedAt: now,
  }
}

/**
 * CX-5 (#8549): local-auth/import custody write for a Claude subscription
 * account — the write-side counterpart to `handlePylonProviderClaudeAuthMaterialApi`
 * (the broker READ that CX-5's first pass landed). Mirrors
 * `connectChatGptCodexLocalAuthForUser` exactly, generalized for Claude Code's
 * simpler credential shape: the owner runs `claude setup-token` locally
 * (never `claude login` against a live default session — same custody
 * discipline as Codex's `codex login --device-auth`) and imports the
 * resulting long-lived `CLAUDE_CODE_OAUTH_TOKEN` bearer string, which has no
 * access/refresh/expires triple to normalize or refresh, unlike
 * `CodexOAuthAuth`.
 */
export const connectClaudeLocalAuthForUser = async (
  repository: ProviderAccountRepository,
  input: Readonly<{
    userId: string
    authContentValue: string
    accountLabel?: string | undefined
    createNew?: boolean | undefined
    providerAccountRef?: string | undefined
  }>,
  storeConnectedAuth: StoreConnectedClaudeAuth,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<
  Readonly<{
    account: PublicProviderAccount
    attempt: PublicProviderConnectionAttempt
    providerAccountRef: string
    generatedAt: string
  }>
> => {
  const authContentValue = input.authContentValue.trim()
  if (authContentValue === '') {
    throw new ProviderAccountCredentialMaterial({
      fieldName: 'auth.authContentValue',
      message: 'Claude local auth material is missing a required field.',
    })
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )
  const explicitAccount =
    requestedProviderAccountRef === undefined
      ? undefined
      : await repository.findAccountByRef(
          input.userId,
          requestedProviderAccountRef,
        )

  if (
    requestedProviderAccountRef !== undefined &&
    explicitAccount === undefined
  ) {
    throw new ProviderAccountNotFound({
      message: 'Provider account not found.',
    })
  }

  if (
    explicitAccount !== undefined &&
    explicitAccount.provider !== ANTHROPIC_CLAUDE_PROVIDER
  ) {
    throw new ProviderAccountRefMismatch({
      message: 'Provider account ref belongs to a different provider.',
    })
  }

  const reusableAccount =
    requestedProviderAccountRef !== undefined || input.createNew === true
      ? undefined
      : await repository.findReusableAccount(input.userId)
  const previous = explicitAccount ?? reusableAccount
  const providerAccountRef =
    previous?.providerAccountRef ?? `provider-account_${makeId('ref')}`
  const secretRef = await storeConnectedAuth({
    ownerUserId: input.userId,
    providerAccountRef,
    authContentValue,
  })
  const accountLabel =
    normalizeAccountLabel(input.accountLabel) ?? previous?.accountLabel ?? null
  const account: ProviderAccountRecord = {
    id: previous?.id ?? makeId('provider_account'),
    userId: input.userId,
    teamId: previous?.teamId ?? null,
    provider: ANTHROPIC_CLAUDE_PROVIDER,
    authMode: 'claude_local_auth',
    status: 'connected',
    health: 'healthy',
    providerAccountRef,
    secretRef,
    accountLabel,
    planType: previous?.planType ?? null,
    connectedAt: now,
    disconnectedAt: null,
    deniedAt: null,
    lastStatusAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'pylon_local_claude_auth',
      status: 'connected',
      ...(accountLabel === null ? {} : { accountLabel }),
      ...(previous?.planType === null || previous?.planType === undefined
        ? {}
        : { planType: previous.planType }),
    }),
    createdAt: previous?.createdAt ?? now,
    updatedAt: now,
    deletedAt: null,
  }
  const attempt: ProviderConnectionAttemptRecord = {
    id: makeId('provider_attempt'),
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    provider: ANTHROPIC_CLAUDE_PROVIDER,
    method: 'claude_local_auth',
    source: 'pylon_local_claude_auth',
    loginRef: null,
    verificationUrl: null,
    userCode: null,
    status: 'connected',
    expiresAt: now,
    completedAt: now,
    failedAt: null,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'pylon_local_claude_auth',
      status: 'connected',
    }),
    createdAt: now,
    updatedAt: now,
  }
  const event = makeEvent({
    id: makeId('provider_event'),
    kind: 'login_connected',
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    summary:
      'Anthropic Claude account connected from a linked Pylon local Claude login.',
    targetRef: providerAccountRef,
    metadata: {
      source: 'pylon_local_claude_auth',
      status: 'connected',
    },
    actorId: input.userId,
    sourceRefs: [`actor:${input.userId}`, `attempt:${attempt.id}`],
    createdAt: now,
  })

  await repository.saveStartedDeviceLogin(account, attempt, event, previous !== undefined)

  return {
    account: toPublicProviderAccount(account, [attempt], nowDate),
    attempt: toPublicProviderConnectionAttempt(attempt, account, nowDate),
    providerAccountRef,
    generatedAt: now,
  }
}

export const recordProviderAccountHealth = async (
  repository: ProviderAccountRepository,
  input: RecordHealthInput,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<PublicProviderAccount | undefined> => {
  const providerAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )

  if (providerAccountRef === undefined) {
    return undefined
  }

  const account =
    await repository.findAccountByProviderAccountRef(providerAccountRef)

  if (account === undefined) {
    return undefined
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const reason = sanitizeOrRejectSecretText(input.reason, 240, 'reason')
  const nextStatus: ProviderAccountStatus =
    input.health === 'healthy' && account.secretRef !== null
      ? 'connected'
      : input.health === 'healthy'
        ? account.status
        : 'unhealthy'
  const updatedAccount: ProviderAccountRecord = {
    ...account,
    health: input.health,
    lastStatusAt: now,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'broker_health',
      status: nextStatus,
    }),
    status: nextStatus,
    updatedAt: now,
  }
  const event = makeEvent({
    id: runtime.makeId('provider_event'),
    kind: 'account_health_updated',
    providerAccountId: account.id,
    userId: account.userId,
    teamId: account.teamId,
    summary:
      reason === undefined
        ? `${providerDisplayName(account.provider)} account health marked ${input.health}.`
        : `${providerDisplayName(account.provider)} account health marked ${input.health}: ${reason}`,
    targetRef: providerAccountRef,
    metadata: {
      source: 'broker_health',
      status: nextStatus,
    },
    actorId: input.actorId,
    sourceRefs: [`actor:${input.actorId}`],
    createdAt: now,
  })
  const saved = await repository.recordAccountHealth(
    providerAccountRef,
    updatedAccount,
    event,
  )

  return saved === undefined
    ? undefined
    : toPublicProviderAccount(saved, [], nowDate)
}

export const issueProviderAccountGrant = async (
  repository: ProviderAccountRepository,
  input: IssueProviderAccountGrantInput,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<PublicProviderAccountGrant | undefined> => {
  const account = await repository.findAccountByRef(
    input.userId,
    input.providerAccountRef,
  )

  if (account === undefined) {
    return undefined
  }

  if (
    account.status !== 'connected' ||
    account.health !== 'healthy' ||
    account.secretRef === null
  ) {
    throw new ProviderAccountNotConnectedHealthy({
      message: 'Provider account is not connected and healthy.',
    })
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const expiresAt = addMilliseconds(nowDate, SESSION_GRANT_TTL_MS)
  const makeId = runtime.makeId
  const requestedAction = sanitizeOrRejectSecretText(
    input.requestedAction,
    160,
    'requestedAction',
  )
  const threadId = sanitizeOrRejectSecretText(input.threadId, 160, 'threadId')
  const workroomId = sanitizeOrRejectSecretText(
    input.workroomId,
    160,
    'workroomId',
  )
  const runnerSessionId = sanitizeOrRejectSecretText(
    input.runnerSessionId,
    180,
    'runnerSessionId',
  )
  const grantRefPrefix =
    account.provider === CHATGPT_CODEX_PROVIDER
      ? 'codex-auth-grant'
      : 'provider-auth-grant'
  const grantRef = `${grantRefPrefix}_${makeId('grant_ref')}`
  const grant: ProviderAccountAuthGrantRecord = {
    id: makeId('provider_grant'),
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    threadId: threadId ?? null,
    workroomId: workroomId ?? null,
    runnerSessionId: runnerSessionId ?? null,
    provider: account.provider,
    providerAccountRef: account.providerAccountRef,
    providerSecretRef: requirePublicSecretReference(account.secretRef),
    grantRef,
    status: 'issued',
    requestedAction: requestedAction ?? null,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef: account.providerAccountRef,
      source: 'browser_grant_issue',
      status: 'issued',
    }),
    createdAt: now,
    updatedAt: now,
    expiresAt,
    usedAt: null,
    revokedAt: null,
    failedAt: null,
  }
  const event = makeEvent({
    id: makeId('provider_event'),
    kind: 'auth_grant_issued',
    providerAccountId: account.id,
    userId: input.userId,
    teamId: account.teamId,
    summary:
      account.provider === CHATGPT_CODEX_PROVIDER
        ? 'Session-scoped Codex auth grant issued for a runner.'
        : `Session-scoped ${providerDisplayName(account.provider)} API grant issued for a runner.`,
    targetRef: grantRef,
    metadata: {
      source: 'browser_grant_issue',
      status: 'issued',
    },
    sourceRefs: [`providerAccount:${account.providerAccountRef}`],
    evidenceRefs: [`providerAccountAuthGrant:${grantRef}`],
    createdAt: now,
  })
  const saved = await repository.createAuthGrant(grant, event)

  return toPublicProviderAccountGrant(saved, nowDate)
}

export const resolveProviderAccountGrant = async (
  repository: ProviderAccountRepository,
  input: ResolveProviderAccountGrantInput,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<ResolvedProviderAccountGrant | undefined> => {
  const grantRef = sanitizeOrRejectSecretText(input.grantRef, 220, 'grantRef')

  if (grantRef === undefined) {
    return undefined
  }

  const grant = await repository.findGrantByRef(grantRef)

  if (grant === undefined) {
    return undefined
  }

  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )

  if (
    requestedProviderAccountRef !== undefined &&
    requestedProviderAccountRef !== grant.providerAccountRef
  ) {
    throw new ProviderGrantAccountMismatch({
      message: 'Grant provider account does not match request.',
    })
  }

  const runnerSessionId = sanitizeOrRejectSecretText(
    input.runnerSessionId,
    180,
    'runnerSessionId',
  )

  if (
    grant.runnerSessionId !== null &&
    runnerSessionId !== undefined &&
    grant.runnerSessionId !== runnerSessionId
  ) {
    throw new ProviderGrantRunnerSessionMismatch({
      message: 'Grant runner session does not match request.',
    })
  }

  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()

  if (Date.parse(grant.expiresAt) <= nowDate.getTime()) {
    throw new ProviderGrantExpired({
      message: 'Grant is expired.',
    })
  }

  if (grant.status !== 'issued') {
    throw new ProviderGrantNotIssued({
      message: 'Grant is not issued.',
    })
  }

  const now = nowDate.toISOString()
  const usedGrant: ProviderAccountAuthGrantRecord = {
    ...grant,
    status: 'used',
    updatedAt: now,
    usedAt: now,
  }
  const event = makeEvent({
    id: runtime.makeId('provider_event'),
    kind: 'auth_grant_used',
    providerAccountId: grant.providerAccountId,
    userId: grant.userId,
    teamId: grant.teamId,
    summary:
      grant.provider === CHATGPT_CODEX_PROVIDER
        ? 'Runner resolved a session-scoped Codex auth grant.'
        : `Runner resolved a session-scoped ${providerDisplayName(grant.provider)} API grant.`,
    targetRef: grant.grantRef,
    metadata: {
      source: 'runner_grant_resolve',
      status: 'used',
    },
    actorId: input.actorId,
    sourceRefs: [
      `actor:${input.actorId}`,
      `providerAccount:${grant.providerAccountRef}`,
    ],
    evidenceRefs: [`providerAccountAuthGrant:${grant.grantRef}`],
    createdAt: now,
  })
  const saved = await repository.markGrantUsed(usedGrant, event)

  return {
    grantRef: saved.grantRef,
    ownerUserId: saved.userId,
    provider: saved.provider,
    providerAccountRef: saved.providerAccountRef,
    providerSecretRef: saved.providerSecretRef,
    requestedAction: textOrUndefined(saved.requestedAction),
    runnerSessionId: textOrUndefined(saved.runnerSessionId),
    expiresAt: saved.expiresAt,
    status: 'used',
    materialization:
      saved.provider === CHATGPT_CODEX_PROVIDER
        ? buildRedactedOpenCodeMaterializationPlan(saved.providerSecretRef)
        : saved.provider === ANTHROPIC_CLAUDE_PROVIDER
          ? buildRedactedAnthropicApiKeyMaterializationPlan(
              saved.providerSecretRef,
            )
          : buildRedactedGeminiApiKeyMaterializationPlan(
              saved.providerSecretRef,
            ),
  }
}

export const disconnectProviderAccountForUser = async (
  repository: ProviderAccountRepository,
  userId: string,
  providerAccountRef: string,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<PublicProviderAccount | undefined> => {
  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const normalizedProviderAccountRef =
    normalizeProviderAccountRef(providerAccountRef)

  if (normalizedProviderAccountRef === undefined) {
    return undefined
  }

  const secretRef = requirePublicSecretReference(
    `codex-auth://${normalizedProviderAccountRef}`,
  )
  const metadataJson = providerAccountPublicMetadataJson({
    providerAccountRef: normalizedProviderAccountRef,
    source: secretRef,
    status: 'disconnected',
  })
  const event = makeEvent({
    id: runtime.makeId('provider_event'),
    kind: 'account_disconnected',
    providerAccountId: null,
    userId,
    teamId: null,
    summary:
      'Provider account was disconnected and outstanding grants were revoked.',
    targetRef: normalizedProviderAccountRef,
    metadata: {
      source: 'browser',
      status: 'disconnected',
    },
    createdAt: now,
  })
  const account = await repository.disconnectAccount(
    userId,
    normalizedProviderAccountRef,
    now,
    metadataJson,
    event,
  )

  return account === undefined
    ? undefined
    : toPublicProviderAccount(account, [], nowDate)
}

export type ProviderAccountLifecycleServiceDependencies = Readonly<{
  repository: ProviderAccountRepository
  startDeviceLogin: StartCodexDeviceLogin
  pollDeviceLogin: PollCodexDeviceLogin
  readStartedDeviceLogin: ReadStartedCodexDeviceLogin
  storeStartedDeviceLogin?: StoreStartedCodexDeviceLogin | undefined
  storeConnectedAuth: StoreConnectedCodexAuth
  deleteStartedDeviceLogin?: DeleteStartedCodexDeviceLogin | undefined
  now?: (() => Date) | undefined
  makeId?: IdFactory | undefined
}>

export type ProviderAccountLifecycleServiceShape = Readonly<{
  listForUser: (
    userId: string,
  ) => Effect.Effect<ProviderAccountBundle, ProviderAccountError>
  startChatGptCodexDeviceLogin: (
    input: StartDeviceLoginInput,
  ) => Effect.Effect<
    Readonly<{
      account: PublicProviderAccount
      attempt: PublicProviderConnectionAttempt
      expiresAt: string
      intervalSeconds: number
      providerAccountRef: string
      verificationUrl: string
      userCode: string
    }>,
    ProviderAccountError
  >
  refreshChatGptCodexDeviceLoginForUser: (
    input: Readonly<{
      userId: string
      attemptId: string
    }>,
  ) => Effect.Effect<
    | Readonly<{
        account: PublicProviderAccount
        attempt: PublicProviderConnectionAttempt
      }>
    | undefined,
    ProviderAccountError
  >
  recordDeviceLoginConnected: (
    input: RecordConnectedInput,
  ) => Effect.Effect<
    | Readonly<{
        account: PublicProviderAccount
        attempt: PublicProviderConnectionAttempt
      }>
    | undefined,
    ProviderAccountError
  >
  recordDeviceLoginFailed: (
    input: RecordFailedInput,
  ) => Effect.Effect<
    | Readonly<{
        account: PublicProviderAccount
        attempt: PublicProviderConnectionAttempt
      }>
    | undefined,
    ProviderAccountError
  >
  recordProviderAccountHealth: (
    input: RecordHealthInput,
  ) => Effect.Effect<PublicProviderAccount | undefined, ProviderAccountError>
  issueProviderAccountGrant: (
    input: IssueProviderAccountGrantInput,
  ) => Effect.Effect<PublicProviderAccountGrant | undefined, ProviderAccountError>
  resolveProviderAccountGrant: (
    input: ResolveProviderAccountGrantInput,
  ) => Effect.Effect<
    ResolvedProviderAccountGrant | undefined,
    ProviderAccountError
  >
  disconnectProviderAccountForUser: (
    userId: string,
    providerAccountRef: string,
  ) => Effect.Effect<PublicProviderAccount | undefined, ProviderAccountError>
}>

export class ProviderAccountLifecycleService extends Context.Service<
  ProviderAccountLifecycleService,
  ProviderAccountLifecycleServiceShape
>()('openagents/ProviderAccountLifecycleService') {}

const lifecycleEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ProviderAccountError> =>
  Effect.tryPromise({
    try: run,
    catch: error => providerAccountErrorFromUnknown(operation, error),
  })

const lifecycleRuntimeOptions = (
  dependencies: ProviderAccountLifecycleServiceDependencies,
) => ({
  ...(dependencies.now === undefined ? {} : { now: dependencies.now }),
  ...(dependencies.makeId === undefined ? {} : { makeId: dependencies.makeId }),
})

export const makeProviderAccountLifecycleService = (
  dependencies: ProviderAccountLifecycleServiceDependencies,
): ProviderAccountLifecycleServiceShape => ({
  listForUser: userId =>
    lifecycleEffect('list_provider_accounts_for_user', () =>
      listProviderAccountsForUser(
        dependencies.repository,
        userId,
        dependencies.now?.() ?? systemProviderAccountRuntime.now(),
      ),
    ),
  startChatGptCodexDeviceLogin: input =>
    lifecycleEffect('start_chatgpt_codex_device_login', () =>
      startChatGptCodexDeviceLogin(
        dependencies.repository,
        input,
        dependencies.startDeviceLogin,
        {
          ...lifecycleRuntimeOptions(dependencies),
          ...(dependencies.storeStartedDeviceLogin === undefined
            ? {}
            : { storeStartedDeviceLogin: dependencies.storeStartedDeviceLogin }),
        },
      ),
    ),
  refreshChatGptCodexDeviceLoginForUser: input =>
    lifecycleEffect('refresh_chatgpt_codex_device_login_for_user', () =>
      refreshChatGptCodexDeviceLoginForUser(
        dependencies.repository,
        input,
        dependencies.readStartedDeviceLogin,
        dependencies.storeConnectedAuth,
        dependencies.pollDeviceLogin,
        dependencies.deleteStartedDeviceLogin,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
  recordDeviceLoginConnected: input =>
    lifecycleEffect('record_device_login_connected', () =>
      recordDeviceLoginConnected(
        dependencies.repository,
        input,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
  recordDeviceLoginFailed: input =>
    lifecycleEffect('record_device_login_failed', () =>
      recordDeviceLoginFailed(
        dependencies.repository,
        input,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
  recordProviderAccountHealth: input =>
    lifecycleEffect('record_provider_account_health', () =>
      recordProviderAccountHealth(
        dependencies.repository,
        input,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
  issueProviderAccountGrant: input =>
    lifecycleEffect('issue_provider_account_grant', () =>
      issueProviderAccountGrant(
        dependencies.repository,
        input,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
  resolveProviderAccountGrant: input =>
    lifecycleEffect('resolve_provider_account_grant', () =>
      resolveProviderAccountGrant(
        dependencies.repository,
        input,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
  disconnectProviderAccountForUser: (userId, providerAccountRef) =>
    lifecycleEffect('disconnect_provider_account_for_user', () =>
      disconnectProviderAccountForUser(
        dependencies.repository,
        userId,
        providerAccountRef,
        lifecycleRuntimeOptions(dependencies),
      ),
    ),
})

export const makeProviderAccountLifecycleLayer = (
  dependencies: ProviderAccountLifecycleServiceDependencies,
) =>
  Layer.succeed(
    ProviderAccountLifecycleService,
    makeProviderAccountLifecycleService(dependencies),
  )
