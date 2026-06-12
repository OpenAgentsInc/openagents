import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { BuyerPaymentLedgerAmount } from './buyer-payment-ledger'
import { OpenAgentsHostedMdkCheckoutRequest } from './hosted-mdk-client'
import { OpenAgentsL402CredentialPayload } from './l402-credential-service'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsMdkCoreCheckoutRoute = S.Literals([
  'confirm_checkout',
  'create_checkout',
  'get_checkout',
])
export type OpenAgentsMdkCoreCheckoutRoute =
  typeof OpenAgentsMdkCoreCheckoutRoute.Type

export const OpenAgentsMdkCoreCheckoutMode = S.Literals([
  'amount',
  'product',
])
export type OpenAgentsMdkCoreCheckoutMode =
  typeof OpenAgentsMdkCoreCheckoutMode.Type

export const OpenAgentsMdkCoreCheckoutErrorReason = S.Literals([
  'invalid_route',
  'metadata_invalid',
  'secret_leakage_detected',
  'signature_invalid',
  'signature_missing',
  'unsupported_checkout',
])
export type OpenAgentsMdkCoreCheckoutErrorReason =
  typeof OpenAgentsMdkCoreCheckoutErrorReason.Type

export const OpenAgentsMdkCoreCheckoutCustomerInput = S.Record(
  S.String,
  S.String,
)
export type OpenAgentsMdkCoreCheckoutCustomerInput =
  typeof OpenAgentsMdkCoreCheckoutCustomerInput.Type

export const OpenAgentsMdkCoreCheckoutMetadata = S.Record(S.String, S.String)
export type OpenAgentsMdkCoreCheckoutMetadata =
  typeof OpenAgentsMdkCoreCheckoutMetadata.Type

export const OpenAgentsMdkCoreCheckoutCommonInput = S.Struct({
  cancelRef: S.String,
  checkoutPath: S.optionalKey(S.String),
  customer: S.optionalKey(OpenAgentsMdkCoreCheckoutCustomerInput),
  metadata: S.optionalKey(OpenAgentsMdkCoreCheckoutMetadata),
  requireCustomerData: S.optionalKey(S.Array(S.String)),
  returnRef: S.String,
  sandbox: S.optionalKey(S.Boolean),
})
export type OpenAgentsMdkCoreCheckoutCommonInput =
  typeof OpenAgentsMdkCoreCheckoutCommonInput.Type

export const OpenAgentsMdkCoreAmountCheckoutInput = S.Struct({
  ...OpenAgentsMdkCoreCheckoutCommonInput.fields,
  amount: BuyerPaymentLedgerAmount,
  descriptionRef: S.optionalKey(S.String),
  mode: S.Literal('amount'),
  titleRef: S.optionalKey(S.String),
})
export type OpenAgentsMdkCoreAmountCheckoutInput =
  typeof OpenAgentsMdkCoreAmountCheckoutInput.Type

export const OpenAgentsMdkCoreProductCheckoutInput = S.Struct({
  ...OpenAgentsMdkCoreCheckoutCommonInput.fields,
  mode: S.Literal('product'),
  productId: S.String,
  productPriceRef: S.optionalKey(S.String),
})
export type OpenAgentsMdkCoreProductCheckoutInput =
  typeof OpenAgentsMdkCoreProductCheckoutInput.Type

export const OpenAgentsMdkCoreCreateCheckoutInput = S.Union([
  OpenAgentsMdkCoreAmountCheckoutInput,
  OpenAgentsMdkCoreProductCheckoutInput,
])
export type OpenAgentsMdkCoreCreateCheckoutInput =
  typeof OpenAgentsMdkCoreCreateCheckoutInput.Type

export const OpenAgentsMdkCorePreparedCheckout = S.Struct({
  amount: S.NullOr(BuyerPaymentLedgerAmount),
  cancelRef: S.String,
  checkoutPath: S.String,
  customerFieldKeys: S.Array(S.String),
  customerValueCount: S.Number,
  metadataKeys: S.Array(S.String),
  mode: OpenAgentsMdkCoreCheckoutMode,
  productId: S.NullOr(S.String),
  productPriceRef: S.NullOr(S.String),
  requireCustomerData: S.Array(S.String),
  returnRef: S.String,
  sandbox: S.Boolean,
})
export type OpenAgentsMdkCorePreparedCheckout =
  typeof OpenAgentsMdkCorePreparedCheckout.Type

