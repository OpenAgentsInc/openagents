import { Effect, Schema as S } from 'effect'
import { readFile } from 'node:fs/promises'
import { describe, expect, test } from 'vitest'

import { openAgentsSpendCapPreviewHasPrivateMaterial } from './agent-spend-cap-preview'
import {
  artanisPublicReportHasPrivateMaterial,
  artanisPublicReportSnapshot,
} from './artanis-public-report'
import {
  OpenAgentsAuditExportBundleProjection,
  OpenAgentsAuditExportItem,
  OpenAgentsAuditExportRequest,
  OpenAgentsAuditExportUnsafe,
  buildOpenAgentsAuditExportBundle,
  openAgentsAuditExportProjectionHasPrivateMaterial,
  projectOpenAgentsAuditExportBundle,
} from './audit-export-contracts'
import { BlueprintDeveloperPackageContributionRecord } from './blueprint/schemas/developer-package-contribution'
import {
  BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
  blueprintDeveloperPackageContributionProjectionHasPrivateMaterial,
  projectBlueprintDeveloperPackageContribution,
} from './blueprint/services/developer-package-contribution'
import { openAgentsBuyerPaymentEntitlementPolicyHasPrivateMaterial } from './buyer-payment-entitlement-policy'
import {
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  buyerPaymentLedgerProjectionHasPrivateMaterial,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  ForumPublicProjectionUnsafe,
  decodeForumPublicProjection,
} from './forum/schemas'
import { forumTipEarningsProjectionHasPrivateMaterial } from './forum/tip-earnings'
import { forumTipSettlementProjectionForState } from './forum/tip-settlement'
import { openAgentsGeneratedSitePaymentSmokeFixtureHasPrivateMaterial } from './generated-site-payment-smoke-fixture'
import { openAgentsHostedMdkPayloadHasPrivateMaterial } from './hosted-mdk-client'
import { openAgentsL402DeferredSettlementHasPrivateMaterial } from './l402-deferred-settlement'
import {
  type OpenAgentsL402ResponseContract,
  l402ResponseContractHasPrivateMaterial,
} from './l402-response-contract'
import {
  MarketplaceMarginMemoryUnsafe,
  exampleMarketplaceMarginMemory,
  projectMarketplaceMarginMemory,
} from './marketplace-margin-memory'
import { openAgentsMdkAgentWalletSmokeHasPrivateMaterial } from './mdk-agent-wallet-smoke-fixture'
import { openAgentsMdkSidecarOptionHasPrivateMaterial } from './mdk-sidecar-option'
import {
  type NexusTreasuryPayoutLedgerProjection,
  nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial,
} from './nexus-treasury-payout-ledger'
import {
  OmniDataClassificationValidationError,
  OmniDataPolicyEnvelope,
  projectOmniDataPolicyEnvelope,
} from './omni-data-classification'
import {
  OpenAgentsAgentOnboardingUnsafe,
  openAgentsAgentOnboardingMarkdownEffect,
} from './openagents-agent-onboarding'
import {
  OpenAgentsPaymentDestinationInput,
  OpenAgentsPaymentDestinationUnsafe,
  classifyOpenAgentsPaymentDestinationInput,
  openAgentsPaymentDestinationHasPrivateMaterial,
} from './payment-destination-input'
import {
  OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY,
  OpenAgentsPolicyExceptionReceipt,
  OpenAgentsPolicyExceptionUnsafe,
  projectOpenAgentsPolicyException,
} from './policy-exception-receipts'
import {
  OpenAgentsProviderPlacementRequest,
  OpenAgentsProviderPlacementUnsafe,
  OpenAgentsProviderPolicy,
  evaluateOpenAgentsProviderPlacement,
} from './provider-placement-policy'
import { publicPylonStatsFromNexusPayload } from './public-pylon-stats'
import {
  OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES,
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
  openAgentsUnsafePaymentRedactionFixtureValues,
  openAgentsUnsafeRedactionFixtureValues,
} from './redaction-regression-fixtures'
import {
  OpenAgentsRunnerBackendProjection,
  OpenAgentsRunnerBackendRecord as OpenAgentsRunnerBackendRecordSchema,
  openAgentsRunnerBackendProjectionHasPrivateMaterial,
  projectOpenAgentsRunnerBackend,
} from './runner-backends'
import { openAgentsSiteMdkReconciliationHasPrivateMaterial } from './site-mdk-reconciliation'
import { openAgentsSitePaymentProofHasPrivateMaterial } from './site-payment-proof'
import { openAgentsSitePaymentToPayoutBridgeHasPrivateMaterial } from './site-payment-to-payout-bridge'
import { openAgentsUnifiedPaymentDecisionHasPrivateMaterial } from './unified-payment-decision'

