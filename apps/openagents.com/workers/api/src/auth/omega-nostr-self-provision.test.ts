import { describe, expect, test } from 'vitest'

import { makeMemoryAuthKvStore } from './auth-kv'
import {
  defaultOmegaNostrSelfProvisionPolicy,
  isOmegaNostrPubkey,
  isOmegaNostrSelfProvisionedUserId,
  omegaNostrDisplayName,
  omegaNostrSelfProvisionAbuseBound,
  omegaNostrSelfProvisionDailyTokenCeiling,
  omegaNostrSelfProvisionEnabled,
  omegaNostrSelfProvisionPolicy,
  omegaNostrSyntheticEmail,
  omegaNostrUserId,
  reserveOmegaNostrSelfProvision,
} from './omega-nostr-self-provision'

const pubkey = 'a'.repeat(64)
const otherPubkey = 'b'.repeat(64)
const nowMs = Date.parse('2026-07-28T12:00:00.000Z')

describe('omega nostr self-provision kill switch', () => {
  test('is OFF unless the flag is explicitly truthy', () => {
    expect(omegaNostrSelfProvisionEnabled({})).toBe(false)
    expect(
      omegaNostrSelfProvisionEnabled({
        OMEGA_NOSTR_SELF_PROVISION_ENABLED: '',
      }),
    ).toBe(false)
    expect(
      omegaNostrSelfProvisionEnabled({
        OMEGA_NOSTR_SELF_PROVISION_ENABLED: 'false',
      }),
    ).toBe(false)
    expect(
      omegaNostrSelfProvisionEnabled({
        OMEGA_NOSTR_SELF_PROVISION_ENABLED: 'off',
      }),
    ).toBe(false)

    for (const value of ['1', 'true', 'TRUE', ' yes ', 'on']) {
      expect(
        omegaNostrSelfProvisionEnabled({
          OMEGA_NOSTR_SELF_PROVISION_ENABLED: value,
        }),
      ).toBe(true)
    }
  })
})

describe('omega nostr self-provision identity derivation', () => {
  test('binds the subject to the signing pubkey and nothing else', () => {
    expect(omegaNostrUserId(pubkey)).toBe(`nostr:${pubkey}`)
    expect(isOmegaNostrSelfProvisionedUserId(omegaNostrUserId(pubkey))).toBe(
      true,
    )
    expect(isOmegaNostrSelfProvisionedUserId('github:1')).toBe(false)
    expect(isOmegaNostrSelfProvisionedUserId('email:a@b.co')).toBe(false)
    expect(isOmegaNostrSelfProvisionedUserId('user_abc')).toBe(false)
  })

  test('the synthetic address is non-deliverable and never an admin address', () => {
    const email = omegaNostrSyntheticEmail(pubkey)

    expect(email).toBe(`${pubkey}@nostr.invalid`)
    // RFC 2606 reserved TLD: no MX, so it can never receive a sign-in code.
    expect(email.endsWith('.invalid')).toBe(true)
    expect(email).not.toContain('@openagents.com')
    // Still a well-formed address for the session-subject schema.
    expect(/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)).toBe(true)
  })

  test('normalizes case and rejects non-pubkeys', () => {
    expect(omegaNostrUserId(` ${'A'.repeat(64)} `)).toBe(`nostr:${pubkey}`)
    expect(omegaNostrDisplayName(pubkey)).toBe('nostr-aaaaaaaaaaaa')
    expect(isOmegaNostrPubkey(pubkey)).toBe(true)
    expect(isOmegaNostrPubkey('A'.repeat(64))).toBe(false)
    expect(isOmegaNostrPubkey('a'.repeat(63))).toBe(false)
    expect(isOmegaNostrPubkey(`${'a'.repeat(63)}z`)).toBe(false)
    expect(isOmegaNostrPubkey('')).toBe(false)
  })
})

