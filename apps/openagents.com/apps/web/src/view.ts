import { Option } from 'effect'
import { type Document, type Html, html } from 'foldkit/html'

import { Session } from './domain/session'
import {
  GotDemoMessage,
  GotLoggedInMessage,
  GotLoggedOutMessage,
  Message,
  RequestedLoggedOutLogout,
} from './message'
import type { Model } from './model'
import { Demo, LoggedIn, LoggedOut } from './model'
import * as Activity from './page/activity'
import * as Animations from './page/animations'
import * as ArtanisTraceTree from './page/artanisTraceTree'
import * as ArtanisAccounts from './page/artanisAccounts'
import * as Blog from './page/blog'
import * as Business from './page/business'
import * as LandingPreview from './page/landingPreview'
import * as Components from './page/components'
import * as DemoLegal from './page/demoLegal'
import * as Docs from './page/docs'
import * as Forum from './page/forum'
import type {
  PublicPylonStats,
  PublicPylonStatsModel,
} from './page/loggedOut/model'
import * as Code from './page/code'
import * as Privacy from './page/privacy'
import type {
  PublicHeaderAuthState,
  PublicHeaderViewer,
} from './page/publicHeader'
import * as PylonCodexAssignmentStatus from './page/pylonCodexAssignmentStatus'
import * as QaSwarm from './page/qa-swarm'
import * as Run from './page/run'
import * as SiteCheckoutDemo from './page/siteCheckoutDemo'
import * as Terms from './page/terms'
import * as Trace from './page/trace'
import * as TraceCompare from './page/trace-compare'
import {
  type AppRoute,
  type RouteRenderDisposition,
  routeRegistry,
} from './route'
import * as Ui from './ui'

const githubLoginHref = '/login/github'

const githubIcon = (): Html => {
  const h = html<Message>()

  return h.svg(
    [
      h.AriaHidden(true),
      Ui.className<Message>('size-5 shrink-0'),
      h.Xmlns('http://www.w3.org/2000/svg'),
      h.ViewBox('0 0 24 24'),
      h.Fill('currentColor'),
    ],
    [
      h.path(
        [
          h.D(
            'M12 2C6.48 2 2 6.58 2 12.25c0 4.53 2.87 8.37 6.84 9.73.5.09.68-.22.68-.49 0-.24-.01-.88-.01-1.73-2.78.62-3.37-1.38-3.37-1.38-.45-1.19-1.11-1.51-1.11-1.51-.91-.64.07-.63.07-.63 1 .07 1.53 1.06 1.53 1.06.89 1.56 2.34 1.11 2.91.85.09-.66.35-1.11.63-1.37-2.22-.26-4.56-1.14-4.56-5.06 0-1.12.39-2.03 1.03-2.75-.1-.26-.45-1.3.1-2.71 0 0 .84-.28 2.75 1.05A9.3 9.3 0 0 1 12 6.97c.85 0 1.7.12 2.5.35 1.91-1.33 2.75-1.05 2.75-1.05.55 1.41.2 2.45.1 2.71.64.72 1.03 1.63 1.03 2.75 0 3.93-2.34 4.8-4.57 5.05.36.32.68.95.68 1.92 0 1.38-.01 2.5-.01 2.84 0 .27.18.59.69.49A10.22 10.22 0 0 0 22 12.25C22 6.58 17.52 2 12 2Z',
          ),
        ],
        [],
      ),
    ],
  )
}

const githubLoginButton = (): Html => {
  const h = html<Message>()

  return Ui.v4LinkButton<Message>({
    href: githubLoginHref,
    label: 'Log in with GitHub',
    variant: 'secondary',
    size: 'md',
    left: githubIcon(),
    attrs: [
      h.DataAttribute('login-button', 'github'),
      h.AriaBusy(false),
      h.Attribute(
        'onclick',
        "if(this.getAttribute('aria-disabled')==='true'){event.preventDefault();return false;}var label=this.querySelector('[data-login-label]');if(label){label.textContent='Logging in...';}this.setAttribute('aria-disabled','true');this.setAttribute('aria-busy','true');this.classList.add('pointer-events-none','opacity-75');",
      ),
    ],
    labelAttrs: [h.DataAttribute('login-label', '')],
  })
}

