import { Option } from 'effect'
import { Scene } from 'foldkit'
import { Internal } from 'foldkit/navigation'
import { describe, expect, test } from 'vitest'

import {
  authBootstrapFromSession,
  completedOnboardingStatus,
  incompleteOnboardingStatus,
} from './domain/session'
import {
  ChangedUrl,
  ClickedLink,
  GotLoggedInMessage,
  GotLoggedOutMessage,
  LoadedSession,
  RequestedLoggedOutLogout,
} from './message'
import { LoggedIn, LoggedOut } from './model'
import {
  ClickedLogout,
  ClickedNewChat,
  SubmittedInviteCode,
  SucceededSkipOnboardingBilling,
  UpdatedInviteCode,
} from './page/loggedIn/message'
import {
  ClickedOnboardingStep,
  RequestedLandingLogout,
} from './page/loggedOut/message'
import {
  ChatRoute,
  DocsRoute,
  HomeRoute,
  InviteRoute,
  LandingRoute,
  OnboardingRoute,
  OrderRoute,
} from './route'
import { update } from './update'
import { view } from './view'

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

const authWithoutCoreTeam = authBootstrapFromSession({
  email: 'visitor@example.com',
  name: 'Visitor',
  userId: 'github:visitor',
})

const authWithIncompleteOnboarding = {
  ...authWithTeam,
  onboarding: incompleteOnboardingStatus(),
}

const internalRequest = (pathname: string) =>
  ClickedLink({
    request: Internal({
      url: {
        protocol: 'https:',
        host: 'openagents.com',
        port: Option.none(),
        pathname,
        search: Option.none(),
        hash: Option.none(),
      },
    }),
  })

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

