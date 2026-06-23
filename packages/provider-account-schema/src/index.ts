import { Schema as S } from 'effect'

export const ProviderAccountRef = S.String.pipe(S.brand('ProviderAccountRef'))
export type ProviderAccountRef = typeof ProviderAccountRef.Type

export const ProviderAccountId = S.String.pipe(S.brand('ProviderAccountId'))
export type ProviderAccountId = typeof ProviderAccountId.Type

export const ProviderConnectionAttemptId = S.String.pipe(
  S.brand('ProviderConnectionAttemptId'),
)
export type ProviderConnectionAttemptId =
  typeof ProviderConnectionAttemptId.Type

export const ProviderAuthGrantRef = S.String.pipe(
  S.brand('ProviderAuthGrantRef'),
)
export type ProviderAuthGrantRef = typeof ProviderAuthGrantRef.Type

export const ProviderSecretRef = S.String.pipe(S.brand('ProviderSecretRef'))
export type ProviderSecretRef = typeof ProviderSecretRef.Type

export const UserId = S.String.pipe(S.brand('UserId'))
export type UserId = typeof UserId.Type

export const TeamId = S.String.pipe(S.brand('TeamId'))
export type TeamId = typeof TeamId.Type

export const IsoTimestamp = S.String.pipe(S.brand('IsoTimestamp'))
export type IsoTimestamp = typeof IsoTimestamp.Type

export const ChatGptCodexProvider = S.Literal('chatgpt_codex')
export type ChatGptCodexProvider = typeof ChatGptCodexProvider.Type
export const GoogleGeminiProvider = S.Literal('google_gemini')
export type GoogleGeminiProvider = typeof GoogleGeminiProvider.Type
export const AnthropicClaudeProvider = S.Literal('anthropic_claude')
export type AnthropicClaudeProvider = typeof AnthropicClaudeProvider.Type
export const ProviderAccountProvider = S.Union([
  ChatGptCodexProvider,
  GoogleGeminiProvider,
  AnthropicClaudeProvider,
])
export type ProviderAccountProvider = typeof ProviderAccountProvider.Type

export const API_KEY_CONNECT_PROVIDERS = [
  'anthropic_claude',
  'google_gemini',
] as const
export const ApiKeyConnectProvider = S.Literals(API_KEY_CONNECT_PROVIDERS)
export type ApiKeyConnectProvider = typeof ApiKeyConnectProvider.Type

export const ProviderAccountStatus = S.Literals([
  'pending',
  'connected',
  'expired',
  'denied',
  'disconnected',
  'unhealthy',
])
export type ProviderAccountStatus = typeof ProviderAccountStatus.Type

export const ProviderAccountHealth = S.Literals([
  'unknown',
  'healthy',
  'unhealthy',
  'requires_reauth',
])
export type ProviderAccountHealth = typeof ProviderAccountHealth.Type

export const ProviderAccountAuthMode = S.Literals([
  'chatgpt_device_code',
  'codex_device_auth',
  'manual_secret_ref',
  'api_key',
])
export type ProviderAccountAuthMode = typeof ProviderAccountAuthMode.Type

export const ProviderConnectionAttemptMethod = S.Literals([
  'chatgpt_device_code',
  'provider_api_key',
])
export type ProviderConnectionAttemptMethod =
  typeof ProviderConnectionAttemptMethod.Type

export const ProviderConnectionAttemptSource = S.Literals([
  'shc_broker',
  'worker_device_code',
  'manual_placeholder',
  'browser_api_key',
])
export type ProviderConnectionAttemptSource =
  typeof ProviderConnectionAttemptSource.Type

export const ProviderConnectionAttemptStatus = S.Literals([
  'pending',
  'connected',
  'expired',
  'denied',
  'failed',
])
export type ProviderConnectionAttemptStatus =
  typeof ProviderConnectionAttemptStatus.Type

export const ProviderAccountAuthGrantStatus = S.Literals([
  'issued',
  'used',
  'expired',
  'revoked',
  'failed',
])
export type ProviderAccountAuthGrantStatus =
  typeof ProviderAccountAuthGrantStatus.Type

export const ProviderAccountEventKind = S.Literals([
  'login_started',
  'login_connected',
  'login_denied',
  'login_expired',
  'login_failed',
  'account_disconnected',
  'account_health_updated',
  'auth_grant_issued',
  'auth_grant_used',
  'auth_grant_revoked',
  'auth_grant_failed',
])
export type ProviderAccountEventKind = typeof ProviderAccountEventKind.Type

export class PublicProviderAccount extends S.Class<PublicProviderAccount>(
  'PublicProviderAccount',
)({
  id: ProviderAccountId,
  provider: ProviderAccountProvider,
  authMode: ProviderAccountAuthMode,
  status: ProviderAccountStatus,
  publicStatus: ProviderAccountStatus,
  health: ProviderAccountHealth,
  providerAccountRef: ProviderAccountRef,
  hasSecretRef: S.Boolean,
  accountLabel: S.optionalKey(S.String),
  planType: S.optionalKey(S.String),
  connectedAt: S.optionalKey(IsoTimestamp),
  disconnectedAt: S.optionalKey(IsoTimestamp),
  lastStatusAt: IsoTimestamp,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
}) {}

