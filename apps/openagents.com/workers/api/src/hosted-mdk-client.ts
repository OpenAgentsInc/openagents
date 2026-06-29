import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentLedgerAmount,
} from './buyer-payment-ledger'
import { isRecord, nestedUnknown, optionalString } from './json-boundary'
import type { OpenAgentsL402CredentialPayload } from './l402-credential-service'
import { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import {
  currentIsoTimestamp,
  normalizeIsoTimestamp,
} from './runtime-primitives'

export const OpenAgentsHostedMdkCheckoutMode = S.Literals([
  'amount',
  'l402_invoice',
  'product',
])
export type OpenAgentsHostedMdkCheckoutMode =
  typeof OpenAgentsHostedMdkCheckoutMode.Type

export const OpenAgentsHostedMdkEnvironment = S.Literals([
  'production',
  'sandbox',
])
export type OpenAgentsHostedMdkEnvironment =
  typeof OpenAgentsHostedMdkEnvironment.Type

export const OpenAgentsHostedMdkCheckoutStatus = S.Literals([
  'created',
  'expired',
  'payment_received',
  'pending_payment',
])
export type OpenAgentsHostedMdkCheckoutStatus =
  typeof OpenAgentsHostedMdkCheckoutStatus.Type

export const OpenAgentsHostedMdkImplementationState = S.Literals([
  'fake_provider_contract',
  'live_provider_configured',
  'missing_configuration',
])
export type OpenAgentsHostedMdkImplementationState =
  typeof OpenAgentsHostedMdkImplementationState.Type

export const OpenAgentsHostedMdkClientErrorReason = S.Literals([
  'missing_configuration',
  'provider_rejected',
  'provider_unavailable',
  'secret_leakage_detected',
  'stale_challenge',
  'unsafe_metadata',
  'unsupported_asset_denomination',
])
export type OpenAgentsHostedMdkClientErrorReason =
  typeof OpenAgentsHostedMdkClientErrorReason.Type

export const OpenAgentsHostedMdkClientConfig = S.Struct({
  configRef: S.String,
  credentialBindingRef: S.NullOr(S.String),
  environment: OpenAgentsHostedMdkEnvironment,
  providerRef: S.String,
  webhookBindingRef: S.NullOr(S.String),
})
export type OpenAgentsHostedMdkClientConfig =
  typeof OpenAgentsHostedMdkClientConfig.Type

export const OpenAgentsHostedMdkCheckoutRequest = S.Struct({
  amount: BuyerPaymentLedgerAmount,
  cancelRef: S.String,
  challengeExpiresAt: S.String,
  challengeRef: S.String,
  customerDataRefs: S.Array(S.String),
  environment: OpenAgentsHostedMdkEnvironment,
  idempotencyKeyHash: S.String,
  l402CredentialRef: S.NullOr(S.String),
  metadataRefs: S.Array(S.String),
  mode: OpenAgentsHostedMdkCheckoutMode,
  productId: S.String,
  returnRef: S.String,
  sandbox: S.Boolean,
  siteRef: S.NullOr(S.String),
})
export type OpenAgentsHostedMdkCheckoutRequest =
  typeof OpenAgentsHostedMdkCheckoutRequest.Type

export const OpenAgentsHostedMdkCheckoutResponse = S.Struct({
  acceptedWorkSettlementAuthority: S.Literal(false),
  amount: BuyerPaymentLedgerAmount,
  cancelRef: S.String,
  challengeRef: S.String,
  checkoutRef: S.String,
  checkoutLaunchPath: S.optionalKey(S.NullOr(S.String)),
  checkoutUrlRef: S.String,
  createdAt: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  expiresAt: S.String,
  idempotencyKeyHash: S.String,
  invoiceRef: S.String,
  metadataRefs: S.Array(S.String),
  paymentHashRef: S.String,
  productId: S.String,
  provider: S.Literal('mdk_hosted'),
  providerPayoutAuthority: S.Literal(false),
  providerRef: S.String,
  returnRef: S.String,
  sandbox: S.Boolean,
  settlementAuthority: S.Literal('buyer_payment_evidence_only'),
  siteRef: S.NullOr(S.String),
  status: OpenAgentsHostedMdkCheckoutStatus,
})
export type OpenAgentsHostedMdkCheckoutResponse =
  typeof OpenAgentsHostedMdkCheckoutResponse.Type

export const OpenAgentsHostedMdkCheckoutProjection = S.Struct({
  acceptedWorkSettlementAuthority: S.Literal(false),
  amount: BuyerPaymentLedgerAmount,
  audience: OpenAgentsPaymentPolicyAudience,
  challengeRef: S.String,
  checkoutRef: S.String,
  checkoutLaunchPath: S.optionalKey(S.NullOr(S.String)),
  checkoutUrlRef: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  invoiceRef: S.NullOr(S.String),
  paymentHashRef: S.NullOr(S.String),
  productId: S.String,
  provider: S.Literal('mdk_hosted'),
  providerPayoutAuthority: S.Literal(false),
  providerRef: S.String,
  sandbox: S.Boolean,
  settlementAuthority: S.Literal('buyer_payment_evidence_only'),
  siteRef: S.NullOr(S.String),
  status: OpenAgentsHostedMdkCheckoutStatus,
})
export type OpenAgentsHostedMdkCheckoutProjection =
  typeof OpenAgentsHostedMdkCheckoutProjection.Type

export const OpenAgentsHostedMdkCheckoutStatusRequest = S.Struct({
  checkoutRef: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  providerRef: S.String,
  sandbox: S.Boolean,
  siteRef: S.NullOr(S.String),
})
export type OpenAgentsHostedMdkCheckoutStatusRequest =
  typeof OpenAgentsHostedMdkCheckoutStatusRequest.Type

export const OpenAgentsHostedMdkCheckoutStatusResponse = S.Struct({
  checkoutRef: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  expiresAt: S.NullOr(S.String),
  observedAt: S.String,
  provider: S.Literal('mdk_hosted'),
  providerRef: S.String,
  sandbox: S.Boolean,
  siteRef: S.NullOr(S.String),
  status: OpenAgentsHostedMdkCheckoutStatus,
})
export type OpenAgentsHostedMdkCheckoutStatusResponse =
  typeof OpenAgentsHostedMdkCheckoutStatusResponse.Type

export const OpenAgentsHostedMdkPrivateL402PaymentRequest = S.Struct({
  checkoutRef: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  providerRef: S.String,
  sandbox: S.Boolean,
  siteRef: S.NullOr(S.String),
})
export type OpenAgentsHostedMdkPrivateL402PaymentRequest =
  typeof OpenAgentsHostedMdkPrivateL402PaymentRequest.Type

export const OpenAgentsHostedMdkPrivateL402PaymentPayload = S.Struct({
  bolt11: S.String,
  checkoutRef: S.String,
  environment: OpenAgentsHostedMdkEnvironment,
  expiresAt: S.NullOr(S.String),
  provider: S.Literal('mdk_hosted'),
  providerRef: S.String,
  sandbox: S.Boolean,
  siteRef: S.NullOr(S.String),
})
export type OpenAgentsHostedMdkPrivateL402PaymentPayload =
  typeof OpenAgentsHostedMdkPrivateL402PaymentPayload.Type

export class OpenAgentsHostedMdkClientError extends S.TaggedErrorClass<OpenAgentsHostedMdkClientError>()(
  'OpenAgentsHostedMdkClientError',
  {
    detailRef: S.String,
    reason: OpenAgentsHostedMdkClientErrorReason,
  },
) {}

export type OpenAgentsHostedMdkClient = Readonly<{
  createCheckoutPromise: (
    request: OpenAgentsHostedMdkCheckoutRequest,
  ) => Promise<OpenAgentsHostedMdkCheckoutResponse>
  implementationState: OpenAgentsHostedMdkImplementationState
  createCheckout: (
    request: OpenAgentsHostedMdkCheckoutRequest,
  ) => Effect.Effect<
    OpenAgentsHostedMdkCheckoutResponse,
    OpenAgentsHostedMdkClientError
  >
  getCheckoutStatus: (
    request: OpenAgentsHostedMdkCheckoutStatusRequest,
  ) => Effect.Effect<
    OpenAgentsHostedMdkCheckoutStatusResponse,
    OpenAgentsHostedMdkClientError
  >
  getPrivateL402PaymentPayload: (
    request: OpenAgentsHostedMdkPrivateL402PaymentRequest,
  ) => Effect.Effect<
    OpenAgentsHostedMdkPrivateL402PaymentPayload,
    OpenAgentsHostedMdkClientError
  >
}>

type CreateCheckoutRequestInput = Readonly<{
  cancelRef: string
  challenge: BuyerPaymentChallengeRecord
  customerDataRefs?: ReadonlyArray<string>
  environment: OpenAgentsHostedMdkEnvironment
  l402Payload?: OpenAgentsL402CredentialPayload | null
  metadataRefs?: ReadonlyArray<string>
  mode?: OpenAgentsHostedMdkCheckoutMode
  product: OpenAgentsPaidEndpointProductRecord
  returnRef: string
  sandbox: boolean
  siteRef?: string | null
}>

type FakeClientOptions = Readonly<{
  configured?: boolean
  nowIso?: string
  providerAvailable?: boolean
  providerRef?: string
  rejectCheckout?: boolean
}>

export type OpenAgentsHostedMdkRouteClientRuntime = Readonly<{
  checkoutPathBase: string
  fetch?: typeof fetch
  nowIso?: () => string
  routeTimeoutMs?: number | undefined
  routeSecret: string
  routeUrl: string
}>

type MdkRouteBody =
  | Readonly<{
      handler: 'create_checkout'
      params: Record<string, unknown>
    }>
  | Readonly<{
      checkoutId: string
      handler: 'get_checkout'
    }>

const unsafeHostedMdkValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const stableHostedMdkRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,160}$/
const cleanCheckoutPathPattern =
  /^\/(?!\/)(?!.*(?:\?|#|:\/\/|\/\/))[\w./:-]{0,180}$/
const metadataRefLimit = 50
const providerCheckoutRefPrefix = 'mdk_checkout.'
const defaultMdkRouteTimeoutMs = 20_000

const clientError = (
  reason: OpenAgentsHostedMdkClientErrorReason,
  detailRef: string,
): OpenAgentsHostedMdkClientError =>
  new OpenAgentsHostedMdkClientError({ detailRef, reason })

const hostedMdkStringHasPrivateMaterial = (value: string): boolean =>
  containsProviderSecretMaterial(String(value)) ||
  unsafeHostedMdkValuePattern.test(String(value)) ||
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)

const hostedMdkValueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return hostedMdkStringHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(hostedMdkValueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return (
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.values(value).some(hostedMdkValueHasPrivateMaterial)
    )
  }

  return false
}

const hostedMdkRefIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableHostedMdkRefPattern.test(value) &&
  !hostedMdkValueHasPrivateMaterial(value)

const hostedMdkRefsAreSafe = (refs: ReadonlyArray<string>): boolean =>
  refs.length <= metadataRefLimit && refs.every(hostedMdkRefIsSafe)

const hostedMdkAmountSupported = (amount: BuyerPaymentLedgerAmount): boolean =>
  (amount.asset === 'usd' && amount.denomination === 'usd_cent') ||
  (amount.asset === 'bitcoin' && amount.denomination === 'bitcoin_millisatoshi')

const amountEquals = (
  left: BuyerPaymentLedgerAmount,
  right: BuyerPaymentLedgerAmount,
): boolean =>
  left.amountMinorUnits === right.amountMinorUnits &&
  left.asset === right.asset &&
  left.denomination === right.denomination

const cleanRefSegment = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_-]+/g, '_').slice(0, 140)

const cleanInvoiceSegment = (value: string): string =>
  value
    .replaceAll(/[^A-Za-z0-9]+/g, '')
    .toLowerCase()
    .slice(0, 140)

const cleanCheckoutPath = (basePath: string, checkoutRef: string): string => {
  const cleanBase = cleanCheckoutPathPattern.test(basePath)
    ? basePath.replace(/\/$/u, '')
    : '/checkout'
  const idSegment = cleanRefSegment(checkoutRef)

  return `${cleanBase === '' ? '/checkout' : cleanBase}/${idSegment}`
}