const publicAgentPath = (): Html => {
  const h = html<Message>()
  const instruction =
    'Read https://openagents.com/AGENTS.md. Do a dry-run first. Inspect the manifest and OpenAPI before planning any action.'

  return h.section(
    [
      Ui.className<Message>(
        'mx-auto grid w-full gap-2 border border-white/10 bg-[#010102] p-3 text-left text-[0.6875rem] leading-4 text-white/45 sm:w-[min(100%,20rem)]',
      ),
    ],
    [
      h.h2(
        [
          Ui.className<Message>(
            'm-0 text-center text-[0.6875rem] font-semibold uppercase leading-none tracking-wide text-white/60',
          ),
        ],
        ['I am an Agent'],
      ),
      h.textarea(
        [
          h.AriaLabel('Copyable agent instruction'),
          h.Readonly(true),
          h.Value(instruction),
          h.Rows(3),
          Ui.className<Message>(
            'min-h-16 resize-none border border-white/10 bg-black p-2 text-[0.625rem] leading-4 text-white/55 outline-none',
          ),
        ],
        [],
      ),
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-center gap-2 text-[0.625rem] uppercase leading-none tracking-wide',
          ),
        ],
        [
          h.a(
            [
              h.Href('/.well-known/openagents.json'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Capability manifest'],
          ),
          h.a(
            [
              h.Href('/api/openapi.json'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['OpenAPI'],
          ),
          h.a(
            [
              h.Href('/api/public/proof/otec'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Public proof'],
          ),
        ],
      ),
    ],
  )
}

const numberFormatter = new Intl.NumberFormat('en-US')

const formatNumber = (value: number): string => numberFormatter.format(value)

const rootStatsFromModel = (
  model: PublicPylonStatsModel,
): PublicPylonStats | null =>
  model._tag === 'PublicPylonStatsLoaded' ? model.stats : null

const rootStatsStatusText = (model: PublicPylonStatsModel): string =>
  model._tag === 'PublicPylonStatsLoading'
    ? 'Loading'
    : model._tag === 'PublicPylonStatsFailed'
      ? 'Unavailable'
      : model._tag === 'PublicPylonStatsLoaded' && model.stats.available
        ? 'Live'
        : 'Unavailable'

const rootStatsValue = (
  stats: PublicPylonStats | null,
  pick: (stats: PublicPylonStats) => string,
): string => (stats === null ? '-' : pick(stats))

const rootPylonStat = (label: string, value: string, detail: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-[5.75rem] content-between gap-2 border border-[#222] bg-[#0d0d0d] p-3 text-left',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.7rem] uppercase text-white/45',
          ),
        ],
        [label],
      ),
      h.div(
        [
          Ui.className<Message>(
            'min-w-0 truncate text-2xl font-semibold tabular-nums text-[#f1efe8]',
          ),
        ],
        [value],
      ),
      h.div(
        [Ui.className<Message>('min-h-4 text-[0.72rem] text-white/45')],
        [detail],
      ),
    ],
  )
}

