import { describe, expect, test } from 'vitest'

import {
  PROVIDER_ACCOUNT_SECURITY_REVIEW_VERSION,
  reviewProviderAccountSecurityGate,
} from './provider-account-security-review'

describe('provider account security review gate', () => {
  const base = {
    credentialBoundaryRef: 'pack-b.pb1.credential_boundary',
    generatedAt: '2026-06-11T15:00:00.000Z',
    provider: 'anthropic_claude' as const,
    redactionFixtureRefs: ['fixture.provider-account.redaction.pack-b'],
    retentionPolicyRef: 'pack-b.pb5.retention.policy',
    reviewRef: 'provider-peer-security-review.anthropic.pack-b',
    revocationFixtureRefs: ['fixture.provider-account.revocation.pack-b'],
    scope: 'provider_peer_expansion' as const,
    telemetryPrivacyRef: 'pack-b.pb4.telemetry.privacy',
    threatModelRef: 'threat-model.provider-peer.anthropic.pack-b',
    tosReviewRef:
      'docs.autopilot-coder.2026-06-11-provider-peer-tos-compliance-review',
  }

  test('approves provider peer expansion only with required review refs', () => {
    const projection = reviewProviderAccountSecurityGate(base)

    expect(projection).toEqual({
      generatedAt: '2026-06-11T15:00:00.000Z',
      reviewVersion: PROVIDER_ACCOUNT_SECURITY_REVIEW_VERSION,
      reviewRef: 'provider-peer-security-review.anthropic.pack-b',
      provider: 'anthropic_claude',
      scope: 'provider_peer_expansion',
      status: 'approved',
      tosReviewRef:
        'docs.autopilot-coder.2026-06-11-provider-peer-tos-compliance-review',
      credentialBoundaryRef: 'pack-b.pb1.credential_boundary',
      threatModelRef: 'threat-model.provider-peer.anthropic.pack-b',
      telemetryPrivacyRef: 'pack-b.pb4.telemetry.privacy',
      retentionPolicyRef: 'pack-b.pb5.retention.policy',
      redactionFixtureRefs: ['fixture.provider-account.redaction.pack-b'],
      revocationFixtureRefs: ['fixture.provider-account.revocation.pack-b'],
      highRiskControlRefs: [],
      scopedExceptionRef: null,
      blockerRefs: [],
    })
  })

  test('blocks missing ToS, threat, telemetry, retention, and fixture refs', () => {
    const projection = reviewProviderAccountSecurityGate({
      generatedAt: base.generatedAt,
      provider: 'google_gemini',
      reviewRef: 'provider-peer-security-review.gemini.pack-b',
      scope: 'provider_peer_expansion',
    })

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:tos_review',
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:credential_boundary',
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:threat_model',
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:telemetry_privacy',
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:retention_policy',
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:redaction_fixture',
      'provider-account-security-blocker:provider-peer-security-review.gemini.pack-b:missing:revocation_fixture',
    ])
  })

  test('requires explicit high-risk control refs', () => {
    const projection = reviewProviderAccountSecurityGate({
      ...base,
      approvalRef: 'approval.provider-peer.anthropic.pack-b',
      denialRef: 'denial.provider-peer.anthropic.pack-b',
      highRiskFlow: true,
    })

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'provider-account-security-blocker:provider-peer-security-review.anthropic.pack-b:missing:rollback_ref',
      'provider-account-security-blocker:provider-peer-security-review.anthropic.pack-b:missing:incident_boundary',
      'provider-account-security-blocker:provider-peer-security-review.anthropic.pack-b:missing:debug_boundary',
    ])
  })

  test('supports an explicit scoped exception without erasing blockers', () => {
    const projection = reviewProviderAccountSecurityGate({
      generatedAt: base.generatedAt,
      provider: 'chatgpt_codex',
      reviewRef: 'provider-peer-security-review.codex.scoped-exception',
      scope: 'lease_selection',
      scopedExceptionRef: 'security-exception.codex.existing-lease-slice',
    })

    expect(projection).toMatchObject({
      status: 'scoped_exception',
      scopedExceptionRef: 'security-exception.codex.existing-lease-slice',
    })
    expect(projection.blockerRefs.length).toBeGreaterThan(0)
  })

  test('rejects raw credential material in review refs and fixture refs', () => {
    expect(() =>
      reviewProviderAccountSecurityGate({
        ...base,
        redactionFixtureRefs: ['fixture.raw.ANTHROPIC_API_KEY=secret'],
      }),
    ).toThrow(/provider credential material/)

    expect(() =>
      reviewProviderAccountSecurityGate({
        ...base,
        threatModelRef: 'threat-model.sk-proj-secret-value-1234567890',
      }),
    ).toThrow(/provider credential material/)
  })
})
