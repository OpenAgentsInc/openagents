import { describe, expect, test } from 'vitest'

import {
  type ProviderAccountFailoverFailureClass,
  classifyProviderAccountFailover,
  classifyProviderAccountHealthEvent,
} from './provider-account-failover-policy'

describe('provider account failover policy', () => {
  const now = '2026-06-05T12:00:00.000Z'

  test.each<
    [ProviderAccountFailoverFailureClass, string, string | null, boolean]
  >([
    ['token_invalidated', 'requires_reauth', 'requires_reauth', false],
    ['low_credits', 'low_credit_cooldown', 'unhealthy', true],
    ['quota_exhausted', 'low_credit_cooldown', 'unhealthy', true],
    ['rate_limited', 'timed_cooldown', 'unhealthy', false],
    ['provider_outage', 'provider_outage_cooldown', 'unhealthy', false],
    ['launch_timeout', 'timed_cooldown', 'unhealthy', false],
    ['grant_resolution_failed', 'grant_path_failure', null, false],
    ['runner_failure', 'do_not_poison_account', null, false],
    [
      'unknown_provider_failure',
      'unknown_failure_cooldown',
      'unhealthy',
      false,
    ],
  ])(
    'classifies %s without exposing private provider details',
    (failureClass, accountStateAction, health, lowCredit) => {
      const action = classifyProviderAccountFailover(failureClass, now)

      expect(action).toMatchObject({
        failureClass,
        accountStateAction,
        health,
        lowCredit,
        retryAllowed: true,
      })
      expect(action.customerSafeStatus).not.toContain('provider-account_ref')
      expect(action.customerSafeStatus).not.toContain('codex-auth')
    },
  )

  test('uses temporary cooldowns for retryable transient failures', () => {
    expect(
      classifyProviderAccountFailover('rate_limited', now).cooldownUntil,
    ).toBe('2026-06-05T13:00:00.000Z')
    expect(
      classifyProviderAccountFailover('provider_outage', now).cooldownUntil,
    ).toBe('2026-06-05T12:10:00.000Z')
  })

  test.each([
    [{ sanityClassification: 'healthy' }, 'healthy', false, false],
    [
      { sanityClassification: 'requires_reauth' },
      'requires_reauth',
      true,
      true,
    ],
    [{ sanityClassification: 'low_credit' }, 'low_credits', true, true],
    [{ code: 'token_revoked' }, 'token_invalidated', true, true],
    [{ code: 'rate_limit' }, 'rate_limited', true, true],
    [{ code: 'quota_exhausted' }, 'quota_exhausted', true, true],
    [{ providerStatus: 503 }, 'provider_outage', true, true],
    [{ code: 'timeout' }, 'launch_timeout', true, true],
    [
      { code: 'grant_resolution_failed' },
      'grant_resolution_failed',
      true,
      false,
    ],
    [{ code: 'runner_failure' }, 'runner_failure', true, false],
    [
      { collisionClass: 'wrong_account_identity' },
      'wrong_account_or_collision',
      true,
      true,
    ],
    [{ code: 'unrecognized_private_error' }, 'unknown_failure', true, true],
  ])(
    'normalizes health event %j to %s',
    (input, classification, retryAnotherAccount, poisonAccount) => {
      const result = classifyProviderAccountHealthEvent(input, now)

      expect(result).toMatchObject({
        classification,
        retryAnotherAccount,
        poisonAccount,
      })
      expect(result.operatorSummary).not.toContain('unrecognized_private_error')
      expect(result.operatorSummary).not.toContain('access-token')
      expect(result.customerSafeSummary ?? '').not.toContain('codex-auth')
    },
  )
})
