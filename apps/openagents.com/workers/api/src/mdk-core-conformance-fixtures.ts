import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentLedgerAmount,
} from './buyer-payment-ledger'
import {
  OpenAgentsMdkCoreCreateCheckoutInput,
} from './mdk-core-checkout-contract'
import {
  OpenAgentsPaidEndpointProductRecord,
} from './paid-endpoint-product-catalog'

export const OpenAgentsMdkCoreConformanceFixtureKind = S.Literals([
  'amount_checkout_creation',
  'customer_field_normalization',
  'error_envelope',
  'l402_token_parsing',
  'metadata_limits',
  'preimage_proof_boundary',
  'price_recheck',
  'product_checkout_creation',
  'safe_return_path',
  'sandbox_flag',
  'signed_checkout_url',
  'stale_challenge',
])
export type OpenAgentsMdkCoreConformanceFixtureKind =
  typeof OpenAgentsMdkCoreConformanceFixtureKind.Type

export const OpenAgentsMdkCoreConformanceFixtureStatus = S.Literals([
  'adapted',
  'deferred',
  'implemented',
])
export type OpenAgentsMdkCoreConformanceFixtureStatus =
  typeof OpenAgentsMdkCoreConformanceFixtureStatus.Type

export const OpenAgentsMdkCoreConformanceFixture = S.Struct({
  assertionRefs: S.Array(S.String),
  expectedOutputRef: S.String,
  id: S.String,
  kind: OpenAgentsMdkCoreConformanceFixtureKind,
  omegaStatus: OpenAgentsMdkCoreConformanceFixtureStatus,
  redactionPolicyRef: S.String,
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMdkCoreConformanceFixture =
  typeof OpenAgentsMdkCoreConformanceFixture.Type

const unsafeFixturePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|preimage|prompt|runner|run[_-]?log)|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const fixture = (
  input: Omit<
    OpenAgentsMdkCoreConformanceFixture,
    'id' | 'redactionPolicyRef'
  > & Readonly<{ id: string }>,
): OpenAgentsMdkCoreConformanceFixture => ({
  ...input,
  id: `fixture.mdk_core.${input.id}`,
  redactionPolicyRef: 'redaction.mdk_core.public_safe_fixture.v1',
})

export const OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES = [
  fixture({
    assertionRefs: [
      'assertion.mdk_core.amount_checkout.usd_cent',
      'assertion.mdk_core.amount_checkout.bitcoin_millisatoshi',
    ],
    expectedOutputRef: 'expected.mdk_core.amount_checkout.prepared',
    id: 'amount_checkout_creation',
    kind: 'amount_checkout_creation',
    omegaStatus: 'implemented',
    sourceRefs: [
      'mdk.core.actions.create_checkout.amount',
      'mdk.api_contract.checkout.create_input',
    ],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.product_checkout.product_id'],
    expectedOutputRef: 'expected.mdk_core.product_checkout.prepared',
    id: 'product_checkout_creation',
    kind: 'product_checkout_creation',
    omegaStatus: 'implemented',
    sourceRefs: [
      'mdk.core.actions.create_checkout.products',
      'mdk.api_contract.checkout.product_input',
    ],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.customer.camel_case'],
    expectedOutputRef: 'expected.mdk_core.customer.normalized_keys_only',
    id: 'customer_field_normalization',
    kind: 'customer_field_normalization',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.core.actions.clean_customer_input'],
  }),
  fixture({
    assertionRefs: [
      'assertion.mdk_core.metadata.max_50_keys',
      'assertion.mdk_core.metadata.max_1024_bytes',
      'assertion.mdk_core.metadata.no_secret_material',
    ],
    expectedOutputRef: 'expected.mdk_core.metadata.validation_error',
    id: 'metadata_limits',
    kind: 'metadata_limits',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.api_contract.metadata_validation'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.signed_url.web_crypto_hmac'],
    expectedOutputRef: 'expected.mdk_core.signed_url.valid_signature',
    id: 'signed_checkout_url',
    kind: 'signed_checkout_url',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.core.handlers.checkout.create_checkout_url'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.checkout_path.site_local'],
    expectedOutputRef: 'expected.mdk_core.checkout_path.sanitized',
    id: 'safe_return_path',
    kind: 'safe_return_path',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.core.handlers.checkout.sanitize_checkout_path'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.sandbox.flag_preserved'],
    expectedOutputRef: 'expected.mdk_core.sandbox.prepared_true',
    id: 'sandbox_flag',
    kind: 'sandbox_flag',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.api_contract.checkout.sandbox'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.l402.credential_parse'],
    expectedOutputRef: 'expected.mdk_core.l402.valid_credential',
    id: 'l402_token_parsing',
    kind: 'l402_token_parsing',
    omegaStatus: 'adapted',
    sourceRefs: ['mdk.core.mdk402.token'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.l402.price_recheck'],
    expectedOutputRef: 'expected.mdk_core.l402.amount_mismatch',
    id: 'price_recheck',
    kind: 'price_recheck',
    omegaStatus: 'adapted',
    sourceRefs: ['mdk.core.mdk402.with_payment.price_check'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.l402.proof_ref_required'],
    expectedOutputRef: 'expected.mdk_core.l402.proof_missing',
    id: 'preimage_proof_boundary',
    kind: 'preimage_proof_boundary',
    omegaStatus: 'adapted',
    sourceRefs: ['mdk.core.pay402.preimage_verification'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.hosted_mdk.stale_challenge'],
    expectedOutputRef: 'expected.mdk_core.hosted_mdk.stale_challenge_error',
    id: 'stale_challenge',
    kind: 'stale_challenge',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.core.checkout.expiry'],
  }),
  fixture({
    assertionRefs: ['assertion.mdk_core.errors.typed_reason_refs'],
    expectedOutputRef: 'expected.mdk_core.errors.safe_envelope',
    id: 'error_envelope',
    kind: 'error_envelope',
    omegaStatus: 'implemented',
    sourceRefs: ['mdk.core.handlers.checkout.error_mapping'],
  }),
] as const

