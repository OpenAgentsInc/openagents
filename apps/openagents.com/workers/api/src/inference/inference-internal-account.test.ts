import { describe, expect, test } from 'vitest'

import {
  INTERNAL_ACCOUNT_DEMAND_SOURCE,
  applyInternalAccountAttribution,
  parseInternalAccountRefs,
} from './inference-internal-account'
import { type ServedTokensRequestAttribution } from './served-tokens-recorder'

const OPS = 'agent:user_7a6d6dab-b198-4ee8-b7bf-2e00385f9139'

describe('parseInternalAccountRefs (#6298)', () => {
  test('unset => empty set (no-op)', () => {
    expect(parseInternalAccountRefs(undefined).size).toBe(0)
  })

  test('blank / whitespace / empty entries => empty set', () => {
    expect(parseInternalAccountRefs('').size).toBe(0)
    expect(parseInternalAccountRefs('   ').size).toBe(0)
    expect(parseInternalAccountRefs(' , ,, ').size).toBe(0)
  })

  test('parses + trims a comma-separated list', () => {
    const refs = parseInternalAccountRefs(` ${OPS} , agent:user_b `)
    expect(refs.has(OPS)).toBe(true)
    expect(refs.has('agent:user_b')).toBe(true)
    expect(refs.size).toBe(2)
  })
})

describe('applyInternalAccountAttribution (#6298)', () => {
  const allowlist = new Set([OPS])

  test('internal account + NO header => internal / internal_account', () => {
    const out = applyInternalAccountAttribution(undefined, OPS, allowlist)
    expect(out).toEqual({
      demandKind: 'internal',
      demandSource: INTERNAL_ACCOUNT_DEMAND_SOURCE,
    })
  })

  test('internal account + specific internal-source header => source preserved (no downgrade)', () => {
    const header: ServedTokensRequestAttribution = {
      demandKind: 'internal',
      demandSource: 'harbor_terminal_bench',
    }
    const out = applyInternalAccountAttribution(header, OPS, allowlist)
    expect(out).toEqual({
      demandKind: 'internal',
      demandSource: 'harbor_terminal_bench',
    })
  })

  test('internal account + internal_stress header keeps the stress kind', () => {
    const header: ServedTokensRequestAttribution = {
      demandKind: 'internal_stress',
      demandSource: 'glm-stress',
    }
    const out = applyInternalAccountAttribution(header, OPS, allowlist)
    expect(out).toEqual({
      demandKind: 'internal_stress',
      demandSource: 'glm-stress',
    })
  })

  test('internal account + a NON-internal header kind => forced internal, generic source', () => {
    // A request that somehow sent `external` (or any non-internal kind) but is on
    // the ops account is still forced to internal with the generic marker (the
    // account rule wins; it never lets a header keep the dogfood external).
    const header: ServedTokensRequestAttribution = {
      demandKind: 'unlabeled',
      demandClient: 'opencode',
    }
    const out = applyInternalAccountAttribution(header, OPS, allowlist)
    expect(out).toEqual({
      demandKind: 'internal',
      demandSource: INTERNAL_ACCOUNT_DEMAND_SOURCE,
      demandClient: 'opencode',
    })
  })

  test('internal account + internal kind but NO source => generic source default', () => {
    const header: ServedTokensRequestAttribution = { demandKind: 'internal' }
    const out = applyInternalAccountAttribution(header, OPS, allowlist)
    expect(out).toEqual({
      demandKind: 'internal',
      demandSource: INTERNAL_ACCOUNT_DEMAND_SOURCE,
    })
  })

  test('non-internal account => unchanged (undefined stays undefined)', () => {
    expect(
      applyInternalAccountAttribution(undefined, 'agent:external-user', allowlist),
    ).toBeUndefined()
  })

  test('non-internal account => header attribution returned as-is', () => {
    const header: ServedTokensRequestAttribution = {
      demandKind: 'unlabeled',
      demandClient: 'opencode',
    }
    expect(
      applyInternalAccountAttribution(header, 'agent:external-user', allowlist),
    ).toBe(header)
  })

  test('empty allowlist => pure no-op even for what would be the ops account', () => {
    const empty = new Set<string>()
    expect(applyInternalAccountAttribution(undefined, OPS, empty)).toBeUndefined()
    const header: ServedTokensRequestAttribution = { demandKind: 'unlabeled' }
    expect(applyInternalAccountAttribution(header, OPS, empty)).toBe(header)
  })
})
