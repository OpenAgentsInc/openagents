import { describe, expect, test } from 'vitest'

import {
  BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
  BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
  evaluateBuiltinComputeAgentTokenBudget,
  executeBuiltinComputeAgentGrant,
  type BuiltinComputeAgentStore,
} from './builtin-compute-agent-grant'

const fixedRuntime = {
  makeGrantRef: () => 'builtin_compute_grant_test',
  makeQuotaEventId: () => 'builtin_compute_quota_test',
  makeUsageEventId: () => 'builtin_compute_usage_test',
  now: () => new Date('2026-06-15T12:00:00.000Z'),
  nowIso: () => '2026-06-15T12:00:00.000Z',
}

const makeStore = (sessionsUsed: number, tokensUsed = 0) => {
  const recorded: Array<unknown> = []
  const store: BuiltinComputeAgentStore = {
    countSessionsSince: () => Promise.resolve(sessionsUsed),
    sumTokensSince: () => Promise.resolve(tokensUsed),
    recordGrant: input => {
      recorded.push(input)
      return Promise.resolve()
    },
  }
  return { recorded, store }
}

const session = { user: { id: 'user-1' } }

describe('executeBuiltinComputeAgentGrant', () => {
  test('is inert when the hosted key is not configured (grants nothing)', async () => {
    const { recorded, store } = makeStore(0)
    const result = await executeBuiltinComputeAgentGrant({
      hostedKeyConfigured: false,
      runtime: fixedRuntime,
      session,
      store,
    })
    expect(result.kind).toBe('not_configured')
    expect(recorded).toHaveLength(0)
  })

  test('grants within quota, exposing only the redacted secret-ref (never a raw key)', async () => {
    const { recorded, store } = makeStore(0)
    const result = await executeBuiltinComputeAgentGrant({
      hostedKeyConfigured: true,
      runtime: fixedRuntime,
      session,
      store,
    })
    expect(result.kind).toBe('granted')
    if (result.kind !== 'granted') {
      throw new Error('expected granted')
    }
    // Only a secret-REF is handed back; the runner resolves the real key via the broker.
    expect(result.grant.providerSecretRef).toContain(
      'provider-account://google-gemini',
    )
    expect(result.grant.materialization.kind).toBe('probe_gemini_api_key')
    expect(result.grant.status).toBe('issued')
    expect(result.grant.freeAllowance.sessionsRemaining).toBe(
      BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS - 1,
    )
    expect(recorded).toHaveLength(1)
    // No raw key material anywhere in the serialized grant.
    const serialized = JSON.stringify(result)
    expect(serialized).not.toMatch(/AIza[0-9A-Za-z_-]{10,}/)
    expect(serialized.toLowerCase()).not.toContain('apikey')
  })

  test('denies and records nothing once the daily session quota is spent', async () => {
    const { recorded, store } = makeStore(BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS)
    const result = await executeBuiltinComputeAgentGrant({
      hostedKeyConfigured: true,
      runtime: fixedRuntime,
      session,
      store,
    })
    expect(result.kind).toBe('quota_exhausted')
    if (result.kind !== 'quota_exhausted') {
      throw new Error('expected quota_exhausted')
    }
    expect(result.reason).toBe('sessions')
    expect(result.sessionsRemaining).toBe(0)
    expect(recorded).toHaveLength(0)
  })

  test('denies on the metered token ceiling even with free sessions left', async () => {
    const { recorded, store } = makeStore(
      0,
      BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
    )
    const result = await executeBuiltinComputeAgentGrant({
      hostedKeyConfigured: true,
      runtime: fixedRuntime,
      session,
      store,
    })
    expect(result.kind).toBe('quota_exhausted')
    if (result.kind !== 'quota_exhausted') {
      throw new Error('expected quota_exhausted')
    }
    // It is the metered token budget, not the session count, that is spent.
    expect(result.reason).toBe('tokens')
    expect(result.sessionsRemaining).toBe(
      BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
    )
    expect(result.tokensRemaining).toBe(0)
    expect(recorded).toHaveLength(0)
  })

  test('grants and reports remaining token budget when partially metered', async () => {
    const used = Math.floor(BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING / 4)
    const { store } = makeStore(0, used)
    const result = await executeBuiltinComputeAgentGrant({
      hostedKeyConfigured: true,
      runtime: fixedRuntime,
      session,
      store,
    })
    expect(result.kind).toBe('granted')
    if (result.kind !== 'granted') {
      throw new Error('expected granted')
    }
    expect(result.grant.freeAllowance.tokensRemaining).toBe(
      BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING - used,
    )
  })
})

describe('evaluateBuiltinComputeAgentTokenBudget', () => {
  const resetsAt = '2026-06-16T00:00:00.000Z'

  test('is within budget when usage is below the ceiling', () => {
    const result = evaluateBuiltinComputeAgentTokenBudget({
      dailyTokenCeiling: 1000,
      resetsAt,
      usedTokensToday: 250,
    })
    expect(result.kind).toBe('within_budget')
    expect(result.tokensRemaining).toBe(750)
    expect(result.usedTokensToday).toBe(250)
  })

  test('is exhausted exactly at the ceiling', () => {
    const result = evaluateBuiltinComputeAgentTokenBudget({
      dailyTokenCeiling: 1000,
      resetsAt,
      usedTokensToday: 1000,
    })
    expect(result.kind).toBe('budget_exhausted')
    expect(result.tokensRemaining).toBe(0)
  })

  test('clamps malformed counters closed rather than widening the budget', () => {
    const negative = evaluateBuiltinComputeAgentTokenBudget({
      dailyTokenCeiling: 1000,
      resetsAt,
      usedTokensToday: -5,
    })
    expect(negative.kind).toBe('within_budget')
    expect(negative.tokensRemaining).toBe(1000)

    const notFinite = evaluateBuiltinComputeAgentTokenBudget({
      dailyTokenCeiling: 1000,
      resetsAt,
      usedTokensToday: Number.NaN,
    })
    // A NaN counter is treated as fully spent — fail closed on the shared key.
    expect(notFinite.kind).toBe('budget_exhausted')
    expect(notFinite.tokensRemaining).toBe(0)
  })
})
