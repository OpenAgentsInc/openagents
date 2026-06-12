import {
  decodeOpenAiDeviceCodeResponse,
  decodeOpenAiDeviceTokenResponse,
  decodeOpenAiOAuthTokenResponse,
} from '@openagentsinc/provider-account-schema'
import { Context, Effect, Layer } from 'effect'

import {
  CHATGPT_CODEX_VERIFICATION_URL,
  CODEX_CLIENT_ID,
  type CodexOAuthAuth,
  type CodexDeviceLoginPollResult,
  type StartedCodexDeviceLogin,
  type StartedCodexDeviceLoginSecret,
  extractCodexAccountId,
  extractCodexAccountLabel,
  normalizeVerificationUrl,
  parseExpiresAt,
  parseExpiresInSeconds,
  systemProviderAccountRuntime,
} from './provider-account-domain'
import {
  ProviderAccountClientRequestFailed,
  type ProviderAccountError,
  providerAccountErrorFromUnknown,
} from './provider-account-errors'

export type OpenAiCodexOAuthRefreshProbeResult =
  | Readonly<{ status: 'refreshed'; auth: CodexOAuthAuth }>
  | Readonly<{
      status: 'failed'
      code: string
      failureClass:
        | 'token_invalidated'
        | 'rate_limited'
        | 'provider_outage'
        | 'unknown_provider_failure'
      providerStatus: number
    }>

const openAiCodexDeviceLoginStartFailureMessage = (status: number): string =>
  status === 429
    ? 'OpenAI is rate limiting ChatGPT device login. Wait a minute, then try Reconnect ChatGPT again.'
    : `ChatGPT/Codex device login start failed with ${status}.`

const isJsonRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const nestedString = (
  value: unknown,
  path: ReadonlyArray<string>,
): string | undefined => {
  const found = path.reduce<unknown>(
    (current, key) => (isJsonRecord(current) ? current[key] : undefined),
    value,
  )

  return typeof found === 'string' && found.trim() !== ''
    ? found.trim()
    : undefined
}

const openAiRefreshFailureCode = (value: unknown): string =>
  nestedString(value, ['error', 'code']) ??
  nestedString(value, ['error']) ??
  nestedString(value, ['code']) ??
  'unknown_provider_failure'

const openAiRefreshFailureClass = (
  providerStatus: number,
  code: string,
):
  | 'token_invalidated'
  | 'rate_limited'
  | 'provider_outage'
  | 'unknown_provider_failure' =>
  code === 'refresh_token_invalidated' ||
  code === 'refresh_token_reused' ||
  code === 'refresh_token_expired' ||
  code === 'invalid_grant' ||
  code === 'invalid_token'
    ? 'token_invalidated'
    : providerStatus === 429 || code === 'rate_limited' || code === 'rate_limit'
      ? 'rate_limited'
      : providerStatus >= 500
        ? 'provider_outage'
        : 'unknown_provider_failure'

export const startOpenAiCodexDeviceLogin = async (
  fetcher: typeof fetch = fetch,
  now = systemProviderAccountRuntime.now(),
): Promise<StartedCodexDeviceLogin> => {
  const response = await fetcher(
    'https://auth.openai.com/api/accounts/deviceauth/usercode',
    {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'user-agent': 'OpenAgents/0.1',
      },
      body: JSON.stringify({ client_id: CODEX_CLIENT_ID }),
    },
  )

  if (!response.ok) {
    throw new ProviderAccountClientRequestFailed({
      endpoint: 'deviceauth_usercode',
      status: response.status,
      message: openAiCodexDeviceLoginStartFailureMessage(response.status),
    })
  }

  const deviceLogin = decodeOpenAiDeviceCodeResponse(await response.json())
  const intervalSeconds = parseExpiresInSeconds(deviceLogin.interval)

  return {
    deviceAuthId: deviceLogin.device_auth_id,
    verificationUrl: normalizeVerificationUrl(CHATGPT_CODEX_VERIFICATION_URL),
    userCode: deviceLogin.user_code,
    expiresAt: parseExpiresAt(
      deviceLogin.expires_at,
      deviceLogin.expires_in,
      now,
    ),
    intervalSeconds,
  }
}