const nowIso = '2026-06-06T23:20:00.000Z'

const l402ContractForFixture = (
  fixtureValue: string,
): OpenAgentsL402ResponseContract => ({
  audience: 'agent',
  challenge: null,
  credentialStatus: null,
  docsRefs: [fixtureValue],
  errorKind: 'payment_required',
  headerRefs: [],
  policyDecision: null,
  productId: null,
  publicSummaryRef: 'summary.l402_response.safe_fixture',
  reasonRefs: [],
  recoveryActionRefs: [],
  statusCode: 402,
  statusRefs: [],
})

const nexusProjectionForFixture = (
  fixtureValue: string,
): NexusTreasuryPayoutLedgerProjection => ({
  adapterKind: null,
  amount: null,
  assignmentRef: null,
  audience: 'agent',
  metadataRefs: [fixtureValue],
  operatorRefs: [],
  ownerUserId: null,
  payoutAttemptRef: null,
  payoutIntentRef: null,
  payoutTargetApprovalRef: null,
  payoutTargetRef: null,
  publicProjectionJson: '{}',
  receiptRef: null,
  recordKind: 'receipt',
  redactedDestinationRef: null,
  redactedPaymentRef: null,
  status: 'fixture',
})

const artanisReportForFixture = (fixtureValue: string) => {
  const report = artanisPublicReportSnapshot({
    nowIso,
    pylonStats: publicPylonStatsFromNexusPayload({
      hosted_nexus_relay_url: 'wss://nexus.openagents.com/',
      nexus_accepted_work_payout_sats_paid_24h: 0,
      nexus_accepted_work_payout_sats_paid_total: 0,
      pylon_sessions_online_now: 0,
      pylons_online_now: 0,
      recent_pylons: [],
      sellable_pylons_online_now: 0,
      training_accepted_contributors: 0,
      training_assigned_contributors: 0,
    }),
  })

  return {
    ...report,
    autonomousLoop: {
      ...report.autonomousLoop,
      artifactRefs: [fixtureValue],
    },
  }
}

const paymentPrivateMaterialGuards: ReadonlyArray<
  Readonly<{
    hasPrivateMaterial: (value: string) => boolean
    label: string
  }>
