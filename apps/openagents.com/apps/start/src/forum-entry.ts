// #8634 / #8635 scope 5 (live cutover): browser entry for the retained
// /forum* Effect Native surface as served by the Cloud Run monolith
// (workers/api/src/cloudrun/forum-ui.ts) — the same portal-entry.ts pattern.
//
// The forum content is the ONE typed Effect Native view tree authored in
// `routes/-forum-page.tsx` (#8635). This entry only parses the live URL into
// the exact ForumRouteParams contract the TanStack Start route shims use and
// mounts the tree through the DOM renderer. No React shell here — the DOM
// renderer mounts the typed view program directly. Authority is untouched:
// every read/write stays on the existing public Worker /api/forum* contracts
// inside -forum-data.ts.

import { Effect, Exit, Scope } from '@effect-native/core/effect'

import { parseTopicPostSortDirection } from './routes/-forum-data'
import { mountForumSurface, type ForumRouteParams } from './routes/-forum-page'

/**
 * Parse a live document URL into the forum route contract. Returns null for
 * any path this surface does not own (those must never have been routed here
 * by the monolith — see forum-ui.ts route ownership).
 *
 * URL contract (deep-link stable, identical to the legacy Foldkit page and
 * the TanStack Start routes):
 *   /forum                      -> { kind: 'index' }
 *   /forum/f/$forumRef          -> { kind: 'forum', forumRef }
 *   /forum/t/$topicId?sortDir   -> { kind: 'topic', topicId, sortDirection }
 *   /forum/receipts/$receiptRef -> { kind: 'receipt', receiptRef }
 */
export const parseForumEntryRoute = (
  pathname: string,
  search: string,
): ForumRouteParams | null => {
  const path = pathname.replace(/\/+$/, '') || '/'
  if (path === '/forum') {
    return { kind: 'index' }
  }
  const segments = path.split('/').filter((segment) => segment.length > 0)
  if (segments.length !== 3 || segments[0] !== 'forum') {
    return null
  }
  const decode = (raw: string): string => {
    try {
      return decodeURIComponent(raw)
    } catch {
      return raw
    }
  }
  const ref = decode(segments[2]!)
  if (segments[1] === 'f') {
    return { kind: 'forum', forumRef: ref }
  }
  if (segments[1] === 't') {
    return {
      kind: 'topic',
      topicId: ref,
      sortDirection: parseTopicPostSortDirection(search),
    }
  }
  if (segments[1] === 'receipts') {
    return { kind: 'receipt', receiptRef: ref }
  }
  return null
}

const boot = async (): Promise<void> => {
  const root = document.getElementById('forum-root')
  if (root === null) {
    return
  }
  const params = parseForumEntryRoute(
    window.location.pathname,
    window.location.search,
  )
  if (params === null) {
    return
  }
  const scope = await Effect.runPromise(Scope.make())
  window.addEventListener('pagehide', () => {
    void Effect.runPromise(Scope.close(scope, Exit.void))
  })
  await Effect.runPromise(
    Scope.provide(scope)(mountForumSurface(root, params)),
  )
}

if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      void boot()
    })
  } else {
    void boot()
  }
}
