import { Match as M, Option, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import { type AuthBootstrap, onboardingIsComplete } from '../domain/session'
import {
  browserRouteGate,
  defaultLoggedInHrefForAuth,
  defaultLoggedInRouteForAuth,
  loggedInPermissionGate,
  routeAllowedForLoggedInAuth,
} from '../product-policy'
import {
  type AppRoute,
  InviteRoute,
  LoggedInRoute,
  LoggedOutRoute,
  OnboardingRoute,
  PylonRoute,
  homeRouter,
  inviteRouter,
  onboardingRouter,
  orderRouter,
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
      'PublicStatsArchive',
      'Forum',
      'ForumForum',
      'ForumTopic',
      'ForumReceipt',
      'SiteCheckoutDemo',
      'SiteCheckoutDemoReturn',
      'ClientsPreview',
      'Components',
      'ComponentsFamily',
      'Business',
      'Animations',
      'Activity',
      'DemoLegal',
      'Run',
      'Tassadar',
      'TassadarReplay',
      'Login',
      'Blog',
      'BlogPost',
      'PublicAgent',
      'Share',
      'Moksha',
      'Moksha2',
      'Landing',
      'Khala',
      'Terms',
      'Privacy',
      'Pylon',
      'Download',
      'Workspace',
      route =>
        LoggedOutStartupRoute({
          route,
          redirect: Option.none(),
        }),
    ),
    M.tag('NotFound', () =>
      LoggedOutStartupRoute({
        route: PylonRoute(),
        redirect: Option.some(StartupRedirectToHome({ href: homeRouter() })),
      }),
    ),
    M.orElse(() =>
      LoggedOutStartupRoute({
        route: PylonRoute(),
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
      'ProductPromises',
      'PublicStatsArchive',
      'Share',
      'Moksha',
      'Moksha2',
      'Landing',
      'Khala',
      'Terms',
      'Privacy',
      'Pylon',
      'Download',
      'Activity',
      'Workspace',
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
    M.tag(
      'Docs',
      'DocsPage',
      'Forum',
      'ForumForum',
      'ForumTopic',
      'ForumReceipt',
      'SiteCheckoutDemo',
      'SiteCheckoutDemoReturn',
      'ClientsPreview',
      'Components',
      'ComponentsFamily',
      'Business',
      'Animations',
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
    M.tag(
      'PublicAgent',
      'ProductPromises',
      'PublicTrainingRuns',
      'PublicTrainingRun',
      'PublicStatsArchive',
      'Share',
      'Moksha',
      'Moksha2',
      'Landing',
      'Khala',
      'Terms',
      'Privacy',
      'Pylon',
      'Download',
      'Activity',
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
      'ClientsPreview',
      'Components',
      'ComponentsFamily',
      'Business',
      'Animations',
      'DemoLegal',
      'Run',
      'Tassadar',
      'TassadarReplay',
      'Login',
      'Blog',
      'BlogPost',
      'Admin',
      'Mullet',
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
