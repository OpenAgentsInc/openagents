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

export const OpenAgentsSitePaymentProvider = S.Literals([
  'customer_owned_processor',
  'openagents_hosted_mdk',
])
export type OpenAgentsSitePaymentProvider =
  typeof OpenAgentsSitePaymentProvider.Type

export const OpenAgentsSitePaymentCustomerOwnedProcessorKind = S.Literals([
  'stripe_connect',
])
export type OpenAgentsSitePaymentCustomerOwnedProcessorKind =
  typeof OpenAgentsSitePaymentCustomerOwnedProcessorKind.Type

export const OpenAgentsSitePaymentCustomerOwnedProcessorBinding = S.Struct({
  chargeDestination: S.Literal('customer_account'),
  customerProcessorAccountRef: S.String,
  openAgentsMeteringRefs: S.Array(S.String),
  processor: OpenAgentsSitePaymentCustomerOwnedProcessorKind,
  processorConnectionRef: S.String,
  revenueOwner: S.Literal('customer'),
})
export type OpenAgentsSitePaymentCustomerOwnedProcessorBinding =
  typeof OpenAgentsSitePaymentCustomerOwnedProcessorBinding.Type

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

export const OpenAgentsSitePaymentRecurringBillingKind = S.Literals([
  'retainer',
  'subscription',
])
export type OpenAgentsSitePaymentRecurringBillingKind =
  typeof OpenAgentsSitePaymentRecurringBillingKind.Type

export const OpenAgentsSitePaymentRecurringBillingInterval = S.Literals([
  'month',
  'year',
])
export type OpenAgentsSitePaymentRecurringBillingInterval =
  typeof OpenAgentsSitePaymentRecurringBillingInterval.Type

export const OpenAgentsSitePaymentRecurringBilling = S.Struct({
  billingKind: OpenAgentsSitePaymentRecurringBillingKind,
  entitlementRenewalMode: S.Literal('renew_on_payment_receipt'),
  interval: OpenAgentsSitePaymentRecurringBillingInterval,
  renewalReceiptScopeRefs: S.Array(S.String),
})
export type OpenAgentsSitePaymentRecurringBilling =
  typeof OpenAgentsSitePaymentRecurringBilling.Type

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
  recurringBilling: S.optionalKey(
    S.NullOr(OpenAgentsSitePaymentRecurringBilling),
  ),
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
  customerOwnedProcessor: S.optionalKey(
    S.NullOr(OpenAgentsSitePaymentCustomerOwnedProcessorBinding),
  ),
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
  recurringBilling: S.NullOr(OpenAgentsSitePaymentRecurringBilling),
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
  customerOwnedProcessor: S.NullOr(
    S.Struct({
      chargeDestination: S.Literal('customer_account'),
      meteringSeparated: S.Boolean,
      openAgentsMeteringRefs: S.Array(S.String),
      processor: OpenAgentsSitePaymentCustomerOwnedProcessorKind,
      revenueOwner: S.Literal('customer'),
    }),
  ),
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
  /(acct_[a-z0-9]+|bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|preimage|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|stripe[_-]?(account|secret|webhook)|\S+@\S+|wallet[_-]?state)/i

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

const recurringBillingIsSafe = (
  recurringBilling: OpenAgentsSitePaymentRecurringBilling | null | undefined,
): boolean =>
  recurringBilling === null ||
  recurringBilling === undefined ||
  (recurringBilling.entitlementRenewalMode === 'renew_on_payment_receipt' &&
    recurringBilling.renewalReceiptScopeRefs.length > 0 &&
    recurringBilling.renewalReceiptScopeRefs.every(stableRefIsSafe))

const customerOwnedProcessorBindingIsSafe = (
  binding: OpenAgentsSitePaymentCustomerOwnedProcessorBinding,
): boolean =>
  stableRefIsSafe(binding.customerProcessorAccountRef) &&
  stableRefIsSafe(binding.processorConnectionRef) &&
  binding.openAgentsMeteringRefs.length > 0 &&
  binding.openAgentsMeteringRefs.every(stableRefIsSafe)

const productIsSafe = (product: OpenAgentsSitePaymentProduct): boolean =>
  stableIdIsSafe(product.id) &&
  stableRefIsSafe(product.displayRef) &&
  checkoutPathIsSafe(product.checkoutPath) &&
  priceIsSupported(product.price) &&
  product.metadataRefs.every(stableRefIsSafe) &&
  product.customerDataRequirements.every(dataRequirementIsSafe) &&
  recurringBillingIsSafe(product.recurringBilling)

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

  const customerOwnedProcessor =
    manifest.payments.customerOwnedProcessor ?? null

  if (
    manifest.payments.provider === 'customer_owned_processor' &&
    (customerOwnedProcessor === null ||
      !customerOwnedProcessorBindingIsSafe(customerOwnedProcessor))
  ) {
    throw new OpenAgentsSitePaymentManifestUnsafe({
      reason:
        'Customer-owned processor manifests must carry only opaque customer processor refs and separate OpenAgents metering refs.',
    })
  }

  if (
    manifest.payments.provider !== 'customer_owned_processor' &&
    customerOwnedProcessor !== null
  ) {
    throw new OpenAgentsSitePaymentManifestUnsafe({
      reason:
        'Customer-owned processor binding is only valid for customer_owned_processor manifests.',
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
): OpenAgentsSitePaymentManifestProjection => {
  const customerOwnedProcessor =
    manifest.payments.customerOwnedProcessor ?? null

  return {
    agentReadable: manifest.payments.agentReadable,
    audience,
    customerOwnedProcessor:
      customerOwnedProcessor === null
        ? null
        : {
            chargeDestination: customerOwnedProcessor.chargeDestination,
            meteringSeparated: true,
            openAgentsMeteringRefs:
              customerOwnedProcessor.openAgentsMeteringRefs,
            processor: customerOwnedProcessor.processor,
            revenueOwner: customerOwnedProcessor.revenueOwner,
          },
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
        recurringBilling: product.recurringBilling ?? null,
        sandbox: product.sandbox,
        settlementMode: product.settlementMode,
      })),
    provider: manifest.payments.provider,
    sandboxDefault: manifest.payments.sandboxDefault,
  }
}

export const openAgentsSitePaymentManifestHasPrivateMaterial = (
  value: unknown,
): boolean => valueHasPrivateMaterial(value)
