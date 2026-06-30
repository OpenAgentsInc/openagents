import {
  providerAccountPublicMetadataJson,
  requirePublicSecretReference,
} from '@openagentsinc/provider-account-schema'

import {
  ANTHROPIC_CLAUDE_PROVIDER,
  GOOGLE_GEMINI_PROVIDER,
  OPENROUTER_PROVIDER,
  type IdFactory,
  type ProviderAccountHealth,
  type ProviderAccountRecord,
  type ProviderAccountRepository,
  type ProviderConnectionAttemptRecord,
  type PublicProviderAccount,
  type PublicProviderConnectionAttempt,
  makeEvent,
  normalizeAccountLabel,
  normalizeProviderAccountRef,
  providerDisplayName,
  systemProviderAccountRuntime,
  toPublicProviderAccount,
  toPublicProviderConnectionAttempt,
} from './provider-account-domain'
import {
  ProviderAccountNotFound,
  ProviderAccountRefMismatch,
  ProviderApiKeyInvalid,
  ProviderApiKeyRejected,
} from './provider-account-errors'

/**
 * Provider peers connectable by API-key BYOK, per the ToS-compliance
 * review in
 * docs/autopilot-coder/2026-06-11-provider-peer-tos-compliance-review.md.
 * API-key BYOK is the only compliant connect shape for these providers:
 * subscription-account connect (Claude.ai/Pro/Max login, Google account
 * OAuth) is forbidden by the providers' terms and must not be added here
 * without a new dated review.
 */
export type ApiKeyConnectProvider =
  | typeof ANTHROPIC_CLAUDE_PROVIDER
  | typeof GOOGLE_GEMINI_PROVIDER
  | typeof OPENROUTER_PROVIDER

export type ProviderApiKeyConnectPolicy = Readonly<{
  provider: ApiKeyConnectProvider
  routeSegment: 'anthropic' | 'google-gemini' | 'openrouter'
  secretRefSegment: string
  probeUrl: string
  probeHeaders: (apiKey: string) => Readonly<Record<string, string>>
}>

export const PROVIDER_API_KEY_CONNECT_POLICIES: ReadonlyArray<ProviderApiKeyConnectPolicy> =
  [
    {
      provider: OPENROUTER_PROVIDER,
      routeSegment: 'openrouter',
      secretRefSegment: 'openrouter',
      probeUrl: 'https://openrouter.ai/api/v1/models',
      probeHeaders: apiKey => ({
        authorization: `Bearer ${apiKey}`,
      }),
    },
    {
      provider: ANTHROPIC_CLAUDE_PROVIDER,
      routeSegment: 'anthropic',
      secretRefSegment: 'anthropic',
      probeUrl: 'https://api.anthropic.com/v1/models?limit=1',
      probeHeaders: apiKey => ({
        'anthropic-version': '2023-06-01',
        'x-api-key': apiKey,
      }),
    },
    {
      provider: GOOGLE_GEMINI_PROVIDER,
      routeSegment: 'google-gemini',
      secretRefSegment: 'google-gemini',
      probeUrl:
        'https://generativelanguage.googleapis.com/v1beta/models?pageSize=1',
      probeHeaders: apiKey => ({
        'x-goog-api-key': apiKey,
      }),
    },
  ]

export const providerApiKeyConnectPolicyForRouteSegment = (
  routeSegment: string,
): ProviderApiKeyConnectPolicy | undefined =>
  PROVIDER_API_KEY_CONNECT_POLICIES.find(
    policy => policy.routeSegment === routeSegment,
  )

export const providerApiKeyConnectPolicyForProvider = (
  provider: ApiKeyConnectProvider,
): ProviderApiKeyConnectPolicy => {
  const policy = PROVIDER_API_KEY_CONNECT_POLICIES.find(
    candidate => candidate.provider === provider,
  )

  if (policy === undefined) {
    throw new ProviderApiKeyInvalid({
      message: 'Provider does not support API-key connect.',
    })
  }

  return policy
}

export const providerApiKeyUserSecretRef = (
  policy: ProviderApiKeyConnectPolicy,
  providerAccountRef: string,
): string =>
  requirePublicSecretReference(
    `provider-account://${policy.secretRefSegment}/user-api-key/${providerAccountRef}`,
  )

/**
 * Shape gate only: keys are otherwise opaque (Gemini key formats are
 * migrating through September 2026, and key shapes must not be
 * pattern-gated beyond redaction-safety bounds). The value is never
 * logged or echoed.
 */
export const requireProviderApiKeyShape = (value: unknown): string => {
  const apiKey = typeof value === 'string' ? value.trim() : ''

  if (apiKey.length < 8 || apiKey.length > 512 || /\s/.test(apiKey)) {
    throw new ProviderApiKeyInvalid({
      message: 'apiKey must be a single non-empty API key value.',
    })
  }

  return apiKey
}

export type ProviderApiKeyProbeResult = Readonly<{
  health: ProviderAccountHealth
  probeStatus: number | undefined
}>

export const providerApiKeyHealthFromStatus = (
  status: number,
): ProviderAccountHealth =>
  status === 200 || status === 429
    ? 'healthy'
    : status === 401 || status === 403
      ? 'requires_reauth'
      : status >= 500
        ? 'unknown'
        : 'unhealthy'

