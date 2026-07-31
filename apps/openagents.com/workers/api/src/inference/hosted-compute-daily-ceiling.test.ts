// P0 2026-07-31. `POST /api/agents/register` is unauthenticated public
// self-service and mints an `oa_agent_` token; `requireHostedComputeActor`
// accepts any such token; the hosted-Gemini proxy then calls Google with the
// owner's shared `GEMINI_API_KEY`. The ceiling that guarded that route
// inspected ONLY self-provisioned `nostr:` identities, so an agent token drew
// owner-funded inference without any bound. These tests pin the replacement:
// every actor is bounded unless explicitly admitted.

import { describe, expect, test } from 'vitest'

import {
  DEFAULT_HOSTED_COMPUTE_DAILY_TOKEN_CEILING,
  HOSTED_COMPUTE_CEILING_ERROR,
  decideHostedComputeDailyCeiling,
  hostedComputeDailyTokenCeiling,
  isAdmittedHostedComputeActor,
  makeHostedComputeDailyCeilingGate,
} from './hosted-compute-daily-ceiling'

const noAdmissions = new Set<string>()

describe('hostedComputeDailyTokenCeiling', () => {
  test('falls back to the compiled default when unset or malformed', () => {
    for (const raw of [undefined, '', '  ', 'not-a-number', '-1']) {
      expect(hostedComputeDailyTokenCeiling(raw)).toBe(DEFAULT_HOSTED_COMPUTE_DAILY_TOKEN_CEILING)
    }
  })

  test('honours an owner-configured value, including zero', () => {
    expect(hostedComputeDailyTokenCeiling('50000')).toBe(50_000)
    // Zero is the useful "provision identities but serve nobody new" state and
    // must NOT be treated as absent.
    expect(hostedComputeDailyTokenCeiling('0')).toBe(0)
  })
})

describe('isAdmittedHostedComputeActor', () => {
  test('accepts the bare id and both account-ref forms', () => {
    expect(isAdmittedHostedComputeActor('user_a', new Set(['user_a']))).toBe(true)
    expect(isAdmittedHostedComputeActor('user_a', new Set(['agent:user_a']))).toBe(true)
    expect(isAdmittedHostedComputeActor('github:1', new Set(['openauth:github:1']))).toBe(true)
  })

  test('does not admit a stranger, and never admits on an empty allowlist', () => {
    expect(isAdmittedHostedComputeActor('user_b', new Set(['agent:user_a']))).toBe(false)
    expect(isAdmittedHostedComputeActor('user_a', noAdmissions)).toBe(false)
  })
})

describe('decideHostedComputeDailyCeiling', () => {
  test('refuses at or above the ceiling and admits below it', () => {
    expect(decideHostedComputeDailyCeiling({ servedToday: 9, tokensPerDay: 10 })).toBeUndefined()
    // Reached exactly is refused rather than allowed to tip over.
    expect(decideHostedComputeDailyCeiling({ servedToday: 10, tokensPerDay: 10 })).toMatchObject({
      dailyTokenCeiling: 10,
      error: HOSTED_COMPUTE_CEILING_ERROR,
      tokensServedToday: 10,
    })
    // A ceiling of zero refuses every request.
    expect(decideHostedComputeDailyCeiling({ servedToday: 0, tokensPerDay: 0 })).toBeDefined()
  })
})

