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
    })
  }
})
