import { describe, expect, test } from 'vitest'

import {
  nestedUnknown,
  parseEmbeddedJsonRecord,
  parseJsonValue,
} from './json-boundary'

describe('sync-schema JSON boundary decoders', () => {
  test('decodes embedded runner log JSON records', () => {
    expect(
      parseEmbeddedJsonRecord('stdout: {"usage":{"total_tokens":12}}'),
    ).toEqual({
      usage: { total_tokens: 12 },
    })
  })

  test('ignores malformed JSON values', () => {
    expect(parseJsonValue('{bad json')).toBeUndefined()
  })

  test('reads nested token usage fields from decoded records', () => {
    const record = parseEmbeddedJsonRecord('{"usage":{"input_tokens":4}}')

    expect(nestedUnknown(record, ['usage', 'input_tokens'])).toBe(4)
  })
})
