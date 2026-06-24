import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AdminRoute,
  type AppRoute,
  AutopilotRoute,
  AutopilotWorkDetailRoute,
  AutopilotWorkRoute,
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
  GymOssRoute,
  ImagesRoute,
  KhalaRoute,
  LandingRoute,
  LoginRoute,
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
  type RouteSpec,
  ShareRoute,
  SiteCheckoutDemoReturnRoute,
  SiteCheckoutDemoRoute,
  StatsRoute,
  TassadarReplayRoute,
  TassadarRoute,
  TermsRoute,
  WorkspaceRoute,
  routeRegistry,
  unregisteredParserRouters,
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

  test('accepts the public Autopilot onboarding route', () => {
    // `/autopilot` is the public onboarding entry (#6124/#6129); the operator
    // cockpit lives under `/autopilot/work`. The logged-in-with-workspace
    // resolution to the cockpit happens in the startup router, not the parser.
    expect(urlToAppRoute(appUrl('/autopilot'))).toEqual(AutopilotRoute())
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

  test('accepts the login page route', () => {
    expect(urlToAppRoute(appUrl('/login'))).toEqual(LoginRoute())
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

  test('does not claim the bare Gym route as a client document', () => {
    expect(urlToAppRoute(appUrl('/gym'))).toEqual(
      NotFoundRoute({ path: '/gym' }),
    )
  })

  test('accepts the owner-gated GPT-OSS Gym latency playground route', () => {
    expect(urlToAppRoute(appUrl('/gym/oss'))).toEqual(GymOssRoute())
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

// Behavior-preservation snapshot of the FULL canonical URL -> route-tag mapping.
// This is the registry-refactor's before/after evidence: every path resolves to
// exactly the same route tag as before the registry was introduced. A change in
// parser ordering (e.g. a more-generic router shadowing a more-specific one)
// would flip one of these and fail loudly.
const CANONICAL_URL_TO_TAG: ReadonlyArray<readonly [string, string]> = [
  ['/', 'Landing'],
  ['/landing', 'Landing'],
  ['/invite', 'Invite'],
  ['/onboarding', 'Onboarding'],
  ['/order', 'Order'],
  ['/orders/software_order_1', 'OrderDetail'],
  ['/autopilot', 'Autopilot'],
  ['/autopilot/legal', 'AutopilotVertical'],
  ['/autopilot/work', 'AutopilotWork'],
  ['/autopilot/work/wo_1', 'AutopilotWorkDetail'],
  ['/forge', 'Forge'],
  ['/decisions', 'Decisions'],
  ['/workspaces/ws_1', 'Workspace'],
  ['/workrooms/wr_1', 'Workroom'],
  ['/workrooms/wr_1/files', 'WorkroomTab'],
  ['/chat', 'NotFound'],
  ['/teams/t1/chat', 'TeamChat'],
  ['/teams/t1/projects/p1/chat', 'TeamProjectChat'],
  ['/teams/t1/files', 'TeamFiles'],
  ['/teams/t1/files/f1', 'TeamFile'],
  ['/files/f1', 'PersonalFile'],
  ['/t/th1', 'Thread'],
  ['/docs', 'Docs'],
  ['/docs/getting-started', 'DocsPage'],
  ['/promises', 'ProductPromises'],
  ['/training/runs', 'PublicTrainingRuns'],
  ['/training/runs/r1', 'PublicTrainingRun'],
  ['/forum', 'Forum'],
  ['/forum/f/void', 'ForumForum'],
  ['/forum/t/topic1', 'ForumTopic'],
  ['/forum/receipts/rcpt1', 'ForumReceipt'],
  ['/sites/demo-checkout', 'SiteCheckoutDemo'],
  ['/sites/demo-checkout/success', 'SiteCheckoutDemoReturn'],
  ['/clients-preview', 'ClientsPreview'],
  ['/components', 'Components'],
  ['/components/buttons', 'ComponentsFamily'],
  ['/business', 'Business'],
  ['/animations', 'Animations'],
  ['/activity', 'Activity'],
  ['/run', 'Run'],
  ['/gym/oss', 'GymOss'],
  ['/tassadar', 'Tassadar'],
  ['/tassadar/replay/s1', 'TassadarReplay'],
  ['/login', 'Login'],
  ['/blog', 'Blog'],
  ['/blog/post-1', 'BlogPost'],
  ['/agents/artanis', 'PublicAgent'],
  ['/share/s1', 'Share'],
  ['/moksha', 'Moksha'],
  ['/moksha2', 'Moksha2'],
  ['/terms', 'Terms'],
  ['/privacy', 'Privacy'],
  ['/khala', 'Khala'],
  ['/pylons', 'Pylon'],
  ['/download', 'Download'],
  ['/billing', 'Billing'],
  ['/usage', 'Usage'],
  ['/stats', 'Stats'],
  ['/stats-old', 'PublicStatsArchive'],
  ['/admin', 'Admin'],
  ['/mullet', 'Mullet'],
  ['/images', 'Images'],
  ['/settings', 'Settings'],
  ['/settings/profile', 'SettingsSection'],
  ['/demo', 'Demo'],
  ['/demo/legal', 'DemoLegal'],
  ['/demo/order', 'DemoOrder'],
  ['/demo/t/d1', 'DemoThread'],
  ['/demo/teams/t1/projects/p1/chat', 'DemoTeamProjectChat'],
  ['/demo/teams/t1/files', 'DemoTeamFiles'],
  ['/demo/teams/t1/files/f1', 'DemoTeamFile'],
  ['/demo2', 'Demo2'],
  ['/demo2/order', 'Demo2Order'],
  ['/demo2/t/d1', 'Demo2Thread'],
  ['/demo2/teams/t1/projects/p1/chat', 'Demo2TeamProjectChat'],
  ['/demo2/teams/t1/files', 'Demo2TeamFiles'],
  ['/demo2/teams/t1/files/f1', 'Demo2TeamFile'],
  ['/artanis', 'PublicAgent'],
  ['/adjutant', 'PublicAgent'],
  ['/pylon', 'NotFound'],
  ['/live', 'NotFound'],
  ['/totally-unknown-path', 'NotFound'],
]

describe('registry-driven route parser (behavior preservation)', () => {
  test.each(CANONICAL_URL_TO_TAG)(
    '%s resolves to the expected route tag',
    (path, expectedTag) => {
      expect(urlToAppRoute(appUrl(path))._tag).toBe(expectedTag)
    },
  )

  test('registry is keyed by exactly the AppRoute tag union', () => {
    // The compile-time `satisfies Record<AppRoute['_tag'], RouteSpec>` guard in
    // route.ts is the primary guarantee; this asserts it at runtime too.
    const tags = Object.keys(routeRegistry)
    expect(new Set(tags).size).toBe(tags.length)
    // Every canonical resolved tag (minus aliases) is present in the registry.
    for (const [, tag] of CANONICAL_URL_TO_TAG) {
      expect(routeRegistry).toHaveProperty(tag)
    }
  })

  test('every registry spec has well-formed fields', () => {
    const gates = new Set(['open', 'workroom', 'admin', 'mullet'])
    const renders = new Set([
      'submodel',
      'statelessShell',
      'loggedInOnly',
      'demo',
      'special',
      'maintenance',
    ])
    for (const spec of Object.values(routeRegistry) as ReadonlyArray<RouteSpec>) {
      expect(typeof spec.requiresAuthBootstrap).toBe('boolean')
      expect(gates.has(spec.loggedInGate)).toBe(true)
      expect(typeof spec.inLoggedOutUnion).toBe('boolean')
      expect(typeof spec.inLoggedInUnion).toBe('boolean')
      expect(renders.has(spec.render)).toBe(true)
    }
  })

  test('keeps deprecated/duplicate routers OUT of the parser', () => {
    // chatRouter, landingRouter, and gymRouter must stay unregistered: /chat
    // and /gym are NotFound, while / is covered by the Landing alias.
    expect(unregisteredParserRouters.length).toBe(3)
    expect(urlToAppRoute(appUrl('/chat'))).toEqual(
      NotFoundRoute({ path: '/chat' }),
    )
    expect(urlToAppRoute(appUrl('/gym'))).toEqual(
      NotFoundRoute({ path: '/gym' }),
    )
  })

  test('NotFound carries the original path', () => {
    const route: AppRoute = urlToAppRoute(appUrl('/totally-unknown-path'))
    expect(route).toEqual(NotFoundRoute({ path: '/totally-unknown-path' }))
  })
})
