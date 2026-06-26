import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, test } from 'vitest'

import {
  authBootstrapFromSession,
  completedOnboardingStatus,
} from './domain/session'
import {
  browserCommandProductIntents,
  browserFeatureFlags,
  browserRouteGate,
  browserRouteProductIntent,
  loggedInAdminAccessAllowed,
  loggedInMulletAccessAllowed,
  loggedInOperatorAccessAllowed,
  loggedInPermissionGate,
  loggedInWorkroomAllowed,
  projectMissionVisible,
  routeAllowedForLoggedInAuth,
  routeRequiresAuthBootstrap,
} from './product-policy'
import {
  AdminRoute,
  type AppRoute,
  ChatRoute,
  GymOssRoute,
  GymRoute,
  KhalaRoute,
  MulletRoute,
  OrderRoute,
  PrivacyRoute,
  SiteCheckoutDemoRoute,
  StatsRoute,
  TassadarRoute,
  TeamProjectChatRoute,
  TermsRoute,
  routeRegistry,
} from './route'

const sourceRoot = join(process.cwd(), 'src')

const sourceFiles = (root: string): ReadonlyArray<string> => {
  if (!existsSync(root)) {
    return []
  }

  const visit = (dir: string): ReadonlyArray<string> =>
    readdirSync(dir).flatMap(entry => {
      const path = join(dir, entry)
      const stat = statSync(path)

      if (stat.isDirectory()) {
        return visit(path)
      }

      return path.endsWith('.ts') &&
        !path.endsWith('.test.ts') &&
        !path.endsWith('.story.test.ts') &&
        !path.endsWith('.scene.test.ts')
        ? [path]
        : []
    })

  return visit(root)
}

