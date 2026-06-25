import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import { unregisteredParserRouters, urlToAppRoute } from './route'

// Guard against the "router defined but never registered in the parser" bug
// (e.g. `/login` once parsed to NotFound because `loginRouter` was missing from
// the `routeParser` oneOf list, so the logged-out startup redirected it home).
//
// Each entry below is a canonical public/top-level path that MUST parse to its
// named route tag. If a router is dropped from the parser (or a route is added
// without registering it), the path falls through to NotFound and the matching
// case here fails loudly instead of silently redirecting users to the homepage.
//
// Param-heavy, overlapping, and intentionally-unrouted paths (`/`, `/chat`,
// `/dashboard`, the demo/team/workroom families) are covered by the assertions
// in route.test.ts; this file locks the flat public surface that is easiest to
// half-wire.
const PUBLIC_ROUTE_PARSE_COVERAGE: ReadonlyArray<readonly [string, string]> = [
  ['/login', 'Login'],
  ['/khala', 'Khala'],
  ['/stats', 'Stats'],
  ['/stats-old', 'PublicStatsArchive'],
  ['/terms', 'Terms'],
  ['/privacy', 'Privacy'],
  ['/business', 'Business'],
  ['/pylons', 'Pylon'],
  ['/run', 'Run'],
  ['/tassadar', 'Tassadar'],
  ['/blog', 'Blog'],
  ['/docs', 'Docs'],
  ['/forum', 'Forum'],
  ['/promises', 'ProductPromises'],
  ['/components', 'Components'],
  ['/animations', 'Animations'],
  ['/activity', 'Activity'],
  ['/download', 'Download'],
  ['/landing', 'Landing'],
  ['/moksha', 'Moksha'],
  ['/moksha2', 'Moksha2'],
  ['/clients-preview', 'ClientsPreview'],
  ['/gym', 'Gym'],
  ['/gym/oss', 'GymOss'],
  // The public shareable ATIF trace render (#6209). Parses regardless of
  // session; it is public-safe with no auth to view a shared trace.
  ['/trace/0e08d2db-2026-4624-9a39-f1efe8000001', 'Trace'],
  // The public shareable trace comparison (#6211). The literal `compare`
  // segment must win over `/trace/{uuid}`; the `ids` segment is a
  // comma-separated uuid list (baseline first).
  ['/trace/compare/a,b,c', 'TraceCompare'],
  // Authenticated top-level surfaces (parse the same regardless of session;
  // auth gating happens in the startup policy, not the parser).
  ['/order', 'Order'],
  ['/pro', 'Pro'],
  // The `/pro/runs` + `/pro/evals` fixture subpages were retired in #6215
  // (superseded by the public `/trace` surfaces); they now fall through to
  // NotFound. See the dedicated retirement test below.
  ['/billing', 'Billing'],
  ['/usage', 'Usage'],
  ['/images', 'Images'],
  ['/settings', 'Settings'],
  ['/admin', 'Admin'],
  ['/mullet', 'Mullet'],
  ['/forge', 'Forge'],
  ['/decisions', 'Decisions'],
  ['/onboarding', 'Onboarding'],
]

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

describe('public route parser coverage', () => {
  test.each(PUBLIC_ROUTE_PARSE_COVERAGE)(
    '%s parses to its route (not NotFound)',
    (path, expectedTag) => {
      const route = urlToAppRoute(appUrl(path))
      expect(route._tag).toBe(expectedTag)
    },
  )

  test('covers the documented public surface', () => {
    // Public/top-level routes incl. the authenticated `/pro` operator console
    // top-level route, the public shareable `/trace/{uuid}` render (#6209), and
    // the public shareable `/trace/compare/{ids}` comparison (#6211). The four
    // `/pro/runs` + `/pro/evals` fixture subpages were retired in #6215, so the
    // covered count dropped from 40 to 36; the public `/gym` Terminal-Bench
    // visualizer brings the parser-covered surface to 37.
    expect(PUBLIC_ROUTE_PARSE_COVERAGE.length).toBe(37)
  })

  // The public shareable trace render (#6209) must capture the uuid param so the
  // page can look up the right trajectory; a missing/dropped router would fall
  // through to NotFound (the bug this file guards against).
  test('parses /trace/{uuid} and captures the uuid', () => {
    const route = urlToAppRoute(
      appUrl('/trace/0e08d2db-2026-4624-9a39-f1efe8000001'),
    )
    expect(route._tag).toBe('Trace')
    if (route._tag === 'Trace') {
      expect(route.uuid).toBe('0e08d2db-2026-4624-9a39-f1efe8000001')
    }
  })

  // The trace comparison (#6211) must capture the `ids` list AND beat the more
  // generic `/trace/{uuid}` route — the literal `compare` segment is registered
  // first, so `/trace/compare/...` resolves to TraceCompare, not Trace.
  test('parses /trace/compare/{ids} with the correct specificity', () => {
    const route = urlToAppRoute(appUrl('/trace/compare/aaa,bbb,ccc'))
    expect(route._tag).toBe('TraceCompare')
    if (route._tag === 'TraceCompare') {
      expect(route.ids).toBe('aaa,bbb,ccc')
    }
    // A single-segment `/trace/{uuid}` is still the single-trace render.
    expect(urlToAppRoute(appUrl('/trace/compare'))._tag).toBe('Trace')
  })

  // #6215: the `/pro/runs` + `/pro/evals` fixture subpages (and their detail
  // variants) were retired in favor of the public `/trace/{uuid}` +
  // `/trace/compare/{ids}` surfaces. They no longer parse to a Pro route; they
  // fall through to NotFound. `/pro` itself stays the operator console.
  test('retires the /pro/runs + /pro/evals fixture subpaths to NotFound', () => {
    expect(urlToAppRoute(appUrl('/pro'))._tag).toBe('Pro')
    expect(urlToAppRoute(appUrl('/pro/runs'))._tag).toBe('NotFound')
    expect(urlToAppRoute(appUrl('/pro/runs/abc'))._tag).toBe('NotFound')
    expect(urlToAppRoute(appUrl('/pro/evals'))._tag).toBe('NotFound')
    expect(urlToAppRoute(appUrl('/pro/evals/xyz'))._tag).toBe('NotFound')
  })

  // The registry-driven parser derives its `oneOf` list from a single ordered
  // source; the deprecated/duplicate routers stay explicitly excluded.
  test('keeps deprecated/duplicate routers out of the parser', () => {
    expect(unregisteredParserRouters.length).toBe(2)
    expect(urlToAppRoute(appUrl('/chat'))._tag).toBe('NotFound')
    expect(urlToAppRoute(appUrl('/gym'))._tag).toBe('Gym')
  })
})
