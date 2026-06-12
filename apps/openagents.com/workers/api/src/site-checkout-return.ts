import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import { OpenAgentsHostedMdkCheckoutProjection } from './hosted-mdk-client'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import { OpenAgentsSiteCheckoutUiPrimitiveProjection } from './site-checkout-ui-primitives'

export const OpenAgentsSiteCheckoutReturnAction = S.Literals([
  'cancel',
  'status',
  'success',
])
export type OpenAgentsSiteCheckoutReturnAction =
  typeof OpenAgentsSiteCheckoutReturnAction.Type

export const OpenAgentsSiteCheckoutReturnState = S.Literals([
  'blocked',
  'cancel',
  'entitled',
  'expired',
  'paid',
  'pending',
  'success',
  'unpaid',
])
export type OpenAgentsSiteCheckoutReturnState =
  typeof OpenAgentsSiteCheckoutReturnState.Type

export const OpenAgentsSiteCheckoutEntitlementStatus = S.Literals([
  'active',
  'blocked',
  'consumed',
  'expired',
  'none',
  'pending_reconciliation',
  'revoked',
])
export type OpenAgentsSiteCheckoutEntitlementStatus =
  typeof OpenAgentsSiteCheckoutEntitlementStatus.Type

export const OpenAgentsSiteCheckoutReturnRoute = S.Struct({
  cancelPath: S.String,
  checkoutIntentRef: S.String,
  checkoutRef: S.NullOr(S.String),
  siteId: S.String,
  siteVersionId: S.String,
  successPath: S.String,
})
export type OpenAgentsSiteCheckoutReturnRoute =
  typeof OpenAgentsSiteCheckoutReturnRoute.Type

export const OpenAgentsSiteCheckoutReturnServerRefs = S.Struct({
  buyerPaymentChallengeRef: S.String,
  checkoutIntentRef: S.String,
  checkoutRef: S.NullOr(S.String),
  entitlementRef: S.NullOr(S.String),
  receiptRef: S.NullOr(S.String),
})
export type OpenAgentsSiteCheckoutReturnServerRefs =
  typeof OpenAgentsSiteCheckoutReturnServerRefs.Type

export const OpenAgentsSiteCheckoutReturnProjection = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: S.NullOr(BuyerPaymentLedgerProjection),
  cleanReturnPath: S.String,
  entitlement: S.NullOr(BuyerPaymentLedgerProjection),
  entitlementStatus: OpenAgentsSiteCheckoutEntitlementStatus,
  finalEntitlementCreated: S.Literal(false),
  hostedCheckout: S.NullOr(OpenAgentsHostedMdkCheckoutProjection),
  receipt: S.NullOr(BuyerPaymentLedgerProjection),
  reasonRefs: S.Array(S.String),
  returnAction: OpenAgentsSiteCheckoutReturnAction,
  returnState: OpenAgentsSiteCheckoutReturnState,
  serverRefs: OpenAgentsSiteCheckoutReturnServerRefs,
  uiPrimitiveRefs: S.Array(S.String),
})
export type OpenAgentsSiteCheckoutReturnProjection =
  typeof OpenAgentsSiteCheckoutReturnProjection.Type

export const OpenAgentsSiteCheckoutReturnInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: BuyerPaymentChallengeRecord,
  entitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  hostedCheckout: S.NullOr(OpenAgentsHostedMdkCheckoutProjection),
  nowEpochMillis: S.Number,
  observedReturnPath: S.String,
  receipt: S.NullOr(BuyerPaymentReceiptRecord),
  returnAction: OpenAgentsSiteCheckoutReturnAction,
  route: OpenAgentsSiteCheckoutReturnRoute,
  uiPrimitives: S.NullOr(OpenAgentsSiteCheckoutUiPrimitiveProjection),
})
export type OpenAgentsSiteCheckoutReturnInput =
  typeof OpenAgentsSiteCheckoutReturnInput.Type

