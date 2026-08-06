// PRO-101: the shared guarded path-ref decode seam.
//
// `decodeURIComponent('%')` throws `URIError`. Every public receipt reader used
// to call it unguarded inside its path matcher, and those matchers run inside
// the `Effect.gen` body of `makeWorkerRouteRequest` — so the throw was a
// DEFECT, not a typed failure. It escaped to the `Effect.catchCause` in
// `index.ts`, which answered 500 `internal_server_error` and filed a
// `severity: 'critical'` `unhandled_exception` backend incident, while a
// well-formed unknown ref correctly answered 404.

import { describe, expect, test } from 'vitest'

import { pathRefFromPrefix, safeDecodeUriComponent } from './router'

const PREFIX = '/api/public/cloud/receipts/'

describe('safeDecodeUriComponent', () => {
  test('decodes a well-formed escape', () => {
    expect(safeDecodeUriComponent('receipt%2Eabc')).toBe('receipt.abc')
  })

  test('returns undefined instead of throwing on a malformed escape', () => {
    // The exact input from the production repro.
    expect(() => decodeURIComponent('%')).toThrow(URIError)
    expect(safeDecodeUriComponent('%')).toBeUndefined()
    expect(safeDecodeUriComponent('%zz')).toBeUndefined()
    expect(safeDecodeUriComponent('%E0%A4%A')).toBeUndefined()
  })
})

describe('pathRefFromPrefix', () => {
  test('does not claim a path outside the prefix', () => {
    expect(pathRefFromPrefix('/api/public/other/thing', PREFIX)).toEqual({
      _tag: 'no_match',
    })
  })

  test('does not claim the bare prefix with no ref after it', () => {
    expect(pathRefFromPrefix('/api/public/cloud/receipts/', PREFIX)).toEqual({
      _tag: 'no_match',
    })
  })

  test('returns the whole decoded tail as one opaque ref', () => {
    expect(pathRefFromPrefix(`${PREFIX}receipt.test.abc`, PREFIX)).toEqual({
      _tag: 'ref',
      ref: 'receipt.test.abc',
    })
    expect(pathRefFromPrefix(`${PREFIX}receipt%2Ftest`, PREFIX)).toEqual({
      _tag: 'ref',
      ref: 'receipt/test',
    })
  })

  test('reports a malformed escape rather than throwing', () => {
    expect(pathRefFromPrefix(`${PREFIX}%`, PREFIX)).toEqual({
      _tag: 'malformed',
    })
    expect(pathRefFromPrefix(`${PREFIX}receipt.%zz`, PREFIX)).toEqual({
      _tag: 'malformed',
    })
  })

  test('a malformed tail is still CLAIMED, so the reader can 404 it itself', () => {
    // The distinction that makes 404-not-500 possible: `no_match` means fall
    // through to the next route, `malformed` means this reader owns the path
    // and answers it exactly as it answers an unknown ref.
    expect(pathRefFromPrefix(`${PREFIX}%`, PREFIX)._tag).not.toBe('no_match')
  })
})
