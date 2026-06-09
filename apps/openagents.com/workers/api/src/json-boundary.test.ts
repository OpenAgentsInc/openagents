import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  parseBase64UrlJsonRecord,
  parseJsonRecord,
  parseJsonStringArray,
  parseJsonWithSchema,
} from './json-boundary'

describe('Worker JSON boundary decoders', () => {
  test('decodes valid JSON records', () => {
    expect(parseJsonRecord('{"status":"ok"}')).toEqual({ status: 'ok' })
  })

  test('returns undefined for malformed JSON and non-record values', () => {
    expect(parseJsonRecord('{bad json')).toBeUndefined()
    expect(parseJsonRecord('["not","a","record"]')).toBeUndefined()
  })

  test('throws schema decode failures at named boundaries', () => {
    const ResponseSchema = S.Struct({ id: S.String })

    expect(() => parseJsonWithSchema(ResponseSchema, '{"id":1}')).toThrow()
  })

  test('keeps mixed string-array compatibility for stored scopes', () => {
    expect(parseJsonStringArray('["repo", 123, "workflow", null]')).toEqual([
      'repo',
      'workflow',
    ])
  })

  test('decodes base64url JWT claim records', () => {
    const payload = Buffer.from(
      JSON.stringify({
        organizations: [{ id: 'org_1' }],
      }),
    ).toString('base64url')

    expect(parseBase64UrlJsonRecord(payload)).toEqual({
      organizations: [{ id: 'org_1' }],
    })
  })
})