describe('app link routing', () => {
  test('loads auth routes as document navigations', () => {
    const [, commands] = update(
      LoggedOut.init(HomeRoute()),
      internalRequest('/login/github'),
    )

    expect(commands).toHaveLength(1)
    expect(commands[0]?.name).toBe('LoadExternal')
    expect(commands[0]?.args).toEqual({
      href: 'https://openagents.com/login/github',
    })
  })

  test('keeps normal app routes as internal navigation', () => {
    const [, commands] = update(
      LoggedOut.init(HomeRoute()),
      internalRequest('/'),
    )

    expect(commands).toHaveLength(1)
    expect(commands[0]?.name).toBe('NavigateInternal')
    expect(commands[0]?.args).toEqual({ url: 'https://openagents.com/' })
  })

  test('handles onboarding CTA messages through the logged-out wrapper', () => {
    const [model, commands] = update(
      LoggedOut.init(OnboardingRoute()),
      GotLoggedOutMessage({
        message: ClickedOnboardingStep({ step: 'funding' }),
      }),
    )

    expect(model._tag).toBe('LoggedOut')
    expect(model._tag === 'LoggedOut' ? model.onboarding.step : '').toBe(
      'funding',
    )
    expect(commands).toHaveLength(0)
  })

  test('renders onboarding CTA as a GitHub login link', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(OnboardingRoute())),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toHaveAttr('href', '/login/github'),
    )
  })

  test('renders the public header login link as GitHub OAuth', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsRoute())),
      Scene.expect(Scene.selector('[data-login-popover]')).toExist(),
      Scene.expect(Scene.selector('[data-agent-access-panel]')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toHaveAttr('href', '/login/github'),
      Scene.expect(
        Scene.role('link', { name: 'Agent instructions' }),
      ).toHaveAttr('href', '/AGENTS.md'),
      Scene.expect(Scene.role('link', { name: 'OpenAPI' })).toHaveAttr(
        'href',
        '/api/openapi.json',
      ),
    )
  })

  test('redirects unknown logged-out URL changes to the homepage', () => {
    const [model, commands] = update(
      LoggedOut.init(HomeRoute()),
      ChangedUrl({ url: appUrl('/f324f23f') }),
    )

    // The homepage route is the landing scene; Pylon lives at /pylons.
    // Unknown logged-out URLs redirect home. Landing now also seeds the live
    // "Khala Tokens Served" pill, so the seed commands precede the redirect.
    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Landing' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadKhalaTokensServedSnapshot',
      'LoadPublicKhalaTokensServed',
      'RedirectToHome',
    ])
  })

  test('keeps team room routes as internal navigation', () => {
    const [, commands] = update(
      LoggedOut.init(HomeRoute()),
      internalRequest('/teams/openagents-core-team/chat'),
    )

    expect(commands).toHaveLength(1)
    expect(commands[0]?.name).toBe('NavigateInternal')
    expect(commands[0]?.args).toEqual({
      url: 'https://openagents.com/teams/openagents-core-team/chat',
    })
  })

  test('loads public document pages through document navigation', () => {
    const [, docsCommands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      internalRequest('/docs'),
    )

    expect(docsCommands).toHaveLength(1)
    expect(docsCommands[0]?.name).toBe('LoadExternal')
    expect(docsCommands[0]?.args).toEqual({
      href: 'https://openagents.com/docs',
    })

    const [, blogCommands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      internalRequest('/blog'),
    )

    expect(blogCommands).toHaveLength(1)
    expect(blogCommands[0]?.name).toBe('LoadExternal')
    expect(blogCommands[0]?.args).toEqual({
      href: 'https://openagents.com/blog',
    })

    const [, forumCommands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      internalRequest('/forum/t/1f4e8c11-2330-403f-aa4b-82dd1a673e9f'),
    )

    expect(forumCommands).toHaveLength(1)
    expect(forumCommands[0]?.name).toBe('LoadExternal')
    expect(forumCommands[0]?.args).toEqual({
      href: 'https://openagents.com/forum/t/1f4e8c11-2330-403f-aa4b-82dd1a673e9f',
    })

    const [, trainingRunsCommands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      internalRequest('/training/runs/run.cs336.a1.demo'),
    )

    expect(trainingRunsCommands).toHaveLength(1)
    expect(trainingRunsCommands[0]?.name).toBe('LoadExternal')
    expect(trainingRunsCommands[0]?.args).toEqual({
      href: 'https://openagents.com/training/runs/run.cs336.a1.demo',
    })
  })

  test('loads public API and agent document links through document navigation', () => {
    for (const path of [
      '/api/openapi.json',
      '/api/omni/sdk-seed',
      '/.well-known/openagents.json',
      '/AGENTS.md',
      '/HEARTBEAT.md',
      '/RULES.md',
      '/skill.json',
    ]) {
      const [, commands] = update(
        LoggedIn.init(ChatRoute(), authWithTeam),
        internalRequest(path),
      )

      expect(commands).toHaveLength(1)
      expect(commands[0]?.name).toBe('LoadExternal')
      expect(commands[0]?.args).toEqual({
        href: `https://openagents.com${path}`,
      })
    }
  })

  test('loads authenticated root sessions into order status', () => {
    const [model, commands] = update(
      LoggedOut.init(HomeRoute()),
      LoadedSession({
        session: Option.some(authWithTeam),
      }),
    )

    expect(model).toEqual(LoggedIn.init(OrderRoute(), authWithTeam))
    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
    ])
  })

  test('loads authenticated sessions with teams into order status', () => {
    const [model, commands] = update(
      LoggedOut.init(HomeRoute()),
      LoadedSession({
        session: Option.some(authWithTeam),
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
    ])
    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Order' },
      auth: { teams: authWithTeam.teams },
    })
  })

  test('loads incomplete authenticated sessions into onboarding', () => {
    const [model, commands] = update(
      LoggedOut.init(HomeRoute()),
      LoadedSession({
        session: Option.some(authWithIncompleteOnboarding),
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadOnboardingRepositories',
      'RedirectToOnboarding',
    ])
    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Onboarding' },
      auth: { onboarding: { step: 'repository' } },
    })
  })

  test('loads authenticated sessions without Core Team access into order status', () => {
    const [model, commands] = update(
      LoggedOut.init(HomeRoute()),
      LoadedSession({
        session: Option.some(authWithoutCoreTeam),
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'InstallAccountMenuOutsideClick',
      'LoadCustomerOrders',
    ])
    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Order' },
    })
  })

  test('rejects invite codes until codes exist', () => {
    const [typedModel] = update(
      LoggedIn.init(InviteRoute(), authWithoutCoreTeam),
      GotLoggedInMessage({
        message: UpdatedInviteCode({ value: 'EARLY' }),
      }),
    )

    const [submittedModel, commands] = update(
      typedModel,
      GotLoggedInMessage({ message: SubmittedInviteCode() }),
    )

    expect(commands).toHaveLength(0)
    expect(submittedModel).toMatchObject({
      _tag: 'LoggedIn',
      inviteCodeValue: 'EARLY',
      inviteCodeAction: {
        _tag: 'InviteCodeActionFailed',
        error: 'Invalid invite code.',
      },
    })
  })

  test('redirects after authenticated onboarding completes', () => {
    const [model, commands] = update(
      LoggedIn.init(OnboardingRoute(), authWithIncompleteOnboarding),
      GotLoggedInMessage({
        message: SucceededSkipOnboardingBilling({
          response: { onboarding: completedOnboardingStatus() },
        }),
      }),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      auth: { onboarding: { step: 'complete' } },
    })
    expect(commands.map(command => command.name)).toEqual([
      'RedirectToDefaultLoggedInRoute',
    ])
  })

  test('hydrates mission routes when the sidebar changes the URL', () => {
    const runId = 'f1f1bd76-fdb6-42c6-b0b6-d82d92f84212'
    const [model, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      ChangedUrl({ url: appUrl(`/t/${runId}`) }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'ScrollChatTimelineToEnd',
    ])
    expect(commands[0]?.args).toEqual({
      href: `/api/sync/thread/${runId}/snapshot`,
      scope: `thread:${runId}`,
    })
    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Thread', threadId: runId },
      chatRun: { _tag: 'Loading', runId },
      chatMessages: [],
    })
  })

  test('loads team files when the sidebar changes the URL', () => {
    const [model, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      ChangedUrl({ url: appUrl('/teams/openagents-core-team/files') }),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'TeamFiles', teamRef: 'openagents-core-team' },
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

  test('loads team file details when the URL changes', () => {
    const [model, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      ChangedUrl({ url: appUrl('/teams/openagents-core-team/files/file_1') }),
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

  test('loads personal file details when the URL changes', () => {
    const [model, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      ChangedUrl({ url: appUrl('/files/file_personal_1') }),
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

  test('treats dashboard URL changes as not found', () => {
    const [model, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      ChangedUrl({ url: appUrl('/dashboard') }),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'NotFound', path: '/dashboard' },
    })
    expect(commands).toHaveLength(0)
  })

  test('clicking new chat navigates to the chat route', () => {
    const [model, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      GotLoggedInMessage({ message: ClickedNewChat() }),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Chat' },
    })
    expect(commands.map(command => command.name)).toEqual([
      'FocusChatComposer',
      'RedirectToChat',
    ])
  })
  test('logging out from the LoggedOut (public) context clears the session and loads /auth/logout', () => {
    const [model, commands] = update(
      LoggedOut.init(HomeRoute(), Option.some(authWithTeam.session)),
      RequestedLoggedOutLogout(),
    )

    // Same model, and the SAME reused logout pair the LoggedIn path uses.
    expect(model._tag).toBe('LoggedOut')
    expect(commands.map(command => command.name)).toEqual([
      'ClearSession',
      'LoadExternal',
    ])
    expect(commands[1]?.args).toEqual({ href: '/auth/logout' })
  })

  test('logging out from the LoggedIn context still fires the ClearSession + /auth/logout pair', () => {
    const [, commands] = update(
      LoggedIn.init(ChatRoute(), authWithTeam),
      GotLoggedInMessage({ message: ClickedLogout() }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'ClearSession',
      'LoadExternal',
    ])
    expect(commands[1]?.args).toEqual({ href: '/auth/logout' })
  })

  test('logging out from the homepage hero floating avatar reuses the same logout (clear session + /auth/logout)', () => {
    const [model, commands] = update(
      LoggedOut.init(
        LandingRoute(),
        Option.some(authWithTeam.session),
      ),
      GotLoggedOutMessage({ message: RequestedLandingLogout() }),
    )

    // Stays on the public LoggedOut model and fires the single logout command,
    // which itself clears the cached session then full-page navigates to
    // /auth/logout (the same endpoint the header logout uses).
    expect(model._tag).toBe('LoggedOut')
    expect(commands.map(command => command.name)).toEqual(['LogoutFromLanding'])
  })
})
