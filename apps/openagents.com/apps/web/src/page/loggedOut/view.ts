import { Match as M } from 'effect'
import { Submodel } from 'foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { notFoundView } from '../../notFoundView'
import { homeRouter } from '../../route'
import * as Ui from '../../ui'
import * as AutopilotOnboardingPage from '../autopilot-onboarding/page'
import * as KhalaChatPage from '../khala-chat/page'
import * as Activity from '../activity'
import * as Animations from '../animations'
import * as Blog from '../blog'
import * as Business from '../business'
import * as ClientsPreview from '../clientsPreview'
import * as Components from '../components'
import * as DemoLegal from '../demoLegal'
import * as Docs from '../docs'
import * as Download from '../download'
import * as Forum from '../forum'
import * as Login from '../login'
import * as Privacy from '../privacy'
import * as Run from '../run'
import * as SiteCheckoutDemo from '../siteCheckoutDemo'
import * as Terms from '../terms'
import * as Trace from '../trace'
import * as TraceCompare from '../trace-compare'
import {
  ClickedAutopilotOnboardingCreditKickoff,
  ClickedAutopilotOnboardingStartOver,
  ClosedKhalaChatInfo,
  Message,
  OpenedKhalaChatInfo,
  SubmittedAutopilotOnboardingTurn,
  SubmittedKhalaChatTurn,
  UpdatedAutopilotOnboardingComposer,
  UpdatedKhalaChatComposer,
} from './message'
import { Model } from './model'
import * as Home from './page/home'
import * as Gym from './page/gym'
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
      PersistentScene.view(model.route._tag, model.copiedAgentInstructions),
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

  // /khala: the generic Khala chat box mounts as the overlay of the SAME
  // persistent scene at the `khala` pose (no second scene). The page renders ONLY
  // the chat box + the "What is Khala?" info popup over the dimmed scene; the
  // long-form explainer is gone (condensed into the popup).
  if (model.route._tag === 'Khala') {
    return Ui.pageShell<Message>([
      PersistentScene.view(
        'Khala',
        model.copiedAgentInstructions,
        undefined,
        KhalaChatPage.overlayView<Message>(model.khalaChat, {
          updatedComposer: value => UpdatedKhalaChatComposer({ value }),
          submittedTurn: () => SubmittedKhalaChatTurn(),
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
        AutopilotOnboardingPage.overlayView<Message>(model.autopilotOnboarding, {
          updatedComposer: value =>
            UpdatedAutopilotOnboardingComposer({ value }),
          submittedTurn: () => SubmittedAutopilotOnboardingTurn(),
          clickedCreditKickoff: () =>
            ClickedAutopilotOnboardingCreditKickoff(),
          clickedStartOver: () => ClickedAutopilotOnboardingStartOver(),
        }),
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
                  publicPylonStats: model.publicPylonStats,
                  settledFeed: model.settledFeed,
                }),
              Stats: () =>
                Home.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
                  publicPylonStats: model.publicPylonStats,
                  settledFeed: model.settledFeed,
                }),
              PublicStatsArchive: () =>
                Stats.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
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
              Trace: route =>
                Trace.view(route, { _tag: 'LoggedOut' }, model.trace),
              TraceCompare: route =>
                TraceCompare.view(route, { _tag: 'LoggedOut' }),
              Download: () => Download.view({ _tag: 'LoggedOut' }),
              Animations: () => Animations.view({ _tag: 'LoggedOut' }),
              Activity: () => Activity.view({ _tag: 'LoggedOut' }),
              DemoLegal: () => DemoLegal.view({ _tag: 'LoggedOut' }),
              Gym: () => Gym.view(model.gym),
              Run: () => Run.view({ _tag: 'LoggedOut' }),
              TassadarReplay: route =>
                Run.view({ _tag: 'LoggedOut' }, route.replaySlug),
              // /login is handled by the persistent-scene early-return above (it
              // mounts the sign-in card over the shared canvas), so the route
              // union here no longer includes it.
              Blog: route => Blog.view(route, { _tag: 'LoggedOut' }),
              BlogPost: route => Blog.view(route, { _tag: 'LoggedOut' }),
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