describe('omega nostr self-provision policy', () => {
  test('defaults are the documented conservative bounds', () => {
    expect(omegaNostrSelfProvisionPolicy({})).toEqual(
      defaultOmegaNostrSelfProvisionPolicy,
    )
    expect(defaultOmegaNostrSelfProvisionPolicy.creationGlobal).toEqual({
      limit: 200,
      windowSeconds: 86_400,
    })
    expect(defaultOmegaNostrSelfProvisionPolicy.creationIp).toEqual({
      limit: 3,
      windowSeconds: 3_600,
    })
    expect(omegaNostrSelfProvisionDailyTokenCeiling({})).toBe(1_000_000)
  })

  test('the owner can retune or hard-stop from env without a code change', () => {
    const policy = omegaNostrSelfProvisionPolicy({
      OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT: '0',
      OMEGA_NOSTR_SELF_PROVISION_IP_HOURLY_LIMIT: '1',
    })

    expect(policy.creationGlobal.limit).toBe(0)
    expect(policy.creationIp.limit).toBe(1)
    expect(
      omegaNostrSelfProvisionDailyTokenCeiling({
        OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING: '50000',
      }),
    ).toBe(50_000)
  })

  test('malformed env values fall back to the defaults instead of unbounding', () => {
    const policy = omegaNostrSelfProvisionPolicy({
      OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT: 'unlimited',
      OMEGA_NOSTR_SELF_PROVISION_IP_HOURLY_LIMIT: '-5',
    })

    expect(policy.creationGlobal.limit).toBe(200)
    expect(policy.creationIp.limit).toBe(3)
    expect(
      omegaNostrSelfProvisionDailyTokenCeiling({
        OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING: 'none',
      }),
    ).toBe(1_000_000)
  })
})

