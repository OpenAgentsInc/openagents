import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  OpenAgentsGeneratedSitePaymentHelperRequestPlan,
  generatedSiteCheckoutIntentPlan,
  generatedSiteCheckoutReturnPlan,
  generatedSiteL402ChallengePlan,
  generatedSiteL402RedemptionPlan,
  generatedSitePaymentDiscoveryPlan,
  generatedSitePaymentProofPlan,
} from './site-mdk-generated-helpers'
import {
  OPENAGENTS_SITE_MDK_SMOKE_RECORD_ONLY_AUTHORITY,
  OpenAgentsSiteMdkSmokeImplementationState,
  OpenAgentsSiteMdkSmokeProjection,
  exampleOpenAgentsSiteMdkSmokeRecord,
  projectOpenAgentsSiteMdkSmoke,
} from './site-mdk-smoke'
import {
  OpenAgentsSitePaymentCatalogProjection,
  type OpenAgentsSitePaymentPaidActionCatalogRecord,
  type OpenAgentsSitePaymentProductCatalogRecord,
  projectOpenAgentsSitePaymentCatalog,
  sitePaymentCatalogFromManifest,
} from './site-payment-catalog'
import {
  OpenAgentsSitePaymentDiscoveryProjection,
  projectOpenAgentsSitePaymentDiscovery,
} from './site-payment-discovery'
import {
  OpenAgentsSitePaymentManifest,
  OpenAgentsSitePaymentManifestProjection,
  decodeOpenAgentsSitePaymentManifest,
  projectOpenAgentsSitePaymentManifest,
} from './site-payment-manifest'

export const OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_FIXTURE_REF =
  'fixture.generated_site_payment_smoke.v1'
export const OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID =
  'site_payment_smoke'
export const OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_VERSION_ID =
  'version_site_payment_smoke_v1'

export const OpenAgentsGeneratedSitePaymentLaunchGateState = S.Literals([
  'blocked',
  'checkout_evidence_only',
  'live_bitcoin_checkout_verified',
  'payout_settlement_verified',
])
export type OpenAgentsGeneratedSitePaymentLaunchGateState =
  typeof OpenAgentsGeneratedSitePaymentLaunchGateState.Type

export class OpenAgentsGeneratedSitePaymentLaunchGate extends S.Class<OpenAgentsGeneratedSitePaymentLaunchGate>(
  'OpenAgentsGeneratedSitePaymentLaunchGate',
)({
  activeEntitlementRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  checkoutEvidenceOnly: S.Boolean,
  checkoutIntentRefs: S.Array(S.String),
  liveBitcoinCheckoutClaimAllowed: S.Boolean,
  paymentProofRefs: S.Array(S.String),
  payoutSettlementClaimAllowed: S.Boolean,
  publicCopyRefs: S.Array(S.String),
  receiptBundleRefs: S.Array(S.String),
  reconciliationEventRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  state: OpenAgentsGeneratedSitePaymentLaunchGateState,
  stateLabel: S.String,
}) {}

export type OpenAgentsGeneratedSitePaymentLaunchGateInput = Readonly<{
  activeEntitlementRefs: ReadonlyArray<string>
  checkoutIntentRefs: ReadonlyArray<string>
  implementationState: OpenAgentsSiteMdkSmokeImplementationState
  paymentProofRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  reconciliationEventRefs: ReadonlyArray<string>
  settlementReceiptRefs: ReadonlyArray<string>
}>

