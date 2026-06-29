import { Match as M, Option } from 'effect'
import { Submodel } from 'foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import type { Session } from '../../domain/session'
import { notFoundView } from '../../notFoundView'
import { homeRouter } from '../../route'
import * as Ui from '../../ui'
import * as Activity from '../activity'
import * as Animations from '../animations'
import * as ArtanisTraceTree from '../artanisTraceTree'
import * as ArtanisAccounts from '../artanisAccounts'
import * as AutopilotOnboardingPage from '../autopilot-onboarding/page'
import * as Blog from '../blog'
import * as Business from '../business'
import * as ClientsPreview from '../clientsPreview'
import * as Components from '../components'
import * as DemoLegal from '../demoLegal'
import * as Docs from '../docs'
import * as Download from '../download'
import * as Forum from '../forum'
import * as KhalaChatPage from '../khala-chat/page'
import * as Code from '../code'
import * as Login from '../login'
import * as Privacy from '../privacy'
import type { PublicHeaderViewer } from '../publicHeader'
import { viewerAvatarMenu } from '../publicHeader'
import * as Run from '../run'
import * as SiteCheckoutDemo from '../siteCheckoutDemo'
import * as Terms from '../terms'
import * as Trace from '../trace'
import * as TraceCompare from '../trace-compare'
import {
  ClickedAutopilotOnboardingCreditKickoff,
  ClickedAutopilotOnboardingStartOver,
  ClickedKhalaChatJumpToLatest,
  ClosedKhalaChatInfo,
  Message,
  OpenedKhalaChatInfo,
  RequestedLandingLogout,
  SubmittedAutopilotOnboardingTurn,
  SubmittedKhalaChatTurn,
  UpdatedAutopilotOnboardingComposer,
  UpdatedKhalaChatComposer,
} from './message'
import { Model } from './model'
import { backButton, khalaTokensServedPill } from './page/backButton'
import * as Gym from './page/gym'
import * as Home from './page/home'
import * as MirrorCode from './page/mirrorcode'
import * as Moksha from './page/moksha'
import * as Moksha2 from './page/moksha2'
import * as Onboarding from './page/onboarding'
import * as PersistentScene from './page/persistentScene'
import * as Promises from './page/promises'
import * as PublicAgent from './page/publicAgent'
import * as Pylon from './page/pylon'
import * as Share from './page/share'
import * as Stats from './page/stats'
import * as TrainingRuns from './page/trainingRuns'
import * as WorkspaceInvite from './page/workspaceInvite'

// Map the signed-in session to the public-header viewer shape (GitHub avatar
// when present, monogram fallback handled by Ui.avatar). Mirrors the top-level
// shell's `viewerFromSession` so the homepage avatar matches the header.
const viewerFromSession = (session: Session): PublicHeaderViewer => ({
  displayName: session.name,
  email: session.email,
  ...(session.avatarUrl !== undefined && session.avatarUrl !== ''
    ? { avatarUrl: session.avatarUrl }
    : {}),
})

// The shared signed-in avatar menu for the chrome-less homepage hero, or
// `undefined` when the viewer is logged out (then nothing floats over the hero).
// Reuses the SAME `viewerAvatarMenu` and the SAME logout wire as the public
// header — here the logout dispatches the loggedOut submodel's
// `RequestedLandingLogout`, which clears the cached session and full-page
// navigates to `/auth/logout`, identical to the header's behavior.
const landingFloatingMenu = (model: Model): Html | undefined =>
  Option.match(model.viewerSession, {
    onNone: () => undefined,
    onSome: session =>
      viewerAvatarMenu<Message>({
        viewer: viewerFromSession(session),
        onLogout: RequestedLandingLogout(),
      }),
  })