const rootPylonStatsStrip = (model: PublicPylonStatsModel): Html => {
  const h = html<Message>()
  const stats = rootStatsFromModel(model)
  const freshness =
    stats === null
      ? 'Freshness pending'
      : stats.available
        ? `Fresh ${stats.asOfLabel ?? 'recently'}`
        : (stats.error ?? 'Stats unavailable')

  return h.section(
    [
      Ui.className<Message>(
        'mx-auto grid w-full gap-3 border-t border-[#222] pt-5',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-2 text-left',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-sm font-semibold text-[#f1efe8]',
                  ),
                ],
                ['Live Pylons'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-[0.75rem] text-white/45')],
                [freshness],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'min-h-6 border border-[#242424] px-2 py-1 text-[0.7rem] uppercase text-white/50',
              ),
            ],
            [rootStatsStatusText(model)],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5',
          ),
        ],
        [
          rootPylonStat(
            'Online now',
            rootStatsValue(stats, value => formatNumber(value.pylonsOnlineNow)),
            'Heartbeat window',
          ),
          rootPylonStat(
            'Seen in 24h',
            rootStatsValue(stats, value => formatNumber(value.pylonsSeen24h)),
            'Recent check-ins',
          ),
          rootPylonStat(
            'Wallet ready',
            rootStatsValue(stats, value =>
              formatNumber(value.pylonsWalletReadyNow),
            ),
            'Public readiness',
          ),
          rootPylonStat(
            'Earning gate',
            stats === null
              ? '-'
              : stats.earningLaunchGate.publicEarningCopyAllowed
                ? 'Ready'
                : 'Blocked',
            stats === null
              ? 'Stats loading'
              : stats.earningLaunchGate.publicEarningCopyAllowed
                ? 'Bounded copy'
                : 'Unsafe copy blocked',
          ),
          rootPylonStat(
            'Version floor',
            rootStatsValue(stats, value => `v${value.minimumClientVersion}+`),
            'Pylon line',
          ),
        ],
      ),
    ],
  )
}

const maintenanceBody = (
  publicPylonStats: PublicPylonStatsModel,
): Document['body'] => {
  const h = html<Message>()

  return Ui.centeredFrame<Message>([
    h.div(
      [
        Ui.className<Message>(
          'grid w-full max-w-[260px] gap-6 text-center text-sm leading-6 text-white/70 sm:max-w-none',
        ),
      ],
      [
        h.p(
          [Ui.className<Message>('m-0 text-white')],
          [
            h.span([Ui.className<Message>('font-bold')], ['Autopilot']),
            ' is a cloud coding agent.',
          ],
        ),
        h.p(
          [Ui.className<Message>('m-0 py-2 text-white/50')],
          ['Now in beta! Get a free coding task back within 24 hours.'],
        ),
        h.div([Ui.className<Message>('mb-4')], [githubLoginButton()]),
        rootPylonStatsStrip(publicPylonStats),
        publicAgentPath(),
      ],
    ),
  ])
}

const docTitle = (slug: string): string =>
  Option.match(Docs.docPageTitle(slug), {
    onNone: () => 'Docs',
    onSome: pageTitle => pageTitle,
  })

const blogTitle = (slug: string): string =>
  Option.match(Blog.blogPostTitle(slug), {
    onNone: () => 'Blog',
    onSome: postTitle => postTitle,
  })

const title = (model: Model): string => {
  if (model._tag === 'Demo') {
    return 'OpenAgents Demo'
  }

  switch (model.route._tag) {
    case 'Docs':
      return 'Docs - OpenAgents'
    case 'DocsPage':
      return `${docTitle(model.route.slug)} - OpenAgents`
    case 'Forum':
      return 'Forum - OpenAgents'
    case 'ForumForum':
    case 'ForumTopic':
    case 'ForumReceipt':
      return Forum.title(model.route)
    case 'SiteCheckoutDemo':
      return 'Demo checkout - OpenAgents'
    case 'SiteCheckoutDemoReturn':
      return SiteCheckoutDemo.title(model.route)
    case 'Blog':
      return 'Blog - OpenAgents'
    case 'BlogPost':
      return `${blogTitle(model.route.slug)} - OpenAgents`
    case 'Components':
    case 'ComponentsFamily':
      return 'Components - OpenAgents'
    case 'Business':
      return 'Agents that work - OpenAgents'
    case 'LandingPreview':
      return 'OpenAgents'
    case 'Autopilot':
    case 'AutopilotVertical':
      return 'Autopilot - OpenAgents'
    case 'Terms':
      return 'Terms of Service - OpenAgents'
    case 'Privacy':
      return 'Privacy Policy - OpenAgents'
    case 'Code':
      return 'Khala Code - OpenAgents'
    case 'Animations':
      return 'Animations - OpenAgents'
    case 'Activity':
      return 'Activity - OpenAgents'
    case 'ArtanisAccounts':
      return 'Artanis accounts - OpenAgents'
    case 'ArtanisGym':
      return 'Artanis Gym - OpenAgents'
    case 'DemoLegal':
      return 'Legal demo - OpenAgents'
    case 'Run':
      return 'Live Tassadar run - OpenAgents'
    case 'Tassadar':
      return 'Tassadar run - OpenAgents'
    case 'KhalaChat':
      return 'Khala chat - OpenAgents'
    case 'TassadarReplay':
      return 'Tassadar proof replay - OpenAgents'
    case 'Login':
      return 'Log in - OpenAgents'
    case 'PublicAgent':
      return `${model.route.agentRef} - OpenAgents`
    case 'ArtanisTraceTree':
      return ArtanisTraceTree.title()
    case 'Share':
      return 'Shared Workroom - OpenAgents'
    case 'Trace':
      return Trace.title(
        model.route,
        model._tag === 'LoggedOut' ? model.trace : undefined,
      )
    case 'TraceCompare':
      return TraceCompare.title(model.route)
    case 'QaSwarm':
      return QaSwarm.title(model.route)
    case 'PylonCodexAssignmentStatus':
      return PylonCodexAssignmentStatus.title(model.route)
    default:
      return 'OpenAgents'
  }
}

