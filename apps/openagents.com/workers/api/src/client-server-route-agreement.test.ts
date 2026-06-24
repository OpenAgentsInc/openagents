import { describe, expect, test } from 'vitest'

import {
  knownDocumentPathPatterns as tableDocumentPatterns,
  routeTable,
} from '../../../apps/web/src/route-table'
import {
  knownDocumentPathPatterns as workerDocumentPatterns,
  shouldRedirectUnknownDocumentToHome,
} from './worker-routes'

// ---------------------------------------------------------------------------
// Client ⇄ Server route agreement (#6222)
// ---------------------------------------------------------------------------
//
// This is the structural proof that the single route table
// (`apps/web/src/route-table.ts`) is the ONE source of truth for BOTH the client
// router AND the server document allowlist, so the `/trace/{uuid}` prod 302 bug
// (a client route the server did not admit) is impossible going forward.
//
// The complementary client-side half of this proof lives in
// `apps/web/src/client-server-route-agreement.test.ts`, which asserts every
// table example path PARSES client-side to its tag (it needs foldkit, which the
// Worker bundle deliberately does not depend on, so it cannot live here).

const requestFor = (pathname: string) =>
  new Request(`https://openagents.com${pathname}`, {
    headers: { accept: 'text/html' },
    method: 'GET',
  })

const admittedServerSide = (pathname: string): boolean =>
  !shouldRedirectUnknownDocumentToHome(requestFor(pathname), pathname)

describe('client⇄server route table agreement (#6222)', () => {
  test('the Worker allowlist IS the table-derived allowlist (no hand-maintained list)', () => {
    // The Worker re-exports the same derived array it uses internally; it must be
    // the exact list the table produces, by `RegExp.source`.
    expect(workerDocumentPatterns.map(p => p.source)).toEqual(
      tableDocumentPatterns.map(p => p.source),
    )
  })

  // Every `spaDocument` route's canonical example paths MUST be admitted
  // server-side. A new SPA route added to the table without a server pattern (or
  // with a wrong one) fails here instead of silently 302-ing in production — the
  // exact failure mode of the `/trace` bug.
  const documentExamples = Object.entries(routeTable).flatMap(([tag, entry]) =>
    entry.surface === 'spaDocument'
      ? entry.examplePaths.map(path => [tag, path] as const)
      : [],
  )

  test.each(documentExamples)(
    'spaDocument route %s example %s is admitted server-side',
    (_tag, path) => {
      expect(admittedServerSide(path)).toBe(true)
    },
  )

  // Every `spaDocument` route MUST declare a server pattern (otherwise it cannot
  // be admitted). Conversely, `redirectOnly` / `clientOnly` routes MUST NOT be
  // admitted as server documents — preserving the exact current behavior where a
  // hard navigation to e.g. bare `/gym`, `/pro`, or `/decisions` 302s home.
  test('spaDocument routes declare a server pattern; non-document routes do not', () => {
    for (const [tag, entry] of Object.entries(routeTable)) {
      if (entry.surface === 'spaDocument') {
        expect(entry.serverDocument, `${tag} must declare a serverDocument`).not.toBe(
          null,
        )
      } else {
        expect(
          entry.serverDocument,
          `${tag} (${entry.surface}) must not declare a serverDocument`,
        ).toBe(null)
      }
    }
  })

  const nonDocumentExamples = Object.entries(routeTable).flatMap(
    ([tag, entry]) =>
      entry.surface === 'spaDocument'
        ? []
        : entry.examplePaths.map(path => [tag, entry.surface, path] as const),
  )

  test.each(nonDocumentExamples)(
    'non-document route %s (%s) example %s is NOT admitted server-side (302 → /)',
    (_tag, _surface, path) => {
      expect(admittedServerSide(path)).toBe(false)
    },
  )

  // The inverse direction: every regex in the Worker allowlist must be reachable
  // from some `spaDocument` table example (or be a known public-agent alias).
  // This catches a server pattern that admits a shape no client route owns —
  // the reverse of the `/trace` desync.
  test('every server document pattern is owned by a table route or a known alias', () => {
    const knownAliasSources = new Set(['^\\/artanis$', '^\\/adjutant$'])
    for (const pattern of workerDocumentPatterns) {
      if (knownAliasSources.has(pattern.source)) {
        continue
      }
      const ownedByTable = Object.values(routeTable).some(
        entry =>
          entry.surface === 'spaDocument' &&
          entry.serverDocument !== null &&
          entry.serverDocument.source === pattern.source &&
          entry.examplePaths.some(path => pattern.test(path)),
      )
      expect(
        ownedByTable,
        `server pattern ${pattern.source} is not owned by any spaDocument table route example`,
      ).toBe(true)
    }
  })
})
