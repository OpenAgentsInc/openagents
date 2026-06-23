import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  authBootstrapFromSession,
  completedOnboardingStatus,
  incompleteOnboardingStatus,
} from '../domain/session'
import {
  AutopilotWorkRoute,
  ChatRoute,
  Demo2OrderRoute,
  Demo2Route,
  Demo2TeamFileRoute,
  Demo2TeamFilesRoute,
  Demo2TeamProjectChatRoute,
  Demo2ThreadRoute,
  DemoOrderRoute,
  DemoRoute,
  DemoTeamFileRoute,
  DemoTeamFilesRoute,
  DemoTeamProjectChatRoute,
  DemoThreadRoute,
  ForumForumRoute,
  ForumReceiptRoute,
  HomeRoute,
  InviteRoute,
  LandingRoute,
  Moksha2Route,
  MokshaRoute,
  MulletRoute,
  NotFoundRoute,
  OnboardingRoute,
  OrderRoute,
  PublicAgentRoute,
  PublicStatsArchiveRoute,
  PylonRoute,
  ShareRoute,
  SiteCheckoutDemoReturnRoute,
  SiteCheckoutDemoRoute,
  StatsRoute,
  TassadarRoute,
  TassadarReplayRoute,
  TeamChatRoute,
  TeamProjectChatRoute,
} from '../route'
import {
  routeRequiresAuthBootstrap,
  startupRouteForLoggedIn,
  startupRouteForLoggedOut,
} from './startup'

const auth = authBootstrapFromSession({
  email: 'chris@openagents.com',
  name: 'Christopher David',
  userId: 'github:14167547',
})

const authWithCoreTeam = {
  ...auth,
  teams: [
    {
      id: 'team_openagents_core',
      name: 'OpenAgents Core Team',
      slug: 'openagents-core-team',
      role: 'owner',
      members: [],
    },
  ],
}

const completeAuth = {
  ...authWithCoreTeam,
  onboarding: completedOnboardingStatus(),
}

const completeAdminAuth = {
  ...completeAuth,
  isAdmin: true,
}

const wrongAdminEmailAuth = {
  ...authBootstrapFromSession({
    email: 'admin@openagents.com',
    name: 'Wrong Admin',
    userId: 'github:wrong-admin',
  }),
  teams: authWithCoreTeam.teams,
  onboarding: completedOnboardingStatus(),
  isAdmin: true,
}

const incompleteAuth = {
  ...authWithCoreTeam,
  onboarding: incompleteOnboardingStatus(),
}