const commandNamesFromSource = (): ReadonlyArray<string> =>
  sourceFiles(sourceRoot).flatMap(file => {
    const source = readFileSync(file, 'utf8')
    const matches = [
      ...source.matchAll(/Command\.define\(\s*['"`]([^'"`]+)['"`]/g),
    ]

    return matches.map(match => `${relative(sourceRoot, file)}:${match[1]}`)
  })

describe('browser product policy', () => {
  test('keeps project workrooms disabled behind the central feature gate', () => {
    expect(browserFeatureFlags.projectWorkrooms).toBe(false)
    expect(
      browserRouteGate(
        TeamProjectChatRoute({
          projectRef: 'artanis',
          teamRef: 'openagents-core-team',
        }),
      ),
    ).toEqual({
      _tag: 'BrowserRouteRedirected',
      href: '/autopilot',
      reason: 'disabledProductArea',
      route: { _tag: 'Chat' },
    })
    expect(
      browserRouteGate(
        TeamProjectChatRoute({
          projectRef: 'artanis',
          teamRef: 'openagents-core-team',
        }),
        { projectWorkrooms: true },
      ),
    ).toEqual({
      _tag: 'BrowserRouteAllowed',
      route: {
        _tag: 'TeamProjectChat',
        projectRef: 'artanis',
        teamRef: 'openagents-core-team',
      },
    })
    expect(
      browserRouteGate(
        TeamProjectChatRoute({
          projectRef: 'adjutant',
          teamRef: 'openagents-core-team',
        }),
      ),
    ).toEqual({
      _tag: 'BrowserRouteAllowed',
      route: {
        _tag: 'TeamProjectChat',
        projectRef: 'adjutant',
        teamRef: 'openagents-core-team',
      },
    })
  })

  test('hides disabled project mission records from sidebar policy', () => {
    expect(
      projectMissionVisible({
        projectId: 'project_artanis',
        title: 'Artanis project smoke: verify answer-back',
      }),
    ).toBe(false)
    expect(
      projectMissionVisible({
        title: 'Artanis project smoke: verify answer-back',
      }),
    ).toBe(false)
    expect(projectMissionVisible({ title: 'Regular mission' })).toBe(true)
  })

  test('allows signed-in customer access without Core Team membership', () => {
    const auth = authBootstrapFromSession({
      email: 'visitor@example.com',
      name: 'Visitor',
      userId: 'github:visitor',
    })

    expect(loggedInPermissionGate(auth)).toEqual({
      _tag: 'BrowserPermissionAllowed',
    })
    expect(loggedInOperatorAccessAllowed(auth)).toBe(false)
    expect(loggedInWorkroomAllowed(auth)).toBe(false)
  })

  test('allows admin access only through the admin session flag', () => {
    const auth = authBootstrapFromSession({
      email: 'chris@openagents.com',
      name: 'Christopher David',
      userId: 'github:chris',
    })

    expect(loggedInAdminAccessAllowed(auth)).toBe(false)
    expect(loggedInAdminAccessAllowed({ ...auth, isAdmin: true })).toBe(true)
  })

  test('allows mullet access only for the confirmed admin email', () => {
    const chris = authBootstrapFromSession({
      email: 'chris@openagents.com',
      name: 'Christopher David',
      userId: 'github:chris',
    })
    const wrongAdmin = authBootstrapFromSession({
      email: 'admin@openagents.com',
      name: 'Wrong Admin',
      userId: 'github:wrong-admin',
    })

    expect(loggedInMulletAccessAllowed(chris)).toBe(false)
    expect(loggedInMulletAccessAllowed({ ...chris, isAdmin: true })).toBe(true)
    expect(loggedInMulletAccessAllowed({ ...wrongAdmin, isAdmin: true })).toBe(
      false,
    )
  })

  test('documents route names as product intents', () => {
    expect(
      browserRouteProductIntent(
        TeamProjectChatRoute({
          projectRef: 'artanis',
          teamRef: 'openagents-core-team',
        }),
      ),
    ).toBe('workroom.chat.project')
    expect(browserRouteProductIntent(SiteCheckoutDemoRoute())).toBe(
      'public.sites.demo-checkout',
    )
    expect(browserRouteProductIntent(MulletRoute())).toBe('mullet.runner')
    expect(browserRouteProductIntent(StatsRoute())).toBe('public.stats')
    expect(browserRouteProductIntent(KhalaRoute())).toBe('public.khala')
  })

  test('keeps the public Khala route bootstrap-free', () => {
    expect(routeRequiresAuthBootstrap(KhalaRoute())).toBe(false)
  })

  test('keeps the public stats route bootstrap-free', () => {
    expect(routeRequiresAuthBootstrap(StatsRoute())).toBe(false)
  })

  test('keeps the public Gym route bootstrap-free', () => {
    expect(browserRouteProductIntent(GymRoute())).toBe(
      'public.gym.terminal-bench',
    )
    expect(routeRequiresAuthBootstrap(GymRoute())).toBe(false)
    expect(browserRouteGate(GymRoute())._tag).toBe('BrowserRouteAllowed')
  })

  test('classifies the public Tassadar route like Khala (public, no bootstrap)', () => {
    expect(browserRouteProductIntent(TassadarRoute())).toBe(
      'public.tassadar.run',
    )
    expect(routeRequiresAuthBootstrap(TassadarRoute())).toBe(false)
  })

  test('catalogs every browser command name with product intent', () => {
    const declaredNames = commandNamesFromSource().map(entry =>
      entry.slice(entry.indexOf(':') + 1),
    )
    const missing = declaredNames.filter(
      name => !(name in browserCommandProductIntents),
    )
    const stale = Object.keys(browserCommandProductIntents).filter(
      name => !declaredNames.includes(name),
    )

    expect(missing).toEqual([])
    expect(stale).toEqual([])
  })

  test('classifies the legal routes as public with no auth bootstrap', () => {
    expect(routeRequiresAuthBootstrap(TermsRoute())).toBe(false)
    expect(routeRequiresAuthBootstrap(PrivacyRoute())).toBe(false)
    expect(browserRouteGate(TermsRoute())._tag).toBe('BrowserRouteAllowed')
    expect(browserRouteGate(PrivacyRoute())._tag).toBe('BrowserRouteAllowed')
    expect(browserRouteProductIntent(TermsRoute())).toBe('public.terms')
    expect(browserRouteProductIntent(PrivacyRoute())).toBe('public.privacy')
  })

  test('routeRequiresAuthBootstrap is derived from the registry for every tag', () => {
    // `routeRequiresAuthBootstrap` must return exactly the registry's
    // `requiresAuthBootstrap` flag for every route tag (no hand list drift).
    for (const [tag, spec] of Object.entries(routeRegistry)) {
      const route = { _tag: tag } as unknown as AppRoute
      expect(routeRequiresAuthBootstrap(route)).toBe(spec.requiresAuthBootstrap)
    }
  })

  test('routeAllowedForLoggedInAuth follows the registry loggedInGate', () => {
    const customer = authBootstrapFromSession({
      email: 'visitor@example.com',
      name: 'Visitor',
      userId: 'github:visitor',
    })
    const adminChris = {
      ...authBootstrapFromSession({
        email: 'chris@openagents.com',
        name: 'Christopher David',
        userId: 'github:chris',
      }),
      isAdmin: true,
      onboarding: completedOnboardingStatus(),
    }

    // 'open' -> always allowed
    expect(routeAllowedForLoggedInAuth(OrderRoute(), customer)).toBe(true)
    // 'workroom' -> needs Core Team + onboarding; a plain customer is denied
    expect(routeAllowedForLoggedInAuth(ChatRoute(), customer)).toBe(false)
    // 'admin' -> needs admin flag + onboarding
    expect(routeAllowedForLoggedInAuth(AdminRoute(), customer)).toBe(false)
    expect(routeAllowedForLoggedInAuth(AdminRoute(), adminChris)).toBe(true)
    expect(routeAllowedForLoggedInAuth(GymOssRoute(), adminChris)).toBe(true)
    // 'mullet' -> needs admin + onboarding + owner email
    expect(routeAllowedForLoggedInAuth(MulletRoute(), adminChris)).toBe(true)
  })

  test('every registry loggedInGate is one of the four known gates', () => {
    const gates = new Set(['open', 'workroom', 'admin', 'mullet'])
    for (const spec of Object.values(routeRegistry)) {
      expect(gates.has(spec.loggedInGate)).toBe(true)
    }
  })
})
