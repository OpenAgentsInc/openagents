import { describe, expect, test } from 'vitest'

import {
  buildForgeCredentialStorageInput,
  projectForgeCredentialStorage,
} from './credential-storage'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T23:30:00.000Z',
  snapshotRef: 'credential-storage-snapshot.public.work_1',
  versionRef: 'credential-storage-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge credential storage projection', () => {
  test('projects public credential readiness as refs-only non-authoritative state', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          accountRefs: ['account.public.provider_pool.openai'],
          credentialRef: 'credential.public.provider_pool.openai.default',
          entitlementRefs: ['entitlement.public.provider_pool.openai'],
          freshness: 'fresh',
          kind: 'api_key',
          leaseRefs: ['lease.public.provider_pool.openai.available'],
          policyRefs: ['policy.public.credentials.provider_pool'],
          redactionClass: 'public',
          scopeRefs: ['credential-scope.public.provider_pool.openai'],
          sessionRefs: ['session.public.provider_pool.openai'],
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.secret_store'],
          validationRefs: ['validation.public.credential.openai.ready'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      expired: 0,
      missing: 0,
      revoked: 0,
      total: 1,
      usable: 1,
    })
    expect(view.entries[0]?.credentialRef).toBe(
      'credential.public.provider_pool.openai.default',
    )
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      authenticationAuthority: false,
      credentialMintAuthority: false,
      credentialReadAuthority: false,
      credentialRefreshAuthority: false,
      credentialRevokeAuthority: false,
      credentialRotateAuthority: false,
      credentialWriteAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      providerAccountAuthority: false,
      publicClaimAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolExecutionAuthority: false,
      toolRoutingAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing credential storage state as empty', () => {
    const view = projectForgeCredentialStorage({
      generatedAt: '2026-06-17T23:30:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale credential evidence', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          credentialRef: 'credential.public.stale',
          freshness: 'stale',
          kind: 'api_key',
          policyRefs: ['policy.public.credentials.provider_pool'],
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.secret_store'],
          validationRefs: ['validation.public.credential.stale'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:stale-credential-evidence:credential.public.stale',
    )
  })

  test('blocks usable credentials without policy refs', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          credentialRef: 'credential.public.no_policy',
          freshness: 'fresh',
          kind: 'api_key',
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.secret_store'],
          validationRefs: ['validation.public.credential.no_policy'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:credential-policy-ref-missing:credential.public.no_policy',
    )
  })

  test('blocks usable credentials without validation refs', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          credentialRef: 'credential.public.no_validation',
          freshness: 'fresh',
          kind: 'api_key',
          policyRefs: ['policy.public.credentials.provider_pool'],
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.secret_store'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:credential-validation-ref-missing:credential.public.no_validation',
    )
  })

  test('blocks usable credentials without storage refs', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          credentialRef: 'credential.public.no_storage',
          freshness: 'fresh',
          kind: 'api_key',
          policyRefs: ['policy.public.credentials.provider_pool'],
          state: 'usable',
          validationRefs: ['validation.public.credential.no_storage'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:credential-storage-ref-missing:credential.public.no_storage',
    )
  })

  test('blocks revoked and expired credentials without closeout refs', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          credentialRef: 'credential.public.revoked',
          freshness: 'fresh',
          kind: 'oauth_token',
          state: 'revoked',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:credential-closeout-ref-missing:credential.public.revoked',
    )
  })

  test('blocks private and local credential refs without redaction refs', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      entries: [
        {
          credentialRef: 'credential.public.private_ref',
          freshness: 'fresh',
          kind: 'session',
          policyRefs: ['policy.public.credentials.private_ref'],
          redactionClass: 'private_ref',
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.secret_store'],
          validationRefs: ['validation.public.credential.private_ref'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:credential-redaction-ref-missing:credential.public.private_ref',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeCredentialStorage({
      generatedAt: '2026-06-17T23:30:00.000Z',
      entries: [
        {
          credentialRef: 'credential.public.no_snapshot',
          freshness: 'fresh',
          kind: 'api_key',
          policyRefs: ['policy.public.credentials.provider_pool'],
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.secret_store'],
          validationRefs: ['validation.public.credential.no_snapshot'],
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.no_snapshot:missing-credential-storage-snapshot-ref',
    )
  })

  test('omits unsafe private credential material before projection', () => {
    const view = projectForgeCredentialStorage({
      ...baseInput,
      blockerRefs: [
        'credential-blocker.public.safe',
        'raw credential /Users/christopher/.env',
      ],
      entries: [
        {
          accountRefs: ['account.public.safe', 'raw token sk-private'],
          credentialRef: 'credential.public.safe',
          entitlementRefs: ['entitlement.public.safe'],
          freshness: 'fresh',
          kind: 'api_key',
          leaseRefs: ['lease.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          redactionClass: 'private_ref',
          redactionRefs: ['redaction.public.safe'],
          scopeRefs: ['credential-scope.public.safe'],
          sessionRefs: ['session.public.safe', 'raw session /Users/christopher/session'],
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.safe'],
          validationRefs: ['validation.public.safe', 'private credential material'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.accountRefs).toEqual(['account.public.safe'])
    expect(view.entries[0]?.credentialRef).toBe('credential.public.safe')
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-credential-storage-blocker:work.public.work_1:unsafe-credential-storage-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw credential')
    expect(payload).not.toContain('raw token')
    expect(payload).not.toContain('raw session')
    expect(payload).not.toContain('private credential')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      credentialStorage: {
        entries: [
          {
            credentialRef: 'credential.public.work_2',
            freshness: 'fresh',
            kind: 'api_key',
            policyRefs: ['policy.public.work_2'],
            state: 'usable',
            storageBackendRefs: ['storage-backend.public.work_2'],
            validationRefs: ['validation.public.work_2'],
          },
        ],
        snapshotRef: 'credential-storage-snapshot.public.work_2',
        versionRef: 'credential-storage-version.public.v2',
      },
      generatedAt: '2026-06-17T23:31:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeCredentialStorageInput(work)).toEqual({
      entries: [
        {
          credentialRef: 'credential.public.work_2',
          freshness: 'fresh',
          kind: 'api_key',
          policyRefs: ['policy.public.work_2'],
          state: 'usable',
          storageBackendRefs: ['storage-backend.public.work_2'],
          validationRefs: ['validation.public.work_2'],
        },
      ],
      generatedAt: '2026-06-17T23:31:00.000Z',
      snapshotRef: 'credential-storage-snapshot.public.work_2',
      versionRef: 'credential-storage-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
