import { Match as M, Option } from 'effect'
import { Submodel } from 'foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { notFoundView } from '../../notFoundView'
import { homeRouter } from '../../route'
import * as Ui from '../../ui'
import * as Activity from '../activity'
import * as Animations from '../animations'
import * as AutopilotOnboarding from '../autopilot-onboarding'
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
import { Message } from './message'
import { Model } from './model'
import * as Home from './page/home'
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

  if (
    model.route._tag === 'Landing' ||
    model.route._tag === 'Khala' ||
    model.route._tag === 'Tassadar'
  ) {
    return Ui.pageShell<Message>([
      PersistentScene.view(model.route._tag, model.copiedAgentInstructions),
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
              Autopilot: () =>
                AutopilotOnboarding.view({ _tag: 'LoggedOut' }),
              AutopilotVertical: route =>
                AutopilotOnboarding.view(
                  { _tag: 'LoggedOut' },
                  Option.some(route.vertical),
                ),
              Terms: () => Terms.view({ _tag: 'LoggedOut' }),
              Privacy: () => Privacy.view({ _tag: 'LoggedOut' }),
              Download: () => Download.view({ _tag: 'LoggedOut' }),
              Animations: () => Animations.view({ _tag: 'LoggedOut' }),
              Activity: () => Activity.view({ _tag: 'LoggedOut' }),
              DemoLegal: () => DemoLegal.view({ _tag: 'LoggedOut' }),
              Run: () => Run.view({ _tag: 'LoggedOut' }),
              TassadarReplay: route =>
                Run.view({ _tag: 'LoggedOut' }, route.replaySlug),
              Login: () => Login.view({ _tag: 'LoggedOut' }),
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
