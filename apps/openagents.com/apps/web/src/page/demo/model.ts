import { Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import {
  type AppRoute,
  OnboardingRoute,
  TeamFileRoute,
  TeamFilesRoute,
  TeamProjectChatRoute,
  ThreadRoute,
} from '../../route'
import * as LoggedIn from '../loggedIn'
import {
  DEMO_FILE_PLAN_ID,
  DEMO_PROJECT_REF,
  DEMO_RUN_ID,
  DEMO_TEAM_REF,
  demoAuthBootstrap,
  demoCustomerAuthBootstrap,
} from './fixtures'

export const DemoPlaybackState = S.Literals(['playing', 'paused', 'complete'])
export type DemoPlaybackState = typeof DemoPlaybackState.Type

export const DemoMode = S.Literals(['workroom', 'order'])
export type DemoMode = typeof DemoMode.Type

export const DemoCueName = S.Literals([
  'LoadedProjectRoom',
  'FilledComposer',
  'SubmittedPrompt',
  'ReceivedRunEvents',
  'LoadedRunContext',
  'OpenedThread',
  'CompletedRun',
  'ReturnedToProjectRoom',
  'OpenedTeamFiles',
  'OpenedFileDetail',
  'LoadedOrderRepositories',
  'SelectedOrderRepository',
  'FilledOrderGoal',
  'SubmittedOrderGoal',
  'ConfirmedPublicWork',
  'LoadedSubmittedOrder',
  'AdvancedOrderScoping',
  'AdvancedOrderQueued',
  'AdvancedOrderRunning',
  'CompletedPlayback',
])
export type DemoCueName = typeof DemoCueName.Type

export const DemoCue = S.Struct({
  name: DemoCueName,
  atMs: S.Number,
})
export type DemoCue = typeof DemoCue.Type

export const Model = ts('Demo', {
  cueIndex: S.Number,
  elapsedMs: S.Number,
  loggedIn: LoggedIn.Model,
  mode: DemoMode,
  playback: DemoPlaybackState,
  routeKey: S.String,
})
export type Model = typeof Model.Type

export const demoProjectRoute = () =>
  TeamProjectChatRoute({ teamRef: DEMO_TEAM_REF, projectRef: DEMO_PROJECT_REF })

export const demoThreadRoute = () => ThreadRoute({ threadId: DEMO_RUN_ID })

export const demoFilesRoute = () => TeamFilesRoute({ teamRef: DEMO_TEAM_REF })

export const demoFileRoute = () =>
  TeamFileRoute({ teamRef: DEMO_TEAM_REF, fileId: DEMO_FILE_PLAN_ID })

export const loggedInRouteForDemoRoute = (
  route: AppRoute,
): LoggedIn.Model['route'] => {
  if (route._tag === 'DemoOrder') {
    return OnboardingRoute()
  }

  if (route._tag === 'DemoThread') {
    return ThreadRoute({ threadId: route.threadId })
  }

  if (route._tag === 'DemoTeamFiles') {
    return TeamFilesRoute({ teamRef: route.teamRef })
  }

  if (route._tag === 'DemoTeamFile') {
    return TeamFileRoute({ teamRef: route.teamRef, fileId: route.fileId })
  }

  if (route._tag === 'DemoTeamProjectChat') {
    return TeamProjectChatRoute({
      teamRef: route.teamRef,
      projectRef: route.projectRef,
    })
  }

  return demoProjectRoute()
}

const demoModeForRoute = (route: AppRoute): DemoMode =>
  route._tag === 'DemoOrder' ? 'order' : 'workroom'

export const init = (route: AppRoute): Model =>
  demoModeForRoute(route) === 'order'
    ? Model({
        cueIndex: -1,
        elapsedMs: 0,
        loggedIn: LoggedIn.init(
          loggedInRouteForDemoRoute(route),
          demoCustomerAuthBootstrap,
        ),
        mode: 'order',
        playback: 'playing',
        routeKey: 'demo:customer-order',
      })
    : Model({
        cueIndex: -1,
        elapsedMs: 0,
        loggedIn: LoggedIn.init(
          loggedInRouteForDemoRoute(route),
          demoAuthBootstrap,
        ),
        mode: 'workroom',
        playback: 'playing',
        routeKey: 'demo:pylon-release',
      })