const checkoutModeFromProduct = (
  product: OpenAgentsPaidEndpointProductRecord,
): OpenAgentsHostedMdkCheckoutMode =>
  product.binding.kind === 'site_checkout' ? 'product' : 'l402_invoice'

const providerCheckoutIdFromRef = (checkoutRef: string): string | undefined => {
  if (!checkoutRef.startsWith(providerCheckoutRefPrefix)) {
    return undefined
  }

  const providerCheckoutId = checkoutRef.slice(providerCheckoutRefPrefix.length)

  return hostedMdkRefIsSafe(providerCheckoutId) ? providerCheckoutId : undefined
}

const hostedStatusFromProviderStatus = (
  value: unknown,
): OpenAgentsHostedMdkCheckoutStatus => {
  const status = typeof value === 'string' ? value.toUpperCase() : ''

  switch (status) {
    case 'EXPIRED':
      return 'expired'
    case 'PAYMENT_RECEIVED':
      return 'payment_received'
    case 'PENDING_PAYMENT':
      return 'pending_payment'
    case 'CONFIRMED':
    case 'UNCONFIRMED':
    default:
      return 'created'
  }
}

const isoFromUnknown = (value: unknown, fallback: string): string => {
  if (typeof value === 'string' && !Number.isNaN(Date.parse(value))) {
    return normalizeIsoTimestamp(value)
  }

  if (value instanceof Date && !Number.isNaN(value.getTime())) {
    return value.toISOString()
  }

  return fallback
}

const providerCheckoutFromPayload = (
  payload: unknown,
): Record<string, unknown> | undefined => {
  if (!isRecord(payload)) {
    return undefined
  }

  const data = nestedUnknown(payload, ['data'])
  const json = nestedUnknown(payload, ['json'])
  const nestedCheckout = nestedUnknown(payload, ['data', 'checkout'])

  if (isRecord(nestedCheckout)) {
    return nestedCheckout
  }

  if (isRecord(data)) {
    return data
  }

  if (isRecord(json)) {
    return json
  }

  return payload
}

const providerCheckoutSafeId = (
  checkout: Record<string, unknown>,
): string | undefined => {
  const id = optionalString(checkout.id)

  return id !== undefined && hostedMdkRefIsSafe(id) ? id : undefined
}

const invoiceRefForProviderCheckout = (
  checkoutId: string,
  checkout: Record<string, unknown>,
): string => {
  const hasInvoice =
    nestedUnknown(checkout, ['invoice']) !== null &&
    nestedUnknown(checkout, ['invoice']) !== undefined

  return hasInvoice || optionalString(checkout.invoiceScid) !== undefined
    ? `mdk_invoice.redacted.${cleanRefSegment(checkoutId)}`
    : `mdk_invoice.redacted.pending.${cleanRefSegment(checkoutId)}`
}

const paymentHashRefForProviderCheckout = (
  checkoutId: string,
  checkout: Record<string, unknown>,
): string => {
  const hasPaymentHash =
    optionalString(nestedUnknown(checkout, ['invoice', 'paymentHash'])) !==
    undefined

  return hasPaymentHash
    ? `mdk_payment_hash.redacted.${cleanRefSegment(checkoutId)}`
    : `mdk_payment_hash.redacted.pending.${cleanRefSegment(checkoutId)}`
}