export class OpenAgentsGeneratedSitePaymentSmokeFixture extends S.Class<OpenAgentsGeneratedSitePaymentSmokeFixture>(
  'OpenAgentsGeneratedSitePaymentSmokeFixture',
)({
  catalogProjection: OpenAgentsSitePaymentCatalogProjection,
  discoveryProjection: OpenAgentsSitePaymentDiscoveryProjection,
  fixtureRef: S.String,
  helperPlans: S.Array(OpenAgentsGeneratedSitePaymentHelperRequestPlan),
  manifestProjection: OpenAgentsSitePaymentManifestProjection,
  noDeploymentAuthority: S.Literal(true),
  noLiveCheckoutCreated: S.Literal(true),
  noRealInvoiceCreated: S.Literal(true),
  noWalletSpendAuthority: S.Literal(true),
  paymentLaunchGate: OpenAgentsGeneratedSitePaymentLaunchGate,
  siteId: S.String,
  siteVersionId: S.String,
  smokeProjection: OpenAgentsSiteMdkSmokeProjection,
  sourceRefs: S.Array(S.String),
}) {}

export class OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe extends S.TaggedErrorClass<OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe>()(
  'OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe',
  {
    reason: S.String,
  },
) {}

const stableFixtureRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/u
const unsafeFixtureValuePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|bearer\s+|bolt11|bolt12|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|wallet)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(checkout|customer|email|invoice|log|payment|payload|provider|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token[_-]?secret|treasury[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend|state))/i

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

const valueHasPrivateMaterial = (value: unknown): boolean =>
  stringValues(value).some(
    item =>
      containsProviderSecretMaterial(item) ||
      unsafeFixtureValuePattern.test(item),
  )

const assertStableFixtureRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = refs.find(
    ref => !stableFixtureRefPattern.test(ref) || valueHasPrivateMaterial(ref),
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe({
      reason: `${label} must use stable public-safe refs.`,
    })
  }
}

const uniqueRefs = (refs: ReadonlyArray<string>): string[] => [
  ...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== '')),
]

const publicSafeRefs = (refs: ReadonlyArray<string>): string[] =>
  uniqueRefs(refs).filter(
    ref => stableFixtureRefPattern.test(ref) && !valueHasPrivateMaterial(ref),
  )

export const projectOpenAgentsGeneratedSitePaymentLaunchGate = (
  input: OpenAgentsGeneratedSitePaymentLaunchGateInput,
): OpenAgentsGeneratedSitePaymentLaunchGate => {
  const checkoutIntentRefs = publicSafeRefs(input.checkoutIntentRefs)
  const paymentProofRefs = publicSafeRefs(input.paymentProofRefs)
  const receiptRefs = publicSafeRefs(input.receiptRefs)
  const activeEntitlementRefs = publicSafeRefs(input.activeEntitlementRefs)
  const reconciliationEventRefs = publicSafeRefs(input.reconciliationEventRefs)
  const settlementReceiptRefs = publicSafeRefs(input.settlementReceiptRefs)
  const hasVerifiedCheckoutEvidence =
    checkoutIntentRefs.length > 0 &&
    paymentProofRefs.length > 0 &&
    receiptRefs.length > 0 &&
    activeEntitlementRefs.length > 0 &&
    reconciliationEventRefs.length > 0
  const liveBitcoinCheckoutClaimAllowed =
    input.implementationState === 'live_provider' && hasVerifiedCheckoutEvidence
  const payoutSettlementClaimAllowed =
    liveBitcoinCheckoutClaimAllowed && settlementReceiptRefs.length > 0
  const state: OpenAgentsGeneratedSitePaymentLaunchGateState =
    payoutSettlementClaimAllowed
      ? 'payout_settlement_verified'
      : liveBitcoinCheckoutClaimAllowed
        ? 'live_bitcoin_checkout_verified'
        : hasVerifiedCheckoutEvidence
          ? 'checkout_evidence_only'
          : 'blocked'

  return new OpenAgentsGeneratedSitePaymentLaunchGate({
    activeEntitlementRefs,
    blockerRefs: publicSafeRefs([
      ...(hasVerifiedCheckoutEvidence
        ? []
        : ['blocker.generated_site_payment.missing_verified_checkout_bundle']),
      ...(liveBitcoinCheckoutClaimAllowed
        ? []
        : ['blocker.generated_site_payment.no_live_bitcoin_checkout_claim']),
      ...(payoutSettlementClaimAllowed
        ? []
        : ['blocker.generated_site_payment.no_payout_settlement_receipts']),
    ]),
    caveatRefs: publicSafeRefs([
      'caveat.generated_site_payment.checkout_return_not_payout_authority',
      'caveat.generated_site_payment.payment_proof_not_settlement',
      'caveat.generated_site_payment.payout_bridge_requires_separate_gates',
    ]),
    checkoutEvidenceOnly: !payoutSettlementClaimAllowed,
    checkoutIntentRefs,
    liveBitcoinCheckoutClaimAllowed,
    paymentProofRefs,
    payoutSettlementClaimAllowed,
    publicCopyRefs: publicSafeRefs([
      state === 'payout_settlement_verified'
        ? 'copy.generated_site_payment.payout_settlement_receipts_visible'
        : state === 'live_bitcoin_checkout_verified'
          ? 'copy.generated_site_payment.live_checkout_evidence_only'
          : state === 'checkout_evidence_only'
            ? 'copy.generated_site_payment.checkout_evidence_only'
            : 'copy.generated_site_payment.checkout_claim_blocked',
    ]),
    receiptBundleRefs: publicSafeRefs([
      ...receiptRefs,
      ...activeEntitlementRefs,
      ...reconciliationEventRefs,
      ...paymentProofRefs,
    ]),
    reconciliationEventRefs,
    settlementReceiptRefs,
    state,
    stateLabel:
      state === 'payout_settlement_verified'
        ? 'Generated Site checkout evidence has payout settlement receipts'
        : state === 'live_bitcoin_checkout_verified'
          ? 'Generated Site live Bitcoin checkout evidence is verified'
          : state === 'checkout_evidence_only'
            ? 'Generated Site checkout evidence is verified without payout settlement'
            : 'Generated Site checkout evidence is blocked',
  })
}