> = [
  {
    hasPrivateMaterial: openAgentsHostedMdkPayloadHasPrivateMaterial,
    label: 'hosted MDK payload',
  },
  {
    hasPrivateMaterial: value =>
      l402ResponseContractHasPrivateMaterial(l402ContractForFixture(value)),
    label: 'L402 response contract',
  },
  {
    hasPrivateMaterial: openAgentsL402DeferredSettlementHasPrivateMaterial,
    label: 'L402 deferred settlement',
  },
  {
    hasPrivateMaterial: openAgentsSitePaymentProofHasPrivateMaterial,
    label: 'Site payment proof',
  },
  {
    hasPrivateMaterial: openAgentsSiteMdkReconciliationHasPrivateMaterial,
    label: 'Site MDK reconciliation',
  },
  {
    hasPrivateMaterial: openAgentsSitePaymentToPayoutBridgeHasPrivateMaterial,
    label: 'Site payment-to-payout bridge',
  },
  {
    hasPrivateMaterial: openAgentsMdkAgentWalletSmokeHasPrivateMaterial,
    label: 'MDK agent-wallet smoke fixture',
  },
  {
    hasPrivateMaterial: openAgentsMdkSidecarOptionHasPrivateMaterial,
    label: 'self-hosted mdkd sidecar option',
  },
  {
    hasPrivateMaterial: openAgentsPaymentDestinationHasPrivateMaterial,
    label: 'payment destination projection',
  },
  {
    hasPrivateMaterial: openAgentsSpendCapPreviewHasPrivateMaterial,
    label: 'agent spend-cap preview',
  },
  {
    hasPrivateMaterial:
      openAgentsBuyerPaymentEntitlementPolicyHasPrivateMaterial,
    label: 'buyer payment entitlement policy',
  },
  {
    hasPrivateMaterial: openAgentsUnifiedPaymentDecisionHasPrivateMaterial,
    label: 'unified payment decision',
  },
  {
    hasPrivateMaterial:
      openAgentsGeneratedSitePaymentSmokeFixtureHasPrivateMaterial,
    label: 'generated Site payment smoke fixture',
  },
  {
    hasPrivateMaterial: value =>
      forumTipEarningsProjectionHasPrivateMaterial({
        actorRef: 'actor.route-test',
        earnings: [
          {
            acceptedWorkPayoutEvidence: false,
            actionKind: 'post_reward',
            amount: { amount: 100, asset: 'sats' },
            createdAt: nowIso,
            creatorReceivedSpendableValue: false,
            earningActorRef: 'actor.route-test',
            earningRef: value,
            moneyActionRef: 'forum_money_action:safe',
            paymentEventRef: null,
            paymentState: 'unverified',
            receiptRef: 'receipt.forum.safe',
            recipientActorRef: 'actor.route-test',
            settlementState: 'evidence_only',
            target: {
              forumId: null,
              postId: '66666666-6666-4666-8666-666666666666',
              topicId: '55555555-5555-4555-8555-555555555555',
            },
            targetPostPermalink:
              'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-66666666-6666-4666-8666-666666666666',
            tipSettlement:
              forumTipSettlementProjectionForState('evidence_only'),
          },
        ],
        generatedAt: nowIso,
        pagination: {
          cursor: null,
          hasMore: false,
          limit: 50,
          nextCursor: null,
        },
        summary: {
          failedCount: 0,
          paidCount: 0,
          pendingCount: 1,
          refundedCount: 0,
          reversedCount: 0,
          settledCount: 0,
          totalCount: 1,
          totalPaidSats: 100,
          totalSettledSats: 0,
        },
      } as unknown as Parameters<
        typeof forumTipEarningsProjectionHasPrivateMaterial
      >[0]),
    label: 'Forum creator tip earnings projection',
  },
  {
    hasPrivateMaterial: value =>
      nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial(
        nexusProjectionForFixture(value),
      ),
    label: 'Nexus treasury payout ledger',
  },
  {
    hasPrivateMaterial: value =>
      artanisPublicReportHasPrivateMaterial(artanisReportForFixture(value)),
    label: 'Artanis public report',
  },
]