export const pollOpenAiCodexDeviceLogin = async (
  secret: StartedCodexDeviceLoginSecret,
  fetcher: typeof fetch = fetch,
  now = systemProviderAccountRuntime.now(),
): Promise<CodexDeviceLoginPollResult> => {
  const deviceTokenResponse = await fetcher(
    'https://auth.openai.com/api/accounts/deviceauth/token',
    {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'user-agent': 'OpenAgents/0.1',
      },
      body: JSON.stringify({
        device_auth_id: secret.deviceAuthId,
        user_code: secret.userCode,
      }),
    },
  )

  if (
    deviceTokenResponse.status === 403 ||
    deviceTokenResponse.status === 404
  ) {
    return { status: 'pending' }
  }

  if (!deviceTokenResponse.ok) {
    return {
      status: 'failed',
      reason: `device token returned ${deviceTokenResponse.status}`,
    }
  }

  const deviceToken = decodeOpenAiDeviceTokenResponse(
    await deviceTokenResponse.json(),
  )
  const tokenResponse = await fetcher('https://auth.openai.com/oauth/token', {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'authorization_code',
      code: deviceToken.authorization_code,
      redirect_uri: 'https://auth.openai.com/deviceauth/callback',
      client_id: CODEX_CLIENT_ID,
      code_verifier: deviceToken.code_verifier,
    }).toString(),
  })

  if (!tokenResponse.ok) {
    return {
      status: 'failed',
      reason: `oauth token exchange returned ${tokenResponse.status}`,
    }
  }

  const tokens = decodeOpenAiOAuthTokenResponse(await tokenResponse.json())
  const accountId = extractCodexAccountId(tokens)

  return {
    status: 'connected',
    accountLabel: extractCodexAccountLabel(tokens) ?? accountId,
    auth: {
      type: 'oauth',
      refresh: tokens.refresh_token,
      access: tokens.access_token,
      expires: now.getTime() + (tokens.expires_in ?? 3600) * 1000,
      ...(accountId === undefined ? {} : { accountId }),
      ...(tokens.id_token === undefined ? {} : { idToken: tokens.id_token }),
    },
  }
}

export const refreshOpenAiCodexOAuthAuth = async (
  auth: CodexOAuthAuth,
  fetcher: typeof fetch = fetch,
  now = systemProviderAccountRuntime.now(),
): Promise<OpenAiCodexOAuthRefreshProbeResult> => {
  const tokenResponse = await fetcher('https://auth.openai.com/oauth/token', {
    method: 'POST',
    headers: {
      'content-type': 'application/x-www-form-urlencoded',
      'user-agent': 'OpenAgents/0.1',
    },
    body: new URLSearchParams({
      grant_type: 'refresh_token',
      refresh_token: auth.refresh,
      client_id: CODEX_CLIENT_ID,
    }).toString(),
  })

  if (!tokenResponse.ok) {
    const body = await tokenResponse.json().catch(() => undefined)
    const code = openAiRefreshFailureCode(body)

    return {
      status: 'failed',
      code,
      failureClass: openAiRefreshFailureClass(tokenResponse.status, code),
      providerStatus: tokenResponse.status,
    }
  }

  const tokens = decodeOpenAiOAuthTokenResponse(await tokenResponse.json())
  const accountId = extractCodexAccountId(tokens) ?? auth.accountId
  const idToken = tokens.id_token ?? auth.idToken

  return {
    status: 'refreshed',
    auth: {
      type: 'oauth',
      access: tokens.access_token,
      refresh: tokens.refresh_token,
      expires: now.getTime() + (tokens.expires_in ?? 3600) * 1000,
      ...(accountId === undefined ? {} : { accountId }),
      ...(idToken === undefined ? {} : { idToken }),
    },
  }
}

export type OpenAiCodexProviderClientShape = Readonly<{
  startDeviceLogin: () => Effect.Effect<
    StartedCodexDeviceLogin,
    ProviderAccountError
  >
  pollDeviceLogin: (
    secret: StartedCodexDeviceLoginSecret,
  ) => Effect.Effect<CodexDeviceLoginPollResult, ProviderAccountError>
}>

export class OpenAiCodexProviderClient extends Context.Service<
  OpenAiCodexProviderClient,
  OpenAiCodexProviderClientShape
>()('openagents/OpenAiCodexProviderClient') {}

export const makeOpenAiCodexProviderClient = (
  fetcher: typeof fetch = fetch,
  now: () => Date = systemProviderAccountRuntime.now,
): OpenAiCodexProviderClientShape => ({
  startDeviceLogin: () =>
    Effect.tryPromise({
      try: () => startOpenAiCodexDeviceLogin(fetcher, now()),
      catch: error =>
        providerAccountErrorFromUnknown(
          'start_openai_codex_device_login',
          error,
        ),
    }),
  pollDeviceLogin: secret =>
    Effect.tryPromise({
      try: () => pollOpenAiCodexDeviceLogin(secret, fetcher, now()),
      catch: error =>
        providerAccountErrorFromUnknown(
          'poll_openai_codex_device_login',
          error,
        ),
    }),
})

export const makeOpenAiCodexProviderClientLayer = (
  fetcher: typeof fetch = fetch,
  now: () => Date = systemProviderAccountRuntime.now,
) =>
  Layer.succeed(
    OpenAiCodexProviderClient,
    makeOpenAiCodexProviderClient(fetcher, now),
  )
