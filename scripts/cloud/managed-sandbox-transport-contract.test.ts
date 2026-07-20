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
    expect(source).toContain("--chdir /workspace /bin/pwd)\" = '/workspace'")
  })

  test('guest commands rebind the validated cwd at the canonical workspace path', () => {
    const source = readFileSync(
      resolve(import.meta.dirname, 'managed-sandbox-guest-io.py'),
      'utf8',
    )
    expect(source).toContain('canonical_cwd = WORKSPACE / relative if relative else WORKSPACE')
    expect(source).toContain('f"/proc/self/fd/{cwd_fd}",')
    expect(source).toContain('str(canonical_cwd),')
    expect(source).not.toContain('"--chdir",\n                f"/proc/self/fd/{cwd_fd}",')
  })

  test('provider streams emit at most one structural terminal event', () => {
    const source = readFileSync(
      resolve(import.meta.dirname, 'managed-sandbox-guest-turn.mjs'),
      'utf8',
    )
    const codex = source.slice(
      source.indexOf('const runCodex = async () => {'),
      source.indexOf('const runClaude = async () => {'),
    )
    expect(codex).toContain('if (settled) continue;')
    expect(codex).toContain(
      'if (!settled) throw new Error("codex_stream_ended_without_result");',
    )
    expect(codex.match(/settled = true;/g)).toHaveLength(2)

    const claude = source.slice(source.indexOf('const runClaude = async () => {'))
    expect(claude).toContain('if (settled) continue;')
  })

  test('guest emitter admits exactly one terminal event per turn', () => {
    const source = readFileSync(
      resolve(import.meta.dirname, 'managed-sandbox-guest-turn.mjs'),
      'utf8',
    )
    expect(source).toContain('const terminalEventTags = new Set([')
    expect(source).toContain('if (terminalEventTag !== undefined) return false;')
    expect(source).toContain(
      'if (terminalEventTags.has(next._tag)) terminalEventTag = next._tag;',
    )
    expect(source).toContain('if (terminalEventTag === undefined) {')
  })
})
