import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  OpenAgentsHostedMdkCheckoutProjection,
  OpenAgentsHostedMdkCheckoutStatus,
} from './hosted-mdk-client'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import { OpenAgentsSiteCheckoutReturnProjection } from './site-checkout-return'

export const OpenAgentsSiteMdkProviderEventKind = S.Literals([
  'checkout_expired',
  'checkout_observed',
  'payment_pending',
  'payment_received',
])
export type OpenAgentsSiteMdkProviderEventKind =
  typeof OpenAgentsSiteMdkProviderEventKind.Type

export const OpenAgentsSiteMdkReconciliationImplementationState = S.Literals([
  'fake_provider_only',
  'verification_config_gated',
])
export type OpenAgentsSiteMdkReconciliationImplementationState =
  typeof OpenAgentsSiteMdkReconciliationImplementationState.Type

export const OpenAgentsSiteMdkProviderEvent = S.Struct({
  challengeRef: S.String,
  checkoutRef: S.String,
  checkoutStatus: OpenAgentsHostedMdkCheckoutStatus,
  environment: S.Literals(['production', 'sandbox']),
  eventBodyDigestRef: S.String,
  eventKind: OpenAgentsSiteMdkProviderEventKind,
  eventRef: S.String,
  fakeProvider: S.Boolean,
  metadataRefs: S.Array(S.String),
  occurredAt: S.String,
  productId: S.String,
  providerEventRef: S.String,
  providerRef: S.String,
  sandbox: S.Boolean,
  signatureBindingRef: S.NullOr(S.String),
  signatureVerified: S.Boolean,
  siteId: S.String,
  siteVersionId: S.String,
})
export type OpenAgentsSiteMdkProviderEvent =
  typeof OpenAgentsSiteMdkProviderEvent.Type

export const OpenAgentsSiteMdkReconciliationInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  entitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  hostedCheckout: OpenAgentsHostedMdkCheckoutProjection,
  previousEventRef: S.NullOr(S.String),
  providerEvent: OpenAgentsSiteMdkProviderEvent,
  receipt: S.NullOr(BuyerPaymentReceiptRecord),
  returnProjection: S.NullOr(OpenAgentsSiteCheckoutReturnProjection),
})
export type OpenAgentsSiteMdkReconciliationInput =
  typeof OpenAgentsSiteMdkReconciliationInput.Type

export const OpenAgentsSiteMdkReconciliationProjection = S.Struct({
  acceptedWorkSettlementAuthority: S.Literal(false),
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentReconciliationEvent: BuyerPaymentLedgerProjection,
  entitlement: S.NullOr(BuyerPaymentLedgerProjection),
  fakeProviderOnly: S.Boolean,
  hostedCheckout: OpenAgentsHostedMdkCheckoutProjection,
  implementationState: OpenAgentsSiteMdkReconciliationImplementationState,
  operatorRefs: S.Array(S.String),
  providerEvent: OpenAgentsSiteMdkProviderEvent,
  payoutAuthority: S.Literal(false),
  reasonRefs: S.Array(S.String),
  receipt: S.NullOr(BuyerPaymentLedgerProjection),
  returnProjection: S.NullOr(OpenAgentsSiteCheckoutReturnProjection),
})
export type OpenAgentsSiteMdkReconciliationProjection =
  typeof OpenAgentsSiteMdkReconciliationProjection.Type

export class OpenAgentsSiteMdkReconciliationUnsafe extends S.TaggedErrorClass<OpenAgentsSiteMdkReconciliationUnsafe>()(
  'OpenAgentsSiteMdkReconciliationUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer[_-]?(credential|secret|token)|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|value)|email[_-]?body|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.entries(value).some(([key, item]) =>
        (item !== null && unsafeKeyPattern.test(key)) ||
        valueHasPrivateMaterial(item),
      )
  }

  return false
}

const stableRefIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !valueHasPrivateMaterial(value)