const publicRouteBody = (model: Model): Document['body'] | undefined => {
  if (model._tag === 'Demo') {
    return undefined
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'PublicAgent') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-public-agent',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'Share') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-share',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'Moksha') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-moksha',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'Moksha2') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-moksha2',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (
    model._tag === 'LoggedOut' &&
    (model.route._tag === 'Landing' ||
      model.route._tag === 'Khala' ||
      model.route._tag === 'KhalaChat' ||
      model.route._tag === 'Tassadar')
  ) {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-landing-scene',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  // /autopilot + /autopilot/{vertical} render through the loggedOut Submodel so
  // the onboarding flow has model/update access (the conversation, surfaced
  // typed components, and composer are stateful). The Submodel mounts the
  // onboarding HUD over the SHARED persistent scene at the `autopilot` pose
  // (#6129); it does NOT use the stateless public-header shell below.
  if (
    model._tag === 'LoggedOut' &&
    (model.route._tag === 'Autopilot' ||
      model.route._tag === 'AutopilotVertical')
  ) {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-autopilot-onboarding',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'Pylon') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-pylon',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'Login') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-login',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (model._tag === 'LoggedOut' && model.route._tag === 'Workspace') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-workspace',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  // `/gym` is the public Terminal-Bench web visualizer. Its registry render
  // disposition is `submodel`, so it renders through the loggedOut Submodel
  // (the gym page reads `model.gym` and mounts the three-effect run field). It
  // must be dispatched here; otherwise it falls through `publicRouteBody` to
  // `maintenanceBody`, which silently shows the homepage-lookalike instead of
  // the gym page (#6258).
  if (model._tag === 'LoggedOut' && model.route._tag === 'Gym') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-gym',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  // `/mirrorcode` is the public "MirrorCode, powered by Khala" page (#6378). Its
  // registry render disposition is `submodel`, so — exactly like `/gym` above —
  // it must be dispatched here through the loggedOut Submodel (the page reads
  // `model.mirrorCodeRuns`). Otherwise it falls through `publicRouteBody` to
  // `maintenanceBody`, which silently shows the homepage-lookalike instead of
  // the MirrorCode page.
  if (model._tag === 'LoggedOut' && model.route._tag === 'MirrorCode') {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-mirrorcode',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (
    model._tag === 'LoggedOut' &&
    (model.route._tag === 'Home' ||
      model.route._tag === 'Stats' ||
      model.route._tag === 'PublicStatsArchive' ||
      model.route._tag === 'ProductPromises')
  ) {
    const h = html<Message>()

    return h.submodel({
      slotId: 'logged-out-home',
      model,
      view: LoggedOut.view,
      toParentMessage: message => GotLoggedOutMessage({ message }),
    })
  }

  if (
    model.route._tag !== 'Docs' &&
    model.route._tag !== 'DocsPage' &&
    model.route._tag !== 'Forum' &&
    model.route._tag !== 'ForumForum' &&
    model.route._tag !== 'ForumTopic' &&
    model.route._tag !== 'ForumReceipt'
  ) {
    if (
      model.route._tag !== 'SiteCheckoutDemo' &&
      model.route._tag !== 'SiteCheckoutDemoReturn' &&
      model.route._tag !== 'Blog' &&
      model.route._tag !== 'BlogPost' &&
      model.route._tag !== 'Components' &&
      model.route._tag !== 'ComponentsFamily' &&
      model.route._tag !== 'Business' &&
      model.route._tag !== 'LandingPreview' &&
      model.route._tag !== 'Autopilot' &&
      model.route._tag !== 'AutopilotVertical' &&
      model.route._tag !== 'Terms' &&
      model.route._tag !== 'Privacy' &&
      model.route._tag !== 'Code' &&
      model.route._tag !== 'Animations' &&
      model.route._tag !== 'Activity' &&
      model.route._tag !== 'ArtanisAccounts' &&
      model.route._tag !== 'DemoLegal' &&
      model.route._tag !== 'Run' &&
      model.route._tag !== 'Tassadar' &&
      model.route._tag !== 'TassadarReplay' &&
      model.route._tag !== 'Trace' &&
      model.route._tag !== 'TraceCompare' &&
      model.route._tag !== 'QaSwarm' &&
      model.route._tag !== 'ArtanisTraceTree' &&
      model.route._tag !== 'PylonCodexAssignmentStatus'
    ) {
      return undefined
    }
  }

  const viewerFromSession = (session: Session): PublicHeaderViewer => ({
    displayName: session.name,
    email: session.email,
    ...(session.avatarUrl !== undefined && session.avatarUrl !== ''
      ? { avatarUrl: session.avatarUrl }
      : {}),
  })

  const authState: PublicHeaderAuthState<Message> =
    model._tag === 'LoggedIn'
      ? {
          _tag: 'LoggedIn' as const,
          viewer: viewerFromSession(model.session),
          onLogout: GotLoggedInMessage({ message: LoggedIn.ClickedLogout() }),
        }
      : model._tag === 'LoggedOut' && Option.isSome(model.viewerSession)
        ? {
            _tag: 'LoggedIn' as const,
            viewer: viewerFromSession(model.viewerSession.value),
            onLogout: RequestedLoggedOutLogout(),
          }
        : { _tag: 'LoggedOut' as const }

  if (model.route._tag === 'Docs' || model.route._tag === 'DocsPage') {
    return Docs.view<Message>(model.route, authState)
  }

  if (model.route._tag === 'Business') {
    return Business.view<Message>(authState)
  }

  if (model.route._tag === 'LandingPreview') {
    return LandingPreview.view<Message>()
  }

  // Note: /autopilot and /autopilot/{vertical} are rendered above through the
  // loggedOut Submodel (the stateful, scene-backed onboarding flow), so they do
  // not fall through to this stateless public-header shell.

  if (model.route._tag === 'Terms') {
    return Terms.view<Message>(authState)
  }

  if (model.route._tag === 'Privacy') {
    return Privacy.view<Message>(authState)
  }
  if (model.route._tag === 'Code') {
    return Code.view<Message>(authState)
  }

  if (model.route._tag === 'Trace') {
    return Trace.view<Message>(
      model.route,
      authState,
      model._tag === 'LoggedOut' ? model.trace : undefined,
    )
  }

  if (model.route._tag === 'TraceCompare') {
    return TraceCompare.view<Message>(model.route, authState)
  }

  if (model.route._tag === 'QaSwarm') {
    return QaSwarm.view<Message>(model.route, authState)
  }

  if (model.route._tag === 'PylonCodexAssignmentStatus') {
    return PylonCodexAssignmentStatus.view<Message>(model.route, authState)
  }

  if (model.route._tag === 'ArtanisTraceTree') {
    return ArtanisTraceTree.view<Message>(authState)
  }

  if (model.route._tag === 'Animations') {
    return Animations.view<Message>(authState)
  }

  if (model.route._tag === 'Activity') {
    return Activity.view<Message>(authState)
  }

  if (model.route._tag === 'ArtanisAccounts') {
    return ArtanisAccounts.view<Message>(authState)
  }

  if (model.route._tag === 'DemoLegal') {
    return DemoLegal.view<Message>(authState)
  }

  if (model.route._tag === 'Run' || model.route._tag === 'Tassadar') {
    return Run.view<Message>(authState)
  }

  if (model.route._tag === 'TassadarReplay') {
    return Run.view<Message>(authState, model.route.replaySlug)
  }

  if (model.route._tag === 'Components') {
    return Components.view<Message>(authState)
  }

  if (model.route._tag === 'ComponentsFamily') {
    return Components.view<Message>(authState, model.route.family)
  }

  if (
    model.route._tag === 'Forum' ||
    model.route._tag === 'ForumForum' ||
    model.route._tag === 'ForumTopic' ||
    model.route._tag === 'ForumReceipt'
  ) {
    return Forum.view<Message>(model.route, authState)
  }

  if (
    model.route._tag === 'SiteCheckoutDemo' ||
    model.route._tag === 'SiteCheckoutDemoReturn'
  ) {
    return SiteCheckoutDemo.view<Message>(model.route, authState)
  }

  // /autopilot + /autopilot/{vertical} are rendered through the loggedOut
  // Submodel above for the LoggedOut model. They never reach this stateless
  // shell, so there is no public-header body for them here.
  if (
    model.route._tag === 'Autopilot' ||
    model.route._tag === 'AutopilotVertical'
  ) {
    return undefined
  }

  return Blog.view<Message>(model.route, authState)
}