const committedPublicPaymentFiles = [
  { label: 'AGENTS.md', path: '../../docs/live/AGENTS.md' },
  {
    label: 'OpenAPI source',
    path: 'src/openagents-openapi.ts',
  },
  {
    label: 'capability manifest source',
    path: 'src/openagents-capability-manifest.ts',
  },
  {
    label: 'agent onboarding source',
    path: 'src/openagents-agent-onboarding.ts',
  },
  {
    label: 'redaction regression docs',
    path: '../../docs/2026-06-06-redaction-regression-suite.md',
  },
  {
    label: 'MDK agent-wallet pay402 smoke docs',
    path: '../../docs/sites/2026-06-07-mdk-agent-wallet-pay402-smoke.md',
  },
  {
    label: 'Nexus/Pylon visibility runbook',
    path: '../../docs/nexus/2026-06-07-nexus-pylon-visibility-runbook.md',
  },
  {
    label: 'Pylon release gate runbook',
    path: '../../docs/nexus/2026-06-07-pylon-v02-openagents-release-gate-runbook.md',
  },
  {
    label: 'MDK payout adapter runbook',
    path: '../../docs/nexus/2026-06-07-mdk-agent-wallet-payout-adapter-runbook.md',
  },
  {
    label: 'generated Site payment smoke fixture docs',
    path: '../../docs/sites/2026-06-07-generated-site-payment-smoke-fixture.md',
  },
  {
    label: 'generated Site human checkout smoke docs',
    path: '../../docs/sites/2026-06-07-generated-site-human-checkout-smoke.md',
  },
  {
    label: 'generated Site agent-paid L402 smoke docs',
    path: '../../docs/sites/2026-06-07-generated-site-agent-paid-l402-smoke.md',
  },
  {
    label: 'generated Site reconciliation smoke docs',
    path: '../../docs/sites/2026-06-07-generated-site-reconciliation-smoke.md',
  },
  {
    label: 'generated Site payment smoke runbook',
    path: '../../docs/sites/2026-06-07-generated-site-payment-smoke-runbook.md',
  },
] as const

