import { describe, expect, test } from 'vitest'

import {
  projectProviderAccountCredentialBoundary,
  PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_VERSION,
} from './provider-account-credential-boundary'
import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountRecord,
} from './provider-account-domain'

const account = (
  overrides: Partial<ProviderAccountRecord> = {},
): ProviderAccountRecord => ({
  id: 'provider_account_1',
  userId: 'github:1',
  teamId: null,
  provider: 'anthropic_claude',
  authMode: 'api_key',
  status: 'connected',
  health: 'healthy',
  providerAccountRef: 'provider-account_1',
  secretRef: 'provider-account://anthropic/user-api-key/provider-account_1',
  accountLabel: 'Claude work key',
  planType: null,
  connectedAt: '2026-06-11T12:00:00.000Z',
  disconnectedAt: null,
  deniedAt: null,
  lastStatusAt: '2026-06-11T12:00:00.000Z',
  metadataJson: null,
  createdAt: '2026-06-11T12:00:00.000Z',
  updatedAt: '2026-06-11T12:00:00.000Z',
  deletedAt: null,
  ...overrides,
})

const grant = (
  overrides: Partial<ProviderAccountAuthGrantRecord> = {},
): ProviderAccountAuthGrantRecord => ({
  id: 'provider_grant_1',
  providerAccountId: 'provider_account_1',
  userId: 'github:1',
  teamId: null,
  threadId: null,
  workroomId: null,
  runnerSessionId: 'runner_session_1',
  provider: 'anthropic_claude',
  providerAccountRef: 'provider-account_1',
  providerSecretRef: 'provider-account://anthropic/user-api-key/provider-account_1',
  grantRef: 'provider-auth-grant_1',
  status: 'issued',
  requestedAction: 'autopilot_coder_run',
  metadataJson: null,
  createdAt: '2026-06-11T12:00:00.000Z',
  updatedAt: '2026-06-11T12:00:00.000Z',
  expiresAt: '2026-06-11T14:00:00.000Z',
  usedAt: null,
  revokedAt: null,
  failedAt: null,
  ...overrides,
})

describe('provider account credential boundary', () => {
  const now = '2026-06-11T13:00:00.000Z'

  test('projects only safe account, credential, lease, artifact, and receipt refs', () => {
    const projection = projectProviderAccountCredentialBoundary({
      account: account(),
      activeLeaseRefs: ['provider-account-lease_ref_1'],
      artifactRefs: ['artifact.provider-account.redacted.1'],
      grant: grant(),
      now,
      receiptRefs: ['receipt.provider-account.credential-boundary.1'],
    })

    expect(projection).toEqual({
      generatedAt: now,
      boundaryVersion: PROVIDER_ACCOUNT_CREDENTIAL_BOUNDARY_VERSION,
      provider: 'anthropic_claude',
      providerAccountRef: 'provider-account_1',
      accountRef: 'providerAccount:provider-account_1',
      credentialRef:
        'provider-account://anthropic/user-api-key/provider-account_1',
      hasCredentialRef: true,
      status: 'connected',
      health: 'healthy',
      leaseAuthority: 'eligible',
      blockerRefs: [],
      reconnectActionRef: null,
      cacheInvalidationRefs: [],
      activeLeaseRefs: ['provider-account-lease_ref_1'],
      artifactRefs: ['artifact.provider-account.redacted.1'],
      receiptRefs: ['receipt.provider-account.credential-boundary.1'],
    })
    expect(JSON.stringify(projection)).not.toContain('providerSecretRef')
    expect(JSON.stringify(projection)).not.toContain('ANTHROPIC_API_KEY')
  })

  test('turns revoked or expired grants into typed blockers and cache invalidation refs', () => {
    const revoked = projectProviderAccountCredentialBoundary({
      account: account(),
      grant: grant({
        revokedAt: now,
        status: 'revoked',
        updatedAt: now,
      }),
      now,
    })

    expect(revoked).toMatchObject({
      leaseAuthority: 'blocked',
      blockerRefs: [
        'provider-account-blocker:provider-account_1:grant_status.revoked',
      ],
      reconnectActionRef: 'provider-account-reconnect:provider-account_1',
      cacheInvalidationRefs: [
        'provider-account-cache:provider-account_1',
        'provider-account-grant-cache:provider-auth-grant_1',
      ],
    })

    const expired = projectProviderAccountCredentialBoundary({
      account: account(),
      grant: grant({ expiresAt: '2026-06-11T12:59:59.000Z' }),
      now,
    })

    expect(expired.blockerRefs).toEqual([
      'provider-account-blocker:provider-account_1:grant_expired',
    ])
  })

  test('blocks stale disconnected credential state without exposing raw secrets', () => {
    const projection = projectProviderAccountCredentialBoundary({
      account: account({
        disconnectedAt: now,
        health: 'requires_reauth',
        secretRef: null,
        status: 'disconnected',
        updatedAt: now,
      }),
      activeLeaseRefs: ['provider-account-lease_ref_stale'],
      now,
    })

    expect(projection).toMatchObject({
      credentialRef: null,
      hasCredentialRef: false,
      leaseAuthority: 'blocked',
      blockerRefs: [
        'provider-account-blocker:provider-account_1:status.disconnected',
        'provider-account-blocker:provider-account_1:health.requires_reauth',
        'provider-account-blocker:provider-account_1:missing_credential_ref',
      ],
      cacheInvalidationRefs: ['provider-account-cache:provider-account_1'],
    })
    expect(JSON.stringify(projection)).not.toContain('refresh_token')
    expect(JSON.stringify(projection)).not.toContain('sk-')
  })

  test('rejects raw credential material in credential refs and joined artifacts', () => {
    expect(() =>
      projectProviderAccountCredentialBoundary({
        account: account({ secretRef: 'sk-proj-secret-value-1234567890' }),
        now,
      }),
    ).toThrow(/stable refs/)

    expect(() =>
      projectProviderAccountCredentialBoundary({
        account: account(),
        artifactRefs: ['artifact.raw.ANTHROPIC_API_KEY=secret'],
        now,
      }),
    ).toThrow(/provider credential material/)
  })
})
