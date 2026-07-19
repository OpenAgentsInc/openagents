import { afterEach, describe, expect, test } from 'vitest'

import {
  configureOpenAgentsAdminEmailsFromEnv,
  getOpenAgentsAdminEmails,
  isOpenAgentsAdminEmail,
  resetOpenAgentsAdminEmailsForTest,
} from './admin-identity'

describe('OpenAgents admin allowlist (SARAH-ACT-1, #9065)', () => {
  afterEach(() => {
    resetOpenAgentsAdminEmailsForTest()
  })

  test('defaults to the single compiled admin email when unconfigured', () => {
    configureOpenAgentsAdminEmailsFromEnv(undefined)
    expect(getOpenAgentsAdminEmails()).toEqual(['chris@openagents.com'])
    expect(isOpenAgentsAdminEmail('chris@openagents.com')).toBe(true)
  })

  test('keeps the compiled default for an empty or blank env value', () => {
    configureOpenAgentsAdminEmailsFromEnv('')
    expect(getOpenAgentsAdminEmails()).toEqual(['chris@openagents.com'])

    configureOpenAgentsAdminEmailsFromEnv('   ')
    expect(getOpenAgentsAdminEmails()).toEqual(['chris@openagents.com'])
  })

  test('keeps the compiled default when the env value has no usable emails', () => {
    configureOpenAgentsAdminEmailsFromEnv(' , , ')
    expect(getOpenAgentsAdminEmails()).toEqual(['chris@openagents.com'])
  })

  test('replaces the admin list with a configured comma-separated value', () => {
    configureOpenAgentsAdminEmailsFromEnv('Owner@Example.com, second@example.com')
    expect(getOpenAgentsAdminEmails()).toEqual([
      'owner@example.com',
      'second@example.com',
    ])
    expect(isOpenAgentsAdminEmail('owner@example.com')).toBe(true)
    expect(isOpenAgentsAdminEmail(' Owner@Example.com ')).toBe(true)
    expect(isOpenAgentsAdminEmail('chris@openagents.com')).toBe(false)
  })

  test('trims, lowercases, and dedupes configured emails', () => {
    configureOpenAgentsAdminEmailsFromEnv(
      'Dup@Example.com, dup@example.com , dup@example.com',
    )
    expect(getOpenAgentsAdminEmails()).toEqual(['dup@example.com'])
  })

  test('isOpenAgentsAdminEmail stays case-insensitive and whitespace-tolerant', () => {
    configureOpenAgentsAdminEmailsFromEnv(undefined)
    expect(isOpenAgentsAdminEmail(' Chris@OpenAgents.com ')).toBe(true)
    expect(isOpenAgentsAdminEmail('someone-else@example.com')).toBe(false)
  })

  test('reconfiguring on a later call replaces the prior configured list', () => {
    configureOpenAgentsAdminEmailsFromEnv('first@example.com')
    expect(getOpenAgentsAdminEmails()).toEqual(['first@example.com'])

    configureOpenAgentsAdminEmailsFromEnv('second@example.com')
    expect(getOpenAgentsAdminEmails()).toEqual(['second@example.com'])
    expect(isOpenAgentsAdminEmail('first@example.com')).toBe(false)
  })
})
