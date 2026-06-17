import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeSecurityReviewEvidenceInput,
  projectForgeSecurityReviewEvidence,
} from './security-review-evidence'

const baseInput = {
  generatedAt: '2026-06-18T05:00:00.000Z',
  snapshotRef: 'security-review-snapshot.public.work_1',
  versionRef: 'security-review-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const approvedEntry = {
  approvalGateRefs: ['security-gate.public.shell.write'],
  denialReceiptRefs: ['security-receipt.public.no_denial'],
  domain: 'shell_execution' as const,
  domainRef: 'security-domain.public.shell',
  freshness: 'fresh' as const,
  ownerPolicyRefs: ['security-policy.public.shell'],
  redactionScanRefs: ['security-redaction.public.shell'],
  regressionFixtureRefs: ['security-fixture.public.shell'],
  risk: 'high' as const,
  status: 'approved' as const,
  threatModelRefs: ['threat-model.public.shell'],
}

describe('Forge security review evidence projection', () => {
  test('projects security review evidence as refs-only non-authoritative state', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [approvedEntry],
    })

    expect(view.status).toBe('approved')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      approved: 1,
      denied: 0,
      domains: 1,
      exceptions: 0,
      highRisk: 1,
      needsReview: 0,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      approvalGrantAuthority: false,
      capabilityMutationAuthority: false,
      credentialReadAuthority: false,
      diagnosticBundleAuthority: false,
      exceptionMutationAuthority: false,
      productPromiseMutationAuthority: false,
      publicProjectionMutationAuthority: false,
      redactionScanAuthority: false,
      releaseVerificationAuthority: false,
      securityGateExecutionAuthority: false,
      settlementAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing security review evidence as empty', () => {
    const view = projectForgeSecurityReviewEvidence({
      generatedAt: '2026-06-18T05:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks high-risk domains missing review evidence', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          domain: 'remote_session_bridge',
          domainRef: 'security-domain.public.remote_bridge',
          risk: 'high',
          status: 'approved',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:high-risk-review-missing-evidence:security-domain.public.remote_bridge',
    )
  })

  test('blocks exceptions without expiry and receipt refs', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          ...approvedEntry,
          domainRef: 'security-domain.public.exception',
          exceptionExpiryRefs: [],
          exceptionRefs: ['security-exception.public.temporary'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:security-exception-missing-expiry-receipt:security-domain.public.exception',
    )
  })

  test('blocks provider credential domains without provider policy refs', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          ...approvedEntry,
          domain: 'provider_credentials',
          domainRef: 'security-domain.public.provider_credentials',
          providerCredentialPolicyRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:provider-credential-policy-missing:security-domain.public.provider_credentials',
    )
  })

  test('blocks release artifact domains without integrity refs', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          ...approvedEntry,
          domain: 'release_artifacts',
          domainRef: 'security-domain.public.release',
          releaseIntegrityRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:release-integrity-missing:security-domain.public.release',
    )
  })

  test('blocks public projection domains without scan refs', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          ...approvedEntry,
          domain: 'public_projection_claims',
          domainRef: 'security-domain.public.projection',
          publicProjectionScanRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:public-projection-scan-missing:security-domain.public.projection',
    )
  })

  test('blocks denials without denial receipt refs', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          ...approvedEntry,
          denialReceiptRefs: [],
          domainRef: 'security-domain.public.denied',
          status: 'denied',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:security-denial-receipt-missing:security-domain.public.denied',
    )
  })

  test('blocks stale security evidence', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      entries: [
        {
          ...approvedEntry,
          domainRef: 'security-domain.public.stale',
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:stale-security-review-evidence:security-domain.public.stale',
    )
  })

  test('blocks populated security review entries without snapshot refs', () => {
    const view = projectForgeSecurityReviewEvidence({
      entries: [approvedEntry],
      generatedAt: '2026-06-18T05:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.no_snapshot:missing-security-review-evidence-snapshot-ref',
    )
  })

  test('omits unsafe private security material before projection', () => {
    const view = projectForgeSecurityReviewEvidence({
      ...baseInput,
      blockerRefs: [
        'security-blocker.public.safe',
        'raw secret /Users/christopher/secret.txt',
      ],
      entries: [
        {
          ...approvedEntry,
          approvalGateRefs: ['security-gate.public.safe', 'credential value password private'],
          diagnosticBundleRefs: ['security-diagnostic.public.safe', 'diagnostic content /Users/christopher/diag.json'],
          domainRef: 'security-domain.public.safe',
          ownerPolicyRefs: ['security-policy.public.safe'],
          redactionScanRefs: ['security-redaction.public.safe', 'provider payload sk-private'],
          threatModelRefs: ['threat-model.public.safe', 'shell log bearer token private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.approvalGateRefs).toEqual(['security-gate.public.safe'])
    expect(view.entries[0]?.diagnosticBundleRefs).toEqual([
      'security-diagnostic.public.safe',
    ])
    expect(view.entries[0]?.redactionScanRefs).toEqual([
      'security-redaction.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-security-review-evidence-blocker:work.public.work_1:unsafe-security-review-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw secret')
    expect(payload).not.toContain('credential value')
    expect(payload).not.toContain('diagnostic content')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('shell log')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T05:00:00.000Z',
      securityReviewEvidence: {
        entries: [approvedEntry],
        generatedAt: '2026-06-18T05:01:00.000Z',
        snapshotRef: 'security-review-snapshot.public.work_2',
        versionRef: 'security-review-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeSecurityReviewEvidenceInput(work)).toEqual({
      entries: [approvedEntry],
      generatedAt: '2026-06-18T05:01:00.000Z',
      snapshotRef: 'security-review-snapshot.public.work_2',
      versionRef: 'security-review-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
