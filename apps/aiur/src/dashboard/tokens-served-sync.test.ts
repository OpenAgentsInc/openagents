import {
  ChangelogEntry,
  DeltaFrame,
  encodePublicCounterEntity,
  EntityId,
  EntityType,
  PUBLIC_COUNTER_ENTITY_TYPE,
  PublicCounterEntity,
  SyncVersion,
} from '@openagentsinc/khala-sync'
import { describe, expect, test } from 'vitest'

import {
  applyTokensServedDelta,
  buildTokensServedBootstrapRequest,
  extractTokensServedSnapshot,
  TOKENS_SERVED_SCOPE,
} from './tokens-served-sync'

const entityId = (value: string) => EntityId.make(value)
const entityType = (value: string) => EntityType.make(value)

const counterPostImage = (total: number, lastEventAt: string | null) =>
  encodePublicCounterEntity(
    new PublicCounterEntity({
      counterId: 'tokens-served',
      total,
      lastEventAt,
    }),
  )

describe('buildTokensServedBootstrapRequest', () => {
  test('targets the public tokens-served scope', () => {
    const request = buildTokensServedBootstrapRequest('aiur-dashboard')
    expect(String(request.scope)).toBe(String(TOKENS_SERVED_SCOPE))
    expect(String(TOKENS_SERVED_SCOPE)).toBe('scope.public.tokens-served')
  })
})

describe('extractTokensServedSnapshot', () => {
  test('decodes the matching bootstrap entity', () => {
    const snapshot = extractTokensServedSnapshot([
      {
        entityType: entityType(PUBLIC_COUNTER_ENTITY_TYPE),
        entityId: entityId('tokens-served'),
        postImageJson: JSON.stringify(counterPostImage(42, '2026-07-06T00:00:00Z')),
      },
      {
        entityType: entityType('something_else'),
        entityId: entityId('ignored'),
        postImageJson: '{}',
      },
    ])
    expect(snapshot).toEqual({ total: 42, lastEventAt: '2026-07-06T00:00:00Z' })
  })

  test('returns undefined when no matching entity is present', () => {
    expect(extractTokensServedSnapshot([])).toBeUndefined()
  })

  test('returns undefined for an undecodable post-image', () => {
    const snapshot = extractTokensServedSnapshot([
      {
        entityType: entityType(PUBLIC_COUNTER_ENTITY_TYPE),
        entityId: entityId('tokens-served'),
        postImageJson: 'not json',
      },
    ])
    expect(snapshot).toBeUndefined()
  })
})

describe('applyTokensServedDelta', () => {
  test('updates the snapshot from a matching upsert entry', () => {
    const frame = new DeltaFrame({
      scope: TOKENS_SERVED_SCOPE,
      cursor: SyncVersion.make(7),
      entries: [
        new ChangelogEntry({
          scope: TOKENS_SERVED_SCOPE,
          version: SyncVersion.make(7),
          entityType: entityType(PUBLIC_COUNTER_ENTITY_TYPE),
          entityId: entityId('tokens-served'),
          op: 'upsert',
          postImageJson: JSON.stringify(counterPostImage(100, '2026-07-06T01:00:00Z')),
          committedAt: '2026-07-06T01:00:00Z',
        }),
      ],
    })

    const next = applyTokensServedDelta(
      { total: 42, lastEventAt: '2026-07-06T00:00:00Z' },
      frame,
    )
    expect(next).toEqual({ total: 100, lastEventAt: '2026-07-06T01:00:00Z' })
  })

  test('ignores entries for other entity types/ids and returns the same snapshot', () => {
    const frame = new DeltaFrame({
      scope: TOKENS_SERVED_SCOPE,
      cursor: SyncVersion.make(8),
      entries: [
        new ChangelogEntry({
          scope: TOKENS_SERVED_SCOPE,
          version: SyncVersion.make(8),
          entityType: entityType('other_type'),
          entityId: entityId('tokens-served'),
          op: 'upsert',
          postImageJson: '{}',
          committedAt: '2026-07-06T01:01:00Z',
        }),
      ],
    })

    const current = { total: 42, lastEventAt: '2026-07-06T00:00:00Z' } as const
    expect(applyTokensServedDelta(current, frame)).toBe(current)
  })

  test('a delete entry is ignored (counters never delete)', () => {
    const frame = new DeltaFrame({
      scope: TOKENS_SERVED_SCOPE,
      cursor: SyncVersion.make(9),
      entries: [
        new ChangelogEntry({
          scope: TOKENS_SERVED_SCOPE,
          version: SyncVersion.make(9),
          entityType: entityType(PUBLIC_COUNTER_ENTITY_TYPE),
          entityId: entityId('tokens-served'),
          op: 'delete',
          committedAt: '2026-07-06T01:02:00Z',
        }),
      ],
    })

    const current = { total: 42, lastEventAt: '2026-07-06T00:00:00Z' } as const
    expect(applyTokensServedDelta(current, frame)).toBe(current)
  })
})