const committedRawPaymentLeakPatterns = [
  {
    label: 'assigned MDK access token',
    pattern: /\bMDK_ACCESS_TOKEN\s*=\s*(?!<|$|`|\s)[^\s`]+/iu,
  },
  {
    label: 'assigned MDK mnemonic',
    pattern: /\bMDK_MNEMONIC\s*=\s*(?!<|$|`|\s)[^\s`]+/iu,
  },
  {
    label: 'assigned MDK webhook secret',
    pattern: /\bMDK_WEBHOOK_SECRET\s*=\s*(?!<|$|`|\s)[^\s`]+/iu,
  },
  {
    label: 'assigned MDK withdrawal destination',
    pattern: /\bWITHDRAWAL_DESTINATION\s*=\s*(?!<|$|`|\s)[^\s`]+/iu,
  },
  {
    label: 'raw BOLT11 invoice',
    pattern: /\b(?:lnbc|lntb|lnbcrt)[0-9a-z]{48,}\b/iu,
  },
  {
    label: 'raw BOLT12 offer',
    pattern: /\blno1[0-9a-z]{48,}\b/iu,
  },
  {
    label: 'raw payment hash assignment',
    pattern: /\bpayment_hash\s*[:=]\s*["']?(?!<redacted|<)[A-Za-z0-9_-]{24,}/iu,
  },
  {
    label: 'raw payment preimage assignment',
    pattern:
      /\b(?:payment_)?preimage\s*[:=]\s*["']?(?!<redacted|<)[A-Za-z0-9_-]{24,}/iu,
  },
  {
    label: 'Stripe secret key',
    pattern: /\bsk_(?:live|test)_[A-Za-z0-9]{16,}\b/u,
  },
  {
    label: 'webhook signing secret',
    pattern: /\bwhsec_[A-Za-z0-9]{16,}\b/u,
  },
  {
    label: 'private MDK wallet path',
    pattern: /\/Users\/[^`\s]+\/\.mdk-wallet\/[^`\s]+/u,
  },
  {
    label: 'exact wallet balance output',
    pattern: /\bbalance_sats["']?\s*:\s*[1-9][0-9]*/u,
  },
] as const

const paymentDestinationRejectsRawInputLabels = new Set([
  'MDK access token',
  'MDK mnemonic',
  'MDK webhook secret',
  'agent wallet home',
  'raw payment preimage',
  'provider grant',
  'Stripe secret',
  'Treasury secret',
  'private checkout ref',
  'private customer or operator data',
])

const dataPolicy = (unsafeRef?: string) =>
  S.decodeUnknownSync(OmniDataPolicyEnvelope)({
    classificationCaveatRef: 'classification.caveat.public',
    dataClassification: 'public',
    evidenceRefs: unsafeRef === undefined ? ['evidence.safe'] : [unsafeRef],
    exportPolicyRefs: ['export.public'],
    providerEligibilityRefs: [],
    redactionPolicyRefs: [],
    retentionPolicyRefs: ['retention.standard'],
    subjectRef: 'site.public_safe',
    surface: 'site',
    trustTier: 'reviewed',
  })

const auditExportRequest = () =>
  S.decodeUnknownSync(OpenAgentsAuditExportRequest)({
    approvedByRef: 'approved_by.operator.review',
    audience: 'operator',
    caveatRefs: ['caveat.audit_export.safe'],
    createdAtIso: '2026-06-06T23:00:00.000Z',
    exportPolicyRefs: ['export.operator_safe.audit'],
    generatedAtIso: nowIso,
    id: 'audit_export.safe',
    requestedScopeRefs: ['scope.site'],
    requestedScopes: ['site'],
    requesterRef: 'requested_by.operator',
    retentionPolicyRefs: ['retention.standard'],
  })

const auditExportItem = (unsafeRef?: string) =>
  S.decodeUnknownSync(OpenAgentsAuditExportItem)({
    caveatRefs: ['caveat.audit_item.safe'],
    createdAtIso: '2026-06-06T23:05:00.000Z',
    dataPolicy: dataPolicy(),
    evidenceRefs:
      unsafeRef === undefined ? ['evidence.audit_safe'] : [unsafeRef],
    exportPolicyRefs: ['export.public'],
    itemRef: 'site.public_safe',
    receiptRefs: ['receipt.audit_safe'],
    retentionPolicyRefs: ['retention.standard'],
    scope: 'site',
    sourceRefs: ['source.public_safe'],
  })

const provider = () =>
  S.decodeUnknownSync(OpenAgentsProviderPolicy)({
    allowedDataClassifications: ['public'],
    allowedSurfaces: ['site'],
    allowedWorkKinds: ['site'],
    backendKind: 'cloudflare_container',
    caveatRefs: ['caveat.provider.public'],
    cooldownRefs: [],
    disabledReasonRefs: [],
    id: 'provider.public_container',
    maxWorkloadTrust: 'low',
    policyRefs: ['policy.provider.public'],
    providerEligibilityRefs: ['provider.eligibility.public'],
    state: 'available',
    trustTier: 'public',
  })

const providerRequest = (unsafeRef?: string) =>
  S.decodeUnknownSync(OpenAgentsProviderPlacementRequest)({
    dataPolicy: dataPolicy(unsafeRef),
    evidenceRefs: ['evidence.provider_placement'],
    id: 'provider_placement.public_site',
    legalReviewRefs: [],
    operatorApprovalRefs: [],
    ownerGrantRefs: [],
    paymentPolicyRefs: [],
    policyExceptionRefs: [],
    requestedBackendKind: 'cloudflare_container',
    requiredWorkloadTrust: 'low',
    workKind: 'site',
  })

const policyException = (unsafeRef?: string) =>
  S.decodeUnknownSync(OpenAgentsPolicyExceptionReceipt)({
    approvedByRef: 'approved_by.operator_review',
    authority: OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY,
    blockerRefs: [],
    createdAtIso: '2026-06-06T23:00:00.000Z',
    evidenceRefs: unsafeRef === undefined ? ['evidence.safe'] : [unsafeRef],
    expiresAtIso: null,
    family: 'provider_placement',
    id: 'policy_exception.public_safe',
    requestedByRef: 'requested_by.operator',
    reviewState: 'approved',
    riskRefs: ['risk.limited'],
    scopeRefs: ['scope.public_safe'],
    subjectRefs: ['subject.public_safe'],
    updatedAtIso: '2026-06-06T23:10:00.000Z',
  })

const blueprintContribution = () =>
  S.decodeUnknownSync(BlueprintDeveloperPackageContributionRecord)({
    authority: BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
    backendProjectionAdapterRefs: ['adapter.probe.apple_fm.blueprint_tools.v1'],
    capabilityFamily: 'program_signature',
    capabilitySummaryRef: 'summary.developer_package.safe',
    contextPackageRefs: openAgentsUnsafeRedactionFixtureValues,
    contributorRefs: ['contributor.agent.safe'],
    createdAt: '2026-06-06T00:00:00.000Z',
    dogfoodScopeRef: 'dogfood.autopilot.continue.candidate_only',
    id: 'developer_package.safe',
    intendedProgramFamily: 'continuation',
    noProductionRuntimeAuthority: true,
    outcomeTemplateRefs: ['outcome_template.safe'],
    paymentAttributionRefs: ['payment_attribution.safe'],
    promotionRef: null,
    proposedModuleVersionRefs: ['module_version.safe'],
    proposedProgramSignatureRefs: ['program_signature.safe'],
    proposedProgramTypeRefs: ['program_type.safe'],
    rejectionRef: null,
    releaseGateRefs: ['release_gate.safe'],
    requiredFixtureRefs: ['fixture.safe'],
    retainedFailureRefs: ['failure.safe'],
    reviewStatus: 'approved',
    riskClass: 'medium',
    selfPromotionAttempt: false,
    sourceRefs: openAgentsUnsafeRedactionFixtureValues,
    status: 'approved_for_release_gate',
    toolPackageRefs: ['tool_package.safe'],
    uiBindingRefs: ['ui_binding.safe'],
    updatedAt: '2026-06-06T00:00:00.000Z',
  })

const buyerReceipt = () =>
  S.decodeUnknownSync(BuyerPaymentReceiptRecord)({
    actorRef: 'actor.agent.safe',
    amount: {
      amountMinorUnits: 100,
      asset: 'bitcoin',
      denomination: 'bitcoin_millisatoshi',
    },
    archivedAt: null,
    challengeRef: 'challenge.safe',
    createdAt: '2026-06-06T23:00:00.000Z',
    entitlementRef: 'entitlement.safe',
    id: 'buyer_payment_receipt.safe',
    metadataRefs: openAgentsUnsafeRedactionFixtureValues,
    ownerUserId: 'user.safe',
    productId: 'product.safe',
    publicProjectionJson: JSON.stringify({
      refs: openAgentsUnsafeRedactionFixtureValues,
    }),
    receiptRef: 'receipt.safe',
    redactedPaymentRef: 'payment.redacted.safe',
    status: 'issued',
    surface: 'forum_paid_action',
  })

describe('OpenAgents redaction regression fixtures', () => {
  test('shared unsafe fixtures are rejected by policy boundary modules', () => {
    for (const fixture of OPENAGENTS_UNSAFE_REDACTION_FIXTURES) {
      expect(() =>
        projectOmniDataPolicyEnvelope(dataPolicy(fixture.value), 'public'),
      ).toThrow(OmniDataClassificationValidationError)
      expect(() =>
        evaluateOpenAgentsProviderPlacement(
          provider(),
          providerRequest(fixture.value),
        ),
      ).toThrow(OpenAgentsProviderPlacementUnsafe)
      expect(() =>
        projectOpenAgentsPolicyException(
          policyException(fixture.value),
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsPolicyExceptionUnsafe)
      expect(() =>
        projectMarketplaceMarginMemory(
          {
            ...exampleMarketplaceMarginMemory(),
            evidenceRefs: [fixture.value],
          },
          'public',
          nowIso,
        ),
      ).toThrow(MarketplaceMarginMemoryUnsafe)
      expect(() =>
        buildOpenAgentsAuditExportBundle(auditExportRequest(), [
          auditExportItem(fixture.value),
        ]),
      ).toThrow(OpenAgentsAuditExportUnsafe)
    }
  })

  test.each(OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES)(
    'payment fixture $label is unsafe across payment-facing projections',
    fixture => {
      const missedGuards = paymentPrivateMaterialGuards
        .filter(guard => !guard.hasPrivateMaterial(fixture.value))
        .map(guard => guard.label)

      expect(missedGuards).toEqual([])
    },
  )

  test.each(OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES)(
    'payment destination raw input handles $label without projecting raw values',
    fixture => {
      const input = new OpenAgentsPaymentDestinationInput({
        allowCashu: false,
        allowNetworkResolution: false,
        allowOnchain: false,
        inputRef: 'payment_destination.fixture',
        rawInput: fixture.value,
        source: 'raw_text',
      })

      if (paymentDestinationRejectsRawInputLabels.has(fixture.label)) {
        expect(() => classifyOpenAgentsPaymentDestinationInput(input)).toThrow(
          OpenAgentsPaymentDestinationUnsafe,
        )

        return
      }

      const projection = classifyOpenAgentsPaymentDestinationInput(input)

      expect(projection.dispatchAllowed).toBe(false)
      expect(projection.payoutAuthorityCreated).toBe(false)
      expect(projection.rawDestinationProjected).toBe(false)
      expect(JSON.stringify(projection)).not.toContain(fixture.value)
      expect(openAgentsPaymentDestinationHasPrivateMaterial(projection)).toBe(
        false,
      )
    },
  )

  test('committed public docs and agent API sources omit raw payment secrets', async () => {
    const fileResults = await Promise.all(
      committedPublicPaymentFiles.map(async file => ({
        ...file,
        contents: await readFile(file.path, 'utf8'),
      })),
    )
    const exactFixtureFindings = fileResults.flatMap(file =>
      openAgentsUnsafePaymentRedactionFixtureValues
        .filter(fixtureValue => file.contents.includes(fixtureValue))
        .map(
          fixtureValue =>
            `${file.label} contains exact unsafe fixture ${fixtureValue}`,
        ),
    )
    const rawPatternFindings = fileResults.flatMap(file =>
      committedRawPaymentLeakPatterns
        .filter(pattern => pattern.pattern.test(file.contents))
        .map(pattern => `${file.label} contains ${pattern.label}`),
    )

    expect([...exactFixtureFindings, ...rawPatternFindings]).toEqual([])
  })

  test('representative projections omit exact unsafe fixture values', async () => {
    const runnerProjection = projectOpenAgentsRunnerBackend(
      S.decodeUnknownSync(OpenAgentsRunnerBackendRecordSchema)({
        artifactRefs: openAgentsUnsafeRedactionFixtureValues,
        backendKind: 'cloudflare_container',
        capacityRefs: ['capacity.safe'],
        configured: false,
        costRefs: ['cost.safe'],
        dispatchStatus: 'blocked',
        displayNameRef: 'runner.safe',
        enabled: false,
        healthRefs: openAgentsUnsafeRedactionFixtureValues,
        id: 'runner_backend.safe',
        lifecycleEventRefs: ['runner_event.safe'],
        operatorDiagnosticRefs: openAgentsUnsafeRedactionFixtureValues,
        policyRefs: ['policy.safe'],
        publicSummaryRef: 'summary.safe',
        receiptRefs: ['receipt.safe'],
        trustLevel: 'medium',
      }),
      'operator',
    )
    const blueprintProjection = projectBlueprintDeveloperPackageContribution(
      blueprintContribution(),
    )
    const buyerProjection = projectBuyerPaymentLedgerRecord(
      'receipt',
      buyerReceipt(),
      'operator',
    )
    const auditExportProjection = projectOpenAgentsAuditExportBundle(
      buildOpenAgentsAuditExportBundle(auditExportRequest(), [
        auditExportItem(),
      ]),
      nowIso,
    )
    const onboarding = await Effect.runPromise(
      openAgentsAgentOnboardingMarkdownEffect(),
    )

    expect(
      S.decodeUnknownSync(OpenAgentsRunnerBackendProjection)(runnerProjection),
    ).toEqual(runnerProjection)
    expect(
      S.decodeUnknownSync(BuyerPaymentLedgerProjection)(buyerProjection),
    ).toEqual(buyerProjection)
    expect(
      S.decodeUnknownSync(OpenAgentsAuditExportBundleProjection)(
        auditExportProjection,
      ),
    ).toEqual(auditExportProjection)
    expect(
      openAgentsRunnerBackendProjectionHasPrivateMaterial(runnerProjection),
    ).toBe(false)
    expect(
      blueprintDeveloperPackageContributionProjectionHasPrivateMaterial(
        blueprintProjection,
      ),
    ).toBe(false)
    expect(
      buyerPaymentLedgerProjectionHasPrivateMaterial(buyerProjection),
    ).toBe(false)
    expect(
      openAgentsAuditExportProjectionHasPrivateMaterial(auditExportProjection),
    ).toBe(false)
    expect(
      openAgentsSerializedValueContainsUnsafeFixture(runnerProjection),
    ).toBe(false)
    expect(
      openAgentsSerializedValueContainsUnsafeFixture(blueprintProjection),
    ).toBe(false)
    expect(
      openAgentsSerializedValueContainsUnsafeFixture(buyerProjection),
    ).toBe(false)
    expect(
      openAgentsSerializedValueContainsUnsafeFixture(auditExportProjection),
    ).toBe(false)
    expect(openAgentsSerializedValueContainsUnsafeFixture(onboarding)).toBe(
      false,
    )
  })

  test('Forum public projections reject unsafe receipt and artifact refs', () => {
    for (const fixture of OPENAGENTS_UNSAFE_REDACTION_FIXTURES) {
      expect(() =>
        decodeForumPublicProjection({
          classificationCaveatRef: 'classification.public_forum_projection',
          customerSafe: true,
          dataClassification: 'public',
          excludedPrivateRefs: [],
          publicSafe: true,
          redactionPolicyRef: 'redaction.public_forum_projection',
          safeArtifactRefs: [fixture.value],
          safeReceiptRefs: ['receipt.safe'],
          trustTier: 'reviewed',
        }),
      ).toThrow(ForumPublicProjectionUnsafe)
      expect(() =>
        decodeForumPublicProjection({
          classificationCaveatRef: 'classification.public_forum_projection',
          customerSafe: true,
          dataClassification: 'public',
          excludedPrivateRefs: [],
          publicSafe: true,
          redactionPolicyRef: 'redaction.public_forum_projection',
          safeArtifactRefs: ['artifact.safe'],
          safeReceiptRefs: [fixture.value],
          trustTier: 'reviewed',
        }),
      ).toThrow(ForumPublicProjectionUnsafe)
    }
  })

  test('agent onboarding document rejects provider-secret-shaped edits', async () => {
    await expect(
      Effect.runPromise(openAgentsAgentOnboardingMarkdownEffect()),
    ).resolves.toContain('OpenAgents Agent Onboarding')

    expect(
      openAgentsSerializedValueContainsUnsafeFixture(
        'sk-openagents-secret-test-value',
      ),
    ).toBe(true)
    await expect(
      Effect.runPromise(
        Effect.fail(
          new OpenAgentsAgentOnboardingUnsafe({
            reason:
              'fixture proves typed agent guidance failures stay explicit',
          }),
        ),
      ),
    ).rejects.toBeInstanceOf(OpenAgentsAgentOnboardingUnsafe)
  })
})
