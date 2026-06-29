import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY,
  OpenAgentsPolicyExceptionProjection,
  OpenAgentsPolicyExceptionReceipt,
  OpenAgentsPolicyExceptionUnsafe,
  openAgentsPolicyExceptionAppliesNow,
  openAgentsPolicyExceptionHasRuntimeAuthority,
  openAgentsPolicyExceptionIsExpired,
  openAgentsPolicyExceptionIsOverbroad,
  openAgentsPolicyExceptionIsRejected,
  openAgentsPolicyExceptionIsRevoked,
  openAgentsPolicyExceptionIsUnreviewed,
  openAgentsPolicyExceptionProjectionHasPrivateMaterial,
  projectOpenAgentsPolicyException,
} from './policy-exception-receipts'

const nowIso = '2026-06-06T23:00:00.000Z'

const receipt = (
  overrides: Partial<OpenAgentsPolicyExceptionReceipt> = {},
): OpenAgentsPolicyExceptionReceipt =>
  S.decodeUnknownSync(OpenAgentsPolicyExceptionReceipt)({
    approvedByRef: 'approved_by.operator_review',
    authority: OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY,
    blockerRefs: ['blocker.provider_placement.classification_not_allowed'],
    createdAtIso: '2026-06-06T22:45:00.000Z',
    evidenceRefs: ['evidence.operator.policy_review_summary'],
    expiresAtIso: '2026-06-07T00:00:00.000Z',
    family: 'provider_placement',
    id: 'policy_exception.provider_placement.site_revision_4',
    requestedByRef: 'requested_by.operator_chris',
    reviewState: 'approved',
    riskRefs: ['risk.policy_exception.limited_scope'],
    scopeRefs: ['scope.site_revision_4.provider_placement'],
    subjectRefs: ['subject.site_revision_4'],
    updatedAtIso: '2026-06-06T22:50:00.000Z',
    ...overrides,
  })

describe('OpenAgents policy exception receipts', () => {
  test('projects approved evidence-only exception receipts without runtime authority', () => {
    const record = receipt()
    const customerProjection = projectOpenAgentsPolicyException(
      record,
      'customer',
      nowIso,
    )
    const operatorProjection = projectOpenAgentsPolicyException(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsPolicyExceptionReceipt)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(OpenAgentsPolicyExceptionProjection)(
      customerProjection,
    )).toEqual(customerProjection)
    expect(openAgentsPolicyExceptionHasRuntimeAuthority(record)).toBe(false)
    expect(openAgentsPolicyExceptionAppliesNow(record, nowIso)).toBe(true)
    expect(customerProjection).toMatchObject({
      appliesNow: true,
      approvedByRef: null,
      createdAtDisplay: '15 minutes ago',
      evidenceRefs: [],
      expired: false,
      expiresAtDisplay: 'Active',
      runtimeAuthorityPresent: false,
      updatedAtDisplay: '10 minutes ago',
    })
    expect(operatorProjection.approvedByRef).toBe(
      'approved_by.operator_review',
    )
    expect(operatorProjection.evidenceRefs).toEqual([
      'evidence.operator.policy_review_summary',
    ])
    expect(openAgentsPolicyExceptionProjectionHasPrivateMaterial(
      customerProjection,
    )).toBe(false)
  })

  test('detects expired, rejected, revoked, unreviewed, and overbroad exceptions', () => {
    const expired = receipt({ expiresAtIso: '2026-06-06T22:00:00.000Z' })
    const rejected = receipt({ reviewState: 'rejected' })
    const revoked = receipt({ reviewState: 'revoked' })
    const unreviewed = receipt({
      approvedByRef: null,
      reviewState: 'requested',
    })
    const overbroad = receipt({ scopeRefs: ['scope.all'] })

    expect(openAgentsPolicyExceptionIsExpired(expired, nowIso)).toBe(true)
    expect(openAgentsPolicyExceptionIsRejected(rejected)).toBe(true)
    expect(openAgentsPolicyExceptionIsRevoked(revoked)).toBe(true)
    expect(openAgentsPolicyExceptionIsUnreviewed(unreviewed)).toBe(true)
    expect(openAgentsPolicyExceptionIsOverbroad(overbroad)).toBe(true)
    expect(openAgentsPolicyExceptionAppliesNow(expired, nowIso)).toBe(false)
    expect(openAgentsPolicyExceptionAppliesNow(rejected, nowIso)).toBe(false)
    expect(openAgentsPolicyExceptionAppliesNow(revoked, nowIso)).toBe(false)
    expect(openAgentsPolicyExceptionAppliesNow(unreviewed, nowIso)).toBe(false)
    expect(openAgentsPolicyExceptionAppliesNow(overbroad, nowIso)).toBe(false)
    expect(projectOpenAgentsPolicyException(expired, 'team', nowIso))
      .toMatchObject({
        appliesNow: false,
        expired: true,
        expiresAtDisplay: 'Expired',
      })
  })

  test('blocks runtime-authoritative exception receipts', () => {
    const authoritative = receipt({
      authority: {
        ...OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY,
        noDeploy: false,
        noRuntimeDispatch: false,
      },
    })

    expect(openAgentsPolicyExceptionHasRuntimeAuthority(authoritative))
      .toBe(true)
    expect(openAgentsPolicyExceptionAppliesNow(authoritative, nowIso))
      .toBe(false)
    expect(projectOpenAgentsPolicyException(authoritative, 'operator', nowIso))
      .toMatchObject({
        appliesNow: false,
        runtimeAuthorityPresent: true,
      })
  })

  test('supports every required exception family', () => {
    const families = [
      'access_control',
      'email_delivery',
      'environment_secret_policy',
      'forum_moderation',
      'legal_sensitive_rule',
      'payment_l402',
      'provider_placement',
      'public_proof',
      'research_policy',
      'site_deployment',
    ] as const

    expect(families.map(family =>
      projectOpenAgentsPolicyException(receipt({
        family,
        id: `policy_exception.${family}`,
        subjectRefs: [`subject.${family}`],
      }), 'operator', nowIso).family,
    )).toEqual(families)
  })

  test('redacts public/customer surfaces and rejects unsafe refs', () => {
    const publicProjection = projectOpenAgentsPolicyException(
      receipt({
        approvedByRef: 'approved_by.operator_private_review',
        requestedByRef: 'requested_by.operator_private_request',
        scopeRefs: [
          'scope.public_site_review',
          'scope.private.operator_only',
        ],
        subjectRefs: [
          'subject.public_site_revision',
          'subject.private.operator_only',
        ],
      }),
      'public',
      nowIso,
    )

    expect(publicProjection.approvedByRef).toBe(null)
    expect(publicProjection.requestedByRef).toBe(null)
    expect(publicProjection.scopeRefs).toEqual(['scope.public_site_review'])
    expect(publicProjection.subjectRefs).toEqual([
      'subject.public_site_revision',
    ])
    expect(openAgentsPolicyExceptionProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)

    for (const unsafeRef of [
      'raw_secret_value',
      'provider_grant.chatgpt_account',
      'wallet.mnemonic.local',
      'payment_proof.preimage',
      'raw_email.customer',
      'github.com/acme/private',
      'raw_runner_log.full',
      '2026-06-06T22:45:00.000Z',
    ]) {
      expect(() =>
        projectOpenAgentsPolicyException(
          receipt({ evidenceRefs: [unsafeRef] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsPolicyExceptionUnsafe)
    }
  })
})
