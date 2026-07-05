import { describe, expect, test } from 'vitest'

import {
  readAgentBearerToken,
  readBearerToken,
  readPrefixedBearerToken,
} from './bearer-token'

const requestWithAuthorization = (authorization?: string): Request =>
  new Request('https://openagents.test/example', {
    ...(authorization === undefined
      ? {}
      : { headers: { authorization } }),
  })

describe('bearer token auth helpers', () => {
  test('reads case-insensitive bearer credentials', () => {
    expect(readBearerToken(requestWithAuthorization('Bearer oa_agent_token'))).toBe(
      'oa_agent_token',
    )
    expect(readBearerToken(requestWithAuthorization('bearer oa_agent_token'))).toBe(
      'oa_agent_token',
    )
  })

  test('preserves existing split parser behavior for extra segments', () => {
    expect(
      readBearerToken(requestWithAuthorization('Bearer oa_agent_token ignored')),
    ).toBe('oa_agent_token')
  })

  test('returns undefined for missing or non-bearer authorization', () => {
    expect(readBearerToken(requestWithAuthorization())).toBeUndefined()
    expect(readBearerToken(requestWithAuthorization('Basic abc'))).toBeUndefined()
    expect(readBearerToken(requestWithAuthorization('Bearer'))).toBeUndefined()
  })

  test('filters bearer credentials by required prefix', () => {
    expect(
      readPrefixedBearerToken(
        requestWithAuthorization('Bearer oa_agent_token'),
        'oa_agent_',
      ),
    ).toBe('oa_agent_token')
    expect(
      readPrefixedBearerToken(
        requestWithAuthorization('Bearer other_token'),
        'oa_agent_',
      ),
    ).toBeUndefined()
  })

  test('reads OpenAgents programmatic agent bearer credentials', () => {
    expect(
      readAgentBearerToken(requestWithAuthorization('Bearer oa_agent_token')),
    ).toBe('oa_agent_token')
    expect(
      readAgentBearerToken(requestWithAuthorization('Bearer other_token')),
    ).toBeUndefined()
  })
})
