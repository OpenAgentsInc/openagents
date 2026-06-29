import { describe, expect, test } from 'vitest'

import {
  ProviderAccountClientRequestFailed,
  ProviderAccountCredentialMaterial,
  ProviderAccountNotConnectedHealthy,
  ProviderAccountNotFound,
  ProviderAccountStorageFailed,
  ProviderDeviceLoginAttemptExpired,
  ProviderGrantExpired,
  ProviderGrantRunnerSessionMismatch,
} from './provider-account-errors'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'

describe('provider account route errors', () => {
  test('maps provider-account typed errors to HTTP statuses by tag', () => {
    expect(
      providerAccountRouteErrorStatus(
        new ProviderAccountCredentialMaterial({
          fieldName: 'reason',
          message: 'reason contains credential-shaped material.',
        }),
      ),
    ).toBe(400)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderAccountNotFound({
          message: 'Provider account not found.',
        }),
      ),
    ).toBe(404)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderAccountNotConnectedHealthy({
          message: 'Provider account is not connected and healthy.',
        }),
      ),
    ).toBe(409)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderDeviceLoginAttemptExpired({
          message: 'Device login attempt is expired.',
        }),
      ),
    ).toBe(409)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderGrantExpired({
          message: 'Grant is expired.',
        }),
      ),
    ).toBe(409)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderGrantRunnerSessionMismatch({
          message: 'Grant runner session does not match request.',
        }),
      ),
    ).toBe(409)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderAccountClientRequestFailed({
          endpoint: 'deviceauth_usercode',
          message: 'ChatGPT/Codex device login start failed with 503.',
          status: 503,
        }),
      ),
    ).toBe(502)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderAccountClientRequestFailed({
          endpoint: 'deviceauth_usercode',
          message: 'OpenAI is rate limiting ChatGPT device login.',
          status: 429,
        }),
      ),
    ).toBe(429)
    expect(
      providerAccountRouteErrorStatus(
        new ProviderAccountStorageFailed({
          operation: 'find_grant_by_ref',
          message: 'D1 failed.',
        }),
      ),
    ).toBe(500)
  })

  test('uses tagged error names and redacts response messages', () => {
    const error = new ProviderAccountCredentialMaterial({
      fieldName: 'reason',
      message: 'reason contains sk-proj-secret credential-shaped material.',
    })

    expect(providerAccountRouteErrorName(error)).toBe(
      'ProviderAccountCredentialMaterial',
    )
    expect(providerAccountRouteErrorMessage(error)).not.toContain(
      'sk-proj-secret',
    )
  })

  test('keeps explicit fallback status only for non-provider failures', () => {
    expect(
      providerAccountRouteErrorStatus(new Error('unexpected network failure')),
    ).toBe(400)
    expect(
      providerAccountRouteErrorStatus(
        new Error('unexpected network failure'),
        502,
      ),
    ).toBe(502)
  })
})