export const openAgentsGeneratedSitePaymentSmokeFixtureHasPrivateMaterial =
  valueHasPrivateMaterial

export const assertOpenAgentsGeneratedSitePaymentSmokeFixtureSafe = (
  fixture: OpenAgentsGeneratedSitePaymentSmokeFixture,
): void => {
  assertStableFixtureRefs('Generated Site payment smoke fixture refs', [
    fixture.fixtureRef,
    fixture.siteId,
    fixture.siteVersionId,
    ...fixture.sourceRefs,
  ])

  if (valueHasPrivateMaterial(fixture)) {
    throw new OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe({
      reason:
        'Generated Site payment smoke fixture must not expose MDK credentials, wallet material, raw invoices, payment hashes, preimages, provider grants, customer private data, raw payout targets, or secret material.',
    })
  }

  if (
    fixture.noDeploymentAuthority !== true ||
    fixture.noLiveCheckoutCreated !== true ||
    fixture.noRealInvoiceCreated !== true ||
    fixture.noWalletSpendAuthority !== true
  ) {
    throw new OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe({
      reason:
        'Generated Site payment smoke fixture cannot create deployment, live checkout, real invoice, or wallet spend authority.',
    })
  }
}

export const exampleOpenAgentsGeneratedSitePaymentManifest =
  (): OpenAgentsSitePaymentManifest =>
    decodeOpenAgentsSitePaymentManifest({
      payments: {
        agentReadable: true,
        enabled: true,
        metadataRefs: [
          'metadata.generated_site_payment_smoke.record_only',
          'metadata.generated_site_payment_smoke.fake_provider_default',
        ],
        paidActions: [
          {
            actionRef: 'action.generated_site_payment_smoke.research_note',
            agentReadable: true,
            checkoutPath: '/agent/research-note',
            customerDataRequirements: [],
            displayRef:
              'display.generated_site_payment_smoke.agent_research_note',
            entitlementScope: 'action',
            id: 'agent_research_note',
            metadataRefs: [
              'metadata.generated_site_payment_smoke.agent_action',
            ],
            method: 'POST',
            path: '/api/actions/research-note',
            price: {
              amountMinorUnits: 250000,
              asset: 'bitcoin',
              denomination: 'bitcoin_millisatoshi',
            },
            publicProjectionState: 'listed',
            sandbox: true,
            settlementMode: 'deferred',
          },
        ],
        products: [
          {
            agentReadable: true,
            checkoutPath: '/checkout/brief',
            customerDataRequirements: [
              {
                key: 'contact',
                kind: 'email',
                labelRef: 'label.generated_site_payment_smoke.contact',
                required: true,
              },
            ],
            displayRef:
              'display.generated_site_payment_smoke.human_brief_checkout',
            entitlementScope: 'product',
            id: 'human_brief_checkout',
            metadataRefs: [
              'metadata.generated_site_payment_smoke.human_checkout',
            ],
            price: {
              amountMinorUnits: 4900,
              asset: 'usd',
              denomination: 'usd_cent',
            },
            publicProjectionState: 'listed',
            sandbox: true,
            settlementMode: 'checkout_only',
          },
        ],
        provider: 'openagents_hosted_mdk',
        sandboxDefault: true,
      },
    })

