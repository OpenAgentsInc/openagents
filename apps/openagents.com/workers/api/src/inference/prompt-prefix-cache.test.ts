import { describe, expect, test } from 'vitest'

import { hashCacheAffinityKey } from './khala-telemetry'
import {
  STABLE_ORDINAL,
  type TaggedPromptMessage,
  assembleStablePromptLayout,
  canonicalJson,
  deriveCacheAffinityKey,
  deriveSessionAffinityValue,
  hashStablePrefix,
  reconcileUsageTokens,
  serializeToolSchemas,
  sessionAffinityParams,
} from './prompt-prefix-cache'

// --- fixtures --------------------------------------------------------------

const IDENTITY = 'You are Khala, the OpenAgents inference model. We are Khala.'
const CONTRACT = 'ACCEPTANCE CONTRACT (REQUIRED): expose window hooks.'
const TOOL_BLOCK = serializeToolSchemas([
  { description: 'run a shell command', name: 'shell' },
  { description: 'read a file', name: 'read_file' },
])

// Build the SAME stable blocks the gateway injects, in a given append order, plus
// a volatile user turn. `userTurn` varies per request; the stable blocks do not.
const taggedFor = (
  userTurn: string,
  order: 'contract_first' | 'identity_first' = 'contract_first',
): ReadonlyArray<TaggedPromptMessage> => {
  const identity: TaggedPromptMessage = {
    message: { content: IDENTITY, role: 'system' },
    stableKind: 'identity',
  }
  const contract: TaggedPromptMessage = {
    message: { content: CONTRACT, role: 'system' },
    stableKind: 'acceptanceContract',
  }
  const tools: TaggedPromptMessage = {
    message: { content: TOOL_BLOCK, role: 'system' },
    stableKind: 'toolSchemas',
  }
  const user: TaggedPromptMessage = {
    message: { content: userTurn, role: 'user' },
  }
  const stable =
    order === 'contract_first'
      ? [contract, identity, tools]
      : [identity, tools, contract]
  return [...stable, user]
}