describe('omega nostr self-provision rate limiting', () => {
  const reserve = (
    kv: ReturnType<typeof makeMemoryAuthKvStore>,
    input: Readonly<{ creating: boolean; ipAddress: string; pubkey: string }>,
    at = nowMs,
    policy = defaultOmegaNostrSelfProvisionPolicy,
  ) => reserveOmegaNostrSelfProvision(kv, input, at, policy)

  test('a single IP can create only the configured number of accounts per hour', async () => {
    const kv = makeMemoryAuthKvStore()
    const policy = {
      ...defaultOmegaNostrSelfProvisionPolicy,
      creationIp: { limit: 2, windowSeconds: 3_600 },
    }

    for (let index = 0; index < 2; index += 1) {
      const result = await reserve(
        kv,
        { creating: true, ipAddress: '203.0.113.9', pubkey: `${index}`.repeat(64) },
        nowMs,
        policy,
      )

      expect(result._tag).toBe('Allowed')
    }

    const rejected = await reserve(
      kv,
      { creating: true, ipAddress: '203.0.113.9', pubkey: otherPubkey },
      nowMs,
      policy,
    )

    expect(rejected).toMatchObject({
      _tag: 'RateLimited',
      limit: 2,
      scope: 'creation_ip',
      windowSeconds: 3_600,
    })
    expect(
      rejected._tag === 'RateLimited' ? rejected.retryAfterSeconds : 0,
    ).toBeGreaterThan(0)
  })

  test('the global ceiling bounds aggregate creation across every IP', async () => {
    const kv = makeMemoryAuthKvStore()
    const policy = {
      ...defaultOmegaNostrSelfProvisionPolicy,
      creationGlobal: { limit: 2, windowSeconds: 86_400 },
    }

    // Different IPs AND different keypairs — the attacker's cheapest rotation.
    for (let index = 0; index < 2; index += 1) {
      const result = await reserve(
        kv,
        {
          creating: true,
          ipAddress: `198.51.100.${index}`,
          pubkey: `${index}`.repeat(64),
        },
        nowMs,
        policy,
      )

      expect(result._tag).toBe('Allowed')
    }

    const rejected = await reserve(
      kv,
      { creating: true, ipAddress: '198.51.100.250', pubkey: otherPubkey },
      nowMs,
      policy,
    )

    expect(rejected).toMatchObject({
      _tag: 'RateLimited',
      limit: 2,
      scope: 'creation_global',
    })
  })

  test('a global ceiling of 0 hard-stops new accounts but not existing installs', async () => {
    const kv = makeMemoryAuthKvStore()
    const policy = {
      ...defaultOmegaNostrSelfProvisionPolicy,
      creationGlobal: { limit: 0, windowSeconds: 86_400 },
    }

    await expect(
      reserve(kv, { creating: true, ipAddress: '1.2.3.4', pubkey }, nowMs, policy),
    ).resolves.toMatchObject({ _tag: 'RateLimited', scope: 'creation_global' })
    await expect(
      reserve(
        kv,
        { creating: false, ipAddress: '1.2.3.4', pubkey },
        nowMs,
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'Allowed' })
  })

  test('a returning install never consumes the creation budget', async () => {
    const kv = makeMemoryAuthKvStore()
    const policy = {
      ...defaultOmegaNostrSelfProvisionPolicy,
      creationIp: { limit: 1, windowSeconds: 3_600 },
    }

    // Ten returning mints from one IP...
    for (let index = 0; index < 10; index += 1) {
      const result = await reserve(
        kv,
        { creating: false, ipAddress: '192.0.2.7', pubkey: `${index}`.repeat(64) },
        nowMs,
        policy,
      )

      expect(result._tag).toBe('Allowed')
    }

    // ...still leave the first NEW account available.
    await expect(
      reserve(
        kv,
        { creating: true, ipAddress: '192.0.2.7', pubkey },
        nowMs,
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'Allowed' })
  })

  test('one pubkey cannot mint sessions without bound', async () => {
    const kv = makeMemoryAuthKvStore()
    const policy = {
      ...defaultOmegaNostrSelfProvisionPolicy,
      mintPubkey: { limit: 2, windowSeconds: 3_600 },
    }

    await reserve(kv, { creating: false, ipAddress: '1.1.1.1', pubkey }, nowMs, policy)
    await reserve(kv, { creating: false, ipAddress: '2.2.2.2', pubkey }, nowMs, policy)

    // A fresh IP does not launder the per-pubkey bucket.
    await expect(
      reserve(
        kv,
        { creating: false, ipAddress: '3.3.3.3', pubkey },
        nowMs,
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'RateLimited', scope: 'mint_pubkey' })
  })

  test('buckets rotate when the window rolls over', async () => {
    const kv = makeMemoryAuthKvStore()
    const policy = {
      ...defaultOmegaNostrSelfProvisionPolicy,
      creationIp: { limit: 1, windowSeconds: 3_600 },
    }

    await expect(
      reserve(kv, { creating: true, ipAddress: '5.5.5.5', pubkey }, nowMs, policy),
    ).resolves.toMatchObject({ _tag: 'Allowed' })
    await expect(
      reserve(
        kv,
        { creating: true, ipAddress: '5.5.5.5', pubkey: otherPubkey },
        nowMs,
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'RateLimited', scope: 'creation_ip' })
    await expect(
      reserve(
        kv,
        { creating: true, ipAddress: '5.5.5.5', pubkey: otherPubkey },
        nowMs + 3_600_000,
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'Allowed' })
  })

  test('the bucket subject never stores the raw IP or pubkey', async () => {
    const kv = makeMemoryAuthKvStore()

    await reserve(kv, { creating: true, ipAddress: '203.0.113.42', pubkey })

    const stored = await kv.listPrefix('omega_nostr:self_provision_rate')

    expect(stored.length).toBeGreaterThan(0)
    for (const entry of stored) {
      expect(entry.key).not.toContain('203.0.113.42')
      expect(entry.key).not.toContain(pubkey)
      expect(entry.value).not.toContain('203.0.113.42')
      expect(entry.value).not.toContain(pubkey)
    }
  })
})

describe('omega nostr self-provision abuse bound', () => {
  test('states the worst case in tokens, not in adjectives', () => {
    expect(
      omegaNostrSelfProvisionAbuseBound(
        defaultOmegaNostrSelfProvisionPolicy,
        1_000_000,
      ),
    ).toEqual({
      // 200 brand-new free identities per day across the deployment...
      newAccountsPerDay: 200,
      // ...each capped at 1M served tokens/day => 200M tokens/day worst case.
      tokensPerDayCeiling: 200_000_000,
      // A single un-rotated IP tops out at 3 new identities/hour => 3M tokens.
      tokensPerHourSingleIp: 3_000_000,
    })
  })
})