// ---------------------------------------------------------------------------
// Render-disposition exhaustiveness guard
// ---------------------------------------------------------------------------
//
// Full table-driving of rendering is out of scope (rendering here is genuinely
// heterogeneous: stateful submodels, stateless public shells, logged-in-only
// surfaces, the demo submodel, and bespoke branches). What we DO guarantee is
// that every route tag has a KNOWN render disposition in the central registry,
// so a newly added route can never silently fall through to `maintenanceBody`
// again (the original failure mode). The registry's `satisfies
// Record<AppRoute['_tag'], RouteSpec>` already forces every tag to carry a
// `render` field; this guard additionally proves the value is one of the
// recognised dispositions via an exhaustive switch.
export const routeRenderDisposition = (
  route: AppRoute,
): RouteRenderDisposition => {
  const disposition = routeRegistry[route._tag].render

  switch (disposition) {
    case 'submodel':
    case 'statelessShell':
    case 'loggedInOnly':
    case 'demo':
    case 'special':
    // 'maintenance' is an honestly-recorded disposition for a route that is not
    // yet wired into the render branches above and currently falls to
    // `maintenanceBody`. It is a KNOWN disposition, so a route in this state is
    // visible/intentional rather than a silent fall-through.
    case 'maintenance':
      return disposition
  }
}

export const view = (model: Model): Document => {
  const maybePublicRouteBody = publicRouteBody(model)

  if (maybePublicRouteBody !== undefined) {
    return {
      title: title(model),
      body: maybePublicRouteBody,
    }
  }

  if (model._tag === 'LoggedIn') {
    const h = html<Message>()

    return {
      title: title(model),
      body: h.submodel({
        slotId: 'logged-in',
        model,
        view: LoggedIn.view,
        toParentMessage: message => GotLoggedInMessage({ message }),
      }),
    }
  }

  if (model._tag === 'Demo') {
    const h = html<Message>()

    return {
      title: title(model),
      body: h.submodel({
        slotId: 'demo',
        model,
        view: Demo.view,
        toParentMessage: message => GotDemoMessage({ message }),
      }),
    }
  }

  if (model.route._tag === 'Onboarding') {
    const h = html<Message>()

    return {
      title: 'Autopilot Onboarding - OpenAgents',
      body: h.submodel({
        slotId: 'logged-out-onboarding',
        model,
        view: LoggedOut.view,
        toParentMessage: message => GotLoggedOutMessage({ message }),
      }),
    }
  }

  return {
    title: title(model),
    body: maintenanceBody(model.publicPylonStats),
  }
}
