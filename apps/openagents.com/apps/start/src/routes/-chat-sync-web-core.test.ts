import { describe, expect, test } from 'vitest'

import {
  applyDeltaFrameOfType,
  buildBootstrapRequestBody,
  buildConnectUrl,
  buildPushRequestBody,
  entitiesOfType,
  KHALA_SYNC_WEB_BOOTSTRAP_PATH,
  KHALA_SYNC_WEB_CONNECT_PATH,
  KHALA_SYNC_WEB_PUSH_PATH,
  makeSafeRef,
  sortByKeyAsc,
  sortByKeyDesc,
  stableArgsJson,
} from './-chat-sync-web-core'

type Item = Readonly<{ id: string; body: string; at: string }>
const decode = (value: unknown): Item => value as Item
const idOf = (item: Item): string => item.id

describe('Khala Sync web wire helpers (#8413)', () => {
  test('path constants point at the local same-origin proxy, never a remote host', () => {
    expect(KHALA_SYNC_WEB_BOOTSTRAP_PATH).toBe('/api/khala-sync/bootstrap')
    expect(KHALA_SYNC_WEB_PUSH_PATH).toBe('/api/khala-sync/push')
    expect(KHALA_SYNC_WEB_CONNECT_PATH).toBe('/api/khala-sync/connect')
  })

  test('buildBootstrapRequestBody matches the wire shape', () => {
    expect(buildBootstrapRequestBody('scope.user.u1', 'group-1')).toEqual({
      clientGroupId: 'group-1',
      protocolVersion: 1,
      schemaVersion: 1,
      scope: 'scope.user.u1',
    })
  })

  test('buildConnectUrl carries scope + cursor and maps https to wss on the local origin', () => {
    expect(
      buildConnectUrl('scope.thread.t1', 3, { protocol: 'https:', host: 'openagents-com-start-staging.workers.dev' }),
    ).toBe(
      'wss://openagents-com-start-staging.workers.dev/api/khala-sync/connect?scope=scope.thread.t1&cursor=3',
    )
  })

  test('buildConnectUrl maps http to ws for local dev', () => {
    expect(buildConnectUrl('scope.thread.t1', 0, { protocol: 'http:', host: 'localhost:3000' })).toBe(
      'ws://localhost:3000/api/khala-sync/connect?scope=scope.thread.t1&cursor=0',
    )
  })

  test('entitiesOfType filters and decodes only the requested entity type', () => {
    const rows = [
      {
        entityId: 't1',
        entityType: 'chat_thread',
        postImageJson: JSON.stringify({ id: 't1', body: 'thread', at: '2026-01-01T00:00:00Z' }),
      },
      {
        entityId: 'm1',
        entityType: 'chat_message',
        postImageJson: JSON.stringify({ id: 'm1', body: 'hi', at: '2026-01-01T00:00:01Z' }),
      },
    ]
    expect(entitiesOfType(rows, 'chat_message', decode)).toEqual([
      { id: 'm1', body: 'hi', at: '2026-01-01T00:00:01Z' },
    ])
  })

  test('applyDeltaFrameOfType upserts a new item', () => {
    const current: ReadonlyArray<Item> = []
    const frame = {
      _tag: 'DeltaFrame',
      entries: [
        {
          entityId: 'm1',
          entityType: 'chat_message',
          op: 'upsert',
          postImageJson: JSON.stringify({ id: 'm1', body: 'hi', at: '2026-01-01T00:00:01Z' }),
        },
      ],
    }
    expect(applyDeltaFrameOfType(current, frame, 'chat_message', idOf, decode)).toEqual([
      { id: 'm1', body: 'hi', at: '2026-01-01T00:00:01Z' },
    ])
  })

  test('applyDeltaFrameOfType replaces an existing item by id', () => {
    const current: ReadonlyArray<Item> = [{ id: 'm1', body: 'hi', at: '2026-01-01T00:00:01Z' }]
    const frame = {
      _tag: 'DeltaFrame',
      entries: [
        {
          entityId: 'm1',
          entityType: 'chat_message',
          op: 'upsert',
          postImageJson: JSON.stringify({ id: 'm1', body: 'edited', at: '2026-01-01T00:00:02Z' }),
        },
      ],
    }
    expect(applyDeltaFrameOfType(current, frame, 'chat_message', idOf, decode)).toEqual([
      { id: 'm1', body: 'edited', at: '2026-01-01T00:00:02Z' },
    ])
  })

  test('applyDeltaFrameOfType removes an item on delete', () => {
    const current: ReadonlyArray<Item> = [{ id: 'm1', body: 'hi', at: '2026-01-01T00:00:01Z' }]
    const frame = {
      _tag: 'DeltaFrame',
      entries: [{ entityId: 'm1', entityType: 'chat_message', op: 'delete' }],
    }
    expect(applyDeltaFrameOfType(current, frame, 'chat_message', idOf, decode)).toEqual([])
  })

  test('applyDeltaFrameOfType ignores entries of a different entity type', () => {
    const current: ReadonlyArray<Item> = []
    const frame = {
      _tag: 'DeltaFrame',
      entries: [
        {
          entityId: 't1',
          entityType: 'chat_thread',
          op: 'upsert',
          postImageJson: JSON.stringify({ id: 't1', body: 'thread', at: '2026-01-01T00:00:00Z' }),
        },
      ],
    }
    expect(applyDeltaFrameOfType(current, frame, 'chat_message', idOf, decode)).toEqual([])
  })

  test('sortByKeyAsc / sortByKeyDesc order by ISO timestamp string', () => {
    const items: ReadonlyArray<Item> = [
      { id: 'a', body: '', at: '2026-01-02T00:00:00Z' },
      { id: 'b', body: '', at: '2026-01-01T00:00:00Z' },
    ]
    expect(sortByKeyAsc(items, i => i.at).map(i => i.id)).toEqual(['b', 'a'])
    expect(sortByKeyDesc(items, i => i.at).map(i => i.id)).toEqual(['a', 'b'])
  })

  test('buildPushRequestBody matches the wire shape', () => {
    expect(
      buildPushRequestBody({
        clientGroupId: 'group-1',
        clientId: 'client-1',
        mutations: [{ argsJson: '{}', mutationId: 1, name: 'chat.appendMessage' }],
      }),
    ).toEqual({
      clientGroupId: 'group-1',
      clientId: 'client-1',
      mutations: [{ argsJson: '{}', mutationId: 1, name: 'chat.appendMessage' }],
      protocolVersion: 1,
      schemaVersion: 1,
    })
  })

  test('stableArgsJson sorts keys and drops undefined values', () => {
    expect(stableArgsJson({ threadId: 't1', body: 'hi', messageId: undefined as unknown as string })).toBe(
      JSON.stringify({ body: 'hi', threadId: 't1' }),
    )
  })

  test('makeSafeRef produces a Khala Sync safe-ref-shaped id', () => {
    const ref = makeSafeRef('thread')
    expect(ref).toMatch(/^thread\.[A-Za-z0-9._:-]+$/)
    expect(makeSafeRef('thread')).not.toBe(ref)
  })
})
