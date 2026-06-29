import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniDataPolicyEnvelope,
} from './omni-data-classification'
import {
  OpenAgentsProviderPlacementProjection,
  OpenAgentsProviderPlacementRecord,
  OpenAgentsProviderPlacementRequest,
  OpenAgentsProviderPlacementUnsafe,
  OpenAgentsProviderPolicy,
  evaluateOpenAgentsProviderPlacement,
  openAgentsProviderPlacementProjectionHasPrivateMaterial,
  projectOpenAgentsProviderPlacement,
} from './provider-placement-policy'

const dataPolicy = (
  overrides: Partial<OmniDataPolicyEnvelope> = {},
): OmniDataPolicyEnvelope =>
  S.decodeUnknownSync(OmniDataPolicyEnvelope)({
    classificationCaveatRef: 'classification.caveat.public_site',
    dataClassification: 'public',
    evidenceRefs: ['evidence.site.public_summary'],
    exportPolicyRefs: ['export.public'],
    providerEligibilityRefs: [],
    redactionPolicyRefs: [],
    retentionPolicyRefs: ['retention.standard'],
    subjectRef: 'site.otec_public',
    surface: 'site',
    trustTier: 'reviewed',
    ...overrides,
  })

const provider = (
  overrides: Partial<OpenAgentsProviderPolicy> = {},
): OpenAgentsProviderPolicy =>
  S.decodeUnknownSync(OpenAgentsProviderPolicy)({
    allowedDataClassifications: ['public', 'customer'],
    allowedSurfaces: ['order', 'site', 'site_revision'],
    allowedWorkKinds: ['order', 'site', 'site_revision'],
    backendKind: 'cloudflare_container',
    caveatRefs: ['caveat.provider.public_work_only'],
    cooldownRefs: [],
    disabledReasonRefs: [],
    id: 'provider.cloudflare_container_public',
    maxWorkloadTrust: 'medium',
    policyRefs: ['policy.provider.public_container'],
    providerEligibilityRefs: [
      'provider.eligibility.public',
      'provider.eligibility.customer_visible',
    ],
    state: 'available',
    trustTier: 'customer_visible',
    ...overrides,
  })

const request = (
  overrides: Partial<OpenAgentsProviderPlacementRequest> = {},
): OpenAgentsProviderPlacementRequest =>
  S.decodeUnknownSync(OpenAgentsProviderPlacementRequest)({
    dataPolicy: dataPolicy(),
    evidenceRefs: ['evidence.provider_placement.public_site'],
    id: 'provider_placement.site_otec',
    legalReviewRefs: [],
    operatorApprovalRefs: [],
    ownerGrantRefs: [],
    paymentPolicyRefs: [],
    policyExceptionRefs: [],
    requestedBackendKind: 'cloudflare_container',
    requiredWorkloadTrust: 'low',
    workKind: 'site',
    ...overrides,
  })

