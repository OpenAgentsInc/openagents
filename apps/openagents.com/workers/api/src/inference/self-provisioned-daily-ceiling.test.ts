import { describe, expect, test } from 'vitest'

import {
  SELF_PROVISIONED_CEILING_ERROR,
  decideSelfProvisionedDailyCeiling,
  makeSelfProvisionedDailyCeilingGate,
  secondsUntilNextUtcDay,
  selfProvisionedUserIdFromAccountRef,
} from './self-provisioned-daily-ceiling'

const PUBKEY = 'a'.repeat(64)
const SELF_PROVISIONED_REF = `openauth:nostr:${PUBKEY}`
const nowMs = Date.UTC(2026, 6, 31, 18, 0, 0)

describe('selfProvisionedUserIdFromAccountRef', () => {
  test('matches only a self-provisioned OpenAuth session', () => {
    expect(selfProvisionedUserIdFromAccountRef(SELF_PROVISIONED_REF)).toBe(
      `nostr:${PUBKEY}`,
    )
  })

  test('never matches an identity class this gate must not ceiling', () => {
    // Ceilinging any of these would cut off live owner/customer workflows.
    for (const accountRef of [
      'openauth:github:12345',
      'openauth:email:person@example.com',
      `agent:nostr:${PUBKEY}`,
      'agent:user_123',
      `nostr:${PUBKEY}`,
      '',
    ]) {
      expect(
        selfProvisionedUserIdFromAccountRef(accountRef),
        accountRef,
      ).toBeUndefined()
    }
  })
})

describe('decideSelfProvisionedDailyCeiling', () => {
  test('admits an identity under its allowance', () => {
    expect(
      decideSelfProvisionedDailyCeiling({
        nowMs,
        servedToday: 999,
        tokensPerDay: 1_000,
      }),
    ).toBeUndefined()
  })

  test('refuses at the ceiling, not one token past it', () => {
    const refusal = decideSelfProvisionedDailyCeiling({
      nowMs,
      servedToday: 1_000,
      tokensPerDay: 1_000,
    })
    expect(refusal?.error).toBe(SELF_PROVISIONED_CEILING_ERROR)
    expect(refusal?.servedToday).toBe(1_000)
    expect(refusal?.tokensPerDay).toBe(1_000)
  })

  test('a ceiling of zero serves nobody', () => {
    // The useful "provision accounts but serve no inference" state.
    expect(
      decideSelfProvisionedDailyCeiling({
        nowMs,
        servedToday: 0,
        tokensPerDay: 0,
      }),
    ).not.toBeUndefined()
  })
})

describe('secondsUntilNextUtcDay', () => {
  test('counts to the next UTC boundary, matching the ledger window', () => {
    expect(secondsUntilNextUtcDay(Date.UTC(2026, 6, 31, 23, 59, 30))).toBe(30)
    expect(secondsUntilNextUtcDay(Date.UTC(2026, 6, 31, 0, 0, 0))).toBe(86_400)
  })

  test('never returns a non-positive retry hint', () => {
    expect(
      secondsUntilNextUtcDay(Date.UTC(2026, 6, 31, 23, 59, 59, 999)),
    ).toBeGreaterThan(0)
  })
})

describe('makeSelfProvisionedDailyCeilingGate', () => {
  test('does not read the ledger for a non-self-provisioned caller', async () => {
    let reads = 0
    const gate = makeSelfProvisionedDailyCeilingGate({
      nowMs: () => nowMs,
      servedTokensToday: async () => {
        reads += 1
        return 10_000_000
      },
      tokensPerDay: () => 1_000,
    })

    await expect(gate('openauth:github:12345')).resolves.toBeUndefined()
    expect(reads).toBe(0)
  })

  test('refuses a self-provisioned identity that spent its allowance', async () => {
    const gate = makeSelfProvisionedDailyCeilingGate({
      nowMs: () => nowMs,
      servedTokensToday: async () => 1_500,
      tokensPerDay: () => 1_000,
    })

    const refusal = await gate(SELF_PROVISIONED_REF)
    expect(refusal?.error).toBe(SELF_PROVISIONED_CEILING_ERROR)
    expect(refusal?.retryAfterSeconds).toBeGreaterThan(0)
  })

  test('reads the ledger for the bare user id, not the account ref', async () => {
    // The ledger keys on `actor_user_id`; passing the `openauth:`-prefixed ref
    // would silently sum zero rows and make the ceiling unreachable.
    const seen: Array<string> = []
    const gate = makeSelfProvisionedDailyCeilingGate({
      nowMs: () => nowMs,
      servedTokensToday: async userId => {
        seen.push(userId)
        return 0
      },
      tokensPerDay: () => 1_000,
    })

    await gate(SELF_PROVISIONED_REF)
    expect(seen).toEqual([`nostr:${PUBKEY}`])
  })

  test('FAILS CLOSED when the ledger read throws', async () => {
    // A spend path must not fall open during a database outage. Only
    // self-provisioned identities are affected.
    const gate = makeSelfProvisionedDailyCeilingGate({
      nowMs: () => nowMs,
      servedTokensToday: async () => {
        throw new Error('ledger unavailable')
      },
      tokensPerDay: () => 1_000,
    })

    await expect(gate(SELF_PROVISIONED_REF)).resolves.toMatchObject({
      error: SELF_PROVISIONED_CEILING_ERROR,
    })
    // ...and a database outage still must not ceiling a normal customer.
    await expect(gate('openauth:github:12345')).resolves.toBeUndefined()
  })
})