export type ProviderApiKeyProbe = (
  policy: ProviderApiKeyConnectPolicy,
  apiKey: string,
) => Promise<ProviderApiKeyProbeResult>

export const probeProviderApiKey =
  (fetchLike: typeof fetch = fetch): ProviderApiKeyProbe =>
  async (policy, apiKey) => {
    try {
      const response = await fetchLike(policy.probeUrl, {
        headers: policy.probeHeaders(apiKey),
        method: 'GET',
      })

      return {
        health: providerApiKeyHealthFromStatus(response.status),
        probeStatus: response.status,
      }
    } catch {
      return { health: 'unknown', probeStatus: undefined }
    }
  }

export type StoreConnectedProviderApiKey = (
  input: Readonly<{
    providerAccountRef: string
    provider: ApiKeyConnectProvider
    apiKey: string
  }>,
) => Promise<void>

export type ConnectProviderApiKeyAccountInput = Readonly<{
  userId: string
  provider: ApiKeyConnectProvider
  apiKey: string
  accountLabel?: string | undefined
  providerAccountRef?: string | undefined
}>

export type ConnectedProviderApiKeyAccount = Readonly<{
  account: PublicProviderAccount
  attempt: PublicProviderConnectionAttempt
  providerAccountRef: string
  generatedAt: string
}>

/**
 * Connects a provider-peer account by API-key BYOK: probes the key
 * against the provider, refuses keys the provider rejects, stores the
 * key through the injected secret store (auth KV), and persists only
 * the secret ref in durable records. Raw key material never reaches
 * D1 rows, events, projections, or logs.
 */
export const connectProviderApiKeyAccount = async (
  repository: ProviderAccountRepository,
  input: ConnectProviderApiKeyAccountInput,
  dependencies: Readonly<{
    probeApiKey: ProviderApiKeyProbe
    storeConnectedApiKey: StoreConnectedProviderApiKey
  }>,
  options: Readonly<{
    now?: () => Date
    makeId?: IdFactory
  }> = {},
): Promise<ConnectedProviderApiKeyAccount> => {
  const policy = providerApiKeyConnectPolicyForProvider(input.provider)
  const apiKey = requireProviderApiKeyShape(input.apiKey)
  const runtime = { ...systemProviderAccountRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const requestedProviderAccountRef = normalizeProviderAccountRef(
    input.providerAccountRef,
  )
  const previous =
    requestedProviderAccountRef === undefined
      ? undefined
      : await repository.findAccountByRef(
          input.userId,
          requestedProviderAccountRef,
        )

  if (requestedProviderAccountRef !== undefined && previous === undefined) {
    throw new ProviderAccountNotFound({
      message: 'Provider account not found.',
    })
  }

  if (previous !== undefined && previous.provider !== input.provider) {
    throw new ProviderAccountRefMismatch({
      message: 'Provider account ref belongs to a different provider.',
    })
  }

  const probed = await dependencies.probeApiKey(policy, apiKey)

  if (probed.health === 'requires_reauth') {
    throw new ProviderApiKeyRejected({
      message: `${providerDisplayName(input.provider)} rejected the API key. Check the key and try again.`,
    })
  }

  const providerAccountRef =
    previous?.providerAccountRef ?? `provider-account_${makeId('ref')}`
  const secretRef = providerApiKeyUserSecretRef(policy, providerAccountRef)
  const accountLabel =
    normalizeAccountLabel(input.accountLabel) ??
    previous?.accountLabel ??
    null
  const account: ProviderAccountRecord = {
    id: previous?.id ?? makeId('provider_account'),
    userId: input.userId,
    teamId: previous?.teamId ?? null,
    provider: input.provider,
    authMode: 'api_key',
    status: 'connected',
    health: probed.health,
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
      source: 'browser_api_key',
      status: 'connected',
      ...(accountLabel === null ? {} : { accountLabel }),
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
    provider: input.provider,
    method: 'provider_api_key',
    source: 'browser_api_key',
    loginRef: null,
    verificationUrl: null,
    userCode: null,
    status: 'connected',
    expiresAt: now,
    completedAt: now,
    failedAt: null,
    metadataJson: providerAccountPublicMetadataJson({
      providerAccountRef,
      source: 'browser_api_key',
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
    summary: `${providerDisplayName(input.provider)} account connected with the user's own API key.`,
    targetRef: providerAccountRef,
    metadata: {
      source: 'browser_api_key',
      status: 'connected',
    },
    sourceRefs: [`actor:${input.userId}`, `attempt:${attempt.id}`],
    createdAt: now,
  })

  await dependencies.storeConnectedApiKey({
    providerAccountRef,
    provider: input.provider,
    apiKey,
  })

  await repository.saveStartedDeviceLogin(
    account,
    attempt,
    event,
    previous !== undefined,
  )

  return {
    account: toPublicProviderAccount(account, [attempt], nowDate),
    attempt: toPublicProviderConnectionAttempt(attempt, account, nowDate),
    providerAccountRef,
    generatedAt: now,
  }
}