export const OpenAgentsMdkCorePreparedCheckoutProjection = S.Struct({
  amount: S.NullOr(BuyerPaymentLedgerAmount),
  audience: OpenAgentsPaymentPolicyAudience,
  checkoutPath: S.String,
  customerFieldKeys: S.Array(S.String),
  customerValuesRedacted: S.Literal(true),
  metadataKeys: S.Array(S.String),
  metadataValuesRedacted: S.Literal(true),
  mode: OpenAgentsMdkCoreCheckoutMode,
  productId: S.NullOr(S.String),
  productPriceRef: S.NullOr(S.String),
  requireCustomerData: S.Array(S.String),
  sandbox: S.Boolean,
})
export type OpenAgentsMdkCorePreparedCheckoutProjection =
  typeof OpenAgentsMdkCorePreparedCheckoutProjection.Type

export const OpenAgentsMdkCoreSignedCheckoutUrlInput = S.Struct({
  cancelRef: S.String,
  checkoutPath: S.String,
  checkoutRef: S.String,
  expiresAt: S.String,
  issuedAt: S.String,
  returnRef: S.String,
  sandbox: S.Boolean,
})
export type OpenAgentsMdkCoreSignedCheckoutUrlInput =
  typeof OpenAgentsMdkCoreSignedCheckoutUrlInput.Type

export const OpenAgentsMdkCoreSignedCheckoutUrl = S.Struct({
  checkoutPath: S.String,
  expiresAt: S.String,
  issuedAt: S.String,
  signatureRef: S.String,
  signedPath: S.String,
  signerRef: S.String,
})
export type OpenAgentsMdkCoreSignedCheckoutUrl =
  typeof OpenAgentsMdkCoreSignedCheckoutUrl.Type

export const OpenAgentsMdkCoreHostedCheckoutPlan = S.Struct({
  hostedRequest: OpenAgentsHostedMdkCheckoutRequest,
  l402Payload: S.NullOr(OpenAgentsL402CredentialPayload),
  preparedCheckout: OpenAgentsMdkCorePreparedCheckout,
  signedCheckoutUrl: OpenAgentsMdkCoreSignedCheckoutUrl,
})
export type OpenAgentsMdkCoreHostedCheckoutPlan =
  typeof OpenAgentsMdkCoreHostedCheckoutPlan.Type

export class OpenAgentsMdkCoreCheckoutError extends S.TaggedErrorClass<OpenAgentsMdkCoreCheckoutError>()(
  'OpenAgentsMdkCoreCheckoutError',
  {
    detailRef: S.String,
    reason: OpenAgentsMdkCoreCheckoutErrorReason,
  },
) {}

export type OpenAgentsMdkCoreCheckoutSigner = Readonly<{
  signerRef: string
  sign: (canonicalPayload: string) => Promise<string>
  verify: (
    canonicalPayload: string,
    signatureBase64Url: string,
  ) => Promise<boolean>
}>

const textEncoder = new TextEncoder()
const metadataKeyLimit = 50
const metadataSizeLimitBytes = 1024
const metadataKeyLengthLimit = 100
const metadataKeyPattern = /^[A-Za-z0-9_-]+$/
const controlCharacterPattern = /[\x00-\x08\x0B-\x0C\x0E-\x1F]/u
const unsafeMetadataKeyPattern =
  /(access[_-]?token|bearer|cookie|customer[_-]?(email|name)|email|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|wallet|webhook)/i
const unsafeMetadataValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const checkoutError = (
  reason: OpenAgentsMdkCoreCheckoutErrorReason,
  detailRef: string,
): OpenAgentsMdkCoreCheckoutError =>
  new OpenAgentsMdkCoreCheckoutError({ detailRef, reason })