const providerCheckoutBolt11Invoice = (
  checkout: Record<string, unknown>,
): string | undefined => {
  const candidates = [
    nestedUnknown(checkout, ['invoice', 'invoice']),
    nestedUnknown(checkout, ['invoice', 'bolt11']),
    nestedUnknown(checkout, ['invoice', 'paymentRequest']),
    checkout.bolt11,
    checkout.bolt11Invoice,
    checkout.invoice,
    checkout.paymentRequest,
  ]

  return candidates.find(
    (candidate): candidate is string =>
      typeof candidate === 'string' &&
      /^(?:lnbc|lntb|lntbs|lnbcrt)[a-z0-9]+$/i.test(candidate),
  )
}

const routeCurrencyAmount = (
  request: OpenAgentsHostedMdkCheckoutRequest,
): Readonly<{ amount: number; currency: 'SAT' | 'USD' }> | undefined => {
  if (
    request.amount.asset === 'usd' &&
    request.amount.denomination === 'usd_cent'
  ) {
    return {
      amount: request.amount.amountMinorUnits,
      currency: 'USD',
    }
  }

  if (
    request.amount.asset === 'bitcoin' &&
    request.amount.denomination === 'bitcoin_millisatoshi' &&
    Number.isInteger(request.amount.amountMinorUnits / 1000)
  ) {
    return {
      amount: request.amount.amountMinorUnits / 1000,
      currency: 'SAT',
    }
  }

  return undefined
}

const metadataFromHostedRequest = (
  request: OpenAgentsHostedMdkCheckoutRequest,
): Record<string, string> => ({
  challenge_ref: request.challengeRef,
  idempotency_key_hash: request.idempotencyKeyHash,
  mode: request.mode,
  product_id: request.productId,
  return_ref: request.returnRef,
  sandbox: String(request.sandbox),
  ...(request.cancelRef === '' ? {} : { cancel_ref: request.cancelRef }),
  ...(request.l402CredentialRef === null
    ? {}
    : { l402_credential_ref: request.l402CredentialRef }),
  ...(request.siteRef === null ? {} : { site_ref: request.siteRef }),
  ...Object.fromEntries(
    request.metadataRefs.map((ref, index) => [`metadata_ref_${index}`, ref]),
  ),
})

const routeCreateCheckoutParams = (
  request: OpenAgentsHostedMdkCheckoutRequest,
): Record<string, unknown> | undefined => {
  const amount = routeCurrencyAmount(request)

  return amount === undefined
    ? undefined
    : ({
        amount: amount.amount,
        currency: amount.currency,
        metadata: metadataFromHostedRequest(request),
        requireCustomerData: request.customerDataRefs,
        title: request.productId,
        type: 'AMOUNT',
        ...(request.environment === 'sandbox'
          ? { sandbox: request.sandbox }
          : {}),
      } satisfies Record<string, unknown>)
}

const runtimeConfigError = (
  runtime: OpenAgentsHostedMdkRouteClientRuntime,
): OpenAgentsHostedMdkClientError | undefined => {
  if (runtime.routeSecret.trim() === '' || runtime.routeUrl.trim() === '') {
    return clientError(
      'missing_configuration',
      'detail.mdk_hosted.route_runtime_missing',
    )
  }

  try {
    const url = new URL(runtime.routeUrl)

    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      return clientError(
        'missing_configuration',
        'detail.mdk_hosted.route_url_invalid',
      )
    }
  } catch {
    return clientError(
      'missing_configuration',
      'detail.mdk_hosted.route_url_invalid',
    )
  }

  if (!cleanCheckoutPathPattern.test(runtime.checkoutPathBase)) {
    return clientError(
      'missing_configuration',
      'detail.mdk_hosted.checkout_path_invalid',
    )
  }

  return undefined
}

const unexpectedClientError = (): OpenAgentsHostedMdkClientError =>
  clientError('provider_unavailable', 'detail.mdk_hosted.unexpected_failure')

const normalizeClientError = (
  error: unknown,
): OpenAgentsHostedMdkClientError =>
  error instanceof OpenAgentsHostedMdkClientError
    ? error
    : unexpectedClientError()

const postMdkRoutePromise = async (
  runtime: OpenAgentsHostedMdkRouteClientRuntime,
  body: MdkRouteBody,
): Promise<unknown> => {
  const routeTimeoutMs =
    runtime.routeTimeoutMs === undefined || runtime.routeTimeoutMs <= 0
      ? defaultMdkRouteTimeoutMs
      : runtime.routeTimeoutMs
  const abortController = new AbortController()
  let timeout: ReturnType<typeof setTimeout> | undefined
  const timeoutPromise = new Promise<never>((_, reject) => {
    timeout = setTimeout(() => {
      abortController.abort()
      reject(
        clientError(
          'provider_unavailable',
          'detail.mdk_hosted.route_timeout',
        ),
      )
    }, routeTimeoutMs)
  })
  const routePromise = (runtime.fetch ?? fetch)(runtime.routeUrl, {
    body: JSON.stringify(body),
    headers: {
      'content-type': 'application/json',
      'x-moneydevkit-webhook-secret': runtime.routeSecret,
    },
    method: 'POST',
    signal: abortController.signal,
  })
    .then(async response => ({
      ok: response.ok,
      payload: await response.json().catch(() => ({})),
      status: response.status,
    }))
    .catch(error => {
      if (error instanceof OpenAgentsHostedMdkClientError) {
        throw error
      }

      throw clientError(
        'provider_unavailable',
        'detail.mdk_hosted.route_unavailable',
      )
    })
  const result = await Promise.race([routePromise, timeoutPromise]).finally(
    () => {
      if (timeout !== undefined) {
        clearTimeout(timeout)
      }
    },
  )

  if (!result.ok) {
    throw clientError(
      result.status >= 500 ? 'provider_unavailable' : 'provider_rejected',
      `detail.mdk_hosted.route_status_${result.status}`,
    )
  }

  return result.payload
}