describe('OpenAgents provider placement policy', () => {
  test('allows public Site placement on an available eligible provider', () => {
    const record = evaluateOpenAgentsProviderPlacement(provider(), request())
    const publicProjection = projectOpenAgentsProviderPlacement(
      record,
      'public',
    )

    expect(S.decodeUnknownSync(OpenAgentsProviderPlacementRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(OpenAgentsProviderPlacementProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(record).toMatchObject({
      allowed: true,
      decision: 'allowed',
      providerRef: 'provider.cloudflare_container_public',
      requestedBackendKind: 'cloudflare_container',
      trustTier: 'customer_visible',
    })
    expect(record.requiredProviderEligibilityRefs).toEqual([
      'provider.eligibility.public',
    ])
    expect(publicProjection.providerRef).toBe('provider.redacted')
    expect(publicProjection.evidenceRefs).toEqual([])
    expect(openAgentsProviderPlacementProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)
  })

  test('denies unavailable providers and non-overrideable no-external-provider work', () => {
    const disabled = evaluateOpenAgentsProviderPlacement(
      provider({ state: 'disabled' }),
      request({ policyExceptionRefs: ['policy_exception.operator_override'] }),
    )
    const noExternal = evaluateOpenAgentsProviderPlacement(
      provider({
        allowedDataClassifications: ['secret_bearing'],
        allowedSurfaces: ['provider_account'],
        allowedWorkKinds: ['agent_api_action'],
      }),
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'secret_bearing',
          subjectRef: 'provider_account.binding_ref',
          surface: 'provider_account',
        }),
        policyExceptionRefs: ['policy_exception.operator_override'],
        workKind: 'agent_api_action',
      }),
    )

    expect(disabled).toMatchObject({
      allowed: false,
      decision: 'denied',
    })
    expect(disabled.blockerRefs).toContain(
      'blocker.provider_placement.provider_disabled',
    )
    expect(noExternal.allowed).toBe(false)
    expect(noExternal.blockerRefs).toContain(
      'blocker.provider_placement.no_external_provider_allowed',
    )
  })

  test('requires explicit policy exception refs for overrideable placement mismatches', () => {
    const mismatchedWithoutException = evaluateOpenAgentsProviderPlacement(
      provider({
        allowedDataClassifications: ['public'],
        allowedSurfaces: ['site'],
        allowedWorkKinds: ['site'],
        maxWorkloadTrust: 'low',
        providerEligibilityRefs: ['provider.eligibility.public'],
      }),
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'payment_private',
          exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
          providerEligibilityRefs: ['provider.eligibility.owner_approved'],
          subjectRef: 'payment_ref.order_1_redacted',
          surface: 'payment_ref',
        }),
        operatorApprovalRefs: ['operator_approval.payment_review'],
        paymentPolicyRefs: ['payment_policy.order_1'],
        requiredWorkloadTrust: 'sensitive',
        workKind: 'payment_sensitive_action',
      }),
    )
    const mismatchedWithException = evaluateOpenAgentsProviderPlacement(
      provider({
        allowedDataClassifications: ['public'],
        allowedSurfaces: ['site'],
        allowedWorkKinds: ['site'],
        maxWorkloadTrust: 'low',
        providerEligibilityRefs: ['provider.eligibility.public'],
      }),
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'payment_private',
          exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
          providerEligibilityRefs: ['provider.eligibility.owner_approved'],
          subjectRef: 'payment_ref.order_1_redacted',
          surface: 'payment_ref',
        }),
        operatorApprovalRefs: ['operator_approval.payment_review'],
        paymentPolicyRefs: ['payment_policy.order_1'],
        policyExceptionRefs: ['policy_exception.provider_placement.approved'],
        requiredWorkloadTrust: 'sensitive',
        workKind: 'payment_sensitive_action',
      }),
    )

    expect(mismatchedWithoutException.allowed).toBe(false)
    expect(mismatchedWithoutException.blockerRefs).toEqual([
      'blocker.provider_placement.classification_not_allowed',
      'blocker.provider_placement.provider_eligibility_missing',
      'blocker.provider_placement.surface_not_allowed',
      'blocker.provider_placement.work_kind_not_allowed',
      'blocker.provider_placement.workload_trust_too_low',
    ])
    expect(mismatchedWithException.allowed).toBe(true)
    expect(mismatchedWithException.policyRefs).toContain(
      'policy.provider_placement.explicit_exception_applied',
    )
  })

  test('requires owner, legal review, and payment policy refs for sensitive work', () => {
    const capableProvider = provider({
      allowedDataClassifications: [
        'legal_sensitive',
        'payment_private',
        'private',
      ],
      allowedSurfaces: ['payment_ref', 'provider_account', 'site'],
      allowedWorkKinds: [
        'legal_sensitive_work',
        'payment_sensitive_action',
        'private_repo',
      ],
      backendKind: 'shc_vm',
      maxWorkloadTrust: 'sensitive',
      providerEligibilityRefs: [
        'provider.eligibility.legal_sensitive',
        'provider.eligibility.payment_private',
        'provider.eligibility.reviewed_private',
      ],
      trustTier: 'legal_sensitive',
    })
    const legalMissingReview = evaluateOpenAgentsProviderPlacement(
      capableProvider,
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'legal_sensitive',
          exportPolicyRefs: ['export.operator_safe.redacted_legal_summary'],
        }),
        requestedBackendKind: 'shc_vm',
        requiredWorkloadTrust: 'sensitive',
        workKind: 'legal_sensitive_work',
      }),
    )
    const paymentMissingPolicy = evaluateOpenAgentsProviderPlacement(
      capableProvider,
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'payment_private',
          exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
          subjectRef: 'payment_ref.order_1_redacted',
          surface: 'payment_ref',
        }),
        requestedBackendKind: 'shc_vm',
        requiredWorkloadTrust: 'sensitive',
        workKind: 'payment_sensitive_action',
      }),
    )
    const privateMissingOwner = evaluateOpenAgentsProviderPlacement(
      capableProvider,
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'private',
          exportPolicyRefs: ['export.operator_safe.redacted_repo_summary'],
        }),
        requestedBackendKind: 'shc_vm',
        requiredWorkloadTrust: 'sensitive',
        workKind: 'private_repo',
      }),
    )
    const allowedPayment = evaluateOpenAgentsProviderPlacement(
      capableProvider,
      request({
        dataPolicy: dataPolicy({
          dataClassification: 'payment_private',
          exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
          subjectRef: 'payment_ref.order_1_redacted',
          surface: 'payment_ref',
        }),
        operatorApprovalRefs: ['operator_approval.payment_review'],
        paymentPolicyRefs: ['payment_policy.order_1'],
        requestedBackendKind: 'shc_vm',
        requiredWorkloadTrust: 'sensitive',
        workKind: 'payment_sensitive_action',
      }),
    )

    expect(legalMissingReview.blockerRefs).toEqual([
      'blocker.provider_placement.legal_review_required',
    ])
    expect(paymentMissingPolicy.blockerRefs).toEqual([
      'blocker.provider_placement.payment_policy_required',
    ])
    expect(privateMissingOwner.blockerRefs).toEqual([
      'blocker.provider_placement.owner_grant_required',
    ])
    expect(allowedPayment.allowed).toBe(true)
  })

  test('redacts non-operator projections and rejects unsafe provider placement refs', () => {
    const record = evaluateOpenAgentsProviderPlacement(
      provider({
        id: 'provider.private.shc_sensitive',
        providerEligibilityRefs: [
          'provider.eligibility.public',
          'provider.eligibility.customer_visible',
        ],
      }),
      request({
        ownerGrantRefs: ['owner_grant.customer_repo'],
        policyExceptionRefs: ['policy_exception.operator_reviewed'],
      }),
    )
    const customerProjection = projectOpenAgentsProviderPlacement(
      record,
      'customer',
    )
    const operatorProjection = projectOpenAgentsProviderPlacement(
      record,
      'operator',
    )

    expect(customerProjection.providerRef).toBe('provider.redacted')
    expect(customerProjection.policyExceptionRefs).toEqual([])
    expect(operatorProjection.providerRef).toBe(
      'provider.private.shc_sensitive',
    )
    expect(operatorProjection.policyExceptionRefs).toEqual([
      'policy_exception.operator_reviewed',
    ])
    expect(openAgentsProviderPlacementProjectionHasPrivateMaterial(
      customerProjection,
    )).toBe(false)

    expect(() =>
      evaluateOpenAgentsProviderPlacement(
        provider({ providerEligibilityRefs: ['provider_token.raw'] }),
        request(),
      ),
    ).toThrow(OpenAgentsProviderPlacementUnsafe)
    expect(() =>
      evaluateOpenAgentsProviderPlacement(
        provider(),
        request({
          dataPolicy: dataPolicy({
            evidenceRefs: ['raw_runner_log.full'],
          }),
        }),
      ),
    ).toThrow(OpenAgentsProviderPlacementUnsafe)
  })
})
