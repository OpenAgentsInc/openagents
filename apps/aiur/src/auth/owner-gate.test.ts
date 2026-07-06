import { describe, expect, test } from 'vitest'

import { isAllowedOwnerUserId, parseOwnerAllowlist } from './owner-gate'

describe('parseOwnerAllowlist', () => {
  test('undefined env yields an empty set', () => {
    expect(parseOwnerAllowlist(undefined).size).toBe(0)
  })

  test('empty string yields an empty set', () => {
    expect(parseOwnerAllowlist('').size).toBe(0)
  })

  test('parses a comma-separated list, trimming whitespace and dropping empties', () => {
    const allowlist = parseOwnerAllowlist(' user_1 , user_2,, user_3 ')
    expect([...allowlist].sort()).toEqual(['user_1', 'user_2', 'user_3'])
  })
})

describe('isAllowedOwnerUserId (fail-closed)', () => {
  test('denies every user id when the allowlist is empty (unconfigured)', () => {
    const emptyAllowlist = parseOwnerAllowlist(undefined)
    expect(isAllowedOwnerUserId('user_1', emptyAllowlist)).toBe(false)
    expect(isAllowedOwnerUserId(undefined, emptyAllowlist)).toBe(false)
  })

  test('denies a missing/blank user id even with a non-empty allowlist', () => {
    const allowlist = parseOwnerAllowlist('user_1')
    expect(isAllowedOwnerUserId(undefined, allowlist)).toBe(false)
    expect(isAllowedOwnerUserId('  ', allowlist)).toBe(false)
  })

  test('denies a non-owner user id present outside the allowlist', () => {
    const allowlist = parseOwnerAllowlist('user_1,user_2')
    expect(isAllowedOwnerUserId('user_3', allowlist)).toBe(false)
  })

  test('allows exactly the allow-listed owner user id', () => {
    const allowlist = parseOwnerAllowlist('user_1,user_2')
    expect(isAllowedOwnerUserId('user_1', allowlist)).toBe(true)
    expect(isAllowedOwnerUserId('user_2', allowlist)).toBe(true)
  })

  test('trims surrounding whitespace on the candidate user id', () => {
    const allowlist = parseOwnerAllowlist('user_1')
    expect(isAllowedOwnerUserId('  user_1  ', allowlist)).toBe(true)
  })
})