describe('stable prompt layout — the book P0-2 rule (novel tokens last)', () => {
  test('stable content leads, volatile/user content is appended last', () => {
    const layout = assembleStablePromptLayout(taggedFor('build me a crossy road'))
    const roles = layout.messages.map(m => m.role)
    // Three stable system blocks first, then the single user turn last.
    expect(roles).toEqual(['system', 'system', 'system', 'user'])
    // The acceptance contract (lowest ordinal) leads, then identity, then tools.
    expect(layout.messages[0]!.content).toBe(CONTRACT)
    expect(layout.messages[1]!.content).toBe(IDENTITY)
    expect(layout.messages[2]!.content).toBe(TOOL_BLOCK)
    // The novel user turn is strictly last.
    expect(layout.messages.at(-1)!.content).toBe('build me a crossy road')
  })

  test('the stable ordinal ordering matches the documented canonical order', () => {
    // Lower ordinal = earlier in the prefix = more stable/shared.
    expect(STABLE_ORDINAL.acceptanceContract).toBeLessThan(STABLE_ORDINAL.identity)
    expect(STABLE_ORDINAL.identity).toBeLessThan(STABLE_ORDINAL.toolSchemas)
    expect(STABLE_ORDINAL.toolSchemas).toBeLessThan(STABLE_ORDINAL.stablePolicy)
    expect(STABLE_ORDINAL.stablePolicy).toBeLessThan(STABLE_ORDINAL.otherSystem)
  })

  test('the prefix + cache key are IDENTICAL regardless of the append order of stable blocks (determinism)', () => {
    const a = assembleStablePromptLayout(taggedFor('turn A', 'contract_first'))
    const b = assembleStablePromptLayout(taggedFor('turn A', 'identity_first'))
    // Same stable inputs → byte-identical stable prefix and identical hash, even
    // though the gateway appended the blocks in a different order.
    expect(a.stablePrefixText).toBe(b.stablePrefixText)
    expect(a.stablePrefixHash).toBe(b.stablePrefixHash)
  })

  test('same stable inputs → identical prefix + identical cache key hash across two turns of one session', () => {
    const turn1 = assembleStablePromptLayout(taggedFor('first question'))
    const turn2 = assembleStablePromptLayout(taggedFor('a completely different second question'))
    // The user turns differ, but the shared prefix (contract+identity+tools) and
    // its hash are IDENTICAL — exactly what makes the prefix cacheable turn over turn.
    expect(turn1.stablePrefixText).toBe(turn2.stablePrefixText)
    expect(turn1.stablePrefixHash).toBe(turn2.stablePrefixHash)
  })

  test('volatile/user content NEVER pollutes the stable prefix or its hash', () => {
    const layout = assembleStablePromptLayout(taggedFor('SECRET_USER_PAYLOAD_12345'))
    // The user payload appears in the final messages but NOT in the cacheable
    // prefix text (the prefix is the stable region only).
    expect(layout.messages.some(m => m.content.includes('SECRET_USER_PAYLOAD_12345'))).toBe(true)
    expect(layout.stablePrefixText.includes('SECRET_USER_PAYLOAD_12345')).toBe(false)
    // And the same stable blocks with two different user turns hash identically.
    const other = assembleStablePromptLayout(taggedFor('a different payload'))
    expect(layout.stablePrefixHash).toBe(other.stablePrefixHash)
  })

  test('changing a STABLE block DOES change the prefix hash (sensitivity)', () => {
    const base = assembleStablePromptLayout(taggedFor('q'))
    const changed = assembleStablePromptLayout([
      { message: { content: IDENTITY, role: 'system' }, stableKind: 'identity' },
      {
        message: { content: 'A DIFFERENT CONTRACT', role: 'system' },
        stableKind: 'acceptanceContract',
      },
      { message: { content: TOOL_BLOCK, role: 'system' }, stableKind: 'toolSchemas' },
      { message: { content: 'q', role: 'user' } },
    ])
    expect(changed.stablePrefixHash).not.toBe(base.stablePrefixHash)
  })

  test('an untagged client system message is stable, ordered after the known blocks', () => {
    const layout = assembleStablePromptLayout([
      { message: { content: IDENTITY, role: 'system' }, stableKind: 'identity' },
      { message: { content: 'client steer policy', role: 'system' } },
      { message: { content: 'do the thing', role: 'user' } },
    ])
    // identity (ordinal 1) before the otherSystem client steer (ordinal 4),
    // user last.
    expect(layout.messages.map(m => m.content)).toEqual([
      IDENTITY,
      'client steer policy',
      'do the thing',
    ])
    // Both system messages are in the stable prefix; the user turn is not.
    expect(layout.stablePrefixText.includes('client steer policy')).toBe(true)
    expect(layout.stablePrefixText.includes('do the thing')).toBe(false)
  })

  test('multiple volatile messages preserve their original relative order', () => {
    const layout = assembleStablePromptLayout([
      { message: { content: IDENTITY, role: 'system' }, stableKind: 'identity' },
      { message: { content: 'user one', role: 'user' } },
      { message: { content: 'assistant reply', role: 'assistant' } },
      { message: { content: 'user two', role: 'user' } },
    ])
    expect(layout.messages.map(m => m.content)).toEqual([
      IDENTITY,
      'user one',
      'assistant reply',
      'user two',
    ])
  })

  test('hashStablePrefix is a one-way, self-describing digest', () => {
    const hash = hashStablePrefix('some stable prefix text')
    expect(hash).toMatch(/^prefix:fnv1a32:[0-9a-f]{8}$/u)
    // Deterministic.
    expect(hashStablePrefix('some stable prefix text')).toBe(hash)
    // Different input → different digest (collision-resistance at the unit level).
    expect(hashStablePrefix('other text')).not.toBe(hash)
  })
})

describe('deterministic tool-schema + JSON serialization (P0-2 item 2)', () => {
  test('canonicalJson sorts object keys but preserves array order', () => {
    expect(canonicalJson({ b: 1, a: 2 })).toBe('{"a":2,"b":1}')
    expect(canonicalJson([3, 1, 2])).toBe('[3,1,2]')
    expect(canonicalJson({ z: { y: 1, x: 2 } })).toBe('{"z":{"x":2,"y":1}}')
  })

  test('serializeToolSchemas is order-independent in the input tool list', () => {
    const a = serializeToolSchemas([
      { name: 'shell', params: { cmd: 'string' } },
      { name: 'read', params: { path: 'string' } },
    ])
    const b = serializeToolSchemas([
      { params: { path: 'string' }, name: 'read' },
      { params: { cmd: 'string' }, name: 'shell' },
    ])
    // Same tools, different list order + different key order → identical bytes.
    expect(a).toBe(b)
  })

  test('a tool schema feeding the stable prefix keeps the prefix hash stable across tool-list order', () => {
    const tools1 = serializeToolSchemas([{ name: 'a' }, { name: 'b' }])
    const tools2 = serializeToolSchemas([{ name: 'b' }, { name: 'a' }])
    const layout1 = assembleStablePromptLayout([
      { message: { content: tools1, role: 'system' }, stableKind: 'toolSchemas' },
      { message: { content: 'q', role: 'user' } },
    ])
    const layout2 = assembleStablePromptLayout([
      { message: { content: tools2, role: 'system' }, stableKind: 'toolSchemas' },
      { message: { content: 'q', role: 'user' } },
    ])
    expect(layout1.stablePrefixHash).toBe(layout2.stablePrefixHash)
  })
})

