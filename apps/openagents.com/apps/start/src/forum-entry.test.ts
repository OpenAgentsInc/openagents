// #8634 / #8635 scope 5 (live cutover): the monolith-served forum entry must
// parse live URLs into the exact ForumRouteParams contract the TanStack
// Start route shims use — deep links stay stable across the host cutover.
import { describe, expect, test } from 'vitest'

import { parseForumEntryRoute } from './forum-entry'

describe('forum entry route parsing (#8634 live cutover)', () => {
  test('parses the board index', () => {
    expect(parseForumEntryRoute('/forum', '')).toEqual({ kind: 'index' })
    expect(parseForumEntryRoute('/forum/', '')).toEqual({ kind: 'index' })
  })

  test('parses a forum topic list', () => {
    expect(parseForumEntryRoute('/forum/f/product-promises', '')).toEqual({
      forumRef: 'product-promises',
      kind: 'forum',
    })
  })

  test('parses a topic with the default and explicit sort directions', () => {
    expect(
      parseForumEntryRoute(
        '/forum/t/55555555-5555-4555-8555-555555555555',
        '',
      ),
    ).toEqual({
      kind: 'topic',
      sortDirection: 'asc',
      topicId: '55555555-5555-4555-8555-555555555555',
    })
    expect(
      parseForumEntryRoute(
        '/forum/t/55555555-5555-4555-8555-555555555555',
        '?sortDir=desc',
      ),
    ).toEqual({
      kind: 'topic',
      sortDirection: 'desc',
      topicId: '55555555-5555-4555-8555-555555555555',
    })
  })

  test('parses a receipt route', () => {
    expect(parseForumEntryRoute('/forum/receipts/receipt_1', '')).toEqual({
      kind: 'receipt',
      receiptRef: 'receipt_1',
    })
  })

  test('decodes URI-encoded refs', () => {
    expect(parseForumEntryRoute('/forum/f/a%20b', '')).toEqual({
      forumRef: 'a b',
      kind: 'forum',
    })
  })

  test('does not own unrelated or deeper paths', () => {
    expect(parseForumEntryRoute('/', '')).toBeNull()
    expect(parseForumEntryRoute('/forums', '')).toBeNull()
    expect(parseForumEntryRoute('/forum/app.js', '')).toBeNull()
    expect(parseForumEntryRoute('/forum/f/a/b', '')).toBeNull()
    expect(parseForumEntryRoute('/forum/u/actor_1', '')).toBeNull()
    expect(parseForumEntryRoute('/promises', '')).toBeNull()
  })
})