describe('startup route policy', () => {
  test('keeps logged-out root visitors on the public Pylon scene', () => {
    expect(startupRouteForLoggedOut(PylonRoute())).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Pylon' },
    })
  })

  test('keeps moved public stats routes available while logged out', () => {
    expect(startupRouteForLoggedOut(StatsRoute())).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Stats' },
    })
    expect(startupRouteForLoggedOut(PublicStatsArchiveRoute())).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'PublicStatsArchive' },
    })
  })

  test('keeps public agent pages public for every auth state', () => {
    const publicRoute = PublicAgentRoute({ agentRef: 'artanis' })

    expect(startupRouteForLoggedOut(publicRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: publicRoute,
    })
    expect(startupRouteForLoggedIn(publicRoute, completeAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: publicRoute,
    })
    expect(startupRouteForLoggedIn(publicRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: publicRoute,
    })
  })

  test('keeps share pages public for every auth state', () => {
    const shareRoute = ShareRoute({
      shareId: '123e4567-e89b-42d3-a456-426614174000',
    })

    expect(startupRouteForLoggedOut(shareRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: shareRoute,
    })
    expect(startupRouteForLoggedIn(shareRoute, completeAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: shareRoute,
    })
    expect(startupRouteForLoggedIn(shareRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: shareRoute,
    })
  })

  test('keeps Moksha public for every auth state', () => {
    const mokshaRoute = MokshaRoute()

    expect(startupRouteForLoggedOut(mokshaRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: mokshaRoute,
    })
    expect(startupRouteForLoggedIn(mokshaRoute, completeAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: mokshaRoute,
    })
    expect(startupRouteForLoggedIn(mokshaRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: mokshaRoute,
    })
  })

  test('keeps OpenAgents Moksha public for every auth state', () => {
    const mokshaRoute = Moksha2Route()

    expect(startupRouteForLoggedOut(mokshaRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: mokshaRoute,
    })
    expect(startupRouteForLoggedIn(mokshaRoute, completeAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: mokshaRoute,
    })
    expect(startupRouteForLoggedIn(mokshaRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: mokshaRoute,
    })
  })

  test('keeps Pylon public for every auth state', () => {
    const pylonRoute = PylonRoute()

    expect(startupRouteForLoggedOut(pylonRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: pylonRoute,
    })
    expect(startupRouteForLoggedIn(pylonRoute, completeAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: pylonRoute,
    })
    expect(startupRouteForLoggedIn(pylonRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: pylonRoute,
    })
  })

  test('keeps the live Tassadar run route available for every auth state', () => {
    const tassadarRoute = TassadarRoute()
    const tassadarReplayRoute = TassadarReplayRoute({
      replaySlug: 'first-real-settlement',
    })

    expect(startupRouteForLoggedOut(tassadarRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: tassadarRoute,
    })
    expect(startupRouteForLoggedIn(tassadarRoute, completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: tassadarRoute,
    })
    expect(startupRouteForLoggedIn(tassadarRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: tassadarRoute,
    })
    expect(startupRouteForLoggedOut(tassadarReplayRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: tassadarReplayRoute,
    })
    expect(startupRouteForLoggedIn(tassadarReplayRoute, completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: tassadarReplayRoute,
    })
  })

  test('keeps the stats archive public for every auth state', () => {
    const archiveRoute = PublicStatsArchiveRoute()

    expect(startupRouteForLoggedOut(archiveRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: archiveRoute,
    })
    expect(startupRouteForLoggedIn(archiveRoute, completeAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: archiveRoute,
    })
    expect(startupRouteForLoggedIn(archiveRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: archiveRoute,
    })
  })

  test('keeps Site checkout demo pages public for every auth state', () => {
    const checkoutRoute = SiteCheckoutDemoRoute()
    const returnRoute = SiteCheckoutDemoReturnRoute({
      returnAction: 'success',
    })

    expect(startupRouteForLoggedOut(checkoutRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: checkoutRoute,
    })
    expect(startupRouteForLoggedIn(checkoutRoute, completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: checkoutRoute,
    })
    expect(startupRouteForLoggedIn(checkoutRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: checkoutRoute,
    })
    expect(startupRouteForLoggedOut(returnRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: returnRoute,
    })
    expect(startupRouteForLoggedIn(returnRoute, completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: returnRoute,
    })
  })

  test('keeps Forum pages public for every auth state', () => {
    const forumRoute = ForumForumRoute({ forumRef: 'void' })
    const receiptRoute = ForumReceiptRoute({ receiptRef: 'receipt.forum.1' })

    expect(startupRouteForLoggedOut(forumRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: forumRoute,
    })
    expect(startupRouteForLoggedIn(forumRoute, completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: forumRoute,
    })
    expect(startupRouteForLoggedIn(forumRoute, incompleteAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: forumRoute,
    })
    expect(startupRouteForLoggedOut(receiptRoute)).toEqual({
      _tag: 'LoggedOutStartupRoute',
      redirect: Option.none(),
      route: receiptRoute,
    })
  })

  test('maps authenticated root visitors to order status', () => {
    expect(startupRouteForLoggedIn(HomeRoute(), completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Order' },
    })
  })

  test('keeps the operator Autopilot shell on the explicit Autopilot route', () => {
    expect(startupRouteForLoggedIn(ChatRoute(), completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Chat' },
    })
  })

  test('routes authenticated visitors without Core Team access to order status', () => {
    expect(startupRouteForLoggedIn(HomeRoute(), auth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Order' },
    })
  })

  test('redirects disabled project workrooms to order status', () => {
    expect(
      startupRouteForLoggedIn(
        TeamProjectChatRoute({
          projectRef: 'artanis',
          teamRef: 'openagents-core-team',
        }),
        completeAuth,
      ),
    ).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.some({
        _tag: 'StartupRedirectToDefaultLoggedInRoute',
        href: '/order',
      }),
      route: { _tag: 'Order' },
    })
  })

  test('redirects authenticated visitors without Core Team access away from invite', () => {
    expect(startupRouteForLoggedIn(InviteRoute(), auth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.some({
        _tag: 'StartupRedirectToDefaultLoggedInRoute',
        href: '/order',
      }),
      route: { _tag: 'Order' },
    })
  })

  test('keeps authenticated visitors without Core Team access on order status', () => {
    expect(startupRouteForLoggedIn(OrderRoute(), auth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Order' },
    })
  })

  test('redirects authenticated visitors without Core Team access away from workrooms', () => {
    expect(startupRouteForLoggedIn(ChatRoute(), auth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.some({
        _tag: 'StartupRedirectToOrder',
        href: '/order',
      }),
      route: { _tag: 'Order' },
    })
  })

  test('keeps logged-out visitors away from the private mullet route', () => {
    expect(startupRouteForLoggedOut(MulletRoute())).toMatchObject({
      _tag: 'LoggedOutStartupRoute',
      redirect: {
        _tag: 'Some',
        value: { _tag: 'StartupRedirectToHome', href: '/' },
      },
      route: { _tag: 'Pylon' },
    })
  })

  test('redirects non-admin users away from the private mullet route', () => {
    expect(startupRouteForLoggedIn(MulletRoute(), completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.some({
        _tag: 'StartupRedirectToOrder',
        href: '/order',
      }),
      route: { _tag: 'Order' },
    })
  })

  test('redirects wrong admin emails away from the private mullet route', () => {
    expect(startupRouteForLoggedIn(MulletRoute(), wrongAdminEmailAuth)).toEqual(
      {
        _tag: 'LoggedInStartupRoute',
        redirect: Option.some({
          _tag: 'StartupRedirectToOrder',
          href: '/order',
        }),
        route: { _tag: 'Order' },
      },
    )
  })

  test('allows chris@openagents.com to stay on the private mullet route', () => {
    expect(startupRouteForLoggedIn(MulletRoute(), completeAdminAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Mullet' },
    })
  })

  test('routes incomplete authenticated users through onboarding', () => {
    expect(startupRouteForLoggedIn(HomeRoute(), incompleteAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.some({
        _tag: 'StartupRedirectToOnboarding',
        href: '/onboarding',
      }),
      route: { _tag: 'Onboarding' },
    })
  })

  test('keeps incomplete authenticated users on onboarding', () => {
    expect(startupRouteForLoggedIn(OnboardingRoute(), incompleteAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.none(),
      route: { _tag: 'Onboarding' },
    })
  })

  test('sends complete authenticated users away from onboarding', () => {
    expect(startupRouteForLoggedIn(OnboardingRoute(), completeAuth)).toEqual({
      _tag: 'LoggedInStartupRoute',
      redirect: Option.some({
        _tag: 'StartupRedirectToDefaultLoggedInRoute',
        href: '/order',
      }),
      route: { _tag: 'Order' },
    })
  })

  test('redirects logged-out app routes to the public homepage', () => {
    expect(
      startupRouteForLoggedOut(TeamChatRoute({ teamRef: 'openagents' })),
    ).toMatchObject({
      _tag: 'LoggedOutStartupRoute',
      redirect: {
        _tag: 'Some',
        value: { _tag: 'StartupRedirectToHome', href: '/' },
      },
      route: { _tag: 'Pylon' },
    })
  })

  test('redirects the deleted login page route to the public homepage', () => {
    expect(
      startupRouteForLoggedOut(NotFoundRoute({ path: '/login' })),
    ).toMatchObject({
      _tag: 'LoggedOutStartupRoute',
      redirect: {
        _tag: 'Some',
        value: { _tag: 'StartupRedirectToHome', href: '/' },
      },
      route: { _tag: 'Pylon' },
    })
  })

  test('redirects unknown public paths to the homepage', () => {
    expect(
      startupRouteForLoggedOut(NotFoundRoute({ path: '/f324f23f' })),
    ).toMatchObject({
      _tag: 'LoggedOutStartupRoute',
      redirect: {
        _tag: 'Some',
        value: { _tag: 'StartupRedirectToHome', href: '/' },
      },
      route: { _tag: 'Pylon' },
    })
  })

  test('does not fetch auth bootstrap for public-only routes', () => {
    expect(
      routeRequiresAuthBootstrap(NotFoundRoute({ path: '/login/github' })),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(PublicAgentRoute({ agentRef: 'artanis' })),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        ShareRoute({ shareId: '123e4567-e89b-42d3-a456-426614174000' }),
      ),
    ).toBe(false)
    expect(routeRequiresAuthBootstrap(SiteCheckoutDemoRoute())).toBe(false)
    expect(routeRequiresAuthBootstrap(MokshaRoute())).toBe(false)
    expect(routeRequiresAuthBootstrap(Moksha2Route())).toBe(false)
    expect(routeRequiresAuthBootstrap(LandingRoute())).toBe(false)
    expect(routeRequiresAuthBootstrap(PylonRoute())).toBe(false)
    expect(routeRequiresAuthBootstrap(TassadarRoute())).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        TassadarReplayRoute({ replaySlug: 'first-real-settlement' }),
      ),
    ).toBe(false)
    expect(routeRequiresAuthBootstrap(PublicStatsArchiveRoute())).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        SiteCheckoutDemoReturnRoute({ returnAction: 'status' }),
      ),
    ).toBe(false)
  })

  test('fetches auth bootstrap for product routes', () => {
    expect(routeRequiresAuthBootstrap(HomeRoute())).toBe(true)
    expect(routeRequiresAuthBootstrap(ChatRoute())).toBe(true)
    expect(routeRequiresAuthBootstrap(InviteRoute())).toBe(true)
    expect(routeRequiresAuthBootstrap(OnboardingRoute())).toBe(true)
    expect(routeRequiresAuthBootstrap(OrderRoute())).toBe(true)
    expect(routeRequiresAuthBootstrap(AutopilotWorkRoute())).toBe(true)
    expect(routeRequiresAuthBootstrap(MulletRoute())).toBe(true)
    expect(
      routeRequiresAuthBootstrap(TeamChatRoute({ teamRef: 'openagents' })),
    ).toBe(true)
    expect(
      routeRequiresAuthBootstrap(
        TeamProjectChatRoute({
          projectRef: 'artanis',
          teamRef: 'openagents',
        }),
      ),
    ).toBe(true)
  })

  test('does not fetch auth bootstrap for the demo route namespace', () => {
    expect(routeRequiresAuthBootstrap(DemoRoute())).toBe(false)
    expect(routeRequiresAuthBootstrap(DemoOrderRoute())).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        DemoThreadRoute({ threadId: 'pylon-release-demo' }),
      ),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        DemoTeamProjectChatRoute({
          teamRef: 'openagents-core-team',
          projectRef: 'artanis',
        }),
      ),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        DemoTeamFilesRoute({ teamRef: 'openagents-core-team' }),
      ),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        DemoTeamFileRoute({
          teamRef: 'openagents-core-team',
          fileId: 'file_pylon_release_plan',
        }),
      ),
    ).toBe(false)
    expect(routeRequiresAuthBootstrap(Demo2Route())).toBe(false)
    expect(routeRequiresAuthBootstrap(Demo2OrderRoute())).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        Demo2ThreadRoute({ threadId: 'pylon-release-demo' }),
      ),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        Demo2TeamProjectChatRoute({
          teamRef: 'openagents-core-team',
          projectRef: 'artanis',
        }),
      ),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        Demo2TeamFilesRoute({ teamRef: 'openagents-core-team' }),
      ),
    ).toBe(false)
    expect(
      routeRequiresAuthBootstrap(
        Demo2TeamFileRoute({
          teamRef: 'openagents-core-team',
          fileId: 'file_pylon_release_plan',
        }),
      ),
    ).toBe(false)
  })
})
