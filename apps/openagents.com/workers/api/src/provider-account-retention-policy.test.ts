import { describe, expect, test } from 'vitest'

import {
  PACK_B_RETENTION_DATA_CLASSES,
  PROVIDER_ACCOUNT_RETENTION_POLICY_VERSION,
  projectProviderAccountRetentionPolicy,
  type ProviderAccountRetentionDataClassPolicy,
} from './provider-account-retention-policy'

describe('provider account retention policy projection', () => {
  const policies: ReadonlyArray<ProviderAccountRetentionDataClassPolicy> =
    PACK_B_RETENTION_DATA_CLASSES.map(dataClass => ({
      auditRef: `retention-audit:pack-b:${dataClass}`,
      dataClass,
      deletionBehavior:
        dataClass === 'credential'
          ? 'revoke'
          : dataClass === 'receipt'
            ? 'retain_ref_only'
            : 'redact_and_tombstone',
      projectionInvalidation:
        dataClass === 'receipt' ? 'retain_audit_ref' : 'invalidate_cache',
      retentionClass:
        dataClass === 'credential'
          ? 'ephemeral'
          : dataClass === 'receipt'
            ? 'receipt'
            : dataClass === 'debug_support_record'
              ? 'short'
              : 'standard',
      ttlDays:
        dataClass === 'credential'
          ? null
          : dataClass === 'receipt'
            ? 2_555
            : 90,
    }))

  test('declares retention, deletion, and projection behavior for all Pack B data classes', () => {
    const projection = projectProviderAccountRetentionPolicy({
      affectedArtifactRefs: ['artifact:provider-account:redacted-pack-b'],
      affectedReceiptRefs: ['receipt:provider-account:redacted-pack-b'],
      affectedProjectionRefs: ['projection:provider-account:policy-snapshot'],
      dataClassPolicies: policies,
      deletionReceiptRefs: ['deletion-receipt:provider-account:user-delete'],
      generatedAt: '2026-06-11T17:00:00.000Z',
      policyRef: 'provider-account-retention-policy.pack-b',
      reason: 'user_deleted',
      retainedAuditRefs: ['audit:provider-account:user-delete'],
      subjectRef: 'user:usr_pack_b',
      subjectType: 'user',
      tombstoneRefs: ['tombstone:provider-account:user-delete'],
    })

    expect(projection).toMatchObject({
      generatedAt: '2026-06-11T17:00:00.000Z',
      retentionVersion: PROVIDER_ACCOUNT_RETENTION_POLICY_VERSION,
      policyRef: 'provider-account-retention-policy.pack-b',
      subjectRef: 'user:usr_pack_b',
      subjectType: 'user',
      reason: 'user_deleted',
      status: 'declared',
      missingDataClassRefs: [],
      cacheInvalidationRefs: [
        'provider-account-projection-cache:projection:provider-account:policy-snapshot',
      ],
      tombstoneRefs: ['tombstone:provider-account:user-delete'],
      deletionReceiptRefs: ['deletion-receipt:provider-account:user-delete'],
      retainedAuditRefs: ['audit:provider-account:user-delete'],
      affectedArtifactRefs: ['artifact:provider-account:redacted-pack-b'],
      affectedReceiptRefs: ['receipt:provider-account:redacted-pack-b'],
    })
    expect(projection.dataClassPolicies.map(policy => policy.dataClass)).toEqual(
      [...PACK_B_RETENTION_DATA_CLASSES].sort(),
    )
  })

  test('credential revocation invalidates dependent leases and reconnects account state', () => {
    const projection = projectProviderAccountRetentionPolicy({
      affectedLeaseRefs: ['lease:run-1', 'lease:run-2'],
      affectedProjectionRefs: [
        'projection:credential-boundary:acct-1',
        'projection:telemetry:acct-1',
      ],
      dataClassPolicies: policies,
      deletionReceiptRefs: ['deletion-receipt:credential:acct-1'],
      generatedAt: '2026-06-11T17:01:00.000Z',
      policyRef: 'provider-account-retention-policy.pack-b',
      providerAccountRef: 'acct:provider-account-1',
      reason: 'credential_revoked',
      retainedAuditRefs: ['audit:credential:revoked'],
      subjectRef: 'credential:acct-1',
      subjectType: 'credential',
      tombstoneRefs: ['tombstone:credential:acct-1'],
    })

    expect(projection).toMatchObject({
      status: 'declared',
      leaseInvalidationRefs: [
        'provider-account-lease-invalidation:lease:run-1',
        'provider-account-lease-invalidation:lease:run-2',
      ],
      cacheInvalidationRefs: [
        'provider-account-projection-cache:projection:credential-boundary:acct-1',
        'provider-account-projection-cache:projection:telemetry:acct-1',
        'provider-account-cache:acct:provider-account-1',
      ],
      dependentBlockerRefs: [
        'provider-account-retention-dependent-blocker:lease:run-1:credential_revoked',
        'provider-account-retention-dependent-blocker:lease:run-2:credential_revoked',
      ],
      reconnectActionRef: 'provider-account-reconnect:acct:provider-account-1',
    })
  })

  test('blocks incomplete retention declarations', () => {
    const projection = projectProviderAccountRetentionPolicy({
      dataClassPolicies: policies.filter(policy => policy.dataClass !== 'artifact'),
      generatedAt: '2026-06-11T17:02:00.000Z',
      policyRef: 'provider-account-retention-policy.pack-b.partial',
      reason: 'retention_expired',
      subjectRef: 'team:tm_pack_b',
      subjectType: 'team',
    })

    expect(projection.status).toBe('blocked')
    expect(projection.missingDataClassRefs).toEqual([
      'provider-account-retention-blocker:provider-account-retention-policy.pack-b.partial:missing:artifact',
    ])
  })

  test('covers projection and cache invalidation without lease blockers for retention expiry', () => {
    const projection = projectProviderAccountRetentionPolicy({
      affectedProjectionRefs: ['projection:provider-routing:expired-window'],
      dataClassPolicies: policies,
      deletionReceiptRefs: ['deletion-receipt:telemetry:expired-window'],
      generatedAt: '2026-06-11T17:03:00.000Z',
      policyRef: 'provider-account-retention-policy.pack-b.expiry',
      reason: 'retention_expired',
      subjectRef: 'telemetry-window:2026-06-01',
      subjectType: 'provider_account',
      tombstoneRefs: ['tombstone:telemetry:expired-window'],
    })

    expect(projection).toMatchObject({
      status: 'declared',
      cacheInvalidationRefs: [
        'provider-account-projection-cache:projection:provider-routing:expired-window',
      ],
      dependentBlockerRefs: [],
      leaseInvalidationRefs: [],
      reconnectActionRef: null,
    })
  })

  test('rejects raw credentials, raw prompts, private repo data, and raw provider responses', () => {
    expect(() =>
      projectProviderAccountRetentionPolicy({
        dataClassPolicies: policies,
        generatedAt: '2026-06-11T17:04:00.000Z',
        policyRef: 'provider-account-retention-policy.pack-b',
        reason: 'account_deleted',
        subjectRef: 'credential:ANTHROPIC_API_KEY=secret',
        subjectType: 'credential',
      }),
    ).toThrow(/provider credential material/)

    expect(() =>
      projectProviderAccountRetentionPolicy({
        dataClassPolicies: policies,
        generatedAt: '2026-06-11T17:05:00.000Z',
        policyRef: 'provider-account-retention-policy.pack-b',
        reason: 'account_deleted',
        subjectRef: 'provider-account:acct-1',
        subjectType: 'provider_account',
        tombstoneRefs: ['tombstone:raw prompt: customer code'],
      }),
    ).toThrow(/private retention material/)

    expect(() =>
      projectProviderAccountRetentionPolicy({
        dataClassPolicies: policies,
        generatedAt: '2026-06-11T17:06:00.000Z',
        policyRef: 'provider-account-retention-policy.pack-b',
        reason: 'account_deleted',
        retainedAuditRefs: ['git@github.com:OpenAgentsInc/private-repo.git'],
        subjectRef: 'provider-account:acct-1',
        subjectType: 'provider_account',
      }),
    ).toThrow(/private retention material/)

    expect(() =>
      projectProviderAccountRetentionPolicy({
        dataClassPolicies: policies,
        deletionReceiptRefs: ['deletion-receipt:raw provider response: choices'],
        generatedAt: '2026-06-11T17:07:00.000Z',
        policyRef: 'provider-account-retention-policy.pack-b',
        reason: 'account_deleted',
        subjectRef: 'provider-account:acct-1',
        subjectType: 'provider_account',
      }),
    ).toThrow(/private retention material/)
  })
})
