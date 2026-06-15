import { Effect, Option } from 'effect'
import { Scene } from 'foldkit'
import { afterEach, describe, expect, test, vi } from 'vitest'

import {
  authBootstrapFromSession,
  incompleteOnboardingStatus,
} from './domain/session'
import { Flags, flags, init } from './main'
import { update } from './update'
import { view } from './view'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

const authWithTeam = {
  ...authBootstrapFromSession({
    email: 'chris@openagents.com',
    name: 'Christopher David',
    userId: 'github:14167547',
  }),
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

const adminAuth = {
  ...authWithTeam,
  isAdmin: true,
}

const authWithProject = {
  ...authWithTeam,
  teams: [
    {
      ...authWithTeam.teams[0]!,
      projects: [
        {
          id: 'project_artanis',
          teamId: 'team_openagents_core',
          name: 'Artanis',
          slug: 'artanis',
          description: '',
          status: 'active' as const,
        },
      ],
    },
  ],
}

const authWithoutCoreTeam = authBootstrapFromSession({
  email: 'visitor@example.com',
  name: 'Visitor',
  userId: 'github:visitor',
})

const authWithIncompleteOnboarding = {
  ...authWithTeam,
  onboarding: incompleteOnboardingStatus(),
}

describe('auth bootstrap flags', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    window.history.replaceState({}, '', '/')
  })

  test('does not request the auth session on the root Pylon route', async () => {
    window.history.replaceState({}, '', '/')
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  test('does not request the auth session on unknown public paths', async () => {
    window.history.replaceState({}, '', '/f324f23f')
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  test('does not request the auth session on public agent pages', async () => {
    for (const path of [
      '/agents/artanis',
      '/artanis',
      '/agents/adjutant',
      '/adjutant',
    ]) {
      window.history.replaceState({}, '', path)
      const fetchSpy = vi.spyOn(globalThis, 'fetch')

      const loadedFlags = await Effect.runPromise(flags)

      expect(loadedFlags.maybeAuth).toEqual(Option.none())
      expect(fetchSpy).not.toHaveBeenCalled()
      vi.restoreAllMocks()
    }
  })

  test('does not request the auth session on demo routes', async () => {
    for (const path of ['/demo', '/demo2']) {
      window.history.replaceState({}, '', path)
      const fetchSpy = vi.spyOn(globalThis, 'fetch')

      const loadedFlags = await Effect.runPromise(flags)

      expect(loadedFlags.maybeAuth).toEqual(Option.none())
      expect(fetchSpy).not.toHaveBeenCalled()
      vi.restoreAllMocks()
    }
  })

  test('does not request the auth session on the Moksha route', async () => {
    window.history.replaceState({}, '', '/moksha')
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  test('does not request the auth session on the OpenAgents Moksha route', async () => {
    window.history.replaceState({}, '', '/moksha2')
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  test('does not request the auth session on the Pylon route', async () => {
    window.history.replaceState({}, '', '/pylon')
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  test('does not request the auth session on the live Pylon preview route', async () => {
    window.history.replaceState({}, '', '/live')
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  test('requests the auth session on application routes', async () => {
    window.history.replaceState({}, '', '/teams/openagents-core-team/chat')
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(new Response('', { status: 401 }))

    const loadedFlags = await Effect.runPromise(flags)

    expect(loadedFlags.maybeAuth).toEqual(Option.none())
    expect(fetchSpy).toHaveBeenCalledWith('/api/auth/session', {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
  })
})

describe('authenticated startup routing', () => {
  test('opens demo without an auth session', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/demo'),
    )

    expect(model).toMatchObject({
      _tag: 'Demo',
      mode: 'training',
      playback: 'complete',
      routeKey: 'demo:training-fullscreen',
    })
    expect(commands).toHaveLength(0)
  })

  test('opens workroom playback at demo2 without an auth session', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/demo2'),
    )

    expect(model).toMatchObject({
      _tag: 'Demo',
      loggedIn: {
        route: { _tag: 'TeamProjectChat' },
      },
      mode: 'workroom',
      playback: 'playing',
      routeKey: 'demo:pylon-release',
    })
    expect(commands).toHaveLength(0)
  })

  test('opens customer order demo without an auth session', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/demo2/order'),
    )

    expect(model).toMatchObject({
      _tag: 'Demo',
      mode: 'order',
      loggedIn: {
        route: { _tag: 'Onboarding' },
      },
      playback: 'playing',
    })
    expect(commands).toHaveLength(0)
  })

  test('opens demo without using the authenticated product startup gate', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithProject) }),
      appUrl('/demo2/t/pylon-release-demo'),
    )

    expect(model).toMatchObject({
      _tag: 'Demo',
      loggedIn: {
        route: { _tag: 'Thread', threadId: 'pylon-release-demo' },
      },
    })
    expect(commands).toHaveLength(0)
  })

  test('opens Moksha without an auth session', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/moksha'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Moksha' },
    })
    expect(commands).toHaveLength(0)
  })

  test('renders the Moksha route through the top-level view', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/moksha'),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.selector('[data-route="moksha"]')).toExist(),
      Scene.expect(Scene.selector('oa-moksha')).toExist(),
    )
  })

  test('renders the OpenAgents Moksha route through the top-level view', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/moksha2'),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.selector('[data-route="moksha2"]')).toExist(),
      Scene.expect(Scene.selector('oa-moksha')).toExist(),
    )
  })

  test('opens Pylon without an auth session', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/pylon'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Pylon' },
    })
    expect(commands).toHaveLength(0)
  })

  test('renders the Pylon route through the top-level view', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/pylon'),
    )

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.selector('[data-route="pylon"]')).toExist(),
      Scene.expect(Scene.selector('oa-pylon')).toExist(),
      Scene.expect(Scene.selector('oa-pylon-launch-gate')).toExist(),
      Scene.expect(
        Scene.selector('[data-cta="download-autopilot"]'),
      ).toExist(),
    )
  })

  test('renders the live Pylon preview route through the top-level view', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/live'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Live' },
    })
    expect(commands).toHaveLength(0)

    Scene.scene(
      { update, view },
      Scene.with(model),
      Scene.expect(Scene.selector('[data-route="live"]')).toExist(),
      Scene.expect(Scene.selector('oa-pylon')).toExist(),
      Scene.expect(Scene.selector('oa-pylon-bezier-network')).toExist(),
      Scene.expect(Scene.selector('oa-pylon-stats')).toExist(),
      Scene.expect(Scene.selector('oa-live-copy-instructions')).toExist(),
      Scene.expect(Scene.selector('oa-pylon-countdown')).not.toExist(),
    )
  })

  test('keeps onboarding behind the logged-out application gate', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/onboarding'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Onboarding' },
    })
    expect(commands).toHaveLength(0)
  })

  test('redirects complete authenticated onboarding visits to order status', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/onboarding'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Order' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
      'RedirectToDefaultLoggedInRoute',
    ])
  })

  test('keeps incomplete authenticated onboarding visits in the onboarding flow', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithIncompleteOnboarding) }),
      appUrl('/onboarding'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Onboarding' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadOnboardingRepositories',
    ])
  })

  test('keeps incomplete authenticated root visits on the public Pylon scene', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithIncompleteOnboarding) }),
      appUrl('/'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Pylon' },
    })
    expect(commands).toHaveLength(0)
  })

  test('keeps authenticated root visits on the public Pylon scene', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithoutCoreTeam) }),
      appUrl('/'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Pylon' },
    })
    expect(commands).toHaveLength(0)
  })

  test('redirects authenticated visitors without Core Team access away from invite', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithoutCoreTeam) }),
      appUrl('/invite'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Order' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
      'RedirectToDefaultLoggedInRoute',
    ])
  })

  test('keeps logged-out root visitors on the public Pylon scene', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Pylon' },
    })
    expect(commands).toHaveLength(0)
  })

  test('serves the former public homepage from stats', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/stats'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Stats' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadPublicPylonStats',
      'LoadPublicForumLaunchStatus',
      'LoadPublicForumTipLeaderboards',
    ])
  })

  test('redirects unknown logged-out paths to the homepage', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/f324f23f'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Pylon' },
    })
    expect(commands.map(command => command.name)).toEqual(['RedirectToHome'])
  })

  test('loads public Artanis goals and pylon stats without an authenticated shell', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/artanis'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'PublicAgent', agentRef: 'artanis' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadPublicAgentGoal',
      'LoadPublicArtanisReport',
      'LoadPublicPylonStats',
    ])
    expect(commands[0]?.args).toEqual({
      agentId: 'agent_artanis',
      agentRef: 'artanis',
    })
  })

  test('loads public Adjutant goals and activity without pylon stats', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/adjutant'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'PublicAgent', agentRef: 'adjutant' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadPublicAgentGoal',
      'LoadPublicAdjutantActivity',
    ])
    expect(commands[0]?.args).toEqual({
      agentId: 'agent_adjutant',
      agentRef: 'adjutant',
    })
  })

  test('loads share projections through the public shell', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/share/123e4567-e89b-42d3-a456-426614174000'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: {
        _tag: 'Share',
        shareId: '123e4567-e89b-42d3-a456-426614174000',
      },
      shareProjection: {
        _tag: 'ShareProjectionLoading',
        shareId: '123e4567-e89b-42d3-a456-426614174000',
      },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadShareProjection',
    ])
    expect(commands[0]?.args).toEqual({
      shareId: '123e4567-e89b-42d3-a456-426614174000',
    })
  })

  test('keeps authenticated public Artanis visits outside the product shell', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/agents/artanis'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'PublicAgent', agentRef: 'artanis' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadPublicAgentGoal',
      'LoadPublicArtanisReport',
      'LoadPublicPylonStats',
    ])
  })

  test('keeps authenticated root visits on the public Pylon scene', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Pylon' },
    })
    expect(commands).toHaveLength(0)
  })

  test('loads the admin route for configured admins', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(adminAuth) }),
      appUrl('/admin'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Admin' },
      auth: { isAdmin: true },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadAdminOverview',
    ])
  })

  test('loads the private mullet route for the confirmed admin', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(adminAuth) }),
      appUrl('/mullet'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Mullet' },
      auth: { isAdmin: true },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadMulletBootstrap',
    ])
  })

  test('redirects non-admin users away from the admin route', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/admin'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Order' },
      auth: { isAdmin: false },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
      'RedirectToOrder',
    ])
  })

  test('loads the operator Autopilot shell at /autopilot for team members', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/autopilot'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Chat' },
      auth: { teams: authWithTeam.teams },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadAgentGoal',
      'LoadThreadFiles',
      'FocusChatComposer',
      'RequestNotificationPermission',
    ])
    expect(commands[0]?.args).toEqual({
      href: '/api/sync/workspace/github%3A14167547/snapshot',
      scope: 'workspace:github:14167547',
    })
  })

  test('does not expose the deprecated dashboard route', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/dashboard'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'NotFound', path: '/dashboard' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'RequestNotificationPermission',
    ])
  })

  test('loads team files pages directly without redirecting to chat', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/teams/openagents-core-team/files'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'TeamFiles', teamRef: 'openagents-core-team' },
      auth: { teams: authWithTeam.teams },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadThreadFiles',
      'RequestNotificationPermission',
    ])
    expect(commands[2]?.args).toEqual({
      href: '/api/teams/team_openagents_core/files',
      scopeKey: 'team-files:team_openagents_core',
    })
  })

  test('redirects disabled project workroom URLs to order status', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithProject) }),
      appUrl('/teams/openagents-core-team/projects/artanis/chat'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Order' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
      'RedirectToDefaultLoggedInRoute',
    ])
    expect(commands.at(-1)?.args).toEqual({ href: '/order' })
  })

  test('loads team file detail pages directly without redirecting to chat', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/teams/openagents-core-team/files/file_1'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: {
        _tag: 'TeamFile',
        fileId: 'file_1',
        teamRef: 'openagents-core-team',
      },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadThreadFileDetail',
      'RequestNotificationPermission',
    ])
    expect(commands[2]?.args).toEqual({
      fileId: 'file_1',
      href: '/api/thread-files/file_1?teamId=team_openagents_core',
    })
  })

  test('loads personal file detail pages directly without redirecting to chat', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/files/file_personal_1'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'PersonalFile', fileId: 'file_personal_1' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'LoadThreadFileDetail',
      'RequestNotificationPermission',
    ])
    expect(commands[2]?.args).toEqual({
      fileId: 'file_personal_1',
      href: '/api/thread-files/file_personal_1',
    })
  })
})
