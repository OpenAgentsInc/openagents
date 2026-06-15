import { Match as M } from 'effect'
import { Submodel } from 'foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { notFoundView } from '../../notFoundView'
import { homeRouter } from '../../route'
import * as Ui from '../../ui'
import * as Blog from '../blog'
import * as ClientsPreview from '../clientsPreview'
import * as Docs from '../docs'
import * as Forum from '../forum'
import * as SiteCheckoutDemo from '../siteCheckoutDemo'
import { Message } from './message'
import { Model } from './model'
import * as Home from './page/home'
import * as Live from './page/live'
import * as Moksha from './page/moksha'
import * as Moksha2 from './page/moksha2'
import * as Onboarding from './page/onboarding'
import * as Promises from './page/promises'
import * as PublicAgent from './page/publicAgent'
import * as Pylon from './page/pylon'
import * as Share from './page/share'
import * as Stats from './page/stats'
import * as TrainingRuns from './page/trainingRuns'

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

  if (model.route._tag === 'Pylon') {
    return Ui.pageShell<Message>([
      h.keyed('div')(model.route._tag, [], [Pylon.view()]),
    ])
  }

  if (model.route._tag === 'Live') {
    return Ui.pageShell<Message>([
      h.keyed('div')(model.route._tag, [], [Live.view()]),
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
                }),
              Stats: () =>
                Home.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
                  publicPylonStats: model.publicPylonStats,
                }),
              PublicStatsArchive: () =>
                Stats.view({
                  forumLaunchStatus: model.publicForumLaunchStatus,
                  forumTipLeaderboards: model.publicForumTipLeaderboards,
                  publicPylonStats: model.publicPylonStats,
                }),
              Invite: () => notFoundView('/invite', homeRouter(), 'Go Home'),
              Onboarding: () => Onboarding.view(model.onboarding),
              Docs: route => Docs.view(route, { _tag: 'LoggedOut' }),
              DocsPage: route => Docs.view(route, { _tag: 'LoggedOut' }),
              ProductPromises: () => Promises.view(model.publicProductPromises),
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
