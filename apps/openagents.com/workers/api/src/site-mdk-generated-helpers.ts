import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  SiteCheckoutIntentRequest,
  SiteL402ChallengeRequest,
  SiteL402RedemptionRequest,
} from './site-commerce-routes'

export const OpenAgentsGeneratedSitePaymentHelperKind = S.Literals([
  'checkout_intent_create',
  'checkout_return_read',
  'l402_challenge_create',
  'l402_redemption_create',
  'payment_discovery_read',
  'payment_proof_read',
])
export type OpenAgentsGeneratedSitePaymentHelperKind =
  typeof OpenAgentsGeneratedSitePaymentHelperKind.Type

export const OpenAgentsGeneratedSitePaymentHelperMethod = S.Literals([
  'GET',
  'POST',
])
export type OpenAgentsGeneratedSitePaymentHelperMethod =
  typeof OpenAgentsGeneratedSitePaymentHelperMethod.Type

export const OpenAgentsGeneratedSitePaymentHelperReturnAction = S.Literals([
  'cancel',
  'status',
  'success',
])
export type OpenAgentsGeneratedSitePaymentHelperReturnAction =
  typeof OpenAgentsGeneratedSitePaymentHelperReturnAction.Type

export const OpenAgentsGeneratedSitePaymentHelperRuntime = S.Literals([
  'static_site_fetch',
  'wfp_worker_fetch',
])
export type OpenAgentsGeneratedSitePaymentHelperRuntime =
  typeof OpenAgentsGeneratedSitePaymentHelperRuntime.Type

export class OpenAgentsGeneratedSitePaymentHelperRequestPlan extends S.Class<OpenAgentsGeneratedSitePaymentHelperRequestPlan>(
  'OpenAgentsGeneratedSitePaymentHelperRequestPlan',
)({
  body: S.NullOr(S.Unknown),
  headers: S.Record(S.String, S.String),
  helperKind: OpenAgentsGeneratedSitePaymentHelperKind,
  idempotencyKey: S.NullOr(S.String),
  method: OpenAgentsGeneratedSitePaymentHelperMethod,
  runtime: OpenAgentsGeneratedSitePaymentHelperRuntime,
  url: S.String,
}) {}

export class OpenAgentsGeneratedSitePaymentHelperErrorEnvelope extends S.Class<OpenAgentsGeneratedSitePaymentHelperErrorEnvelope>(
  'OpenAgentsGeneratedSitePaymentHelperErrorEnvelope',
)({
  errorRef: S.String,
  helperKind: OpenAgentsGeneratedSitePaymentHelperKind,
  messageRef: S.String,
  redaction: S.Struct({
    exposesCheckoutQueryState: S.Literal(false),
    exposesCustomerPrivateData: S.Literal(false),
    exposesMdkCredentials: S.Literal(false),
    exposesProviderPayoutClaims: S.Literal(false),
    exposesRawPaymentMaterial: S.Literal(false),
    exposesWalletMaterial: S.Literal(false),
  }),
  retryable: S.Boolean,
  status: S.Number,
}) {}

export class OpenAgentsGeneratedSitePaymentHelperUnsafe extends S.TaggedErrorClass<OpenAgentsGeneratedSitePaymentHelperUnsafe>()(
  'OpenAgentsGeneratedSitePaymentHelperUnsafe',
  {
    reason: S.String,
  },
) {}

export type OpenAgentsGeneratedSitePaymentHelperConfig = Readonly<{
  apiBaseUrl?: string
  runtime?: OpenAgentsGeneratedSitePaymentHelperRuntime
  siteId: string
}>

export type OpenAgentsGeneratedSiteCheckoutIntentHelperInput =
  OpenAgentsGeneratedSitePaymentHelperConfig & Readonly<{
    body: typeof SiteCheckoutIntentRequest.Type
    idempotencyKey: string
  }>

export type OpenAgentsGeneratedSiteL402ChallengeHelperInput =
  OpenAgentsGeneratedSitePaymentHelperConfig & Readonly<{
    body: typeof SiteL402ChallengeRequest.Type
    idempotencyKey: string
  }>

export type OpenAgentsGeneratedSiteL402RedemptionHelperInput =
  OpenAgentsGeneratedSitePaymentHelperConfig & Readonly<{
    body: typeof SiteL402RedemptionRequest.Type
    idempotencyKey: string
  }>

