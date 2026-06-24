import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import { urlToAppRoute } from './route'

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
})
