import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import type { RouteTableEntry } from './route-table'
import { routeTable, urlToAppRoute } from './route'

// ---------------------------------------------------------------------------
// Client ⇄ Server route agreement — client half (#6222)
// ---------------------------------------------------------------------------
//
// The single route table (`./route-table.ts`) is the ONE source of truth for the
// client router AND the server document allowlist. This file proves the CLIENT
// half: every table example path parses through the real Foldkit router
// (`urlToAppRoute`) to exactly the tag that owns it. The SERVER half (every
// `spaDocument` example is admitted server-side, every non-document example is
// rejected) lives in
// `workers/api/src/client-server-route-agreement.test.ts`.
//
// Together they make the `/trace/{uuid}` desync structurally impossible: a route
// in the table that the parser cannot resolve, or a route the server cannot
// serve, fails one of these two tests.

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

// Every example path the table declares, paired with the tag the CLIENT parser
// is expected to yield. That is the entry key unless `clientParseTag` overrides
// it (root `/` -> Landing, parser-less `/dashboard` -> NotFound).
const tableExamples = Object.entries(
  routeTable as Record<string, RouteTableEntry>,
).flatMap(([tag, entry]) =>
  entry.examplePaths.map(path => [path, entry.clientParseTag ?? tag] as const),
)

describe('client⇄server route table agreement — client half (#6222)', () => {
  test.each(tableExamples)(
    'table example %s parses client-side to %s',
    (path, expectedTag) => {
      expect(urlToAppRoute(appUrl(path))._tag).toBe(expectedTag)
    },
  )

  // Sanity: the table must actually carry example paths for the routes a user can
  // navigate to directly. Only the deliberately-unnavigable tags (the deprecated
  // ChatRoute duplicate and the NotFound catch-all) are allowed to have none.
  test('only deliberately-unnavigable routes lack example paths', () => {
    const withoutExamples = Object.entries(routeTable)
      .filter(([, entry]) => entry.examplePaths.length === 0)
      .map(([tag]) => tag)
      .sort()
    expect(withoutExamples).toEqual(['Chat', 'NotFound'])
  })
})