export class PublicProviderConnectionAttempt extends S.Class<PublicProviderConnectionAttempt>(
  'PublicProviderConnectionAttempt',
)({
  id: ProviderConnectionAttemptId,
  providerAccountId: ProviderAccountId,
  providerAccountRef: ProviderAccountRef,
  provider: ProviderAccountProvider,
  method: ProviderConnectionAttemptMethod,
  source: ProviderConnectionAttemptSource,
  status: ProviderConnectionAttemptStatus,
  loginRef: S.optionalKey(S.String),
  verificationUrl: S.optionalKey(S.String),
  userCode: S.optionalKey(S.String),
  expiresAt: IsoTimestamp,
  completedAt: S.optionalKey(IsoTimestamp),
  failedAt: S.optionalKey(IsoTimestamp),
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
}) {}

export class PublicProviderAccountAuthGrant extends S.Class<PublicProviderAccountAuthGrant>(
  'PublicProviderAccountAuthGrant',
)({
  id: S.String,
  providerAccountId: ProviderAccountId,
  provider: ProviderAccountProvider,
  providerAccountRef: ProviderAccountRef,
  grantRef: ProviderAuthGrantRef,
  status: ProviderAccountAuthGrantStatus,
  requestedAction: S.optionalKey(S.String),
  expiresAt: IsoTimestamp,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
}) {}

export class PublicProviderAccountEvent extends S.Class<PublicProviderAccountEvent>(
  'PublicProviderAccountEvent',
)({
  id: S.String,
  providerAccountId: S.optionalKey(ProviderAccountId),
  authGrantId: S.optionalKey(S.String),
  kind: ProviderAccountEventKind,
  summary: S.String,
  targetRef: S.optionalKey(S.String),
  createdAt: IsoTimestamp,
}) {}

export const PUBLIC_SECRET_REF_PREFIXES = [
  'secret://',
  'vault://',
  'gcp-secret://',
  'cloud-secret://',
  'provider-account://',
  'codex-auth://',
  'github-write://',
  'provider-account://',
] as const

const SECRET_MARKERS: ReadonlyArray<RegExp> = [
  /sk-[A-Za-z0-9_-]{16,}/,
  /gh[opusr]_[A-Za-z0-9_]{16,}/,
  /Bearer\s+[A-Za-z0-9._-]{16,}/i,
  /\\*"?\\*(access_token|refresh_token|id_token|code_verifier|device_code|device_auth_id|authorization_code)\\*"?\s*[:=]/i,
  /\\*"?\\*(access|refresh)\\*"?\s*:/i,
  /\\*"?\\*type\\*"?\s*:\s*\\*"?oauth\\*"?/i,
  /OPENAI_API_KEY\s*[:=]/i,
  /CODEX_ACCESS_TOKEN\s*[:=]/i,
  /OPENCODE_AUTH_CONTENT\s*[:=]/i,
  /GOOGLE_GENERATIVE_AI_API_KEY\s*[:=]/i,
  /GEMINI_API_KEY\s*[:=]/i,
  /ANTHROPIC_API_KEY\s*[:=]/i,
  /\bAIza[A-Za-z0-9_-]{16,}/,
  /auth\.json/i,
  /-----BEGIN [A-Z ]*PRIVATE KEY-----/,
  /eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/,
]

const MAX_PUBLIC_METADATA_JSON_CHARS = 8_000
const REDACTED = '[REDACTED]'

export type PublicProviderAccountMetadata = Readonly<{
  accountLabel?: string
  planType?: string
  providerAccountRef: string
  source?: string
  status: string
}>

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const StringOrNumber = S.Union([S.String, S.Number])

export const OpenAiDeviceCodeResponse = S.Struct({
  device_auth_id: NonEmptyTrimmedString,
  user_code: NonEmptyTrimmedString,
  interval: S.optionalKey(StringOrNumber),
  expires_at: S.optionalKey(TrimmedString),
  expires_in: S.optionalKey(StringOrNumber),
})
export type OpenAiDeviceCodeResponse = typeof OpenAiDeviceCodeResponse.Type

export const OpenAiDeviceTokenResponse = S.Struct({
  authorization_code: NonEmptyTrimmedString,
  code_verifier: NonEmptyTrimmedString,
})
export type OpenAiDeviceTokenResponse = typeof OpenAiDeviceTokenResponse.Type

export const OpenAiOAuthTokenResponse = S.Struct({
  access_token: NonEmptyTrimmedString,
  refresh_token: NonEmptyTrimmedString,
  expires_in: S.optionalKey(S.Number),
  id_token: S.optionalKey(TrimmedString),
})
export type OpenAiOAuthTokenResponse = typeof OpenAiOAuthTokenResponse.Type

