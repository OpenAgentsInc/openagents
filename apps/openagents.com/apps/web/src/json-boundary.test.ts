import { describe, expect, test } from 'vitest'

import { nestedUnknown, parseJsonRecord } from './json-boundary'

describe('browser JSON boundary decoders', () => {
  test('decodes stored projection JSON records', () => {
    expect(parseJsonRecord('{"payload":{"message":"done"}}')).toEqual({
      payload: { message: 'done' },
    })
  })

  test('ignores malformed stored projection JSON', () => {
    expect(parseJsonRecord('{bad json')).toBeUndefined()
  })

  test('reads nested array paths for runner payload compatibility', () => {
    const record = parseJsonRecord('{"items":[{"text":"first"}]}')

    expect(nestedUnknown(record, ['items', '0', 'text'])).toBe('first')
  })
})
