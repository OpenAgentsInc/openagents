import { describe, expect, test } from 'vitest'

import {
  decodeShareCreateIdempotencyKey,
  deriveShareCreateId,
  matchesShareCreateReplay,
} from './share-create-idempotency'
import type { ShareProjectionRecord } from './share-projections'

const record = (): ShareProjectionRecord =>
  ({
    audience: { _tag: 'Public' },
    canonicalUrl:
      'https://openagents.com/share/95d814e9-7630-5683-b027-cb04d3308df8',
    createdAt: '2026-07-17T20:45:00.000Z',
    expiresAt: null,
    id: '95d814e9-7630-5683-b027-cb04d3308df8',
    ownerUserId: 'user.owner',
    projectId: null,
    projection: {
      audience: { _tag: 'Public' },
      audienceLabel: 'Shared publicly',
      canonicalUrl:
        'https://openagents.com/share/95d814e9-7630-5683-b027-cb04d3308df8',
      createdAt: '2026-07-17T20:45:00.000Z',
      files: [],
      artifacts: [],
      approvals: [],
      receipts: [],
      metrics: { durationMs: null, tokenCount: null, toolCallCount: 0 },
      messages: [],
      source: { kind: 'team-thread', id: 'thread.one', teamId: 'team.one' },
      status: 'active',
      subtitle: '',
      title: 'Thread one',
      version: 1,
    },
    redactionPolicyId: 'default',
    revokedAt: null,
    source: { kind: 'team-thread', id: 'thread.one', teamId: 'team.one' },
    status: 'active',
    summary: null,
    teamId: 'team.one',
    title: 'Thread one',
    updatedAt: '2026-07-17T20:45:00.000Z',
  }) as unknown as ShareProjectionRecord

describe('share create idempotency', () => {
  test('accepts only a bounded visible-ASCII key', () => {
    expect(decodeShareCreateIdempotencyKey(null)).toEqual({ _tag: 'Absent' })
    expect(decodeShareCreateIdempotencyKey('publication.1')).toEqual({
      _tag: 'Valid',
      value: 'publication.1',
    })
    expect(decodeShareCreateIdempotencyKey('')).toEqual({ _tag: 'Invalid' })
    expect(decodeShareCreateIdempotencyKey('has space')).toEqual({
      _tag: 'Invalid',
    })
    expect(decodeShareCreateIdempotencyKey('x'.repeat(129))).toEqual({
      _tag: 'Invalid',
    })
  })

  test('derives stable owner-scoped UUIDv5 identities', async () => {
    const first = await deriveShareCreateId('user.owner', 'publication.1')
    expect(first).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    )
    await expect(
      deriveShareCreateId('user.owner', 'publication.1'),
    ).resolves.toBe(first)
    await expect(
      deriveShareCreateId('user.other', 'publication.1'),
    ).resolves.not.toBe(first)
  })

  test('matches only the exact active unexpired create semantics', () => {
    const expected = {
      audience: { _tag: 'Public' as const },
      canonicalUrl: record().canonicalUrl,
      expiresAt: null,
      ownerUserId: 'user.owner',
      redactionPolicyId: 'default',
      source: {
        kind: 'team-thread' as const,
        id: 'thread.one',
        teamId: 'team.one',
      },
      title: 'Thread one',
    }
    expect(matchesShareCreateReplay(record(), expected, Date.now())).toBe(true)
    expect(
      matchesShareCreateReplay(
        { ...record(), ownerUserId: 'user.other' },
        expected,
        Date.now(),
      ),
    ).toBe(false)
    expect(
      matchesShareCreateReplay(
        { ...record(), status: 'revoked', revokedAt: '2026-07-17T20:46:00Z' },
        expected,
        Date.now(),
      ),
    ).toBe(false)
  })
})
