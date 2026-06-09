import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { publicProductPromisesDocument } from './product-promises'

const ProductPromiseState = S.Literals([
  'degraded',
  'green',
  'planned',
  'red',
  'withdrawn',
  'yellow',
])

const ProductPromise = S.Struct({
  audience: S.Array(S.String),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  claim: S.String,
  evidenceRefs: S.Array(S.String),
  productArea: S.String,
  promiseId: S.String,
  reportPath: S.String,
  safeCopy: S.String,
  sourceRefs: S.Array(S.String),
  state: ProductPromiseState,
  unsafeCopy: S.String,
  verification: S.String,
})

const ProductPromisesDocument = S.Struct({
  canonicalDocsUrl: S.String,
  currentMonorepoStatus: S.Struct({
    caveats: S.Array(S.String),
    liveDeploymentRefs: S.Array(S.String),
    pylonV03Refs: S.Array(S.String),
    status: S.String,
    summary: S.String,
  }),
  latestGapAuditUrl: S.String,
  lastUpdated: S.String,
  notes: S.Array(S.String),
  promises: S.Array(ProductPromise),
  publicDocsUrl: S.String,
  reportPath: S.Struct({
    defaultForumUrl: S.String,
    forumSlug: S.String,
    forumTopicApi: S.String,
    rule: S.String,
    strictBugForm: S.String,
  }),
  schemaVersion: S.String,
  sourceRefs: S.Array(S.String),
  states: S.Record(S.String, S.String),
  version: S.String,
})

describe('public product promises document', () => {
  test('matches the browser-facing schema', () => {
    const decoded = S.decodeUnknownSync(ProductPromisesDocument)(
      publicProductPromisesDocument(),
    )

    expect(decoded.version).toBe('2026-06-09.5')
    expect(decoded.sourceRefs.length).toBeGreaterThan(0)
    expect(decoded.promises.length).toBeGreaterThan(0)
    expect(
      decoded.promises.every(promise => promise.sourceRefs.length > 0),
    ).toBe(true)
  })
})