describe('makeHostedComputeDailyCeilingGate', () => {
  test('bounds a freshly registered agent actor', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: noAdmissions,
      servedTokensToday: () => Promise.resolve(1_000_000),
      tokensPerDay: () => 1_000_000,
    })

    expect(await gate('user_freshly_registered')).toMatchObject({
      error: HOSTED_COMPUTE_CEILING_ERROR,
    })
  })

  test('exempts an admitted actor WITHOUT reading the ledger', async () => {
    let reads = 0
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set(['agent:user_ops']),
      servedTokensToday: () => {
        reads += 1
        return Promise.resolve(999_999_999)
      },
      tokensPerDay: () => 1_000_000,
    })

    expect(await gate('user_ops')).toBeUndefined()
    // No latency added to the owner's existing runners.
    expect(reads).toBe(0)
  })

  test('applies a per-actor ceiling so classes stay independently tunable', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: noAdmissions,
      servedTokensToday: () => Promise.resolve(75_000),
      tokensPerDay: (actorUserId) => (actorUserId.startsWith('nostr:') ? 50_000 : 100_000),
    })

    expect(await gate(`nostr:${'a'.repeat(64)}`)).toMatchObject({
      dailyTokenCeiling: 50_000,
    })
    expect(await gate('user_agent')).toBeUndefined()
  })

  // FAIL-CLOSED: a spend path must not fall open during a database outage.
  test('refuses when the ledger read throws', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: noAdmissions,
      servedTokensToday: () => Promise.reject(new Error('ledger unavailable')),
      tokensPerDay: () => 1_000_000,
    })

    expect(await gate('user_agent')).toMatchObject({
      error: HOSTED_COMPUTE_CEILING_ERROR,
    })
  })

  // ...but an ADMITTED actor is unaffected by an outage, because it never
  // reaches the ledger at all. This is what keeps owner tooling alive.
  test('an admitted actor is served even when the ledger is down', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set(['user_ops']),
      servedTokensToday: () => Promise.reject(new Error('ledger unavailable')),
      tokensPerDay: () => 1_000_000,
    })

    expect(await gate('user_ops')).toBeUndefined()
  })
})

// The same regression guard as on the chat-completions gate. The 2026-07-31
// owner lockout hit BOTH routes — `openagents/gpt-5.6-luna` on
// chat-completions AND the `gemini-3.6-flash` fallback on this proxy — because
// both key on the `nostr:` prefix, which an operator-admitted alpha member
// shares with a self-provisioned install.
describe('admitted identities are not free tier', () => {
  const ADMITTED_ID = `nostr:${'e'.repeat(64)}`
  const STRANGER_ID = `nostr:${'a'.repeat(64)}`

  const gateWith = (admitted: ReadonlySet<string>) =>
    makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set<string>(),
      isAdmittedIdentity: async id => admitted.has(id),
      servedTokensToday: async () => 10_410_253,
      tokensPerDay: () => 1_000_000,
    })

  test('an ADMITTED nostr actor is exempt even far over the ceiling', async () => {
    await expect(
      gateWith(new Set([ADMITTED_ID]))(ADMITTED_ID),
    ).resolves.toBeUndefined()
  })

  test('a SELF-PROVISIONED nostr actor is still bounded', async () => {
    await expect(
      gateWith(new Set([ADMITTED_ID]))(STRANGER_ID),
    ).resolves.toMatchObject({
      dailyTokenCeiling: 1_000_000,
      error: HOSTED_COMPUTE_CEILING_ERROR,
      tokensServedToday: 10_410_253,
    })
  })

  test('the membership check runs BEFORE the ledger read', async () => {
    let ledgerReads = 0
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set<string>(),
      isAdmittedIdentity: async () => true,
      servedTokensToday: async () => {
        ledgerReads += 1
        return 0
      },
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate(ADMITTED_ID)).resolves.toBeUndefined()
    expect(ledgerReads).toBe(0)
  })

  test('a FAILING membership read falls through to the ceiling, never past it', async () => {
    const gate = makeHostedComputeDailyCeilingGate({
      admittedAccountRefs: new Set<string>(),
      isAdmittedIdentity: async () => false,
      servedTokensToday: async () => 10_410_253,
      tokensPerDay: () => 1_000_000,
    })

    await expect(gate(ADMITTED_ID)).resolves.toMatchObject({
      error: HOSTED_COMPUTE_CEILING_ERROR,
    })
  })
})