export const containsProviderSecretMaterial = (
  value: string | undefined,
): boolean => {
  if (value === undefined || value === '') {
    return false
  }

  return SECRET_MARKERS.some(marker => marker.test(value))
}

export const sanitizeProviderAccountText = (
  value: string | undefined,
  maxLength = 160,
): string | undefined => {
  const trimmed = value?.trim()

  if (
    trimmed === undefined ||
    trimmed === '' ||
    containsProviderSecretMaterial(trimmed)
  ) {
    return undefined
  }

  return trimmed.replace(/\s+/g, ' ').slice(0, maxLength)
}

export const isPublicSecretReference = (value: string | undefined): boolean => {
  const trimmed = value?.trim()

  if (
    trimmed === undefined ||
    trimmed === '' ||
    trimmed.length > 240 ||
    trimmed.includes('\n') ||
    containsProviderSecretMaterial(trimmed)
  ) {
    return false
  }

  return PUBLIC_SECRET_REF_PREFIXES.some(prefix => trimmed.startsWith(prefix))
}

export const requirePublicSecretReference = (
  value: string,
): ProviderSecretRef => {
  const trimmed = value.trim()

  if (!isPublicSecretReference(trimmed)) {
    throw new Error(
      'Provider account secret references must be stable refs, not raw credential material.',
    )
  }

  return ProviderSecretRef.make(trimmed)
}

export const providerAccountPublicMetadataJson = (
  metadata: PublicProviderAccountMetadata,
): string => {
  const json = JSON.stringify(metadata)

  if (containsProviderSecretMaterial(json)) {
    throw new Error('Provider account metadata contains secret material.')
  }

  return json.length > MAX_PUBLIC_METADATA_JSON_CHARS
    ? json.slice(0, MAX_PUBLIC_METADATA_JSON_CHARS)
    : json
}

export const redactProviderAccountSecretMaterial = (value: string): string =>
  value
    .replace(/Bearer\s+[A-Za-z0-9._-]{8,}/gi, `Bearer ${REDACTED}`)
    .replace(/sk-[A-Za-z0-9_-]{8,}/g, `sk-${REDACTED}`)
    .replace(/gh[opusr]_[A-Za-z0-9_]{8,}/g, match =>
      `${match.slice(0, 4)}${REDACTED}`,
    )
    .replace(
      /"?(access_token|refresh_token|id_token|code_verifier|device_code|device_auth_id|authorization_code|access|refresh)"?\s*:\s*"[^"]*"/gi,
      (_match, key: string) => `"${key}":"${REDACTED}"`,
    )
    .replace(
      /\\"(access_token|refresh_token|id_token|code_verifier|device_code|device_auth_id|authorization_code|access|refresh)\\"\s*:\s*\\"[^"]*\\"/gi,
      (_match, key: string) => `\\"${key}\\":\\"${REDACTED}\\"`,
    )
    .replace(
      /OPENCODE_AUTH_CONTENT\s*[:=]\s*[^\s]+/gi,
      `OPENCODE_AUTH_CONTENT=${REDACTED}`,
    )
    .replace(
      /ANTHROPIC_API_KEY\s*[:=]\s*[^\s]+/gi,
      `ANTHROPIC_API_KEY=${REDACTED}`,
    )
    .replace(
      /(?:GOOGLE_GENERATIVE_AI_API_KEY|GEMINI_API_KEY)\s*[:=]\s*[^\s]+/gi,
      `GEMINI_API_KEY=${REDACTED}`,
    )
    .replace(/\bAIza[A-Za-z0-9_-]{8,}/g, `AIza${REDACTED}`)
    .replace(/auth\.json/gi, `auth.json:${REDACTED}`)
    .replace(
      /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g,
      REDACTED,
    )

export const redactProviderAccountLogValue = (value: unknown): string => {
  const text =
    typeof value === 'string'
      ? value
      : value instanceof Error
        ? value.message
        : JSON.stringify(value)

  return redactProviderAccountSecretMaterial(text ?? '')
}

export const assertNoProviderSecretMaterial = (
  value: unknown,
  context: string,
): void => {
  const json = typeof value === 'string' ? value : JSON.stringify(value)

  if (containsProviderSecretMaterial(json)) {
    throw new Error(`${context} contains provider credential material.`)
  }
}

export const decodePublicProviderAccount = S.decodeUnknownEffect(
  PublicProviderAccount,
)
export const decodePublicProviderConnectionAttempt = S.decodeUnknownEffect(
  PublicProviderConnectionAttempt,
)
export const decodeOpenAiDeviceCodeResponse = S.decodeUnknownSync(
  OpenAiDeviceCodeResponse,
)
export const decodeOpenAiDeviceTokenResponse = S.decodeUnknownSync(
  OpenAiDeviceTokenResponse,
)
export const decodeOpenAiOAuthTokenResponse = S.decodeUnknownSync(
  OpenAiOAuthTokenResponse,
)
