import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AdminRoute,
  AutopilotWorkDetailRoute,
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
  ForgeRoute,
  ForumForumRoute,
  ForumReceiptRoute,
  ForumRoute,
  ForumTopicRoute,
  ImagesRoute,
  KhalaRoute,
  LandingRoute,
  Moksha2Route,
  MulletRoute,
  NotFoundRoute,
  OrderDetailRoute,
  OrderRoute,
  PrivacyRoute,
  PublicAgentRoute,
  PublicStatsArchiveRoute,
  PublicTrainingRunRoute,
  PublicTrainingRunsRoute,
  PylonRoute,
  RunRoute,
  ShareRoute,
  SiteCheckoutDemoReturnRoute,
  SiteCheckoutDemoRoute,
  StatsRoute,
  TassadarReplayRoute,
  TassadarRoute,
  TermsRoute,
  WorkspaceRoute,
  urlToAppRoute,
} from './route'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

describe('app route parser', () => {
  test('does not accept the legacy personal chat alias', () => {
    expect(urlToAppRoute(appUrl('/chat'))).toEqual(
      NotFoundRoute({ path: '/chat' }),
    )
  })

  test('accepts the operator Autopilot shell route', () => {
    expect(urlToAppRoute(appUrl('/autopilot'))).toEqual(ChatRoute())
  })

  test('accepts Autopilot work visibility routes', () => {
    expect(urlToAppRoute(appUrl('/autopilot/work'))).toEqual(
      AutopilotWorkRoute(),
    )
    expect(
      urlToAppRoute(appUrl('/autopilot/work/autopilot_work_order.visible_1')),
    ).toEqual(
      AutopilotWorkDetailRoute({
        workOrderRef: 'autopilot_work_order.visible_1',
      }),
    )
  })

  test('accepts the Forge factory dashboard route', () => {
    expect(urlToAppRoute(appUrl('/forge'))).toEqual(ForgeRoute())
  })

  test('accepts prefilled workspace invite routes', () => {
    expect(urlToAppRoute(appUrl('/workspaces/workspace_seed'))).toEqual(
      WorkspaceRoute({ workspaceId: 'workspace_seed' }),
    )
  })

  test('does not accept the deleted login page route', () => {
    expect(urlToAppRoute(appUrl('/login'))).toEqual(
      NotFoundRoute({ path: '/login' }),
    )
  })

  test('accepts the short public Artanis campaign route', () => {
    expect(urlToAppRoute(appUrl('/artanis'))).toEqual(
      PublicAgentRoute({ agentRef: 'artanis' }),
    )
  })

  test('accepts the short public Adjutant supervisor route', () => {
    expect(urlToAppRoute(appUrl('/adjutant'))).toEqual(
      PublicAgentRoute({ agentRef: 'adjutant' }),
    )
  })

  test('accepts customer order routes', () => {
    expect(urlToAppRoute(appUrl('/order'))).toEqual(OrderRoute())
    expect(urlToAppRoute(appUrl('/orders/software_order_1'))).toEqual(
      OrderDetailRoute({ orderId: 'software_order_1' }),
    )
  })

  test('accepts share projection routes', () => {
    expect(
      urlToAppRoute(appUrl('/share/123e4567-e89b-42d3-a456-426614174000')),
    ).toEqual(ShareRoute({ shareId: '123e4567-e89b-42d3-a456-426614174000' }))
  })

  test('accepts public training run routes', () => {
    expect(urlToAppRoute(appUrl('/training/runs'))).toEqual(
      PublicTrainingRunsRoute(),
    )
    expect(urlToAppRoute(appUrl('/training/runs/run.cs336.a1.demo'))).toEqual(
      PublicTrainingRunRoute({ runId: 'run.cs336.a1.demo' }),
    )
  })

  test('accepts the OpenAgents Moksha narrative route', () => {
    expect(urlToAppRoute(appUrl('/moksha2'))).toEqual(Moksha2Route())
  })

  test('uses the Landing persistent scene as the root route', () => {
    expect(urlToAppRoute(appUrl('/'))).toEqual(LandingRoute())
  })

  test('keeps the legacy /landing path as an inbound alias to Landing', () => {
    expect(urlToAppRoute(appUrl('/landing'))).toEqual(LandingRoute())
  })

  test('accepts the public legal routes', () => {
    expect(urlToAppRoute(appUrl('/terms'))).toEqual(TermsRoute())
    expect(urlToAppRoute(appUrl('/privacy'))).toEqual(PrivacyRoute())
  })

  test('accepts the public Khala inference route', () => {
    expect(urlToAppRoute(appUrl('/khala'))).toEqual(KhalaRoute())
  })

  test('accepts the public live Tassadar run route', () => {
    expect(urlToAppRoute(appUrl('/run'))).toEqual(RunRoute())
    expect(urlToAppRoute(appUrl('/tassadar'))).toEqual(TassadarRoute())
    expect(
      urlToAppRoute(appUrl('/tassadar/replay/first-real-settlement')),
    ).toEqual(TassadarReplayRoute({ replaySlug: 'first-real-settlement' }))
  })

  test('serves the Pylon scene at /pylons', () => {
    expect(urlToAppRoute(appUrl('/pylons'))).toEqual(PylonRoute())
  })

  test('no longer resolves the old /pylon root alias', () => {
    expect(urlToAppRoute(appUrl('/pylon'))).toEqual(
      NotFoundRoute({ path: '/pylon' }),
    )
  })

  test('does not keep the retired live Pylon launch preview route', () => {
    expect(urlToAppRoute(appUrl('/live'))).toEqual(
      NotFoundRoute({ path: '/live' }),
    )
  })

  test('accepts public stats routes', () => {
    expect(urlToAppRoute(appUrl('/stats'))).toEqual(StatsRoute())
    expect(urlToAppRoute(appUrl('/stats-old'))).toEqual(
      PublicStatsArchiveRoute(),
    )
  })

  test('accepts the admin overview route', () => {
    expect(urlToAppRoute(appUrl('/admin'))).toEqual(AdminRoute())
  })

  test('accepts the image generation route', () => {
    expect(urlToAppRoute(appUrl('/images'))).toEqual(ImagesRoute())
  })

  test('accepts the private mullet operator route', () => {
    expect(urlToAppRoute(appUrl('/mullet'))).toEqual(MulletRoute())
  })

  test('accepts public Forum routes', () => {
    expect(urlToAppRoute(appUrl('/forum'))).toEqual(ForumRoute())
    expect(urlToAppRoute(appUrl('/forum/f/void'))).toEqual(
      ForumForumRoute({ forumRef: 'void' }),
    )
    expect(
      urlToAppRoute(appUrl('/forum/t/55555555-5555-4555-8555-555555555555')),
    ).toEqual(
      ForumTopicRoute({ topicId: '55555555-5555-4555-8555-555555555555' }),
    )
    expect(urlToAppRoute(appUrl('/forum/receipts/receipt.forum.1'))).toEqual(
      ForumReceiptRoute({ receiptRef: 'receipt.forum.1' }),
    )
  })

  test('accepts public Site checkout demo routes', () => {
    expect(urlToAppRoute(appUrl('/sites/demo-checkout'))).toEqual(
      SiteCheckoutDemoRoute(),
    )
    expect(urlToAppRoute(appUrl('/sites/demo-checkout/success'))).toEqual(
      SiteCheckoutDemoReturnRoute({ returnAction: 'success' }),
    )
    expect(urlToAppRoute(appUrl('/sites/demo-checkout/cancel'))).toEqual(
      SiteCheckoutDemoReturnRoute({ returnAction: 'cancel' }),
    )
    expect(urlToAppRoute(appUrl('/sites/demo-checkout/status'))).toEqual(
      SiteCheckoutDemoReturnRoute({ returnAction: 'status' }),
    )
  })

  test('accepts the demo route namespace', () => {
    expect(urlToAppRoute(appUrl('/demo'))).toEqual(DemoRoute())
    expect(urlToAppRoute(appUrl('/demo/order'))).toEqual(DemoOrderRoute())
    expect(urlToAppRoute(appUrl('/demo/t/pylon-release-demo'))).toEqual(
      DemoThreadRoute({ threadId: 'pylon-release-demo' }),
    )
    expect(
      urlToAppRoute(
        appUrl('/demo/teams/openagents-core-team/projects/artanis/chat'),
      ),
    ).toEqual(
      DemoTeamProjectChatRoute({
        teamRef: 'openagents-core-team',
        projectRef: 'artanis',
      }),
    )
    expect(
      urlToAppRoute(appUrl('/demo/teams/openagents-core-team/files')),
    ).toEqual(DemoTeamFilesRoute({ teamRef: 'openagents-core-team' }))
    expect(
      urlToAppRoute(
        appUrl(
          '/demo/teams/openagents-core-team/files/file_pylon_release_plan',
        ),
      ),
    ).toEqual(
      DemoTeamFileRoute({
        teamRef: 'openagents-core-team',
        fileId: 'file_pylon_release_plan',
      }),
    )
    expect(urlToAppRoute(appUrl('/demo2'))).toEqual(Demo2Route())
    expect(urlToAppRoute(appUrl('/demo2/order'))).toEqual(Demo2OrderRoute())
    expect(urlToAppRoute(appUrl('/demo2/t/pylon-release-demo'))).toEqual(
      Demo2ThreadRoute({ threadId: 'pylon-release-demo' }),
    )
    expect(
      urlToAppRoute(
        appUrl('/demo2/teams/openagents-core-team/projects/artanis/chat'),
      ),
    ).toEqual(
      Demo2TeamProjectChatRoute({
        teamRef: 'openagents-core-team',
        projectRef: 'artanis',
      }),
    )
    expect(
      urlToAppRoute(appUrl('/demo2/teams/openagents-core-team/files')),
    ).toEqual(Demo2TeamFilesRoute({ teamRef: 'openagents-core-team' }))
    expect(
      urlToAppRoute(
        appUrl(
          '/demo2/teams/openagents-core-team/files/file_pylon_release_plan',
        ),
      ),
    ).toEqual(
      Demo2TeamFileRoute({
        teamRef: 'openagents-core-team',
        fileId: 'file_pylon_release_plan',
      }),
    )
  })
})