const nullableStableRefIsSafe = (value: string | null): boolean =>
  value === null || stableRefIsSafe(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(stableRefIsSafe)

const implementationState = (
  event: OpenAgentsSiteMdkProviderEvent,
): OpenAgentsSiteMdkReconciliationImplementationState =>
  event.fakeProvider ? 'fake_provider_only' : 'verification_config_gated'

const eventRefsMatch = (
  input: OpenAgentsSiteMdkReconciliationInput,
): boolean =>
  input.providerEvent.challengeRef === input.hostedCheckout.challengeRef &&
  input.providerEvent.checkoutRef === input.hostedCheckout.checkoutRef &&
  input.providerEvent.checkoutStatus === input.hostedCheckout.status &&
  input.providerEvent.productId === input.hostedCheckout.productId &&
  input.providerEvent.providerRef === input.hostedCheckout.providerRef &&
  input.providerEvent.siteId === input.hostedCheckout.siteRef &&
  (
    input.receipt === null ||
    (
      input.receipt.challengeRef === input.providerEvent.challengeRef &&
      input.receipt.productId === input.providerEvent.productId
    )
  ) &&
  (
    input.entitlement === null ||
    (
      input.entitlement.challengeRef === input.providerEvent.challengeRef &&
      input.entitlement.productId === input.providerEvent.productId
    )
  )

const reconciliationStatus = (
  input: OpenAgentsSiteMdkReconciliationInput,
): BuyerPaymentReconciliationEventRecord['status'] =>
  input.previousEventRef !== null
    ? 'replayed'
    : !eventRefsMatch(input)
      ? 'rejected'
      : !input.providerEvent.fakeProvider && !input.providerEvent.signatureVerified
        ? 'rejected'
        : input.providerEvent.checkoutStatus === 'payment_received'
          ? 'matched'
          : 'observed'

const publicProjectionJson = (
  input: OpenAgentsSiteMdkReconciliationInput,
): string =>
  JSON.stringify({
    checkoutRef: input.providerEvent.checkoutRef,
    checkoutStatus: input.providerEvent.checkoutStatus,
    fakeProvider: input.providerEvent.fakeProvider,
    providerEventRef: input.providerEvent.providerEventRef,
    siteId: input.providerEvent.siteId,
    siteVersionId: input.providerEvent.siteVersionId,
  })

const eventRecord = (
  input: OpenAgentsSiteMdkReconciliationInput,
): BuyerPaymentReconciliationEventRecord => {
  const status = reconciliationStatus(input)

  return {
    archivedAt: null,
    challengeRef: input.providerEvent.challengeRef,
    createdAt: input.providerEvent.occurredAt,
    eventRef: input.providerEvent.eventRef,
    externalEventRef: input.providerEvent.providerEventRef,
    id: input.providerEvent.eventRef,
    idempotencyKeyHash: input.providerEvent.eventBodyDigestRef,
    metadataRefs: safeRefs([
      ...input.providerEvent.metadataRefs,
      `metadata.site_mdk_reconciliation.${status}`,
    ]),
    productId: input.providerEvent.productId,
    providerRef: input.providerEvent.providerRef,
    publicProjectionJson: publicProjectionJson(input),
    receiptRef: input.receipt?.receiptRef ?? null,
    resultRef: input.previousEventRef ?? `result.site_mdk_reconciliation.${status}`,
    status,
  }
}

const operatorRefs = (
  input: OpenAgentsSiteMdkReconciliationInput,
): ReadonlyArray<string> =>
  input.audience === 'operator'
    ? safeRefs([
      input.providerEvent.providerRef,
      input.providerEvent.providerEventRef,
      input.providerEvent.eventBodyDigestRef,
      input.providerEvent.signatureBindingRef ?? '',
      input.previousEventRef ?? '',
    ])
    : []

const projectionIsSafe = (
  projection: OpenAgentsSiteMdkReconciliationProjection,
): boolean =>
  projection.reasonRefs.every(stableRefIsSafe) &&
  projection.operatorRefs.every(stableRefIsSafe) &&
  stableRefIsSafe(projection.providerEvent.eventRef) &&
  stableRefIsSafe(projection.providerEvent.providerEventRef) &&
  stableRefIsSafe(projection.providerEvent.eventBodyDigestRef) &&
  stableRefIsSafe(projection.providerEvent.providerRef) &&
  nullableStableRefIsSafe(projection.providerEvent.signatureBindingRef) &&
  !valueHasPrivateMaterial(projection)

export const openAgentsSiteMdkReconciliationHasPrivateMaterial =
  valueHasPrivateMaterial

export const projectOpenAgentsSiteMdkReconciliation = (
  input: OpenAgentsSiteMdkReconciliationInput,
): OpenAgentsSiteMdkReconciliationProjection => {
  if (valueHasPrivateMaterial(input)) {
    throw new OpenAgentsSiteMdkReconciliationUnsafe({
      reason:
        'Site MDK reconciliation input must not contain provider secrets, raw provider payloads, MDK credentials, raw invoices, preimages, wallet state, provider grants, customer private data, provider payout claims, or secrets.',
    })
  }

  const record = eventRecord(input)
  const projection: OpenAgentsSiteMdkReconciliationProjection = {
    acceptedWorkSettlementAuthority: false,
    audience: input.audience,
    buyerPaymentReconciliationEvent: projectBuyerPaymentLedgerRecord(
      'reconciliation_event',
      record,
      input.audience,
    ),
    entitlement: input.entitlement === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'entitlement',
        input.entitlement,
        input.audience,
      ),
    fakeProviderOnly: input.providerEvent.fakeProvider,
    hostedCheckout: input.hostedCheckout,
    implementationState: implementationState(input.providerEvent),
    operatorRefs: operatorRefs(input),
    payoutAuthority: false,
    providerEvent: input.providerEvent,
    reasonRefs: safeRefs([
      `reason.site_mdk_reconciliation.${record.status}`,
      input.providerEvent.fakeProvider
        ? 'reason.site_mdk_reconciliation.fake_provider_only'
        : 'reason.site_mdk_reconciliation.verification_config_gated',
    ]),
    receipt: input.receipt === null
      ? null
      : projectBuyerPaymentLedgerRecord('receipt', input.receipt, input.audience),
    returnProjection: input.returnProjection,
  }

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsSiteMdkReconciliationUnsafe({
      reason: 'Site MDK reconciliation projection is not public-safe.',
    })
  }

  return projection
}