// The regression guard for the 2026-07-31 owner lockout.
//
// The gate keys on the `nostr:` prefix, which an OPERATOR-ADMITTED alpha member
// shares with a self-provisioned install (`admit-sarah-voice-npub.ts` writes
// exactly the same id form). Once the ledger read started counting, that
// conflation refused the owner's own identity
// `free_tier_daily_token_ceiling_reached` on every turn in his own app, at
// 10,410,253 served against a 1,000,000 ceiling he was never the subject of.
//
// Membership — not prefix — is the discriminator. These tests must both hold,
// and must keep holding when self-provisioning is armed.
describe('admitted identities are not free tier', () => {
  const ADMITTED_REF = `openauth:nostr:${'e'.repeat(64)}`

  const gateWith = (
    admitted: ReadonlySet<string>,
    servedToday = 10_410_253,
  ) =>
    makeSelfProvisionedDailyCeilingGate({
      isAdmittedIdentity: async userId => admitted.has(userId),
      nowMs: () => nowMs,
      servedTokensToday: async () => servedToday,
      tokensPerDay: () => 1_000_000,
    })

  test('an ADMITTED nostr identity is exempt even far over the ceiling', async () => {
    const gate = gateWith(new Set([`nostr:${'e'.repeat(64)}`]))
    await expect(gate(ADMITTED_REF)).resolves.toBeUndefined()
  })

  test('a SELF-PROVISIONED nostr identity is still bounded', async () => {
    // Same gate, same ceiling, same overage — only membership differs.
    const gate = gateWith(new Set([`nostr:${'e'.repeat(64)}`]))
    await expect(gate(SELF_PROVISIONED_REF)).resolves.toMatchObject({
      error: SELF_PROVISIONED_CEILING_ERROR,
      servedToday: 10_410_253,
      tokensPerDay: 1_000_000,
    })
  })

  test('the membership check runs BEFORE the ledger read', async () => {
    // An admitted member must cost one indexed lookup, not a ledger scan.
    let ledgerReads = 0
    const gate = makeSelfProvisionedDailyCeilingGate({
      isAdmittedIdentity: async () => true,
      nowMs: () => nowMs,
      servedTokensToday: async () => {
        ledgerReads += 1
        return 0
      },
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate(ADMITTED_REF)).resolves.toBeUndefined()
    expect(ledgerReads).toBe(0)
  })

  test('a FAILING membership read falls through to the ceiling, never past it', async () => {
    // Fail-closed: the cost of a database problem is that an admitted member is
    // briefly metered like free tier — never that a stranger is served unbounded.
    const gate = makeSelfProvisionedDailyCeilingGate({
      isAdmittedIdentity: async () => false,
      nowMs: () => nowMs,
      servedTokensToday: async () => 10_410_253,
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate(ADMITTED_REF)).resolves.toMatchObject({
      error: SELF_PROVISIONED_CEILING_ERROR,
    })
  })

  test('without the lookup the gate keeps its previous (conflating) behavior', async () => {
    const gate = makeSelfProvisionedDailyCeilingGate({
      nowMs: () => nowMs,
      servedTokensToday: async () => 10_410_253,
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate(ADMITTED_REF)).resolves.toMatchObject({
      error: SELF_PROVISIONED_CEILING_ERROR,
    })
  })
})
