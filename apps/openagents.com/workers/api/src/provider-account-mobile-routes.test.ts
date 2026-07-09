import { describe, expect, test } from 'vitest'

import {
  type ProviderAccountBundle,
  type ProviderAccountProvider,
  type ProviderAccountStatus,
  type PublicProviderAccount,
  filterMobileProviderAccountBundleForProvider,
  filterMobileVisibleProviderAccountBundle,
  isMobileVisibleProviderAccount,
} from './provider-account-domain'

const makePublicAccount = (
  providerAccountRef: string,
  status: ProviderAccountStatus,
  publicStatus: ProviderAccountStatus = status,
  provider: ProviderAccountProvider = 'chatgpt_codex',
): PublicProviderAccount => ({
  id: `id_${providerAccountRef}`,
  provider,
  authMode:
    provider === 'anthropic_claude' ? 'claude_local_auth' : 'chatgpt_device_code',
  status,
  publicStatus,
  health: status === 'connected' ? 'healthy' : 'requires_reauth',
  providerAccountRef,
  hasSecretRef: status === 'connected',
  lastStatusAt: '2026-07-09T00:00:00.000Z',
  createdAt: '2026-07-09T00:00:00.000Z',
  updatedAt: '2026-07-09T00:00:00.000Z',
})

describe('mobile codex accounts list projection (#8546)', () => {
  test('isMobileVisibleProviderAccount keeps only connected and in-progress pending', () => {
    expect(
      isMobileVisibleProviderAccount(makePublicAccount('a', 'connected')),
    ).toBe(true)
    expect(
      isMobileVisibleProviderAccount(makePublicAccount('b', 'pending')),
    ).toBe(true)

    // A pending row whose device codes all expired projects publicStatus
    // 'expired' and must be hidden even though its raw status is 'pending'.
    expect(
      isMobileVisibleProviderAccount(
        makePublicAccount('c', 'pending', 'expired'),
      ),
    ).toBe(false)

    for (const dead of ['disconnected', 'denied', 'expired', 'unhealthy'] as const) {
      expect(
        isMobileVisibleProviderAccount(makePublicAccount(`dead_${dead}`, dead)),
      ).toBe(false)
    }
  })

  test('filterMobileVisibleProviderAccountBundle drops dead accounts and their attempts', () => {
    const bundle: ProviderAccountBundle = {
      accounts: [
        makePublicAccount('live-connected', 'connected'),
        makePublicAccount('live-pending', 'pending'),
        makePublicAccount('dead-disconnected', 'disconnected'),
        makePublicAccount('dead-expired', 'pending', 'expired'),
        makePublicAccount('dead-denied', 'denied'),
      ],
      attempts: [
        {
          id: 'attempt_live_pending',
          providerAccountId: 'id_live-pending',
          providerAccountRef: 'live-pending',
          provider: 'chatgpt_codex',
          method: 'chatgpt_device_code',
          source: 'worker_device_code',
          status: 'pending',
          expiresAt: '2026-07-09T00:15:00.000Z',
          createdAt: '2026-07-09T00:00:00.000Z',
          updatedAt: '2026-07-09T00:00:00.000Z',
        },
        {
          id: 'attempt_dead_expired',
          providerAccountId: 'id_dead-expired',
          providerAccountRef: 'dead-expired',
          provider: 'chatgpt_codex',
          method: 'chatgpt_device_code',
          source: 'worker_device_code',
          status: 'expired',
          expiresAt: '2026-07-08T00:15:00.000Z',
          createdAt: '2026-07-08T00:00:00.000Z',
          updatedAt: '2026-07-08T00:00:00.000Z',
        },
      ],
    }

    const filtered = filterMobileVisibleProviderAccountBundle(bundle)

    expect(filtered.accounts.map(account => account.providerAccountRef)).toEqual([
      'live-connected',
      'live-pending',
    ])
    // Only attempts referencing a still-visible account survive.
    expect(filtered.attempts.map(attempt => attempt.providerAccountRef)).toEqual([
      'live-pending',
    ])
  })

  test('a connected account that needs reauth is still shown (not stale residue)', () => {
    const reauth = makePublicAccount('needs-reauth', 'connected')
    expect(isMobileVisibleProviderAccount(reauth)).toBe(true)
  })
})

// CX-5 (#8549): provider-scoped mobile projection so Codex and Claude never
// cross-render in each other's Settings section.
describe('mobile Claude/Codex provider-scoped list projection (CX-5 #8549)', () => {
  test('filterMobileProviderAccountBundleForProvider keeps only the requested provider', () => {
    const bundle: ProviderAccountBundle = {
      accounts: [
        makePublicAccount('codex-live', 'connected', 'connected', 'chatgpt_codex'),
        makePublicAccount(
          'claude-live',
          'connected',
          'connected',
          'anthropic_claude',
        ),
        makePublicAccount(
          'claude-dead',
          'disconnected',
          'disconnected',
          'anthropic_claude',
        ),
      ],
      attempts: [
        {
          id: 'attempt_claude',
          providerAccountId: 'id_claude-live',
          providerAccountRef: 'claude-live',
          provider: 'anthropic_claude',
          method: 'claude_local_auth',
          source: 'pylon_local_claude_auth',
          status: 'connected',
          expiresAt: '2026-07-09T00:15:00.000Z',
          createdAt: '2026-07-09T00:00:00.000Z',
          updatedAt: '2026-07-09T00:00:00.000Z',
        },
        {
          id: 'attempt_codex',
          providerAccountId: 'id_codex-live',
          providerAccountRef: 'codex-live',
          provider: 'chatgpt_codex',
          method: 'chatgpt_device_code',
          source: 'worker_device_code',
          status: 'pending',
          expiresAt: '2026-07-09T00:15:00.000Z',
          createdAt: '2026-07-09T00:00:00.000Z',
          updatedAt: '2026-07-09T00:00:00.000Z',
        },
      ],
    }

    const claude = filterMobileProviderAccountBundleForProvider(
      bundle,
      'anthropic_claude',
    )
    expect(claude.accounts.map(account => account.providerAccountRef)).toEqual([
      'claude-live',
    ])
    expect(claude.attempts.map(attempt => attempt.providerAccountRef)).toEqual([
      'claude-live',
    ])

    const codex = filterMobileProviderAccountBundleForProvider(
      bundle,
      'chatgpt_codex',
    )
    expect(codex.accounts.map(account => account.providerAccountRef)).toEqual([
      'codex-live',
    ])
    expect(codex.attempts.map(attempt => attempt.providerAccountRef)).toEqual([
      'codex-live',
    ])
  })
})
