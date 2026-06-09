import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniDataPolicyEnvelope,
  OmniDataPolicyProjection,
  OmniDataClassificationValidationError,
  assertOmniClassifiedProjectionAllowed,
  canProjectOmniClassifiedRecord,
  isOmniClassificationDowngrade,
  isOmniClassificationMoreRestrictive,
  omniDataPolicyExportAllowed,
  omniDataPolicyProjectionDecision,
  omniRequiredProviderEligibilityRefs,
  omniClassificationProjection,
  projectOmniDataPolicyEnvelope,
  validateOmniClassificationTransition,
} from './omni-data-classification'

const policyEnvelope = (
  overrides: Partial<OmniDataPolicyEnvelope> = {},
): OmniDataPolicyEnvelope =>
  S.decodeUnknownSync(OmniDataPolicyEnvelope)({
    classificationCaveatRef: 'classification.caveat.customer_safe',
    dataClassification: 'customer',
    evidenceRefs: ['evidence.order.request_summary'],
    exportPolicyRefs: ['export.customer_safe'],
    providerEligibilityRefs: [],
    redactionPolicyRefs: ['redaction.customer_summary_available'],
    retentionPolicyRefs: ['retention.standard_order'],
    subjectRef: 'order.customer_request_1',
    surface: 'order',
    trustTier: 'reviewed',
    ...overrides,
  })