const postMdkRoute = (
  runtime: OpenAgentsHostedMdkRouteClientRuntime,
  body: MdkRouteBody,
): Effect.Effect<unknown, OpenAgentsHostedMdkClientError> =>
  Effect.tryPromise({
    catch: normalizeClientError,
    try: () => postMdkRoutePromise(runtime, body),
  })

export const openAgentsHostedMdkPayloadHasPrivateMaterial =
  hostedMdkValueHasPrivateMaterial

export const validateOpenAgentsHostedMdkClientConfig = (
  config: OpenAgentsHostedMdkClientConfig,
): OpenAgentsHostedMdkClientError | undefined => {
  if (hostedMdkValueHasPrivateMaterial(config)) {
    return clientError(
      'secret_leakage_detected',
      'detail.mdk_hosted.config_secret_leakage',
    )
  }

  if (
    !hostedMdkRefIsSafe(config.configRef) ||
    !hostedMdkRefIsSafe(config.providerRef) ||
    (config.credentialBindingRef !== null &&
      !hostedMdkRefIsSafe(config.credentialBindingRef)) ||
    (config.webhookBindingRef !== null &&
      !hostedMdkRefIsSafe(config.webhookBindingRef))
  ) {
    return clientError('unsafe_metadata', 'detail.mdk_hosted.config_ref_unsafe')
  }

  if (config.credentialBindingRef === null) {
    return clientError(
      'missing_configuration',
      'detail.mdk_hosted.credential_binding_missing',
    )
  }

  return undefined
}

export const validateOpenAgentsHostedMdkCheckoutRequest = (
  request: OpenAgentsHostedMdkCheckoutRequest,
  nowIso: string,
): OpenAgentsHostedMdkClientError | undefined => {
  if (hostedMdkValueHasPrivateMaterial(request)) {
    return clientError(
      'secret_leakage_detected',
      'detail.mdk_hosted.request_secret_leakage',
    )
  }

  if (
    !hostedMdkRefIsSafe(request.challengeRef) ||
    !hostedMdkRefIsSafe(request.productId) ||
    !hostedMdkRefIsSafe(request.idempotencyKeyHash) ||
    !hostedMdkRefIsSafe(request.returnRef) ||
    !hostedMdkRefIsSafe(request.cancelRef) ||
    (request.l402CredentialRef !== null &&
      !hostedMdkRefIsSafe(request.l402CredentialRef)) ||
    (request.siteRef !== null && !hostedMdkRefIsSafe(request.siteRef)) ||
    !hostedMdkRefsAreSafe(request.metadataRefs) ||
    !hostedMdkRefsAreSafe(request.customerDataRefs)
  ) {
    return clientError(
      'unsafe_metadata',
      'detail.mdk_hosted.request_ref_unsafe',
    )
  }

  if (!hostedMdkAmountSupported(request.amount)) {
    return clientError(
      'unsupported_asset_denomination',
      'detail.mdk_hosted.amount_unsupported',
    )
  }

  if (
    Number.isNaN(Date.parse(request.challengeExpiresAt)) ||
    Date.parse(request.challengeExpiresAt) <= Date.parse(nowIso)
  ) {
    return clientError('stale_challenge', 'detail.mdk_hosted.challenge_stale')
  }

  return undefined
}

export const buildOpenAgentsHostedMdkCheckoutRequest = (
  input: CreateCheckoutRequestInput,
): OpenAgentsHostedMdkCheckoutRequest | OpenAgentsHostedMdkClientError => {
  const productChallengeMismatch =
    input.product.productId !== input.challenge.productId ||
    !amountEquals(input.product.price, input.challenge.price)
  const l402ChallengeMismatch =
    input.l402Payload !== null &&
    input.l402Payload !== undefined &&
    (input.l402Payload.challengeRef !== input.challenge.challengeRef ||
      input.l402Payload.productId !== input.challenge.productId ||
      !amountEquals(input.l402Payload.amount, input.challenge.price))

  if (productChallengeMismatch || l402ChallengeMismatch) {
    return clientError(
      'provider_rejected',
      'detail.mdk_hosted.challenge_product_mismatch',
    )
  }

  if (
    input.product.status !== 'active' ||
    input.challenge.status !== 'issued'
  ) {
    return clientError(
      'provider_rejected',
      'detail.mdk_hosted.product_or_challenge_not_active',
    )
  }

  return {
    amount: input.challenge.price,
    cancelRef: input.cancelRef,
    challengeExpiresAt: input.challenge.expiresAt,
    challengeRef: input.challenge.challengeRef,
    customerDataRefs: [...(input.customerDataRefs ?? [])],
    environment: input.environment,
    idempotencyKeyHash: input.challenge.idempotencyKeyHash,
    l402CredentialRef: input.l402Payload?.credentialRef ?? null,
    metadataRefs: [
      ...input.challenge.metadataRefs,
      ...(input.metadataRefs ?? []),
    ],
    mode: input.mode ?? checkoutModeFromProduct(input.product),
    productId: input.challenge.productId,
    returnRef: input.returnRef,
    sandbox: input.sandbox,
    siteRef: input.siteRef ?? null,
  }
}