const stableSiteIdPattern = /^[A-Za-z0-9][A-Za-z0-9_-]{0,160}$/u
const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,180}$/u
const stableIdempotencyKeyPattern = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,120}$/u
const cleanApiBasePattern = /^https:\/\/openagents\.com$/u
const generatedHelperUrlPattern =
  /^(?:https:\/\/openagents\.com)?\/api\/sites\/[A-Za-z0-9_-]+\/commerce\/[A-Za-z0-9_./%:-]+$/u
const unsafeGeneratedHelperPattern =
  /(bearer\s+|checkout_id=|cookie|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|invoice|preimage|raw|secret)|preimage|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(checkout|customer|invoice|payment|payload|provider|webhook)|secret|sk-[a-z0-9]|wallet[_-]?(key|material|mnemonic|payment|preimage|secret|seed|state))/i
const forbiddenGeneratedSourcePattern =
  /(@moneydevkit|lightning-js|MDK_ACCESS_TOKEN|MDK_MNEMONIC|MDK_WEBHOOK_SECRET|mnemonic|lnbc|lntb|payment_hash|payment_preimage|preimage|wallet_secret|checkout_id=|providerToken|apiKey|secret)/i

export const OPENAGENTS_GENERATED_SITE_PAYMENT_HELPER_MDK_CORE_PARITY_REFS = [
  'fixture.mdk_core.amount_checkout_creation',
  'fixture.mdk_core.error_envelope',
  'fixture.mdk_core.l402_token_parsing',
  'fixture.mdk_core.metadata_limits',
  'fixture.mdk_core.price_recheck',
  'fixture.mdk_core.product_checkout_creation',
  'fixture.mdk_core.safe_return_path',
  'fixture.mdk_core.sandbox_flag',
  'fixture.mdk_core.stale_challenge',
] as const

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeGeneratedHelperPattern.test(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).some(
      valueHasPrivateMaterial,
    )
  }

  return false
}

const assertSafeGeneratedHelperValue = (
  label: string,
  value: unknown,
): void => {
  if (valueHasPrivateMaterial(value)) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason:
        `${label} must not contain MDK credentials, wallet material, raw invoices, payment hashes, preimages, provider grants, checkout query state, or secrets.`,
    })
  }
}

const assertSiteId = (siteId: string): void => {
  if (!stableSiteIdPattern.test(siteId) || valueHasPrivateMaterial(siteId)) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason: 'Generated Site payment helper siteId must be stable and public-safe.',
    })
  }
}

const assertStableRef = (label: string, value: string): void => {
  if (!stableRefPattern.test(value) || valueHasPrivateMaterial(value)) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason: `${label} must be a stable public-safe ref.`,
    })
  }
}

const assertIdempotencyKey = (idempotencyKey: string): void => {
  if (
    !stableIdempotencyKeyPattern.test(idempotencyKey) ||
    valueHasPrivateMaterial(idempotencyKey)
  ) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason:
        'Generated Site payment helper idempotency keys must be stable public-safe refs.',
    })
  }
}

const assertCleanSiteLocalPath = (label: string, value: string): void => {
  if (
    !value.startsWith('/') ||
    value.includes('?') ||
    value.includes('#') ||
    value.includes('://') ||
    value.includes('//') ||
    valueHasPrivateMaterial(value)
  ) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason:
        `${label} must be a clean site-local path without query strings, fragments, absolute URLs, protocol-relative URLs, or private material.`,
    })
  }
}

const apiBase = (config: OpenAgentsGeneratedSitePaymentHelperConfig): string => {
  const base = config.apiBaseUrl ?? ''

  if (base === '') {
    return ''
  }

  if (!cleanApiBasePattern.test(base) || valueHasPrivateMaterial(base)) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason:
        'Generated Site payment helper apiBaseUrl must be empty or https://openagents.com.',
    })
  }

  return base
}

const endpoint = (
  config: OpenAgentsGeneratedSitePaymentHelperConfig,
  path: string,
): string => {
  assertSiteId(config.siteId)

  const url =
    `${apiBase(config)}/api/sites/${encodeURIComponent(config.siteId)}/commerce${path}`

  if (
    !generatedHelperUrlPattern.test(url) ||
    url.includes('?') ||
    url.includes('#') ||
    valueHasPrivateMaterial(url)
  ) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason: 'Generated Site payment helper URL must be clean and source-safe.',
    })
  }

  return url
}

const runtime = (
  config: OpenAgentsGeneratedSitePaymentHelperConfig,
): OpenAgentsGeneratedSitePaymentHelperRuntime =>
  config.runtime ?? 'static_site_fetch'

