import { describe, expect, test } from 'vitest'

import {
  type ProviderAccountLeaseCandidate,
  selectProviderAccountLeaseCandidate,
} from './provider-account-lease-policy'

const candidate = (
  providerAccountRef: string,
  overrides: Partial<ProviderAccountLeaseCandidate> = {},
): ProviderAccountLeaseCandidate => ({
  providerAccountRef,
  provider: 'chatgpt_codex',
  status: 'connected',
  health: 'healthy',
  hasSecretRef: true,
  activeLeaseCount: 0,
  leaseLimit: 1,
  operatorPriority: 100,
  connectedAt: '2026-06-05T00:00:00.000Z',
  createdAt: '2026-06-05T00:00:00.000Z',
  lastSelectedAt: null,
  lastSanityCheckAt: null,
  lastSanityCheckResult: null,
  lastParallelProbeAt: null,
  recentFailureClass: null,
  cooldownUntil: null,
  lowCredit: false,
  ...overrides,
})

describe('provider account lease selection policy', () => {
  const now = '2026-06-05T12:00:00.000Z'

  test('excludes disconnected, reauth, cooldown, low-credit, and secretless accounts', () => {
    const result = selectProviderAccountLeaseCandidate(
      [
        candidate('provider-account_ref_disconnected', {
          status: 'disconnected',
        }),
        candidate('provider-account_ref_reauth', {
          health: 'requires_reauth',
        }),
        candidate('provider-account_ref_cooldown', {
          cooldownUntil: '2026-06-05T13:00:00.000Z',
        }),
        candidate('provider-account_ref_low_credit', {
          lowCredit: true,
        }),
        candidate('provider-account_ref_secretless', {
          hasSecretRef: false,
        }),
        candidate('provider-account_ref_at_limit', {
          activeLeaseCount: 1,
          leaseLimit: 1,
        }),
      ],
      now,
    )

    expect(result).toMatchObject({
      status: 'none',
    })
  })

  test('prefers no active leases before operator priority', () => {
    const result = selectProviderAccountLeaseCandidate(
      [
        candidate('provider-account_ref_busy_priority', {
          activeLeaseCount: 1,
          operatorPriority: 1,
        }),
        candidate('provider-account_ref_free_later_priority', {
          activeLeaseCount: 0,
          operatorPriority: 100,
        }),
      ],
      now,
    )

    expect(result).toMatchObject({
      status: 'selected',
      candidate: {
        providerAccountRef: 'provider-account_ref_free_later_priority',
      },
    })
  })

  test('selects provider-tagged candidates when a required provider is set', () => {
    const candidates = [
      candidate('provider-account_ref_codex', {
        provider: 'chatgpt_codex',
      }),
      candidate('provider-account_ref_anthropic', {
        provider: 'anthropic_claude',
      }),
      candidate('provider-account_ref_gemini', {
        provider: 'google_gemini',
      }),
    ]

    const anthropicResult = selectProviderAccountLeaseCandidate(
      candidates,
      now,
      { requiredProvider: 'anthropic_claude' },
    )

    expect(anthropicResult).toMatchObject({
      status: 'selected',
      candidate: {
        provider: 'anthropic_claude',
        providerAccountRef: 'provider-account_ref_anthropic',
      },
    })

    const geminiOnlyCodexPool = selectProviderAccountLeaseCandidate(
      [candidate('provider-account_ref_codex', { provider: 'chatgpt_codex' })],
      now,
      { requiredProvider: 'google_gemini' },
    )

    expect(geminiOnlyCodexPool).toMatchObject({
      status: 'none',
      reason:
        'No connected healthy google_gemini account is currently eligible for lease.',
    })
  })

  test('uses priority then oldest use as deterministic tie breakers', () => {
    const priorityResult = selectProviderAccountLeaseCandidate(
      [
        candidate('provider-account_ref_low_priority', {
          operatorPriority: 50,
        }),
        candidate('provider-account_ref_high_priority', {
          operatorPriority: 10,
        }),
      ],
      now,
    )

    expect(priorityResult).toMatchObject({
      status: 'selected',
      candidate: {
        providerAccountRef: 'provider-account_ref_high_priority',
      },
    })

    const recencyResult = selectProviderAccountLeaseCandidate(
      [
        candidate('provider-account_ref_recent', {
          lastSelectedAt: '2026-06-05T11:00:00.000Z',
        }),
        candidate('provider-account_ref_oldest', {
          lastSelectedAt: '2026-06-05T10:00:00.000Z',
        }),
      ],
      now,
    )

    expect(recencyResult).toMatchObject({
      status: 'selected',
      candidate: {
        providerAccountRef: 'provider-account_ref_oldest',
      },
    })
  })
})
