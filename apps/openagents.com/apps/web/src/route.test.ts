import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AdminRoute,
  ChatRoute,
  DemoOrderRoute,
  DemoRoute,
  DemoTeamFileRoute,
  DemoTeamFilesRoute,
  DemoTeamProjectChatRoute,
  DemoThreadRoute,
  ForumForumRoute,
  ForumReceiptRoute,
  ForumRoute,
  ForumTopicRoute,
  ImagesRoute,
  MulletRoute,
  NotFoundRoute,
  OrderDetailRoute,
  OrderRoute,
  PublicAgentRoute,
  PublicTrainingRunRoute,
  PublicTrainingRunsRoute,
  ShareRoute,
  SiteCheckoutDemoReturnRoute,
  SiteCheckoutDemoRoute,
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
  })
})