describe('Omni data classification', () => {
  test('allows safe upgrades and detects downgrades', () => {
    expect(isOmniClassificationMoreRestrictive('private', 'customer')).toBe(true)
    expect(isOmniClassificationDowngrade('customer', 'private')).toBe(true)
    expect(() =>
      validateOmniClassificationTransition('customer', 'private'),
    ).not.toThrow()
  })

  test('requires redaction evidence before legal, payment, or provider downgrade', () => {
    expect(() =>
      validateOmniClassificationTransition('legal_sensitive', 'customer'),
    ).toThrow(OmniDataClassificationValidationError)
    expect(() =>
      validateOmniClassificationTransition(
        'payment_private',
        'operator',
        'redaction_report_payment_safe',
      ),
    ).not.toThrow()
    expect(() =>
      validateOmniClassificationTransition(
        'provider_private',
        'operator',
        'redaction_report_provider_safe',
      ),
    ).not.toThrow()
  })

  test('never downgrades secret-bearing data', () => {
    expect(() =>
      validateOmniClassificationTransition(
        'secret_bearing',
        'private',
        'redaction_report_secret_removed',
      ),
    ).toThrow(OmniDataClassificationValidationError)
  })

  test('enforces projection visibility and trust tier', () => {
    const publicRecord = {
      classificationCaveatRef: 'classification_caveat_public_reviewed',
      dataClassification: 'public' as const,
      trustTier: 'reviewed' as const,
    }
    const paymentRecord = {
      classificationCaveatRef: 'classification_caveat_payment_private',
      dataClassification: 'payment_private' as const,
      trustTier: 'reviewed' as const,
    }
    const blockedRecord = {
      classificationCaveatRef: 'classification_caveat_blocked',
      dataClassification: 'public' as const,
      trustTier: 'blocked' as const,
    }

    expect(canProjectOmniClassifiedRecord(publicRecord, 'public')).toBe(true)
    expect(canProjectOmniClassifiedRecord(paymentRecord, 'customer')).toBe(false)
    expect(canProjectOmniClassifiedRecord(paymentRecord, 'operator')).toBe(true)
    expect(canProjectOmniClassifiedRecord(blockedRecord, 'public')).toBe(false)
    expect(omniClassificationProjection(publicRecord, 'public')).toEqual(
      publicRecord,
    )
    expect(() =>
      assertOmniClassifiedProjectionAllowed(paymentRecord, 'customer'),
    ).toThrow(OmniDataClassificationValidationError)
  })

  test('maps classification and audience into allow, redact, omit, and deny decisions', () => {
    const publicRecord = policyEnvelope({
      dataClassification: 'public',
      redactionPolicyRefs: [],
      subjectRef: 'site.public_otec',
      surface: 'site',
    })
    const customerRecord = policyEnvelope()
    const teamRecord = policyEnvelope({
      dataClassification: 'team',
      redactionPolicyRefs: [],
      subjectRef: 'workroom.team_status',
      surface: 'workroom',
    })
    const secretRecord = policyEnvelope({
      dataClassification: 'secret_bearing',
      redactionPolicyRefs: ['redaction.binding_summary_available'],
      subjectRef: 'provider_account.binding_ref',
      surface: 'provider_account',
    })

    expect(omniDataPolicyProjectionDecision(publicRecord, 'agent')).toBe('allow')
    expect(omniDataPolicyProjectionDecision(customerRecord, 'public'))
      .toBe('redact')
    expect(omniDataPolicyProjectionDecision(teamRecord, 'customer')).toBe('omit')
    expect(omniDataPolicyProjectionDecision(secretRecord, 'operator'))
      .toBe('deny')
    expect(omniDataPolicyProjectionDecision(secretRecord, 'private'))
      .toBe('allow')
  })

  test('projects policy envelopes with classification, retention, export, and provider refs', () => {
    const customerProjection = projectOmniDataPolicyEnvelope(
      policyEnvelope(),
      'customer',
    )
    const publicProjection = projectOmniDataPolicyEnvelope(
      policyEnvelope(),
      'public',
    )
    const omittedProjection = projectOmniDataPolicyEnvelope(
      policyEnvelope({
        dataClassification: 'team',
        redactionPolicyRefs: [],
        subjectRef: 'task_packet.team_only',
        surface: 'task_packet',
      }),
      'customer',
    )

    expect(S.decodeUnknownSync(OmniDataPolicyProjection)(customerProjection))
      .toEqual(customerProjection)
    expect(customerProjection).toMatchObject({
      decision: 'allow',
      exportAllowed: true,
      subjectRef: 'order.customer_request_1',
    })
    expect(customerProjection.providerEligibilityRefs).toEqual([
      'provider.eligibility.customer_visible',
    ])
    expect(publicProjection).toMatchObject({
      decision: 'redact',
      exportAllowed: false,
      subjectRef: 'order.redacted',
    })
    expect(publicProjection.evidenceRefs).toEqual([])
    expect(omittedProjection).toMatchObject({
      decision: 'omit',
      subjectRef: null,
    })
  })

  test('requires explicit sensitive export policy and blocks deletion-sensitive export', () => {
    const payment = policyEnvelope({
      dataClassification: 'payment_private',
      exportPolicyRefs: [],
      redactionPolicyRefs: [],
      subjectRef: 'payment_ref.order_1_redacted',
      surface: 'payment_ref',
    })
    const paymentWithExportPolicy = policyEnvelope({
      dataClassification: 'payment_private',
      exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
      redactionPolicyRefs: [],
      subjectRef: 'payment_ref.order_1_redacted',
      surface: 'payment_ref',
    })
    const deleteRequested = policyEnvelope({
      dataClassification: 'deletion_retention_sensitive',
      exportPolicyRefs: ['export.operator_safe.redacted_deletion_summary'],
      retentionPolicyRefs: ['retention.delete_requested'],
      subjectRef: 'customer_asset.delete_requested_1',
      surface: 'customer_asset',
    })

    expect(omniDataPolicyExportAllowed(payment, 'operator')).toBe(false)
    expect(omniDataPolicyExportAllowed(paymentWithExportPolicy, 'operator'))
      .toBe(true)
    expect(omniDataPolicyExportAllowed(deleteRequested, 'private')).toBe(false)
  })

  test('returns provider eligibility refs by classification for later placement policy', () => {
    expect(omniRequiredProviderEligibilityRefs(policyEnvelope({
      dataClassification: 'public',
    }))).toEqual(['provider.eligibility.public'])
    expect(omniRequiredProviderEligibilityRefs(policyEnvelope({
      dataClassification: 'legal_sensitive',
      providerEligibilityRefs: ['provider.eligibility.owner_approved'],
    }))).toEqual([
      'provider.eligibility.legal_sensitive',
      'provider.eligibility.owner_approved',
    ])
    expect(omniRequiredProviderEligibilityRefs(policyEnvelope({
      dataClassification: 'secret_bearing',
    }))).toEqual(['provider.eligibility.no_external_provider'])
  })

  test('rejects private customer data, provider grants, payment proofs, raw logs, private repos, source archives, and raw timestamps', () => {
    for (const unsafeRef of [
      'ben@example.com',
      'provider_grant.chatgpt_account_1',
      'callback_token.oauth',
      'wallet.mnemonic.local',
      'payment_proof.raw_preimage',
      'raw_runner_log.full',
      'github.com/acme/private',
      'raw_source_archive.customer_zip',
      '2026-06-06T22:45:00.000Z',
    ]) {
      expect(() =>
        projectOmniDataPolicyEnvelope(
          policyEnvelope({ evidenceRefs: [unsafeRef] }),
          'public',
        ),
      ).toThrow(OmniDataClassificationValidationError)
    }
  })
})