const plan = (
  input: Readonly<{
    body: unknown | null
    config: OpenAgentsGeneratedSitePaymentHelperConfig
    helperKind: OpenAgentsGeneratedSitePaymentHelperKind
    idempotencyKey?: string
    method: OpenAgentsGeneratedSitePaymentHelperMethod
    path: string
  }>,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan => {
  const headers = input.body === null
    ? {}
    : { 'content-type': 'application/json' }
  const idempotencyKey = input.idempotencyKey ?? null

  if (idempotencyKey !== null) {
    assertIdempotencyKey(idempotencyKey)
  }

  const requestPlan = new OpenAgentsGeneratedSitePaymentHelperRequestPlan({
    body: input.body,
    headers: {
      ...headers,
      ...(idempotencyKey === null
        ? {}
        : { 'idempotency-key': idempotencyKey }),
    },
    helperKind: input.helperKind,
    idempotencyKey,
    method: input.method,
    runtime: runtime(input.config),
    url: endpoint(input.config, input.path),
  })

  assertSafeGeneratedHelperValue('Generated Site payment helper plan', requestPlan)

  return requestPlan
}

export const generatedSitePaymentDiscoveryPlan = (
  config: OpenAgentsGeneratedSitePaymentHelperConfig,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan =>
  plan({
    body: null,
    config,
    helperKind: 'payment_discovery_read',
    method: 'GET',
    path: '/discovery',
  })

export const generatedSiteCheckoutIntentPlan = (
  input: OpenAgentsGeneratedSiteCheckoutIntentHelperInput,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan => {
  const body = S.decodeUnknownSync(SiteCheckoutIntentRequest)(input.body)

  assertCleanSiteLocalPath('successReturnPath', body.successReturnPath)
  assertCleanSiteLocalPath('cancelReturnPath', body.cancelReturnPath)
  assertSafeGeneratedHelperValue('Generated Site checkout intent body', body)

  return plan({
    body,
    config: input,
    helperKind: 'checkout_intent_create',
    idempotencyKey: input.idempotencyKey,
    method: 'POST',
    path: '/checkout-intents',
  })
}

export const generatedSiteCheckoutReturnPlan = (
  config: OpenAgentsGeneratedSitePaymentHelperConfig &
    Readonly<{
      checkoutIntentRef: string
      returnAction: OpenAgentsGeneratedSitePaymentHelperReturnAction
    }>,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan => {
  assertStableRef('checkoutIntentRef', config.checkoutIntentRef)

  return plan({
    body: null,
    config,
    helperKind: 'checkout_return_read',
    method: 'GET',
    path:
      `/checkout-returns/${encodeURIComponent(config.checkoutIntentRef)}/${config.returnAction}`,
  })
}

export const generatedSitePaymentProofPlan = (
  config: OpenAgentsGeneratedSitePaymentHelperConfig &
    Readonly<{ checkoutIntentRef: string }>,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan => {
  assertStableRef('checkoutIntentRef', config.checkoutIntentRef)

  return plan({
    body: null,
    config,
    helperKind: 'payment_proof_read',
    method: 'GET',
    path: `/payment-proofs/${encodeURIComponent(config.checkoutIntentRef)}`,
  })
}

export const generatedSiteL402ChallengePlan = (
  input: OpenAgentsGeneratedSiteL402ChallengeHelperInput,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan => {
  const body = S.decodeUnknownSync(SiteL402ChallengeRequest)(input.body)

  assertCleanSiteLocalPath('paid action path', body.path)
  assertSafeGeneratedHelperValue('Generated Site L402 challenge body', body)

  if (body.price.asset !== body.spendCap.asset || body.price.amount > body.spendCap.amount) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason:
        'Generated Site L402 challenge helper requires price to fit inside the supplied spend cap.',
    })
  }

  return plan({
    body,
    config: input,
    helperKind: 'l402_challenge_create',
    idempotencyKey: input.idempotencyKey,
    method: 'POST',
    path: '/l402/challenges',
  })
}

export const generatedSiteL402RedemptionPlan = (
  input: OpenAgentsGeneratedSiteL402RedemptionHelperInput,
): OpenAgentsGeneratedSitePaymentHelperRequestPlan => {
  const body = S.decodeUnknownSync(SiteL402RedemptionRequest)(input.body)

  assertCleanSiteLocalPath('paid action path', body.path)
  assertSafeGeneratedHelperValue('Generated Site L402 redemption body', body)

  return plan({
    body,
    config: input,
    helperKind: 'l402_redemption_create',
    idempotencyKey: input.idempotencyKey,
    method: 'POST',
    path: '/l402/redemptions',
  })
}

export const generatedSitePaymentHelperErrorEnvelope = (
  input: Readonly<{
    helperKind: OpenAgentsGeneratedSitePaymentHelperKind
    retryable?: boolean
    status: number
  }>,
): OpenAgentsGeneratedSitePaymentHelperErrorEnvelope => {
  const status = Number.isFinite(input.status) ? input.status : 500
  const retryable =
    input.retryable ?? [408, 409, 425, 429, 500, 502, 503, 504].includes(status)

  return new OpenAgentsGeneratedSitePaymentHelperErrorEnvelope({
    errorRef: `error.site_payment_helper.${input.helperKind}.${status}`,
    helperKind: input.helperKind,
    messageRef: `message.site_payment_helper.${input.helperKind}.${retryable ? 'retryable' : 'terminal'}`,
    redaction: {
      exposesCheckoutQueryState: false,
      exposesCustomerPrivateData: false,
      exposesMdkCredentials: false,
      exposesProviderPayoutClaims: false,
      exposesRawPaymentMaterial: false,
      exposesWalletMaterial: false,
    },
    retryable,
    status,
  })
}

export const assertGeneratedSitePaymentHelperSourceSafe = (
  source: string,
): void => {
  if (
    forbiddenGeneratedSourcePattern.test(source) ||
    containsProviderSecretMaterial(source)
  ) {
    throw new OpenAgentsGeneratedSitePaymentHelperUnsafe({
      reason:
        'Generated Site payment helper source must not import MDK native runtimes or contain credentials, wallet material, raw payment material, or provider grants.',
    })
  }
}

export const OPENAGENTS_STATIC_SITE_PAYMENT_HELPER_EXAMPLE = `
const OPENAGENTS_API_BASE = 'https://openagents.com'

export async function readSitePaymentDiscovery(siteId) {
  return fetch(\`\${OPENAGENTS_API_BASE}/api/sites/\${encodeURIComponent(siteId)}/commerce/discovery\`, {
    method: 'GET',
  })
}

export async function createSiteCheckoutIntent(siteId, idempotencyKey, body) {
  return fetch(\`\${OPENAGENTS_API_BASE}/api/sites/\${encodeURIComponent(siteId)}/commerce/checkout-intents\`, {
    body: JSON.stringify(body),
    headers: {
      'content-type': 'application/json',
      'idempotency-key': idempotencyKey,
    },
    method: 'POST',
  })
}

export async function readSiteCheckoutStatus(siteId, checkoutIntentRef) {
  return fetch(\`\${OPENAGENTS_API_BASE}/api/sites/\${encodeURIComponent(siteId)}/commerce/checkout-returns/\${encodeURIComponent(checkoutIntentRef)}/status\`, {
    method: 'GET',
  })
}

export async function requestSitePaidAction(siteId, idempotencyKey, body) {
  return fetch(\`\${OPENAGENTS_API_BASE}/api/sites/\${encodeURIComponent(siteId)}/commerce/l402/challenges\`, {
    body: JSON.stringify(body),
    headers: {
      'content-type': 'application/json',
      'idempotency-key': idempotencyKey,
    },
    method: 'POST',
  })
}
`.trim()

export const OPENAGENTS_WFP_SITE_PAYMENT_HELPER_EXAMPLE = `
export async function callOpenAgentsSiteCommerce(env, path, init) {
  const url = new URL(path, 'https://openagents.com')
  return env.OPENAGENTS_COMMERCE.fetch(new Request(url, init))
}

export async function createSiteL402Challenge(env, siteId, idempotencyKey, body) {
  return callOpenAgentsSiteCommerce(
    env,
    \`/api/sites/\${encodeURIComponent(siteId)}/commerce/l402/challenges\`,
    {
      body: JSON.stringify(body),
      headers: {
        'content-type': 'application/json',
        'idempotency-key': idempotencyKey,
      },
      method: 'POST',
    },
  )
}

export async function redeemSiteL402(env, siteId, idempotencyKey, body) {
  return callOpenAgentsSiteCommerce(
    env,
    \`/api/sites/\${encodeURIComponent(siteId)}/commerce/l402/redemptions\`,
    {
      body: JSON.stringify(body),
      headers: {
        'content-type': 'application/json',
        'idempotency-key': idempotencyKey,
      },
      method: 'POST',
    },
  )
}
`.trim()

assertGeneratedSitePaymentHelperSourceSafe(
  OPENAGENTS_STATIC_SITE_PAYMENT_HELPER_EXAMPLE,
)
assertGeneratedSitePaymentHelperSourceSafe(
  OPENAGENTS_WFP_SITE_PAYMENT_HELPER_EXAMPLE,
)