export const view = Submodel.defineView<Model, Message>((model): Html => {
  const h = html<Message>()

  if (model.route._tag === 'Share') {
    return Ui.pageShell<Message>([
      h.keyed('div')(model.route._tag, [], [Share.view(model.shareProjection)]),
    ])
  }

  if (model.route._tag === 'Moksha') {
    return Ui.pageShell<Message>([
      h.keyed('div')(model.route._tag, [], [Moksha.view()]),
    ])
  }

  if (model.route._tag === 'Moksha2') {
    return Ui.pageShell<Message>([
      h.keyed('div')(model.route._tag, [], [Moksha2.view()]),
    ])
  }

  if (model.route._tag === 'Landing' || model.route._tag === 'Tassadar') {
    return Ui.pageShell<Message>([
      PersistentScene.view(
        model.route._tag,
        model.copiedAgentInstructions,
        undefined,
        undefined,
        undefined,
        // Only the Landing overlay consults this floating avatar menu; the
        // scene ignores it on the Tassadar pose.
        landingFloatingMenu(model),
        // The live "Khala Tokens Served" pill occupies the top-left slot only on
        // the Landing pose; /tassadar shows the back button in that slot instead
        // (its own overlay), so the pill is omitted on Tassadar — never both.
        model.route._tag === 'Landing'
          ? khalaTokensServedPill(model.publicKhalaTokensServed)
          : undefined,
      ),
    ])
  }

  // /login: the sign-in card + flush public header mount as the overlay of the
  // SAME persistent scene at the `login` pose (no second scene). Navigating
  // home <-> login is a continuous camera glide through ONE scene. The page
  // renders ONLY the header + card over the shared dimmed scene; the scene
  // canvas and readability scrim come from the persistent shell.
  if (model.route._tag === 'Login') {
    return Ui.pageShell<Message>([
      PersistentScene.view(
        'Login',
        model.copiedAgentInstructions,
        undefined,
        undefined,
        Login.overlayView<Message>({ _tag: 'LoggedOut' }),
      ),
    ])
  }

  // /khala: a concise API-instructions panel mounts as the overlay of the SAME
  // persistent scene at the `khala` pose (no second scene). The generic chat box
  // is intentionally NOT shown yet (not ready) — the page renders the AGENTS.md
  // "Run inference" basics (base URL, single model, free self-serve token, curl)
  // over the dimmed scene so a visitor can start calling Khala immediately.
  if (model.route._tag === 'Khala') {
    return Ui.pageShell<Message>([
      PersistentScene.view(
        'Khala',
        model.copiedAgentInstructions,
        undefined,
        // Back button + the API-instructions panel share the Khala overlay
        // layer; the back control mirrors /tassadar exactly (same component,
        // styling, position, and navigate-home wire).
        h.div(
          [],
          [
            // The instructions panel is a full-bleed `pointer-events-auto`
            // layer; render the back button AFTER it (later in DOM, same z-tier)
            // so the fixed top-left control paints above the panel and stays
            // clickable — matching /tassadar's always-on-top back affordance.
            KhalaChatPage.instructionsView<Message>(
              Home.khalaTokensServedCounter(model.publicKhalaTokensServed),
            ),
            backButton('khala'),
          ],
        ),
      ),
    ])
  }

  if (model.route._tag === 'KhalaChat') {
    return Ui.pageShell<Message>([
      PersistentScene.view(
        'KhalaChat',
        model.copiedAgentInstructions,
        undefined,
        KhalaChatPage.bottomOverlayView<Message>(model.khalaChat, {
          updatedComposer: value => UpdatedKhalaChatComposer({ value }),
          submittedTurn: () => SubmittedKhalaChatTurn(),
          jumpedToLatest: () => ClickedKhalaChatJumpToLatest(),
          openedInfo: () => OpenedKhalaChatInfo(),
          closedInfo: () => ClosedKhalaChatInfo(),
        }),
      ),
    ])
  }

  // /autopilot and /autopilot/{vertical}: the onboarding HUD mounts as the
  // overlay of the SAME persistent scene at the `autopilot` pose (no second
  // scene). The HUD (conversation + surfaced typed components + composer) is
  // built from the flow slice and threaded into the scene as its overlay.
  if (
    model.route._tag === 'Autopilot' ||
    model.route._tag === 'AutopilotVertical'
  ) {
    return Ui.pageShell<Message>([
      PersistentScene.view(
        'Autopilot',
        model.copiedAgentInstructions,
        AutopilotOnboardingPage.overlayView<Message>(
          model.autopilotOnboarding,
          {
            updatedComposer: value =>
              UpdatedAutopilotOnboardingComposer({ value }),
            submittedTurn: () => SubmittedAutopilotOnboardingTurn(),
            clickedCreditKickoff: () =>
              ClickedAutopilotOnboardingCreditKickoff(),
            clickedStartOver: () => ClickedAutopilotOnboardingStartOver(),
          },
        ),
      ),
    ])
  }

  if (model.route._tag === 'Pylon') {
    return Ui.pageShell<Message>([
      h.keyed('div')(model.route._tag, [], [Pylon.view()]),
    ])
  }

  if (model.route._tag === 'Workspace') {
    return Ui.pageShell<Message>([
      h.keyed('div')(
        `Workspace:${model.route.workspaceId}`,
        [],
        [WorkspaceInvite.view(model.route.workspaceId)],
      ),
    ])
  }

  return Ui.pageShell<Message>([
    Ui.routeMain<Message>([
      h.keyed('div')(
        model.route._tag,
        [],
        [
          M.value(model.route).pipe(
            M.tagsExhaustive({
              Home: () =>
                Home.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
                  publicKhalaTokensServed: model.publicKhalaTokensServed,
                  publicKhalaTokensServedHistory:
                    model.publicKhalaTokensServedHistory,
                  publicKhalaTokensServedModelMix:
                    model.publicKhalaTokensServedModelMix,
                  publicPylonStats: model.publicPylonStats,
                  settledFeed: model.settledFeed,
                }),
              Stats: () =>
                Stats.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
                  publicKhalaTokensServed: model.publicKhalaTokensServed,
                  publicKhalaTokensServedHistory:
                    model.publicKhalaTokensServedHistory,
                  publicKhalaTokensServedModelMix:
                    model.publicKhalaTokensServedModelMix,
                  publicPylonStats: model.publicPylonStats,
                  settledFeed: model.settledFeed,
                }),
              PublicStatsArchive: () =>
                Stats.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
                  publicKhalaTokensServed: model.publicKhalaTokensServed,
                  publicKhalaTokensServedHistory:
                    model.publicKhalaTokensServedHistory,
                  publicKhalaTokensServedModelMix:
                    model.publicKhalaTokensServedModelMix,
                  publicPylonStats: model.publicPylonStats,
                  settledFeed: model.settledFeed,
                }),
              Invite: () => notFoundView('/invite', homeRouter(), 'Go Home'),
              Onboarding: () => Onboarding.view(model.onboarding),
              Docs: route => Docs.view(route, { _tag: 'LoggedOut' }),
              DocsPage: route => Docs.view(route, { _tag: 'LoggedOut' }),
              ProductPromises: () =>
                Promises.view(
                  model.publicProductPromises,
                  model.publicPromiseTransitions,
                ),
              PublicTrainingRuns: () =>
                TrainingRuns.view(model.publicTrainingRuns, null),
              PublicTrainingRun: route =>
                TrainingRuns.view(model.publicTrainingRuns, route.runId),
              Forum: route => Forum.view(route, { _tag: 'LoggedOut' }),
              ForumForum: route => Forum.view(route, { _tag: 'LoggedOut' }),
              ForumTopic: route => Forum.view(route, { _tag: 'LoggedOut' }),
              ForumReceipt: route => Forum.view(route, { _tag: 'LoggedOut' }),
              SiteCheckoutDemo: route =>
                SiteCheckoutDemo.view(route, { _tag: 'LoggedOut' }),
              SiteCheckoutDemoReturn: route =>
                SiteCheckoutDemo.view(route, { _tag: 'LoggedOut' }),
              ClientsPreview: () => ClientsPreview.view(),
              Components: () => Components.view({ _tag: 'LoggedOut' }),
              ComponentsFamily: route =>
                Components.view({ _tag: 'LoggedOut' }, route.family),
              Business: () => Business.view({ _tag: 'LoggedOut' }),
              // /autopilot + /autopilot/{vertical} are handled by the persistent
              // scene early-return above (they mount the onboarding HUD over the
              // shared canvas), so the route union here no longer includes them.
              Terms: () => Terms.view({ _tag: 'LoggedOut' }),
              Privacy: () => Privacy.view({ _tag: 'LoggedOut' }),
              Code: () => Code.view({ _tag: 'LoggedOut' }),
              Trace: route =>
                Trace.view(route, { _tag: 'LoggedOut' }, model.trace),
              TraceCompare: route =>
                TraceCompare.view(route, { _tag: 'LoggedOut' }),
              PylonCodexAssignmentStatus: route =>
                notFoundView(
                  `/pylon/codex/assignments/${route.assignmentRef}`,
                  homeRouter(),
                  'Go Home',
                ),
              Download: () => Download.view({ _tag: 'LoggedOut' }),
              Animations: () => Animations.view({ _tag: 'LoggedOut' }),
              Activity: () => Activity.view({ _tag: 'LoggedOut' }),
              ArtanisAccounts: () =>
                ArtanisAccounts.view({ _tag: 'LoggedOut' }),
              DemoLegal: () => DemoLegal.view({ _tag: 'LoggedOut' }),
              Gym: () => Gym.view(model.gym, model.gymRunProgress),
              MirrorCode: () => MirrorCode.view(model.mirrorCodeRuns),
              Run: () => Run.view({ _tag: 'LoggedOut' }),
              TassadarReplay: route =>
                Run.view({ _tag: 'LoggedOut' }, route.replaySlug),
              // /login is handled by the persistent-scene early-return above (it
              // mounts the sign-in card over the shared canvas), so the route
              // union here no longer includes it.
              Blog: route => Blog.view(route, { _tag: 'LoggedOut' }),
              BlogPost: route => Blog.view(route, { _tag: 'LoggedOut' }),
              ArtanisTraceTree: () =>
                ArtanisTraceTree.view({ _tag: 'LoggedOut' }),
              PublicAgent: route => PublicAgent.view(model, route.agentRef),
              NotFound: ({ path }) =>
                notFoundView(path, homeRouter(), 'Go Home'),
            }),
          ),
        ],
      ),
    ]),
  ])
})
