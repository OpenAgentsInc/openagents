import { Match as M, Option, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import { type AuthBootstrap, onboardingIsComplete } from '../domain/session'
import {
  browserRouteGate,
  defaultLoggedInHrefForAuth,
  defaultLoggedInRouteForAuth,
  loggedInPermissionGate,
  loggedInWorkroomAllowed,
  routeAllowedForLoggedInAuth,
} from '../product-policy'
import {
  type AppRoute,
  ChatRoute,
  HomeRoute,
  InviteRoute,
  LoggedInRoute,
  LoggedOutRoute,
  OnboardingRoute,
  homeRouter,
  inviteRouter,
  onboardingRouter,
  orderRouter,
  routeRegistry,
} from '../route'

export const StartupRedirectToHome = ts('StartupRedirectToHome', {
  href: S.String,
})
export const StartupRedirectToDefaultLoggedInRoute = ts(
  'StartupRedirectToDefaultLoggedInRoute',
  {
    href: S.String,
  },
)
export const StartupRedirectToOnboarding = ts('StartupRedirectToOnboarding', {
  href: S.String,
})
export const StartupRedirectToInvite = ts('StartupRedirectToInvite', {
  href: S.String,
})
export const StartupRedirectToOrder = ts('StartupRedirectToOrder', {
  href: S.String,
})
export const StartupRedirect = S.Union([
  StartupRedirectToHome,
  StartupRedirectToDefaultLoggedInRoute,
  StartupRedirectToOnboarding,
  StartupRedirectToInvite,
  StartupRedirectToOrder,
])
export type StartupRedirect = typeof StartupRedirect.Type

export const LoggedOutStartupRoute = ts('LoggedOutStartupRoute', {
  route: LoggedOutRoute,
  redirect: S.Option(StartupRedirect),
})
export const LoggedInStartupRoute = ts('LoggedInStartupRoute', {
  route: LoggedInRoute,
  redirect: S.Option(StartupRedirect),
})
export const StartupRoute = S.Union([
  LoggedOutStartupRoute,
  LoggedInStartupRoute,
])
export type StartupRoute = typeof StartupRoute.Type

export { routeRequiresAuthBootstrap } from '../product-policy'

const forumUnknownPathPattern = /^\/forum(?:\/|$)/

const isUnknownForumPath = (path: string): boolean =>
  forumUnknownPathPattern.test(path)

export const startupRouteForLoggedOut = (
  route: AppRoute,
): typeof LoggedOutStartupRoute.Type =>
  M.value(route).pipe(
    M.tag('Home', 'Onboarding', route =>
      LoggedOutStartupRoute({
        route,
        redirect: Option.none(),
      }),
    ),
    M.tag(
      'Docs',
      'DocsPage',
      'ProductPromises',
      'PublicTrainingRuns',
      'PublicTrainingRun',
      'Stats',
      'Forum',
      'ForumForum',
      'ForumTopic',
      'ForumReceipt',
      'SiteCheckoutDemo',
      'SiteCheckoutDemoReturn',
      'Business',
      'BusinessKpi',
      'Autopilot',
      'AutopilotVertical',
      'Activity',
      'DemoLegal',
      'Gym',
      'MirrorCode',
      'Run',
      'Tassadar',
      'TassadarReplay',
      'Login',
      'Blog',
      'BlogPost',
      'PublicAgent',
      'ArtanisTraceTree',
      'Share',
      'Trace',
      'TraceCompare',
      'QaSwarm',
      'PylonCodexAssignmentStatus',
      'Khala',
      'KhalaChat',
      'Terms',
      'Privacy',
      'Code',
      'KhalaCodeDownload',
      'Pylon',
      'Download',
      'Workspace',
      route =>
        LoggedOutStartupRoute({
          route,
          redirect: Option.none(),
        }),
    ),
    M.tag('NotFound', route =>
      isUnknownForumPath(route.path)
        ? LoggedOutStartupRoute({
            route,
            redirect: Option.none(),
          })
        : LoggedOutStartupRoute({
            route: HomeRoute(),
            redirect: Option.some(
              StartupRedirectToHome({ href: homeRouter() }),
            ),
          }),
    ),
    M.orElse(() =>
      LoggedOutStartupRoute({
        route: HomeRoute(),
        redirect: Option.some(StartupRedirectToHome({ href: homeRouter() })),
      }),
    ),
  )

const incompleteOnboardingRoute = (): typeof LoggedInStartupRoute.Type =>
  LoggedInStartupRoute({
    route: OnboardingRoute(),
    redirect: Option.some(
      StartupRedirectToOnboarding({ href: onboardingRouter() }),
    ),
  })

const startupRouteForIncompleteOnboarding = (route: AppRoute): StartupRoute =>
  M.value(route).pipe(
    M.tag('Onboarding', route =>
      LoggedInStartupRoute({
        route,
        redirect: Option.none(),
      }),
    ),
    M.tag(
      'PublicAgent',
      'ArtanisTraceTree',
      'ProductPromises',
      'Stats',
      'Share',
      'Trace',
      'TraceCompare',
      'QaSwarm',
      'PylonCodexAssignmentStatus',
      'Khala',
      'KhalaChat',
      'Terms',
      'Privacy',
      'Code',
      'KhalaCodeDownload',
      'Pylon',
      'Download',
      'Activity',
      'Workspace',
      'Gym',
      'MirrorCode',
      route =>
        route._tag === 'Workspace'
          ? LoggedInStartupRoute({
              route,
              redirect: Option.none(),
            })
          : LoggedOutStartupRoute({
              route,
              redirect: Option.none(),
            }),
    ),
    M.tag('PublicTrainingRuns', 'PublicTrainingRun', route =>
      LoggedOutStartupRoute({
        route,
        redirect: Option.none(),
      }),
    ),
    M.tag('Autopilot', 'AutopilotVertical', route =>
      // Mid-onboarding users have no workspace yet, so the autopilot entry
      // serves the public onboarding page rather than forcing the onboarding
      // flow redirect.
      LoggedOutStartupRoute({
        route,
        redirect: Option.none(),
      }),
    ),
    M.tag(
      'Docs',
      'DocsPage',
      'Forum',
      'ForumForum',
      'ForumTopic',
      'ForumReceipt',
      'SiteCheckoutDemo',
      'SiteCheckoutDemoReturn',
      'Business',
      'BusinessKpi',
      'DemoLegal',
      'Run',
      'Tassadar',
      'TassadarReplay',
      'Login',
      'Blog',
      'BlogPost',
      route =>
        LoggedInStartupRoute({
          route,
          redirect: Option.none(),
        }),
    ),
    M.orElse(() => incompleteOnboardingRoute()),
  )

const startupRouteForCompleteOnboarding = (
  route: AppRoute,
  auth: AuthBootstrap,
): StartupRoute => {
  const routeGate = browserRouteGate(route)
  const defaultRoute = defaultLoggedInRouteForAuth(auth)
  const defaultHref = defaultLoggedInHrefForAuth(auth)

  if (routeGate._tag === 'BrowserRouteRedirected') {
    return LoggedInStartupRoute({
      route: defaultRoute,
      redirect: Option.some(
        StartupRedirectToDefaultLoggedInRoute({ href: defaultHref }),
      ),
    })
  }

  return M.value(routeGate.route).pipe(
    M.tag('Home', () =>
      LoggedInStartupRoute({
        route: defaultRoute,
        redirect: Option.none(),
      }),
    ),
    M.tag('Onboarding', () =>
      LoggedInStartupRoute({
        route: defaultRoute,
        redirect: Option.some(
          StartupRedirectToDefaultLoggedInRoute({
            href: defaultHref,
          }),
        ),
      }),
    ),
    M.tag('Autopilot', 'AutopilotVertical', route =>
      // A logged-in user with a workspace lands on the existing cockpit; a
      // logged-in user without one sees the public onboarding page (same page
      // logged-out users get). #6129 expands that page into the full flow.
      loggedInWorkroomAllowed(auth)
        ? LoggedInStartupRoute({
            route: ChatRoute(),
            redirect: Option.none(),
          })
        : LoggedOutStartupRoute({
            route,
            redirect: Option.none(),
          }),
    ),
    M.tag(
      'PublicAgent',
      'ArtanisTraceTree',
      'ProductPromises',
      'PublicTrainingRuns',
      'PublicTrainingRun',
      'Stats',
      'Share',
      'Trace',
      'TraceCompare',
      'QaSwarm',
      'Khala',
      'KhalaChat',
      'Terms',
      'Privacy',
      'Code',
      'KhalaCodeDownload',
      'Pylon',
      'Download',
      'Activity',
      'Gym',
      'MirrorCode',
      route =>
        LoggedOutStartupRoute({
          route,
          redirect: Option.none(),
        }),
    ),
    M.tag(
      'Chat',
      'Decisions',
      'Order',
      'OrderDetail',
      'TeamChat',
      'TeamProjectChat',
      'TeamFiles',
      'TeamFile',
      'PersonalFile',
      'Thread',
      'Docs',
      'DocsPage',
      'Forum',
      'ForumForum',
      'ForumTopic',
      'ForumReceipt',
      'SiteCheckoutDemo',
      'SiteCheckoutDemoReturn',
      'Business',
      'BusinessKpi',
      'DemoLegal',
      'Run',
      'Tassadar',
      'TassadarReplay',
      'Login',
      'Blog',
      'BlogPost',
      'Admin',
      'Mullet',
      'GymOss',
      'Pro',
      'OperatorDashboard',
      'Billing',
      'Usage',
      'Images',
      'Workspace',
      'Settings',
      'SettingsSection',
      'NotFound',
      route =>
        routeAllowedForLoggedInAuth(route, auth)
          ? LoggedInStartupRoute({
              route,
              redirect: Option.none(),
            })
          : LoggedInStartupRoute({
              route: defaultRoute,
              redirect: Option.some(
                StartupRedirectToOrder({ href: orderRouter() }),
              ),
            }),
    ),
    M.orElse(() =>
      LoggedInStartupRoute({
        route: defaultRoute,
        redirect: Option.some(
          StartupRedirectToDefaultLoggedInRoute({
            href: defaultHref,
          }),
        ),
      }),
    ),
  )
}

const inviteRequiredRoute = (): typeof LoggedInStartupRoute.Type =>
  LoggedInStartupRoute({
    route: InviteRoute(),
    redirect: Option.some(StartupRedirectToInvite({ href: inviteRouter() })),
  })

const startupRouteForInviteRequired = (route: AppRoute): StartupRoute =>
  M.value(route).pipe(
    M.tag('Invite', route =>
      LoggedInStartupRoute({
        route,
        redirect: Option.none(),
      }),
    ),
    M.orElse(() => inviteRequiredRoute()),
  )

// ---------------------------------------------------------------------------
// Startup exhaustiveness guard
// ---------------------------------------------------------------------------
//
// The original `/gym/oss` bug: a route tag was missing from the startup match
// lists in `startupRouteForCompleteOnboarding`, so it fell through `M.orElse` to
// an unintended default-route redirect instead of resolving. The map below is
// keyed exhaustively by the `AppRoute` tag union, so a route tag added or
// removed without being classified here is a COMPILE error. The runtime guard
// test (`routing/startup.test.ts`) additionally asserts this map matches the
// actual `startupRouteForCompleteOnboarding` disposition, so the two cannot
// drift apart.
//
// Disposition values reflect how a logged-in, onboarding-complete user's route
// is dispatched by `startupRouteForCompleteOnboarding`:
//   - 'home'            : resolves to the default route (Home special-case)
//   - 'onboarding'      : redirects onboarding-complete users to the default
//   - 'autopilot'       : autopilot onboarding/cockpit branch
//   - 'public'          : resolved as a logged-out public route
//   - 'gated'           : resolved in-place subject to routeAllowedForLoggedInAuth
//   - 'redirectDefault' : falls to the default-route redirect (orElse)
export type StartupCompleteDisposition =
  | 'home'
  | 'onboarding'
  | 'autopilot'
  | 'public'
  | 'gated'
  | 'redirectDefault'

export const startupCompleteDisposition = {
  Home: 'home',
  Invite: 'redirectDefault',
  Onboarding: 'onboarding',
  Order: 'gated',
  OrderDetail: 'gated',
  Autopilot: 'autopilot',
  AutopilotVertical: 'autopilot',
  AutopilotWork: 'redirectDefault',
  AutopilotWorkDetail: 'redirectDefault',
  Decisions: 'gated',
  Workspace: 'gated',
  Workroom: 'redirectDefault',
  WorkroomTab: 'redirectDefault',
  Chat: 'gated',
  TeamChat: 'gated',
  TeamProjectChat: 'gated',
  TeamFiles: 'gated',
  TeamFile: 'gated',
  PersonalFile: 'gated',
  Thread: 'gated',
  Docs: 'gated',
  DocsPage: 'gated',
  ProductPromises: 'public',
  PublicTrainingRuns: 'public',
  PublicTrainingRun: 'public',
  Forum: 'gated',
  ForumForum: 'gated',
  ForumTopic: 'gated',
  ForumReceipt: 'gated',
  SiteCheckoutDemo: 'gated',
  SiteCheckoutDemoReturn: 'gated',
  Business: 'gated',
  BusinessKpi: 'gated',
  Activity: 'public',
  Run: 'gated',
  Gym: 'public',
  MirrorCode: 'public',
  GymOss: 'gated',
  ArtanisGym: 'gated',
  Tassadar: 'gated',
  TassadarReplay: 'gated',
  Login: 'gated',
  Blog: 'gated',
  BlogPost: 'gated',
  PublicAgent: 'public',
  ArtanisTraceTree: 'public',
  Share: 'public',
  Trace: 'public',
  TraceCompare: 'public',
  QaSwarm: 'public',
  PylonCodexAssignmentStatus: 'public',
  ArtanisAccounts: 'public',
  Terms: 'public',
  Privacy: 'public',
  Code: 'public',
  KhalaCodeDownload: 'public',
  Khala: 'public',
  KhalaChat: 'public',
  Pylon: 'public',
  Download: 'public',
  Dashboard: 'redirectDefault',
  Pro: 'gated',
  OperatorDashboard: 'gated',
  Billing: 'gated',
  Usage: 'gated',
  Stats: 'public',
  Admin: 'gated',
  Mullet: 'gated',
  Images: 'gated',
  Settings: 'gated',
  SettingsSection: 'gated',
  Demo: 'redirectDefault',
  DemoLegal: 'gated',
  DemoOrder: 'redirectDefault',
  DemoThread: 'redirectDefault',
  DemoTeamProjectChat: 'redirectDefault',
  DemoTeamFiles: 'redirectDefault',
  DemoTeamFile: 'redirectDefault',
  Demo2: 'redirectDefault',
  Demo2Order: 'redirectDefault',
  Demo2Thread: 'redirectDefault',
  Demo2TeamProjectChat: 'redirectDefault',
  Demo2TeamFiles: 'redirectDefault',
  Demo2TeamFile: 'redirectDefault',
  NotFound: 'gated',
} as const satisfies Record<AppRoute['_tag'], StartupCompleteDisposition>

// Compile-time guard: every `AppRoute` tag carries the registry union-membership
// flags the startup routers rely on. Combined with the exhaustive
// `startupCompleteDisposition` map above, a new route cannot be added without
// being classified for startup. The runtime test in `routing/startup.test.ts`
// asserts the disposition map matches actual behavior so they cannot drift.
type _RegistryHasStartupFlags = {
  readonly [Tag in AppRoute['_tag']]: (typeof routeRegistry)[Tag]['inLoggedOutUnion'] extends boolean
    ? (typeof routeRegistry)[Tag]['inLoggedInUnion'] extends boolean
      ? true
      : never
    : never
}[AppRoute['_tag']]
const _registryHasStartupFlags: _RegistryHasStartupFlags = true
void _registryHasStartupFlags

export const startupRouteForLoggedIn = (
  route: AppRoute,
  auth: AuthBootstrap,
): StartupRoute => {
  const permissionGate = loggedInPermissionGate(auth)

  if (permissionGate._tag === 'BrowserPermissionDenied') {
    return startupRouteForInviteRequired(route)
  }

  return onboardingIsComplete(auth.onboarding)
    ? startupRouteForCompleteOnboarding(route, auth)
    : startupRouteForIncompleteOnboarding(route)
}