const base64UrlEncode = (bytes: Uint8Array): string =>
  btoa(String.fromCharCode(...bytes))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')

const importHmacKey = (secretKeyMaterial: string) =>
  crypto.subtle.importKey(
    'raw',
    textEncoder.encode(secretKeyMaterial),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign', 'verify'],
  )

const hmacBase64Url = async (
  secretKeyMaterial: string,
  canonicalPayload: string,
): Promise<string> => {
  const key = await importHmacKey(secretKeyMaterial)
  const signature = await crypto.subtle.sign(
    'HMAC',
    key,
    textEncoder.encode(canonicalPayload),
  )

  return base64UrlEncode(new Uint8Array(signature))
}

const metadataValueHasPrivateMaterial = (value: string): boolean =>
  containsProviderSecretMaterial(value) ||
  unsafeMetadataValuePattern.test(value) ||
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)

const objectValueHasPrivateMaterial = (value: unknown): boolean =>
  typeof value === 'string'
    ? metadataValueHasPrivateMaterial(value)
    : Array.isArray(value)
      ? value.some(objectValueHasPrivateMaterial)
      : value !== null && typeof value === 'object'
        ? openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
          Object.values(value).some(objectValueHasPrivateMaterial)
        : false

const normalizeFieldName = (field: string): string => {
  const camel = field
    .split(/[-_\s]+/u)
    .flatMap(word => word.split(/(?<=[a-z])(?=[A-Z])/u))
    .filter(Boolean)
    .map((word, index) => {
      const lower = word.toLowerCase()

      return index === 0
        ? lower
        : `${lower.charAt(0).toUpperCase()}${lower.slice(1)}`
    })
    .join('')

  return ['email', 'externalId', 'name'].includes(camel) ? camel : camel
}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,180}$/

const stableRefIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !metadataValueHasPrivateMaterial(value)

const metadataEntryIsUnsafe = (
  entry: readonly [string, string],
): boolean => {
  const [key, value] = entry

  return key === '' ||
    key.length > metadataKeyLengthLimit ||
    !metadataKeyPattern.test(key) ||
    unsafeMetadataKeyPattern.test(key) ||
    controlCharacterPattern.test(value) ||
    metadataValueHasPrivateMaterial(value)
}

export const validateOpenAgentsMdkCoreMetadata = (
  metadata: OpenAgentsMdkCoreCheckoutMetadata | undefined,
): OpenAgentsMdkCoreCheckoutError | undefined => {
  if (metadata === undefined) {
    return undefined
  }

  const entries = Object.entries(metadata)
  const sizeBytes = textEncoder.encode(JSON.stringify(metadata)).length

  if (entries.length > metadataKeyLimit || sizeBytes > metadataSizeLimitBytes) {
    return checkoutError(
      'metadata_invalid',
      'detail.mdk_core.metadata_limits_exceeded',
    )
  }

  return entries.some(metadataEntryIsUnsafe)
    ? checkoutError(
        'secret_leakage_detected',
        'detail.mdk_core.metadata_unsafe',
      )
    : undefined
}

export const normalizeOpenAgentsMdkCoreCustomer = (
  customer: OpenAgentsMdkCoreCheckoutCustomerInput | undefined,
): Readonly<Record<string, string>> =>
  Object.entries(customer ?? {}).reduce<Readonly<Record<string, string>>>(
    (normalized, [key, value]) => {
      const normalizedKey = normalizeFieldName(key)
      const normalizedValue = value.trim()

      return normalizedKey === '' || normalizedValue === ''
        ? normalized
        : { ...normalized, [normalizedKey]: normalizedValue }
    },
    {},
  )

