import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  MdkPayoutModeGateProjection,
  hostedMdkDirectPayoutDisabledGate,
} from './mdk-payout-mode-gate'
import {
  OpenAgentsPaidEndpointMethod,
  OpenAgentsPaidEndpointPrice,
} from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsSitePaymentProvider = S.Literal('openagents_hosted_mdk')
export type OpenAgentsSitePaymentProvider =
  typeof OpenAgentsSitePaymentProvider.Type

export const OpenAgentsSitePaymentSettlementMode = S.Literals([
  'accepted_work_linked',
  'checkout_only',
  'deferred',
])
export type OpenAgentsSitePaymentSettlementMode =
  typeof OpenAgentsSitePaymentSettlementMode.Type

export const OpenAgentsSitePaymentEntitlementScope = S.Literals([
  'account',
  'action',
  'path',
  'product',
  'site',
])
export type OpenAgentsSitePaymentEntitlementScope =
  typeof OpenAgentsSitePaymentEntitlementScope.Type

export const OpenAgentsSitePaymentPublicProjectionState = S.Literals([
  'hidden',
  'listed',
  'proof_only',
  'redacted',
])
export type OpenAgentsSitePaymentPublicProjectionState =
  typeof OpenAgentsSitePaymentPublicProjectionState.Type

export const OpenAgentsSitePaymentCustomerDataRequirement = S.Struct({
  key: S.String,
  kind: S.Literals(['email', 'name', 'text', 'url']),
  labelRef: S.String,
  required: S.Boolean,
})
export type OpenAgentsSitePaymentCustomerDataRequirement =
  typeof OpenAgentsSitePaymentCustomerDataRequirement.Type

export const OpenAgentsSitePaymentProduct = S.Struct({
  agentReadable: S.Boolean,
  checkoutPath: S.String,
  customerDataRequirements: S.Array(
    OpenAgentsSitePaymentCustomerDataRequirement,
  ),
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  id: S.String,
  metadataRefs: S.Array(S.String),
  price: OpenAgentsPaidEndpointPrice,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
})
export type OpenAgentsSitePaymentProduct =
  typeof OpenAgentsSitePaymentProduct.Type

export const OpenAgentsSitePaymentPaidAction = S.Struct({
  actionRef: S.String,
  agentReadable: S.Boolean,
  checkoutPath: S.String,
  customerDataRequirements: S.Array(
    OpenAgentsSitePaymentCustomerDataRequirement,
  ),
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  id: S.String,
  metadataRefs: S.Array(S.String),
  method: OpenAgentsPaidEndpointMethod,
  path: S.String,
  price: OpenAgentsPaidEndpointPrice,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
})
export type OpenAgentsSitePaymentPaidAction =
  typeof OpenAgentsSitePaymentPaidAction.Type

export const OpenAgentsSitePaymentBlock = S.Struct({
  agentReadable: S.Boolean,
  enabled: S.Boolean,
  metadataRefs: S.Array(S.String),
  paidActions: S.Array(OpenAgentsSitePaymentPaidAction),
  products: S.Array(OpenAgentsSitePaymentProduct),
  provider: OpenAgentsSitePaymentProvider,
  sandboxDefault: S.Boolean,
})
export type OpenAgentsSitePaymentBlock = typeof OpenAgentsSitePaymentBlock.Type

export const OpenAgentsSitePaymentManifest = S.Struct({
  payments: OpenAgentsSitePaymentBlock,
})
export type OpenAgentsSitePaymentManifest =
  typeof OpenAgentsSitePaymentManifest.Type

export const OpenAgentsSitePaymentProductProjection = S.Struct({
  agentReadable: S.Boolean,
  checkoutPath: S.String,
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  id: S.String,
  price: OpenAgentsPaidEndpointPrice,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
})
export type OpenAgentsSitePaymentProductProjection =
  typeof OpenAgentsSitePaymentProductProjection.Type

export const OpenAgentsSitePaymentPaidActionProjection = S.Struct({
  actionRef: S.String,
  agentReadable: S.Boolean,
  checkoutPath: S.String,
  displayRef: S.String,
  entitlementScope: OpenAgentsSitePaymentEntitlementScope,
  id: S.String,
  method: OpenAgentsPaidEndpointMethod,
  path: S.String,
  price: OpenAgentsPaidEndpointPrice,
  publicProjectionState: OpenAgentsSitePaymentPublicProjectionState,
  sandbox: S.Boolean,
  settlementMode: OpenAgentsSitePaymentSettlementMode,
})
export type OpenAgentsSitePaymentPaidActionProjection =
  typeof OpenAgentsSitePaymentPaidActionProjection.Type

export const OpenAgentsSitePaymentManifestProjection = S.Struct({
  agentReadable: S.Boolean,
  audience: OpenAgentsPaymentPolicyAudience,
  enabled: S.Boolean,
  paidActions: S.Array(OpenAgentsSitePaymentPaidActionProjection),
  payoutModeGate: MdkPayoutModeGateProjection,
  products: S.Array(OpenAgentsSitePaymentProductProjection),
  provider: OpenAgentsSitePaymentProvider,
  sandboxDefault: S.Boolean,
})
export type OpenAgentsSitePaymentManifestProjection =
  typeof OpenAgentsSitePaymentManifestProjection.Type

export class OpenAgentsSitePaymentManifestUnsafe extends S.TaggedErrorClass<OpenAgentsSitePaymentManifestUnsafe>()(
  'OpenAgentsSitePaymentManifestUnsafe',
  {
    reason: S.String,
  },
) {}