export const exampleOpenAgentsGeneratedSitePaymentCatalog = () =>
  sitePaymentCatalogFromManifest({
    createdAt: '2026-06-07T12:00:00.000Z',
    deploymentId: null,
    manifest: exampleOpenAgentsGeneratedSitePaymentManifest(),
    manifestRef: 'manifest.generated_site_payment_smoke.v1',
    orderRef: 'order.generated_site_payment_smoke',
    siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
    siteVersionId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_VERSION_ID,
    sourceManifestDigest: 'digest.generated_site_payment_smoke.v1',
    status: 'active',
    updatedAt: '2026-06-07T12:00:00.000Z',
    workroomRef: 'workroom.generated_site_payment_smoke',
  })

export const exampleOpenAgentsGeneratedSitePaymentHelperPlans =
  (): ReadonlyArray<OpenAgentsGeneratedSitePaymentHelperRequestPlan> => {
    const catalog = exampleOpenAgentsGeneratedSitePaymentCatalog()
    const productRecord = catalog.items.find(
      (item): item is OpenAgentsSitePaymentProductCatalogRecord =>
        item.itemKind === 'product' &&
        'productId' in item &&
        item.productId === 'human_brief_checkout',
    )
    const actionRecord = catalog.items.find(
      (item): item is OpenAgentsSitePaymentPaidActionCatalogRecord =>
        item.itemKind === 'paid_action' &&
        'actionId' in item &&
        item.actionId === 'agent_research_note',
    )

    if (productRecord === undefined || actionRecord === undefined) {
      throw new OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe({
        reason:
          'Generated Site payment smoke fixture catalog must include one product and one paid action.',
      })
    }

    return [
      generatedSitePaymentDiscoveryPlan({
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
      generatedSiteCheckoutIntentPlan({
        body: {
          cancelReturnPath: '/checkout/cancel',
          catalogRef: productRecord.catalogRef,
          customerDataRefs: ['contact'],
          expectedPrice: productRecord.price,
          itemKind: 'product',
          productId: productRecord.productId,
          siteVersionId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_VERSION_ID,
          successReturnPath: '/checkout/complete',
        },
        idempotencyKey: 'generated-site-payment-smoke-checkout',
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
      generatedSiteCheckoutReturnPlan({
        checkoutIntentRef: 'site_checkout_intent_generated_site_payment_smoke',
        returnAction: 'status',
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
      generatedSitePaymentProofPlan({
        checkoutIntentRef: 'site_checkout_intent_generated_site_payment_smoke',
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
      generatedSiteL402ChallengePlan({
        body: {
          entitlementScope: 'action',
          method: 'POST',
          paidActionId: actionRecord.actionId,
          path: actionRecord.path,
          price: {
            amount: 250,
            asset: 'sats',
          },
          spendCap: {
            amount: 250,
            asset: 'sats',
          },
        },
        idempotencyKey: 'generated-site-payment-smoke-l402-challenge',
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
      generatedSiteL402RedemptionPlan({
        body: {
          challengeExpiresAt: 'challenge_expiry.generated_site_payment_smoke',
          challengeId: 'site_l402_challenge_generated_site_payment_smoke',
          credentialId: 'site_l402_credential_generated_site_payment_smoke',
          entitlementScope: 'action',
          method: 'POST',
          paidActionId: actionRecord.actionId,
          path: actionRecord.path,
          paymentProofRef:
            'mdk_payment_proof_generated_site_payment_smoke_redacted',
          price: {
            amount: 250,
            asset: 'sats',
          },
        },
        idempotencyKey: 'generated-site-payment-smoke-l402-redemption',
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
    ]
  }

export const exampleOpenAgentsGeneratedSitePaymentSmokeFixture =
  (): OpenAgentsGeneratedSitePaymentSmokeFixture => {
    const manifest = exampleOpenAgentsGeneratedSitePaymentManifest()
    const catalog = exampleOpenAgentsGeneratedSitePaymentCatalog()
    const smokeProjection = projectOpenAgentsSiteMdkSmoke(
      {
        ...exampleOpenAgentsSiteMdkSmokeRecord(),
        authority: OPENAGENTS_SITE_MDK_SMOKE_RECORD_ONLY_AUTHORITY,
        checkoutIntentRefs: [
          'site_checkout_intent_generated_site_payment_smoke',
        ],
        l402ChallengeRefs: ['site_l402_challenge_generated_site_payment_smoke'],
        l402RedemptionRefs: [
          'site_l402_redemption_generated_site_payment_smoke',
        ],
        paymentProofRefs: ['payment_proof.public.generated_site_payment_smoke'],
        receiptRefs: [
          'receipt.public.generated_site_payment_smoke.record_only',
        ],
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
        smokeRef: 'smoke.public.generated_site_payment_smoke.fake_provider',
        sourceRefs: [
          'source.workers.generated_site_payment_smoke_fixture',
          'source.docs.generated_site_payment_smoke_fixture',
        ],
      },
      'agent',
    )
    const fixture = new OpenAgentsGeneratedSitePaymentSmokeFixture({
      catalogProjection: projectOpenAgentsSitePaymentCatalog(catalog, 'agent'),
      discoveryProjection: projectOpenAgentsSitePaymentDiscovery({
        audience: 'agent',
        catalog,
        siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      }),
      fixtureRef: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_FIXTURE_REF,
      helperPlans: [...exampleOpenAgentsGeneratedSitePaymentHelperPlans()],
      manifestProjection: projectOpenAgentsSitePaymentManifest(
        manifest,
        'agent',
      ),
      noDeploymentAuthority: true,
      noLiveCheckoutCreated: true,
      noRealInvoiceCreated: true,
      noWalletSpendAuthority: true,
      paymentLaunchGate: projectOpenAgentsGeneratedSitePaymentLaunchGate({
        activeEntitlementRefs: [
          'entitlement.public.generated_site_payment_smoke.active',
        ],
        checkoutIntentRefs: smokeProjection.checkoutIntentRefs,
        implementationState: smokeProjection.implementationState,
        paymentProofRefs: smokeProjection.paymentProofRefs,
        receiptRefs: smokeProjection.receiptRefs,
        reconciliationEventRefs: [
          'reconciliation.public.generated_site_payment_smoke.matched',
        ],
        settlementReceiptRefs: [],
      }),
      siteId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_SITE_ID,
      siteVersionId: OPENAGENTS_GENERATED_SITE_PAYMENT_SMOKE_VERSION_ID,
      smokeProjection,
      sourceRefs: [
        'source.workers.generated_site_payment_smoke_fixture',
        'source.docs.generated_site_payment_smoke_fixture',
      ],
    })

    assertOpenAgentsGeneratedSitePaymentSmokeFixtureSafe(fixture)

    return fixture
  }