export const hostedMdkCheckoutRequestFromPaymentChallenge = (
  input: CreateCheckoutRequestInput,
): Effect.Effect<
  OpenAgentsHostedMdkCheckoutRequest,
  OpenAgentsHostedMdkClientError
> => {
  const request = buildOpenAgentsHostedMdkCheckoutRequest(input)

  return request instanceof OpenAgentsHostedMdkClientError
    ? Effect.fail(request)
    : Effect.succeed(request)
}

export const projectOpenAgentsHostedMdkCheckoutResponse = (
  response: OpenAgentsHostedMdkCheckoutResponse,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsHostedMdkCheckoutProjection => {
  const privileged =
    audience === 'agent' || audience === 'customer' || audience === 'operator'

  return {
    acceptedWorkSettlementAuthority: false,
    amount: response.amount,
    audience,
    challengeRef: response.challengeRef,
    ...(response.checkoutLaunchPath === undefined
      ? {}
      : { checkoutLaunchPath: response.checkoutLaunchPath }),
    checkoutRef: response.checkoutRef,
    checkoutUrlRef: response.checkoutUrlRef,
    environment: response.environment,
    invoiceRef: privileged ? response.invoiceRef : null,
    paymentHashRef: privileged ? response.paymentHashRef : null,
    productId: response.productId,
    provider: 'mdk_hosted',
    providerPayoutAuthority: false,
    providerRef: response.providerRef,
    sandbox: response.sandbox,
    settlementAuthority: 'buyer_payment_evidence_only',
    siteRef: response.siteRef,
    status: response.status,
  }
}

export const openAgentsHostedMdkCheckoutProjectionHasPrivateMaterial = (
  projection: OpenAgentsHostedMdkCheckoutProjection,
): boolean => hostedMdkValueHasPrivateMaterial(projection)

export const makeFakeOpenAgentsHostedMdkClient = (
  config: OpenAgentsHostedMdkClientConfig,
  options: FakeClientOptions = {},
): OpenAgentsHostedMdkClient => {
  const providerRef = options.providerRef ?? config.providerRef
  const nowIso = options.nowIso ?? '2026-06-06T09:00:00.000Z'
  const createCheckoutPromise = async (
    request: OpenAgentsHostedMdkCheckoutRequest,
  ): Promise<OpenAgentsHostedMdkCheckoutResponse> => {
    const configError = validateOpenAgentsHostedMdkClientConfig({
      ...config,
      providerRef,
    })
    const requestError = validateOpenAgentsHostedMdkCheckoutRequest(
      request,
      nowIso,
    )

    if (configError !== undefined) {
      throw configError
    }

    if (requestError !== undefined) {
      throw requestError
    }

    if (options.configured === false) {
      throw clientError(
        'missing_configuration',
        'detail.mdk_hosted.fake_config_disabled',
      )
    }

    if (options.providerAvailable === false) {
      throw clientError(
        'provider_unavailable',
        'detail.mdk_hosted.fake_provider_unavailable',
      )
    }

    if (options.rejectCheckout === true) {
      throw clientError(
        'provider_rejected',
        'detail.mdk_hosted.fake_provider_rejected',
      )
    }

    const deterministicRef = cleanRefSegment(
      `${request.productId}_${request.idempotencyKeyHash}`,
    )

    return {
      acceptedWorkSettlementAuthority: false,
      amount: request.amount,
      cancelRef: request.cancelRef,
      challengeRef: request.challengeRef,
      checkoutRef: `mdk_checkout.${deterministicRef}`,
      checkoutLaunchPath: `/checkout/${deterministicRef}`,
      checkoutUrlRef: `mdk_checkout_url.${deterministicRef}`,
      createdAt: nowIso,
      environment: request.environment,
      expiresAt: request.challengeExpiresAt,
      idempotencyKeyHash: request.idempotencyKeyHash,
      invoiceRef: `mdk_invoice.redacted.${deterministicRef}`,
      metadataRefs: request.metadataRefs,
      paymentHashRef: `mdk_payment_hash.redacted.${deterministicRef}`,
      productId: request.productId,
      provider: 'mdk_hosted',
      providerPayoutAuthority: false,
      providerRef,
      returnRef: request.returnRef,
      sandbox: request.sandbox,
      settlementAuthority: 'buyer_payment_evidence_only',
      siteRef: request.siteRef,
      status: 'created',
    }
  }

  return {
    createCheckoutPromise,
    implementationState: 'fake_provider_contract',
    createCheckout: (
      request: OpenAgentsHostedMdkCheckoutRequest,
    ): Effect.Effect<
      OpenAgentsHostedMdkCheckoutResponse,
      OpenAgentsHostedMdkClientError
    > =>
      Effect.tryPromise({
        catch: normalizeClientError,
        try: () => createCheckoutPromise(request),
      }),
    getCheckoutStatus: (
      request: OpenAgentsHostedMdkCheckoutStatusRequest,
    ): Effect.Effect<
      OpenAgentsHostedMdkCheckoutStatusResponse,
      OpenAgentsHostedMdkClientError
    > =>
      Effect.gen(function* () {
        const configError = validateOpenAgentsHostedMdkClientConfig({
          ...config,
          providerRef,
        })

        if (configError !== undefined) {
          return yield* configError
        }

        if (
          !hostedMdkRefIsSafe(request.checkoutRef) ||
          !hostedMdkRefIsSafe(request.providerRef) ||
          (request.siteRef !== null && !hostedMdkRefIsSafe(request.siteRef))
        ) {
          return yield* clientError(
            'unsafe_metadata',
            'detail.mdk_hosted.status_ref_unsafe',
          )
        }

        return {
          checkoutRef: request.checkoutRef,
          environment: request.environment,
          expiresAt: null,
          observedAt: nowIso,
          provider: 'mdk_hosted',
          providerRef,
          sandbox: request.sandbox,
          siteRef: request.siteRef,
          status: 'created',
        }
      }),
    getPrivateL402PaymentPayload: (
      request: OpenAgentsHostedMdkPrivateL402PaymentRequest,
    ): Effect.Effect<
      OpenAgentsHostedMdkPrivateL402PaymentPayload,
      OpenAgentsHostedMdkClientError
    > =>
      Effect.gen(function* () {
        const configError = validateOpenAgentsHostedMdkClientConfig({
          ...config,
          providerRef,
        })

        if (configError !== undefined) {
          return yield* configError
        }

        if (
          !hostedMdkRefIsSafe(request.checkoutRef) ||
          !hostedMdkRefIsSafe(request.providerRef) ||
          (request.siteRef !== null && !hostedMdkRefIsSafe(request.siteRef))
        ) {
          return yield* clientError(
            'unsafe_metadata',
            'detail.mdk_hosted.private_payment_ref_unsafe',
          )
        }

        return {
          bolt11: `lntbs100n1${cleanInvoiceSegment(request.checkoutRef)}`,
          checkoutRef: request.checkoutRef,
          environment: request.environment,
          expiresAt: null,
          provider: 'mdk_hosted',
          providerRef,
          sandbox: request.sandbox,
          siteRef: request.siteRef,
        }
      }),
  }
}

export const makeMissingOpenAgentsHostedMdkClient = (
  providerRef = 'provider.openagents.hosted_mdk.missing',
): OpenAgentsHostedMdkClient => ({
  createCheckoutPromise: async () => {
    throw clientError(
      'missing_configuration',
      'detail.mdk_hosted.runtime_missing',
    )
  },
  implementationState: 'missing_configuration',
  createCheckout: () =>
    Effect.fail(
      clientError('missing_configuration', 'detail.mdk_hosted.runtime_missing'),
    ),
  getCheckoutStatus: () =>
    Effect.fail(
      clientError('missing_configuration', 'detail.mdk_hosted.runtime_missing'),
    ),
  getPrivateL402PaymentPayload: () =>
    Effect.fail(
      clientError('missing_configuration', 'detail.mdk_hosted.runtime_missing'),
    ),
})

export const makeOpenAgentsHostedMdkRouteClient = (
  config: OpenAgentsHostedMdkClientConfig,
  runtime: OpenAgentsHostedMdkRouteClientRuntime,
): OpenAgentsHostedMdkClient => {
  const createCheckoutPromise = async (
    request: OpenAgentsHostedMdkCheckoutRequest,
  ): Promise<OpenAgentsHostedMdkCheckoutResponse> => {
    const nowIso = runtime.nowIso?.() ?? currentIsoTimestamp()
    const configError = validateOpenAgentsHostedMdkClientConfig(config)
    const runtimeError = runtimeConfigError(runtime)
    const requestError = validateOpenAgentsHostedMdkCheckoutRequest(
      request,
      nowIso,
    )
    const params = routeCreateCheckoutParams(request)

    if (configError !== undefined) {
      throw configError
    }

    if (runtimeError !== undefined) {
      throw runtimeError
    }

    if (requestError !== undefined) {
      throw requestError
    }

    if (params === undefined) {
      throw clientError(
        'unsupported_asset_denomination',
        'detail.mdk_hosted.route_amount_unsupported',
      )
    }

    const payload = await postMdkRoutePromise(runtime, {
      handler: 'create_checkout',
      params,
    })
    const checkout = providerCheckoutFromPayload(payload)
    const checkoutId =
      checkout === undefined ? undefined : providerCheckoutSafeId(checkout)

    if (checkout === undefined || checkoutId === undefined) {
      throw clientError(
        'provider_rejected',
        'detail.mdk_hosted.route_payload_invalid',
      )
    }

    const checkoutRef = `${providerCheckoutRefPrefix}${checkoutId}`
    const status = hostedStatusFromProviderStatus(checkout.status)
    const createdAt = isoFromUnknown(checkout.createdAt, nowIso)
    const expiresAt = isoFromUnknown(
      checkout.expiresAt,
      request.challengeExpiresAt,
    )

    return {
      acceptedWorkSettlementAuthority: false,
      amount: request.amount,
      cancelRef: request.cancelRef,
      challengeRef: request.challengeRef,
      checkoutLaunchPath: cleanCheckoutPath(
        runtime.checkoutPathBase,
        checkoutId,
      ),
      checkoutRef,
      checkoutUrlRef: `mdk_checkout_url.${cleanRefSegment(checkoutId)}`,
      createdAt,
      environment: request.environment,
      expiresAt,
      idempotencyKeyHash: request.idempotencyKeyHash,
      invoiceRef: invoiceRefForProviderCheckout(checkoutId, checkout),
      metadataRefs: request.metadataRefs,
      paymentHashRef: paymentHashRefForProviderCheckout(checkoutId, checkout),
      productId: request.productId,
      provider: 'mdk_hosted',
      providerPayoutAuthority: false,
      providerRef: config.providerRef,
      returnRef: request.returnRef,
      sandbox: request.sandbox,
      settlementAuthority: 'buyer_payment_evidence_only',
      siteRef: request.siteRef,
      status,
    }
  }

  return {
    createCheckoutPromise,
    implementationState: 'live_provider_configured',
    createCheckout: (
      request: OpenAgentsHostedMdkCheckoutRequest,
    ): Effect.Effect<
      OpenAgentsHostedMdkCheckoutResponse,
      OpenAgentsHostedMdkClientError
    > =>
      Effect.tryPromise({
        catch: normalizeClientError,
        try: () => createCheckoutPromise(request),
      }),
    getCheckoutStatus: (
      request: OpenAgentsHostedMdkCheckoutStatusRequest,
    ): Effect.Effect<
      OpenAgentsHostedMdkCheckoutStatusResponse,
      OpenAgentsHostedMdkClientError
    > =>
      Effect.gen(function* () {
        const nowIso = runtime.nowIso?.() ?? currentIsoTimestamp()
        const configError = validateOpenAgentsHostedMdkClientConfig(config)
        const runtimeError = runtimeConfigError(runtime)
        const checkoutId = providerCheckoutIdFromRef(request.checkoutRef)

        if (configError !== undefined) {
          return yield* configError
        }

        if (runtimeError !== undefined) {
          return yield* runtimeError
        }

        if (
          checkoutId === undefined ||
          !hostedMdkRefIsSafe(request.providerRef) ||
          (request.siteRef !== null && !hostedMdkRefIsSafe(request.siteRef))
        ) {
          return yield* clientError(
            'unsafe_metadata',
            'detail.mdk_hosted.status_ref_unsafe',
          )
        }

        const payload = yield* postMdkRoute(runtime, {
          checkoutId,
          handler: 'get_checkout',
        })
        const checkout = providerCheckoutFromPayload(payload)

        if (checkout === undefined) {
          return yield* clientError(
            'provider_rejected',
            'detail.mdk_hosted.route_payload_invalid',
          )
        }

        return {
          checkoutRef: request.checkoutRef,
          environment: request.environment,
          expiresAt:
            checkout.expiresAt === undefined || checkout.expiresAt === null
              ? null
              : isoFromUnknown(checkout.expiresAt, nowIso),
          observedAt: nowIso,
          provider: 'mdk_hosted',
          providerRef: config.providerRef,
          sandbox: request.sandbox,
          siteRef: request.siteRef,
          status: hostedStatusFromProviderStatus(checkout.status),
        }
      }),
    getPrivateL402PaymentPayload: (
      request: OpenAgentsHostedMdkPrivateL402PaymentRequest,
    ): Effect.Effect<
      OpenAgentsHostedMdkPrivateL402PaymentPayload,
      OpenAgentsHostedMdkClientError
    > =>
      Effect.gen(function* () {
        const nowIso = runtime.nowIso?.() ?? currentIsoTimestamp()
        const configError = validateOpenAgentsHostedMdkClientConfig(config)
        const runtimeError = runtimeConfigError(runtime)
        const checkoutId = providerCheckoutIdFromRef(request.checkoutRef)

        if (configError !== undefined) {
          return yield* configError
        }

        if (runtimeError !== undefined) {
          return yield* runtimeError
        }

        if (
          checkoutId === undefined ||
          !hostedMdkRefIsSafe(request.providerRef) ||
          (request.siteRef !== null && !hostedMdkRefIsSafe(request.siteRef))
        ) {
          return yield* clientError(
            'unsafe_metadata',
            'detail.mdk_hosted.private_payment_ref_unsafe',
          )
        }

        const payload = yield* postMdkRoute(runtime, {
          checkoutId,
          handler: 'get_checkout',
        })
        const checkout = providerCheckoutFromPayload(payload)
        const bolt11 =
          checkout === undefined
            ? undefined
            : providerCheckoutBolt11Invoice(checkout)

        if (checkout === undefined || bolt11 === undefined) {
          return yield* clientError(
            'provider_rejected',
            'detail.mdk_hosted.private_payment_payload_unavailable',
          )
        }

        return {
          bolt11,
          checkoutRef: request.checkoutRef,
          environment: request.environment,
          expiresAt:
            checkout.expiresAt === undefined || checkout.expiresAt === null
              ? null
              : isoFromUnknown(checkout.expiresAt, nowIso),
          provider: 'mdk_hosted',
          providerRef: config.providerRef,
          sandbox: request.sandbox,
          siteRef: request.siteRef,
        }
      }),
  }
}