const stableSitePaymentIdPattern = /^[a-z][a-z0-9]*(?:[._:-][a-z0-9]+)*$/
const stableSitePaymentRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,180}$/
const unsafeSitePaymentValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|preimage|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const valueHasPrivateMaterial = (value: unknown): boolean =>
  typeof value === 'string'
    ? containsProviderSecretMaterial(value) ||
      unsafeSitePaymentValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
    : Array.isArray(value)
      ? value.some(valueHasPrivateMaterial)
      : value !== null && typeof value === 'object'
        ? openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
          Object.values(value).some(valueHasPrivateMaterial)
        : false

const stableRefIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableSitePaymentRefPattern.test(value) &&
  !valueHasPrivateMaterial(value)

const stableIdIsSafe = (value: string): boolean =>
  stableSitePaymentIdPattern.test(value) && !valueHasPrivateMaterial(value)

const checkoutPathIsSafe = (value: string): boolean =>
  value.startsWith('/') &&
  !value.includes('?') &&
  !value.includes('#') &&
  !value.includes('://') &&
  !value.includes('//') &&
  !valueHasPrivateMaterial(value)

const priceIsSupported = (
  price: typeof OpenAgentsPaidEndpointPrice.Type,
): boolean =>
  (price.asset === 'usd' && price.denomination === 'usd_cent') ||
  (price.asset === 'bitcoin' &&
    price.denomination === 'bitcoin_millisatoshi') ||
  (price.asset === 'credits' && price.denomination === 'credit')

const dataRequirementIsSafe = (
  requirement: OpenAgentsSitePaymentCustomerDataRequirement,
): boolean =>
  stableIdIsSafe(requirement.key) && stableRefIsSafe(requirement.labelRef)

const productIsSafe = (product: OpenAgentsSitePaymentProduct): boolean =>
  stableIdIsSafe(product.id) &&
  stableRefIsSafe(product.displayRef) &&
  checkoutPathIsSafe(product.checkoutPath) &&
  priceIsSupported(product.price) &&
  product.metadataRefs.every(stableRefIsSafe) &&
  product.customerDataRequirements.every(dataRequirementIsSafe)

const paidActionIsSafe = (action: OpenAgentsSitePaymentPaidAction): boolean =>
  stableIdIsSafe(action.id) &&
  stableRefIsSafe(action.actionRef) &&
  stableRefIsSafe(action.displayRef) &&
  checkoutPathIsSafe(action.checkoutPath) &&
  checkoutPathIsSafe(action.path) &&
  priceIsSupported(action.price) &&
  action.metadataRefs.every(stableRefIsSafe) &&
  action.customerDataRequirements.every(dataRequirementIsSafe)

const validateOpenAgentsSitePaymentManifest = (
  manifest: OpenAgentsSitePaymentManifest,
): void => {
  if (valueHasPrivateMaterial(manifest)) {
    throw new OpenAgentsSitePaymentManifestUnsafe({
      reason:
        'Site payment manifests must not contain raw payment, wallet, provider, customer, prompt, runner, or secret material.',
    })
  }

  if (!manifest.payments.metadataRefs.every(stableRefIsSafe)) {
    throw new OpenAgentsSitePaymentManifestUnsafe({
      reason: 'Site payment manifest metadata refs must be public-safe refs.',
    })
  }

  if (
    !manifest.payments.products.every(productIsSafe) ||
    !manifest.payments.paidActions.every(paidActionIsSafe)
  ) {
    throw new OpenAgentsSitePaymentManifestUnsafe({
      reason:
        'Site payment products and paid actions must use stable IDs, supported prices, clean local checkout paths, and public-safe refs.',
    })
  }
}

export const decodeOpenAgentsSitePaymentManifest = (
  value: unknown,
): OpenAgentsSitePaymentManifest => {
  const manifest = S.decodeUnknownSync(OpenAgentsSitePaymentManifest)(value)
  validateOpenAgentsSitePaymentManifest(manifest)

  return manifest
}

export const projectOpenAgentsSitePaymentManifest = (
  manifest: OpenAgentsSitePaymentManifest,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsSitePaymentManifestProjection => ({
  agentReadable: manifest.payments.agentReadable,
  audience,
  enabled: manifest.payments.enabled,
  paidActions: manifest.payments.paidActions
    .filter(
      action =>
        audience !== 'public' || action.publicProjectionState !== 'hidden',
    )
    .map(action => ({
      actionRef: action.actionRef,
      agentReadable: action.agentReadable,
      checkoutPath: action.checkoutPath,
      displayRef: action.displayRef,
      entitlementScope: action.entitlementScope,
      id: action.id,
      method: action.method,
      path: action.path,
      price: action.price,
      publicProjectionState: action.publicProjectionState,
      sandbox: action.sandbox,
      settlementMode: action.settlementMode,
    })),
  payoutModeGate: hostedMdkDirectPayoutDisabledGate(),
  products: manifest.payments.products
    .filter(
      product =>
        audience !== 'public' || product.publicProjectionState !== 'hidden',
    )
    .map(product => ({
      agentReadable: product.agentReadable,
      checkoutPath: product.checkoutPath,
      displayRef: product.displayRef,
      entitlementScope: product.entitlementScope,
      id: product.id,
      price: product.price,
      publicProjectionState: product.publicProjectionState,
      sandbox: product.sandbox,
      settlementMode: product.settlementMode,
    })),
  provider: manifest.payments.provider,
  sandboxDefault: manifest.payments.sandboxDefault,
})

export const openAgentsSitePaymentManifestHasPrivateMaterial = (
  value: unknown,
): boolean => valueHasPrivateMaterial(value)
