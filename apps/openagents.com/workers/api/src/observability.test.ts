import { describe, expect, test } from 'vitest'

import {
  redactedWorkerLogFields,
  workerErrorLogEntry,
} from './observability'

describe('Worker observability redaction', () => {
  test('redacts provider credential material before logging', () => {
    const entry = workerErrorLogEntry(
      'provider_device_login_start_failed',
      new Error('Bearer fake-header-token-0000000000 failed'),
      {
        authJson:
          '{"openai":{"refresh":"fake-refresh","access":"fake-access"}}',
        githubToken: 'gho_abcdefghijklmnopqrstuvwxyz',
        path: '/tmp/auth.json',
      },
    )
    const json = JSON.stringify(entry)

    expect(json).toContain('Bearer [REDACTED]')
    expect(json).toContain('gho_[REDACTED]')
    expect(json).toContain('auth.json:[REDACTED]')
    expect(json).not.toContain('fake-refresh')
    expect(json).not.toContain('fake-access')
    expect(json).not.toContain('fake-header-token')
    expect(json).not.toContain('gho_abcdefghijklmnopqrstuvwxyz')
  })

  test('drops undefined fields and stringifies defined fields', () => {
    expect(
      redactedWorkerLogFields({
        attemptId: 'attempt_1',
        missing: undefined,
        status: 502,
      }),
    ).toEqual({
      attemptId: 'attempt_1',
      status: '502',
    })
  })
})