export const OPENAGENTS_MDK_CORE_FIXTURE_AMOUNT: BuyerPaymentLedgerAmount = {
  amountMinorUnits: 2500,
  asset: 'usd',
  denomination: 'usd_cent',
}

export const OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT: BuyerPaymentLedgerAmount = {
  amountMinorUnits: 25_000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
}

export const OPENAGENTS_MDK_CORE_FIXTURE_AMOUNT_CHECKOUT: OpenAgentsMdkCoreCreateCheckoutInput = {
  amount: OPENAGENTS_MDK_CORE_FIXTURE_AMOUNT,
  cancelRef: 'checkout.cancel.fixture',
  checkoutPath: '/checkout/fixture?step=review',
  customer: {
    'External ID': 'fixture-buyer-1',
    name: 'Fixture Buyer',
  },
  metadata: {
    source_ref: 'fixture_checkout',
  },
  mode: 'amount',
  requireCustomerData: ['External ID', 'name'],
  returnRef: 'checkout.success.fixture',
  sandbox: true,
  titleRef: 'title.fixture.checkout',
}

export const OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT_CHECKOUT: OpenAgentsMdkCoreCreateCheckoutInput = {
  cancelRef: 'checkout.cancel.product_fixture',
  checkoutPath: '/checkout/product-fixture',
  metadata: {
    product_ref: 'fixture_report',
  },
  mode: 'product',
  productId: 'product.site_checkout.fixture_report',
  productPriceRef: 'price.site_checkout.fixture_report.standard',
  returnRef: 'checkout.success.product_fixture',
  sandbox: true,
}

export const OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT: OpenAgentsPaidEndpointProductRecord = {
  binding: {
    actionRef: null,
    kind: 'agent_api_endpoint',
    method: 'POST',
    pathTemplate: '/api/agents/proposals',
    resourceRef: 'resource.agent_api.proposals',
  },
  displayName: 'Fixture agent proposal intake',
  entitlement: {
    durationSeconds: 86_400,
    kind: 'duration_quota',
    quotaUnits: 20,
    scopeRefs: ['entitlement.agent_api.proposals.day'],
  },
  internalEconomicsRefs: ['internal_economics.fixture.agent_api'],
  operatorNoteRefs: ['operator_note.fixture.reviewed'],
  price: OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT,
  productId: 'product.agent_api.proposals.day',
  projectionPolicy: 'agent_visible',
  providerBindingRefs: ['provider_binding.openagents.hosted_mdk'],
  publicAgentDocRefs: ['docs.api.agent_proposals'],
  publicSummaryRef: 'summary.fixture.agent_api.proposals.day',
  spendCapHintRefs: ['spend_cap.bitcoin.max_25000_msat'],
  status: 'active',
  surface: 'agent_api',
}

export const OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE: BuyerPaymentChallengeRecord = {
  actorRef: 'agent:user_fixture',
  archivedAt: null,
  challengeRef: 'challenge.fixture.agent_api.proposals.1',
  createdAt: '2026-06-06T09:00:00.000Z',
  expiresAt: '2026-06-06T09:10:00.000Z',
  id: 'buyer_payment_challenge_fixture_1',
  idempotencyKeyHash: 'hash.fixture.checkout.1',
  metadataRefs: ['metadata.fixture.safe'],
  method: 'POST',
  ownerUserId: 'user_owner_fixture',
  path: '/api/agents/proposals',
  price: OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT,
  productId: OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT.productId,
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:fixture_request_body',
  spendCap: OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT,
  status: 'issued',
  surface: 'agent_api',
}

export const openAgentsMdkCoreConformanceFixtureHasPrivateMaterial = (
  fixtureValue: unknown,
): boolean =>
  containsProviderSecretMaterial(JSON.stringify(fixtureValue)) ||
  unsafeFixturePattern.test(JSON.stringify(fixtureValue))
