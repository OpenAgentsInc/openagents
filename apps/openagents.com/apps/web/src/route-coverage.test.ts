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
  ['/gym/oss', 'GymOss'],
  // Authenticated top-level surfaces (parse the same regardless of session;
  // auth gating happens in the startup policy, not the parser).
  ['/order', 'Order'],
  ['/pro', 'Pro'],
  ['/pro/runs', 'ProRuns'],
  ['/pro/runs/login-regression-prod', 'ProRun'],
  ['/pro/evals', 'ProEvals'],
  ['/pro/evals/login-mcp-compare', 'ProEval'],
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
    // 33 original public/top-level routes + the authenticated `/pro` operator
    // console top-level route (34) + the four /pro subpages (runs index/detail,
    // evals index/detail) for the operator console build-out (38).
    expect(PUBLIC_ROUTE_PARSE_COVERAGE.length).toBe(38)
  })

  // The specific-before-generic parser ordering must resolve the parameterized
  // /pro subpaths to their detail routes, not the index routes.
  test('parses /pro subpaths with the correct specificity', () => {
    expect(urlToAppRoute(appUrl('/pro/runs'))._tag).toBe('ProRuns')
    expect(urlToAppRoute(appUrl('/pro/evals'))._tag).toBe('ProEvals')
    const runDetail = urlToAppRoute(appUrl('/pro/runs/abc'))
    expect(runDetail._tag).toBe('ProRun')
    if (runDetail._tag === 'ProRun') expect(runDetail.runId).toBe('abc')
    const evalDetail = urlToAppRoute(appUrl('/pro/evals/xyz'))
    expect(evalDetail._tag).toBe('ProEval')
    if (evalDetail._tag === 'ProEval') expect(evalDetail.evalId).toBe('xyz')
  })

  // The registry-driven parser derives its `oneOf` list from a single ordered
  // source; the deprecated/duplicate routers stay explicitly excluded.
  test('keeps deprecated/duplicate routers out of the parser', () => {
    expect(unregisteredParserRouters.length).toBe(3)
    expect(urlToAppRoute(appUrl('/chat'))._tag).toBe('NotFound')
    expect(urlToAppRoute(appUrl('/gym'))._tag).toBe('NotFound')
  })
})
