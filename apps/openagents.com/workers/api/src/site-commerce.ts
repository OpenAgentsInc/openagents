import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { decodeUnknownWithSchema } from './json-boundary'

export const SiteCommerceAsset = S.Literals(['usd', 'sats', 'credits'])
export type SiteCommerceAsset = typeof SiteCommerceAsset.Type

export const SiteCommerceEntitlementScope = S.Literals([
  'site',
  'product',
  'path',
  'action',
  'account',
])
export type SiteCommerceEntitlementScope =
  typeof SiteCommerceEntitlementScope.Type

export const SiteCommerceSettlementMode = S.Literals([
  'checkout_only',
  'deferred',
  'accepted_work_linked',
])
export type SiteCommerceSettlementMode = typeof SiteCommerceSettlementMode.Type

export const SiteCommercePublicProjectionState = S.Literals([
  'hidden',
  'listed',
  'redacted',
  'proof_only',
])
export type SiteCommercePublicProjectionState =
  typeof SiteCommercePublicProjectionState.Type

export const SiteCommerceCustomerDataRequirement = S.Struct({
  key: S.String,
  label: S.String,
  required: S.Boolean,
  kind: S.Literals(['email', 'name', 'text', 'url']),
})
export type SiteCommerceCustomerDataRequirement =
  typeof SiteCommerceCustomerDataRequirement.Type

export const SiteCommercePrice = S.Struct({
  asset: SiteCommerceAsset,
  amount: S.Number,
})
export type SiteCommercePrice = typeof SiteCommercePrice.Type

export const SiteCommerceProduct = S.Struct({
  id: S.String,
  name: S.String,
  price: SiteCommercePrice,
  checkoutPath: S.String,
  entitlementScope: SiteCommerceEntitlementScope,
  agentReadable: S.Boolean,
  settlementMode: SiteCommerceSettlementMode,
  customerDataRequirements: S.Array(SiteCommerceCustomerDataRequirement),
  publicProjectionState: SiteCommercePublicProjectionState,
})
export type SiteCommerceProduct = typeof SiteCommerceProduct.Type

export const SiteCommercePaidAction = S.Struct({
  id: S.String,
  name: S.String,
  price: SiteCommercePrice,
  method: S.Literals(['GET', 'POST']),
  path: S.String,
  checkoutPath: S.String,
  entitlementScope: SiteCommerceEntitlementScope,
  agentReadable: S.Boolean,
  settlementMode: SiteCommerceSettlementMode,
  customerDataRequirements: S.Array(SiteCommerceCustomerDataRequirement),
  publicProjectionState: SiteCommercePublicProjectionState,
})
export type SiteCommercePaidAction = typeof SiteCommercePaidAction.Type

export const SiteCommercePaymentsBlock = S.Struct({
  enabled: S.Boolean,
  provider: S.Literal('openagents_hosted'),
  products: S.Array(SiteCommerceProduct),
  paidActions: S.Array(SiteCommercePaidAction),
})
export type SiteCommercePaymentsBlock = typeof SiteCommercePaymentsBlock.Type

export const SiteSourceCommerceManifest = S.Struct({
  payments: SiteCommercePaymentsBlock,
})
export type SiteSourceCommerceManifest = typeof SiteSourceCommerceManifest.Type

export class SiteCommerceManifestUnsafe extends S.TaggedErrorClass<SiteCommerceManifestUnsafe>()(
  'SiteCommerceManifestUnsafe',
  {
    reason: S.String,
  },
) {}

const prohibitedKeyPattern =
  /(secret|token|mnemonic|preimage|invoice|wallet|credential|private[_-]?key|webhook|grant|payout)/i

const prohibitedValuePattern =
  /\b(lnbc|lntb|lnbcrt|lno1|preimage|mnemonic|xprv|mdk_access_token|checkout_id=|payment_hash=|payment_preimage=)/i

const scanForProhibitedPaymentMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    if (
      containsProviderSecretMaterial(value) ||
      prohibitedValuePattern.test(value)
    ) {
      return path.join('.') || '<root>'
    }

    return undefined
  }

  if (Array.isArray(value)) {
    for (const [index, item] of value.entries()) {
      const unsafePath = scanForProhibitedPaymentMaterial(item, [
        ...path,
        String(index),
      ])

      if (unsafePath !== undefined) {
        return unsafePath
      }
    }

    return undefined
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  for (const [key, item] of Object.entries(value)) {
    if (prohibitedKeyPattern.test(key)) {
      return [...path, key].join('.')
    }

    const unsafePath = scanForProhibitedPaymentMaterial(item, [...path, key])

    if (unsafePath !== undefined) {
      return unsafePath
    }
  }

  return undefined
}

const validatePath = (
  value: string,
  fieldPath: string,
): SiteCommerceManifestUnsafe | undefined => {
  if (!value.startsWith('/')) {
    return new SiteCommerceManifestUnsafe({
      reason: `${fieldPath} must be a site-local absolute path.`,
    })
  }

  if (value.includes('?') || value.includes('#')) {
    return new SiteCommerceManifestUnsafe({
      reason: `${fieldPath} must not include query strings or fragments.`,
    })
  }

  return undefined
}

const validatePrice = (
  price: SiteCommercePrice,
  fieldPath: string,
): SiteCommerceManifestUnsafe | undefined =>
  Number.isFinite(price.amount) && price.amount > 0
    ? undefined
    : new SiteCommerceManifestUnsafe({
        reason: `${fieldPath} amount must be greater than zero.`,
      })

export const decodeSiteSourceCommerceManifest = (
  value: unknown,
): SiteSourceCommerceManifest => {
  const unsafePath = scanForProhibitedPaymentMaterial(value)

  if (unsafePath !== undefined) {
    throw new SiteCommerceManifestUnsafe({
      reason: `Site commerce manifest contains prohibited payment or secret material at ${unsafePath}.`,
    })
  }

  const manifest = decodeUnknownWithSchema(SiteSourceCommerceManifest, value)

  for (const [index, product] of manifest.payments.products.entries()) {
    const priceError = validatePrice(
      product.price,
      `payments.products.${index}.price`,
    )
    const pathError = validatePath(
      product.checkoutPath,
      `payments.products.${index}.checkoutPath`,
    )

    if (priceError !== undefined) {
      throw priceError
    }

    if (pathError !== undefined) {
      throw pathError
    }
  }

  for (const [index, action] of manifest.payments.paidActions.entries()) {
    const priceError = validatePrice(
      action.price,
      `payments.paidActions.${index}.price`,
    )
    const actionPathError = validatePath(
      action.path,
      `payments.paidActions.${index}.path`,
    )
    const checkoutPathError = validatePath(
      action.checkoutPath,
      `payments.paidActions.${index}.checkoutPath`,
    )

    if (priceError !== undefined) {
      throw priceError
    }

    if (actionPathError !== undefined) {
      throw actionPathError
    }

    if (checkoutPathError !== undefined) {
      throw checkoutPathError
    }
  }

  return manifest
}
