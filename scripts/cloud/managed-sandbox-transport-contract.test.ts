import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, test } from 'vitest'

describe('managed-sandbox guest transport contract', () => {
  for (const driver of [
    'managed-sandbox-io-driver.mjs',
    'managed-sandbox-turn-driver.mjs',
  ]) {
    test(`${driver} uses bounded relative SSH key expiry`, () => {
      const source = readFileSync(resolve(import.meta.dirname, driver), 'utf8')
      expect(source).toContain('--ssh-key-expire-after=10m')
      expect(source).not.toContain('--ssh-key-expiration=10m')
      expect(source).toContain('--internal-ip')
      expect(source).not.toContain('--tunnel-through-iap')
      if (driver === 'managed-sandbox-io-driver.mjs') {
        expect(source).toContain(
          '/var/lib/openagents/managed-sandbox-turns/io-',
        )
        expect(source).not.toContain(
          '/var/lib/openagents/managed-sandbox-io/',
        )
      }
    })
  }

  test('guest image admits unprivileged I/O transport and scratch roots', () => {
    const source = readFileSync(
      resolve(import.meta.dirname, 'build-managed-sandbox-guest-image.sh'),
      'utf8',
    )
    expect(source).toContain('/var/lib/openagents/managed-sandbox-io')
    expect(source).toContain(
      'd /run/openagents-managed-sandbox/io 0700 openagents openagents -',
    )
    expect(source).toContain(
      "stat -c '%U:%G:%a' /var/lib/openagents/managed-sandbox-io",
    )
    expect(source).toContain(
      "stat -c '%U:%G:%a' /run/openagents-managed-sandbox/io",
    )
  })
})