describe('cache-affinity keys + session affinity (P0-2 items 3+4)', () => {
  test('the same account/session/codebase always yields the same raw key', () => {
    const k1 = deriveCacheAffinityKey({ account: 'acct_1', codebase: 'repo_x', session: 'sess_a' })
    const k2 = deriveCacheAffinityKey({ account: 'acct_1', codebase: 'repo_x', session: 'sess_a' })
    expect(k1).toBe(k2)
  })

  test('account-only vs account+session never collide (fixed key shape)', () => {
    const accountOnly = deriveCacheAffinityKey({ account: 'acct_1' })
    const withSession = deriveCacheAffinityKey({ account: 'acct_1', session: 'sess_a' })
    expect(accountOnly).not.toBe(withSession)
    expect(hashCacheAffinityKey(accountOnly)).not.toBe(hashCacheAffinityKey(withSession))
  })

  test('different sessions of the same account get different affinity hashes', () => {
    const a = hashCacheAffinityKey(deriveCacheAffinityKey({ account: 'acct_1', session: 'sess_a' }))
    const b = hashCacheAffinityKey(deriveCacheAffinityKey({ account: 'acct_1', session: 'sess_b' }))
    expect(a).not.toBe(b)
  })

  test('the cache-affinity hash is one-way + public-safe (raw account/session never recoverable from it)', () => {
    const raw = deriveCacheAffinityKey({ account: 'acct_secret', session: 'sess_secret' })
    const hash = hashCacheAffinityKey(raw)
    // Public-safe digest shape, neutral prefix, no raw identifiers leaked.
    expect(hash).toMatch(/^cacheaff:fnv1a32:[0-9a-f]{8}$/u)
    expect(hash.includes('acct_secret')).toBe(false)
    expect(hash.includes('sess_secret')).toBe(false)
  })

  test('the session-affinity VALUE sent to the provider is a hash, not the raw key', () => {
    const raw = deriveCacheAffinityKey({ account: 'acct_1', session: 'sess_a' })
    const value = deriveSessionAffinityValue(raw)
    expect(value.includes('acct_1')).toBe(false)
    expect(value.includes('sess_a')).toBe(false)
    // Deterministic so a session's turns pin to the same replica.
    expect(deriveSessionAffinityValue(raw)).toBe(value)
  })

  test('sessionAffinityParams sets BOTH x-session-affinity (Fireworks) and user (OpenAI) to the same value', () => {
    const value = deriveSessionAffinityValue(deriveCacheAffinityKey({ account: 'acct_1' }))
    const params = sessionAffinityParams(value)
    expect(params['x-session-affinity']).toBe(value)
    expect(params['user']).toBe(value)
  })
})

describe('cached-token telemetry + totalTokens reconciliation (P0-2 item 5)', () => {
  test('reconciles the live discrepancy: total 679 != prompt 347 + completion 20 (reasoning tokens)', () => {
    // The exact live numbers from the issue: a Gemini-backed khala-mini reply
    // whose totalTokenCount includes thinking/tool-use tokens beyond prompt+completion.
    const reconciled = reconcileUsageTokens({
      completionTokens: 20,
      promptTokens: 347,
      totalTokens: 679,
    })
    // The provider's authoritative total is recorded receipt-first (never recomputed).
    expect(reconciled.totalTokens).toBe(679)
    // The gap (679 - 367 = 312) is the real billed reasoning/thinking dimension,
    // disclosed honestly rather than dropped or treated as an error.
    expect(reconciled.unaccountedTokens).toBe(312)
    expect(reconciled.hasUnaccountedTokens).toBe(true)
  })

  test('a normal response where total == prompt + completion has zero unaccounted tokens', () => {
    const reconciled = reconcileUsageTokens({
      completionTokens: 12,
      promptTokens: 19,
      totalTokens: 31,
    })
    expect(reconciled.unaccountedTokens).toBe(0)
    expect(reconciled.hasUnaccountedTokens).toBe(false)
  })

  test('cached input tokens flow through when the provider reports them', () => {
    const reconciled = reconcileUsageTokens({
      cachedPromptTokens: 200,
      completionTokens: 20,
      promptTokens: 347,
      totalTokens: 367,
    })
    expect(reconciled.cachedPromptTokens).toBe(200)
  })

  test('cachedPromptTokens is undefined (honest not_measured upstream) when the provider omits it', () => {
    const reconciled = reconcileUsageTokens({
      completionTokens: 20,
      promptTokens: 347,
      totalTokens: 367,
    })
    expect(reconciled.cachedPromptTokens).toBeUndefined()
  })

  test('a degenerate total below prompt+completion floors the unaccounted delta at 0 (never negative)', () => {
    const reconciled = reconcileUsageTokens({
      completionTokens: 20,
      promptTokens: 347,
      totalTokens: 100,
    })
    expect(reconciled.unaccountedTokens).toBe(0)
    // The provider total is left as reported (we never fabricate a corrected total).
    expect(reconciled.totalTokens).toBe(100)
  })
})