export const sanitizeOpenAgentsMdkCoreCheckoutPath = (
  checkoutPath: string | null | undefined,
): string => {
  const defaultPath = '/checkout'

  if (checkoutPath === null || checkoutPath === undefined) {
    return defaultPath
  }

  if (
    !checkoutPath.startsWith('/') ||
    checkoutPath.includes('://') ||
    checkoutPath.includes('//') ||
    metadataValueHasPrivateMaterial(checkoutPath)
  ) {
    return defaultPath
  }

  const queryIndex = checkoutPath.indexOf('?')
  const hashIndex = checkoutPath.indexOf('#')
  const indexes = [queryIndex, hashIndex].filter(index => index >= 0)
  const endIndex = indexes.length === 0
    ? checkoutPath.length
    : Math.min(...indexes)
  const sanitized = checkoutPath.slice(0, endIndex)

  return sanitized === '' ? defaultPath : sanitized
}

export const resolveOpenAgentsMdkCoreCheckoutRoute = (
  body: unknown,
): Effect.Effect<
  OpenAgentsMdkCoreCheckoutRoute,
  OpenAgentsMdkCoreCheckoutError
> => {
  const record = body !== null && typeof body === 'object'
    ? body as Readonly<Record<string, unknown>>
    : {}
  const candidate = ['handler', 'route', 'target']
    .map(key => record[key])
    .find((value): value is string => typeof value === 'string')
    ?.toLowerCase()
  const route = OpenAgentsMdkCoreCheckoutRoute.literals.find(
    literal => literal === candidate,
  )

  return route === undefined
    ? Effect.fail(checkoutError('invalid_route', 'detail.mdk_core.route'))
    : Effect.succeed(route)
}

export const prepareOpenAgentsMdkCoreCheckout = (
  input: OpenAgentsMdkCoreCreateCheckoutInput,
): Effect.Effect<
  OpenAgentsMdkCorePreparedCheckout,
  OpenAgentsMdkCoreCheckoutError
> => {
  const metadataError = validateOpenAgentsMdkCoreMetadata(input.metadata)

  if (metadataError !== undefined) {
    return Effect.fail(metadataError)
  }

  if (
    objectValueHasPrivateMaterial({
      cancelRef: input.cancelRef,
      returnRef: input.returnRef,
    }) ||
    !stableRefIsSafe(input.cancelRef) ||
    !stableRefIsSafe(input.returnRef)
  ) {
    return Effect.fail(
      checkoutError(
        'secret_leakage_detected',
        'detail.mdk_core.return_ref_unsafe',
      ),
    )
  }

  const customer = normalizeOpenAgentsMdkCoreCustomer(input.customer)
  const requireCustomerData = (input.requireCustomerData ?? [])
    .map(normalizeFieldName)
    .filter(field => field !== '')
  const metadataKeys = Object.keys(input.metadata ?? {}).sort()
  const customerFieldKeys = Object.keys(customer).sort()
  const checkoutPath = sanitizeOpenAgentsMdkCoreCheckoutPath(
    input.checkoutPath,
  )

  return Effect.succeed({
    amount: input.mode === 'amount' ? input.amount : null,
    cancelRef: input.cancelRef,
    checkoutPath,
    customerFieldKeys,
    customerValueCount: customerFieldKeys.length,
    metadataKeys,
    mode: input.mode,
    productId: input.mode === 'product' ? input.productId : null,
    productPriceRef: input.mode === 'product'
      ? input.productPriceRef ?? null
      : null,
    requireCustomerData,
    returnRef: input.returnRef,
    sandbox: input.sandbox ?? false,
  })
}

const canonicalSignedCheckoutParams = (
  input: OpenAgentsMdkCoreSignedCheckoutUrlInput,
): URLSearchParams => {
  const params = new URLSearchParams()
  params.set('action', 'createCheckout')
  params.set('cancel_ref', input.cancelRef)
  params.set('checkout_ref', input.checkoutRef)
  params.set('expires_at', input.expiresAt)
  params.set('issued_at', input.issuedAt)
  params.set('return_ref', input.returnRef)
  params.set('sandbox', input.sandbox ? 'true' : 'false')
  params.sort()

  return params
}