export class OpenAgentsSiteCheckoutReturnUnsafe extends S.TaggedErrorClass<OpenAgentsSiteCheckoutReturnUnsafe>()(
  'OpenAgentsSiteCheckoutReturnUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer[_-]?(credential|secret|token)|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|value)|email[_-]?body|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

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

const cleanPathIsSafe = (value: string): boolean =>
  value.startsWith('/') &&
  !value.includes('?') &&
  !value.includes('#') &&
  !value.includes('://') &&
  !value.includes('//') &&
  !valueHasPrivateMaterial(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(stableRefIsSafe)

const routeIsSafe = (route: OpenAgentsSiteCheckoutReturnRoute): boolean =>
  stableRefIsSafe(route.checkoutIntentRef) &&
  nullableStableRefIsSafe(route.checkoutRef) &&
  stableRefIsSafe(route.siteId) &&
  stableRefIsSafe(route.siteVersionId) &&
  cleanPathIsSafe(route.successPath) &&
  cleanPathIsSafe(route.cancelPath) &&
  !valueHasPrivateMaterial(route)

const cleanReturnPathForAction = (
  input: OpenAgentsSiteCheckoutReturnInput,
): string =>
  input.returnAction === 'cancel'
    ? input.route.cancelPath
    : input.returnAction === 'success'
      ? input.route.successPath
      : input.observedReturnPath

const refsMatch = (
  input: OpenAgentsSiteCheckoutReturnInput,
): boolean => {
  const hostedCheckoutMatches =
    input.hostedCheckout === null ||
    (
      input.hostedCheckout.challengeRef ===
        input.buyerPaymentChallenge.challengeRef &&
      input.hostedCheckout.productId === input.buyerPaymentChallenge.productId &&
      input.hostedCheckout.siteRef === input.route.siteId &&
      (
        input.route.checkoutRef === null ||
        input.hostedCheckout.checkoutRef === input.route.checkoutRef
      )
    )
  const receiptMatches =
    input.receipt === null ||
    (
      input.receipt.challengeRef === input.buyerPaymentChallenge.challengeRef &&
      input.receipt.productId === input.buyerPaymentChallenge.productId
    )
  const entitlementMatches =
    input.entitlement === null ||
    (
      input.entitlement.challengeRef ===
        input.buyerPaymentChallenge.challengeRef &&
      input.entitlement.productId === input.buyerPaymentChallenge.productId
    )

  return hostedCheckoutMatches && receiptMatches && entitlementMatches
}

const challengeExpired = (
  input: OpenAgentsSiteCheckoutReturnInput,
): boolean =>
  input.buyerPaymentChallenge.status === 'expired' ||
  Date.parse(input.buyerPaymentChallenge.expiresAt) <= input.nowEpochMillis

const entitlementStatus = (
  input: OpenAgentsSiteCheckoutReturnInput,
  returnState: OpenAgentsSiteCheckoutReturnState,
): OpenAgentsSiteCheckoutEntitlementStatus =>
  returnState === 'blocked'
    ? 'blocked'
    : input.entitlement !== null
      ? input.entitlement.status
      : returnState === 'paid' || returnState === 'success' ||
          returnState === 'pending'
        ? 'pending_reconciliation'
        : 'none'

const returnState = (
  input: OpenAgentsSiteCheckoutReturnInput,
): OpenAgentsSiteCheckoutReturnState =>
  !routeIsSafe(input.route) ||
  !cleanPathIsSafe(input.observedReturnPath) ||
  !refsMatch(input)
    ? 'blocked'
    : input.returnAction === 'cancel' ||
        input.buyerPaymentChallenge.status === 'cancelled'
      ? 'cancel'
      : challengeExpired(input)
        ? 'expired'
        : input.entitlement?.status === 'active'
          ? 'entitled'
          : input.receipt?.status === 'issued' ||
              input.hostedCheckout?.status === 'payment_received'
            ? 'paid'
            : input.returnAction === 'success'
              ? 'success'
              : input.hostedCheckout?.status === 'pending_payment'
                ? 'pending'
                : 'unpaid'

const uiPrimitiveRefsForState = (
  input: OpenAgentsSiteCheckoutReturnInput,
  state: OpenAgentsSiteCheckoutReturnState,
): ReadonlyArray<string> => {
  const primitives = input.uiPrimitives?.primitives ?? []
  const stateKinds =
    state === 'cancel'
      ? ['cancel_state']
      : state === 'success' || state === 'paid' || state === 'entitled'
        ? ['success_state', 'entitlement_state']
        : ['entitlement_state']

  return safeRefs(
    primitives
      .filter(primitive => stateKinds.includes(primitive.primitiveKind))
      .map(primitive => primitive.id),
  )
}

const projectionIsSafe = (
  projection: OpenAgentsSiteCheckoutReturnProjection,
): boolean =>
  cleanPathIsSafe(projection.cleanReturnPath) &&
  projection.reasonRefs.every(stableRefIsSafe) &&
  projection.uiPrimitiveRefs.every(stableRefIsSafe) &&
  stableRefIsSafe(projection.serverRefs.buyerPaymentChallengeRef) &&
  stableRefIsSafe(projection.serverRefs.checkoutIntentRef) &&
  nullableStableRefIsSafe(projection.serverRefs.checkoutRef) &&
  nullableStableRefIsSafe(projection.serverRefs.entitlementRef) &&
  nullableStableRefIsSafe(projection.serverRefs.receiptRef) &&
  !valueHasPrivateMaterial(projection)

export const openAgentsSiteCheckoutReturnHasPrivateMaterial =
  valueHasPrivateMaterial

export const projectOpenAgentsSiteCheckoutReturn = (
  input: OpenAgentsSiteCheckoutReturnInput,
): OpenAgentsSiteCheckoutReturnProjection => {
  if (valueHasPrivateMaterial(input)) {
    throw new OpenAgentsSiteCheckoutReturnUnsafe({
      reason:
        'Site checkout return input must not contain customer private data, raw checkout query state, raw payment material, wallet state, MDK credentials, provider grants, payout claims, or secrets.',
    })
  }

  const state = returnState(input)
  const cleanReturnPath = cleanReturnPathForAction(input)
  const projection: OpenAgentsSiteCheckoutReturnProjection = {
    audience: input.audience,
    buyerPaymentChallenge: state === 'blocked'
      ? null
      : projectBuyerPaymentLedgerRecord(
        'challenge',
        input.buyerPaymentChallenge,
        input.audience,
      ),
    cleanReturnPath,
    entitlement: input.entitlement === null || state === 'blocked'
      ? null
      : projectBuyerPaymentLedgerRecord(
        'entitlement',
        input.entitlement,
        input.audience,
      ),
    entitlementStatus: entitlementStatus(input, state),
    finalEntitlementCreated: false,
    hostedCheckout: state === 'blocked' ? null : input.hostedCheckout,
    receipt: input.receipt === null || state === 'blocked'
      ? null
      : projectBuyerPaymentLedgerRecord(
        'receipt',
        input.receipt,
        input.audience,
      ),
    reasonRefs: safeRefs([
      `reason.site_checkout_return.${state}`,
      state === 'success'
        ? 'reason.site_checkout_return.awaiting_reconciliation'
        : '',
    ]),
    returnAction: input.returnAction,
    returnState: state,
    serverRefs: {
      buyerPaymentChallengeRef: input.buyerPaymentChallenge.challengeRef,
      checkoutIntentRef: input.route.checkoutIntentRef,
      checkoutRef: input.route.checkoutRef,
      entitlementRef: input.entitlement?.entitlementRef ?? null,
      receiptRef: input.receipt?.receiptRef ?? null,
    },
    uiPrimitiveRefs: uiPrimitiveRefsForState(input, state),
  }

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsSiteCheckoutReturnUnsafe({
      reason: 'Site checkout return projection is not public-safe.',
    })
  }

  return projection
}