export const makeOpenAgentsMdkCoreCheckoutSigner = async (input: {
  secretKeyMaterial: string
  signerRef: string
}): Promise<OpenAgentsMdkCoreCheckoutSigner> => {
  if (
    input.signerRef.trim() === '' ||
    input.secretKeyMaterial.trim() === ''
  ) {
    throw checkoutError(
      'signature_missing',
      'detail.mdk_core.signer_missing',
    )
  }

  return {
    signerRef: input.signerRef,
    sign: (canonicalPayload: string) =>
      hmacBase64Url(input.secretKeyMaterial, canonicalPayload),
    verify: async (
      canonicalPayload: string,
      signatureBase64Url: string,
    ): Promise<boolean> =>
      hmacBase64Url(input.secretKeyMaterial, canonicalPayload)
        .then(expected => expected === signatureBase64Url),
  }
}

export const signOpenAgentsMdkCoreCheckoutUrl = (
  input: OpenAgentsMdkCoreSignedCheckoutUrlInput,
  signer: OpenAgentsMdkCoreCheckoutSigner,
): Effect.Effect<
  OpenAgentsMdkCoreSignedCheckoutUrl,
  OpenAgentsMdkCoreCheckoutError
> => Effect.tryPromise({
  catch: error =>
    error instanceof OpenAgentsMdkCoreCheckoutError
      ? error
      : checkoutError('signature_invalid', 'detail.mdk_core.signing_failed'),
  try: async () => {
    const checkoutPath = sanitizeOpenAgentsMdkCoreCheckoutPath(
      input.checkoutPath,
    )

    if (
      objectValueHasPrivateMaterial(input) ||
      !stableRefIsSafe(input.checkoutRef) ||
      !stableRefIsSafe(input.returnRef) ||
      !stableRefIsSafe(input.cancelRef)
    ) {
      throw checkoutError(
        'secret_leakage_detected',
        'detail.mdk_core.signed_url_unsafe',
      )
    }

    const params = canonicalSignedCheckoutParams({
      ...input,
      checkoutPath,
    })
    const canonicalPayload = params.toString()
    const signature = await signer.sign(canonicalPayload)
    params.set('signature', signature)

    return {
      checkoutPath,
      expiresAt: input.expiresAt,
      issuedAt: input.issuedAt,
      signatureRef: `signature.mdk_core.${signer.signerRef}`,
      signedPath: `${checkoutPath}?${params.toString()}`,
      signerRef: signer.signerRef,
    }
  },
})

export const verifyOpenAgentsMdkCoreCheckoutUrl = (
  signedPath: string,
  signer: OpenAgentsMdkCoreCheckoutSigner,
): Effect.Effect<boolean, OpenAgentsMdkCoreCheckoutError> =>
  Effect.tryPromise({
    catch: error =>
      error instanceof OpenAgentsMdkCoreCheckoutError
        ? error
        : checkoutError(
            'signature_invalid',
            'detail.mdk_core.signature_invalid',
          ),
    try: async () => {
      const url = new URL(signedPath, 'https://openagents.com')
      const signature = url.searchParams.get('signature')

      if (signature === null || signature === '') {
        throw checkoutError(
          'signature_missing',
          'detail.mdk_core.signature_missing',
        )
      }

      const params = new URLSearchParams(url.searchParams)
      params.delete('signature')
      params.sort()

      return signer.verify(params.toString(), signature)
    },
  })

export const projectOpenAgentsMdkCorePreparedCheckout = (
  prepared: OpenAgentsMdkCorePreparedCheckout,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsMdkCorePreparedCheckoutProjection => ({
  amount: prepared.amount,
  audience,
  checkoutPath: prepared.checkoutPath,
  customerFieldKeys: prepared.customerFieldKeys,
  customerValuesRedacted: true,
  metadataKeys: prepared.metadataKeys,
  metadataValuesRedacted: true,
  mode: prepared.mode,
  productId: prepared.productId,
  productPriceRef: prepared.productPriceRef,
  requireCustomerData: prepared.requireCustomerData,
  sandbox: prepared.sandbox,
})

export const openAgentsMdkCoreCheckoutProjectionHasPrivateMaterial = (
  projection: OpenAgentsMdkCorePreparedCheckoutProjection,
): boolean => objectValueHasPrivateMaterial(projection)
