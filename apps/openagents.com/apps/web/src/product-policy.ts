import {
  type AuthBootstrap,
  authHasCoreTeamAccess,
  onboardingIsComplete,
} from './domain/session'
import {
  type AppRoute,
  ChatRoute,
  InviteRoute,
  type LoggedInRoute,
  OrderRoute,
  chatRouter,
  orderRouter,
  routeRegistry,
} from './route'

export type BrowserFeatureName = 'projectWorkrooms'

export type BrowserFeatureFlags = Readonly<Record<BrowserFeatureName, boolean>>

export const browserFeatureFlags: BrowserFeatureFlags = {
  projectWorkrooms: false,
}

export const browserFeatureEnabled = (
  feature: BrowserFeatureName,
  flags: BrowserFeatureFlags = browserFeatureFlags,
): boolean => flags[feature]

export const projectWorkroomsEnabled = (
  flags: BrowserFeatureFlags = browserFeatureFlags,
): boolean => browserFeatureEnabled('projectWorkrooms', flags)

export const teamProjectWorkroomAllowed = (
  route: Readonly<{ projectRef: string }>,
  flags: BrowserFeatureFlags = browserFeatureFlags,
): boolean => projectWorkroomsEnabled(flags) || route.projectRef === 'adjutant'

export const projectMissionVisible = (
  input: Readonly<{ projectId?: string | undefined; title: string }>,
  flags: BrowserFeatureFlags = browserFeatureFlags,
): boolean =>
  projectWorkroomsEnabled(flags) ||
  (input.projectId === undefined &&
    !input.title.toLowerCase().includes('artanis project smoke'))

export type BrowserPermission = 'signedIn'

export type BrowserPermissionGate =
  | Readonly<{ _tag: 'BrowserPermissionAllowed' }>
  | Readonly<{
      _tag: 'BrowserPermissionDenied'
      href: string
      permission: BrowserPermission
      reason: 'missingSignedInSession'
      route: InviteRoute
    }>

export const loggedInPermissionGate = (
  _auth: AuthBootstrap,
): BrowserPermissionGate => ({ _tag: 'BrowserPermissionAllowed' })

export const loggedInOperatorAccessAllowed = (auth: AuthBootstrap): boolean =>
  authHasCoreTeamAccess(auth)

export const loggedInWorkroomAllowed = (auth: AuthBootstrap): boolean =>
  loggedInOperatorAccessAllowed(auth) && onboardingIsComplete(auth.onboarding)

export const loggedInAdminAccessAllowed = (auth: AuthBootstrap): boolean =>
  auth.isAdmin && onboardingIsComplete(auth.onboarding)

export const loggedInMulletAccessAllowed = (auth: AuthBootstrap): boolean =>
  loggedInAdminAccessAllowed(auth) &&
  auth.session.email.trim().toLowerCase() === 'chris@openagents.com'

export const defaultLoggedInRouteForAuth = (_auth: AuthBootstrap): OrderRoute =>
  OrderRoute()

export const defaultLoggedInHrefForAuth = (_auth: AuthBootstrap): string =>
  orderRouter()

// Derived from the central route registry's `loggedInGate` field (route.ts).
// Each gate maps to the same predicate the previous hand-maintained branch
// chain used, so the boolean for every logged-in route is unchanged.
export const routeAllowedForLoggedInAuth = (
  route: LoggedInRoute,
  auth: AuthBootstrap,
): boolean => {
  switch (routeRegistry[route._tag].loggedInGate) {
    case 'workroom':
      return loggedInWorkroomAllowed(auth)
    case 'admin':
      return loggedInAdminAccessAllowed(auth)
    case 'mullet':
      return loggedInMulletAccessAllowed(auth)
    case 'open':
      return true
  }
}

export type BrowserRouteGate =
  | Readonly<{ _tag: 'BrowserRouteAllowed'; route: AppRoute }>
  | Readonly<{
      _tag: 'BrowserRouteRedirected'
      href: string
      reason: 'disabledProductArea'
      route: LoggedInRoute
    }>

export const browserRouteGate = (
  route: AppRoute,
  flags: BrowserFeatureFlags = browserFeatureFlags,
): BrowserRouteGate =>
  route._tag === 'TeamProjectChat' && !teamProjectWorkroomAllowed(route, flags)
    ? {
        _tag: 'BrowserRouteRedirected',
        href: chatRouter(),
        reason: 'disabledProductArea',
        route: ChatRoute(),
      }
    : { _tag: 'BrowserRouteAllowed', route }

export const browserRouteIsEnabled = (
  route: AppRoute,
  flags: BrowserFeatureFlags = browserFeatureFlags,
): boolean => browserRouteGate(route, flags)._tag === 'BrowserRouteAllowed'

// Derived from the central route registry's `requiresAuthBootstrap` field
// (route.ts). The registry is the single source of truth for which routes need
// the auth bootstrap fetched before resolving; the boolean for every route is
// unchanged from the prior hand-maintained negation list.
export const routeRequiresAuthBootstrap = (route: AppRoute): boolean =>
  routeRegistry[route._tag].requiresAuthBootstrap

export const browserRouteProductIntents = {
  Admin: 'admin.overview',
  AutopilotWork: 'autopilot.work.index',
  AutopilotWorkDetail: 'autopilot.work.detail',
  Billing: 'billing.credits',
  Business: 'public.business.landing',
  BusinessKpi: 'public.business.kpi',
  Autopilot: 'public.autopilot.onboarding',
  AutopilotVertical: 'public.autopilot.onboarding.vertical',
  Activity: 'public.activity.timeline',
  ArtanisAccounts: 'operator.artanis.accounts',
  Login: 'public.login',
  Blog: 'public.blog.index',
  BlogPost: 'public.blog.post',
  Chat: 'workroom.chat.personal',
  Dashboard: 'disabled.dashboard',
  Decisions: 'autopilot.decisions.index',
  Demo: 'demo.training.fullscreen',
  DemoLegal: 'public.demo.legal.landing',
  DemoOrder: 'demo.customer.order',
  DemoTeamFile: 'demo.files.team.detail',
  DemoTeamFiles: 'demo.files.team.index',
  DemoTeamProjectChat: 'demo.workroom.project',
  DemoThread: 'demo.workroom.thread',
  Demo2: 'demo2.workroom.project',
  Demo2Order: 'demo2.customer.order',
  Demo2TeamFile: 'demo2.files.team.detail',
  Demo2TeamFiles: 'demo2.files.team.index',
  Demo2TeamProjectChat: 'demo2.workroom.project',
  Demo2Thread: 'demo2.workroom.thread',
  Images: 'image.generation',
  Mullet: 'mullet.runner',
  Terms: 'public.terms',
  Privacy: 'public.privacy',
  Code: 'public.code.landing',
  KhalaCodeDownload: 'public.khala-code.download',
  Khala: 'public.khala',
  KhalaChat: 'public.khala.chat',
  Pylon: 'public.pylon',
  PylonCodexAssignmentStatus: 'pylon.codex.assignment.status',
  Download: 'public.download',
  Docs: 'public.docs.index',
  DocsPage: 'public.docs.page',
  Forum: 'public.forum.index',
  ForumForum: 'public.forum.detail',
  ForumReceipt: 'public.forum.receipt',
  ForumTopic: 'public.forum.topic',
  Home: 'public.home',
  Invite: 'access.invite',
  NotFound: 'navigation.not-found',
  Onboarding: 'onboarding.flow',
  Order: 'customer.order.active',
  OrderDetail: 'customer.order.detail',
  PersonalFile: 'files.personal.detail',
  Pro: 'pro.operator.console',
  OperatorDashboard: 'operator.artanis.dashboard',
  ProductPromises: 'public.product-promises',
  PublicAgent: 'public.agent.profile',
  ArtanisTraceTree: 'public.artanis.rlm-traces',
  ArtanisGym: 'operator.artanis.gym',
  PublicTrainingRun: 'public.training.run',
  PublicTrainingRuns: 'public.training.runs',
  Gym: 'public.gym.terminal-bench',
  MirrorCode: 'public.mirrorcode',
  Run: 'public.tassadar.run',
  GymOss: 'gym.oss.playground',
  Tassadar: 'public.tassadar.run',
  TassadarReplay: 'public.tassadar.replay',
  Share: 'share.projection',
  Trace: 'trace.public.render',
  TraceCompare: 'trace.public.compare',
  QaSwarm: 'qa.swarm.run',
  SiteCheckoutDemo: 'public.sites.demo-checkout',
  SiteCheckoutDemoReturn: 'public.sites.demo-checkout-return',
  Settings: 'account.settings',
  SettingsSection: 'account.settings.section',
  Stats: 'public.stats',
  TeamChat: 'workroom.chat.team',
  TeamFile: 'files.team.detail',
  TeamFiles: 'files.team.index',
  TeamProjectChat: 'workroom.chat.project',
  Thread: 'workroom.thread',
  Usage: 'billing.usage',
  Workspace: 'workspace.prefilled.detail',
  Workroom: 'workroom.delivery.overview',
  WorkroomTab: 'workroom.delivery.tab',
} as const satisfies Record<AppRoute['_tag'], string>

export type BrowserRouteName = keyof typeof browserRouteProductIntents

export const browserRouteProductIntent = (route: AppRoute): string =>
  browserRouteProductIntents[route._tag]

export const browserCommandProductIntents = {
  ClearSession: 'auth.session.clear',
  CopyAgentInstructions: 'public.tassadar.agent-instructions.copy',
  CopyShareLink: 'share.link.copy',
  NavigateToKhala: 'public.khala.navigate',
  NavigateToLanding: 'public.landing.navigate',
  NavigateToTassadar: 'public.tassadar.navigate',
  OpenAutopilotCreditKickoff: 'public.autopilot.onboarding.credit-kickoff',
  ScrollAutopilotOnboardingThreadToEnd:
    'public.autopilot.onboarding.thread.scroll-end',
  PersistAutopilotOnboarding: 'public.autopilot.onboarding.persist',
  ClearAutopilotOnboardingStorage: 'public.autopilot.onboarding.storage.clear',
  RehydrateAutopilotOnboarding: 'public.autopilot.onboarding.rehydrate',
  ReconcileAutopilotOnboardingSession:
    'public.autopilot.onboarding.session.reconcile',
  ScrollKhalaChatThreadToEnd: 'public.khala.chat.thread.scroll-end',
  ScrollKhalaChatLatestTurnIntoView: 'public.khala.chat.thread.scroll-turn',
  FocusKhalaChatComposer: 'public.khala.chat.composer.focus',
  CreateBillingCheckout: 'billing.checkout.create',
  PrepareBillingCardSetup: 'billing.card.setup.prepare',
  RunBillingAutoTopUp: 'billing.auto_top_up.run',
  UpdateBillingAutoTopUpPolicy: 'billing.auto_top_up.policy.update',
  DeployAdminSiteVersion: 'admin.sites.version.deploy',
  DownloadThreadFile: 'files.thread.download',
  FetchAutopilotRun: 'autopilot.run.fetch',
  FocusChatComposer: 'workroom.composer.focus',
  GenerateAdminSite: 'admin.sites.generate',
  GenerateImage: 'image.generation.create',
  InstallAccountMenuOutsideClick: 'account.menu.install-outside-click',
  LaunchAutopilotRun: 'autopilot.run.launch',
  LoadAdminAdjutantAssignments: 'admin.adjutant.assignments.load',
  LoadAdminOverview: 'admin.overview.load',
  LoadAdminAdjutantReview: 'admin.adjutant.review.load',
  LoadAgentGoal: 'autopilot.goal.load',
  LoadAutopilotDecisions: 'autopilot.decisions.load',
  LoadAutopilotMorningReport: 'autopilot.morning-report.load',
  LoadAutopilotWorkBriefing: 'autopilot.work.briefing.load',
  LoadAutopilotWorkDetail: 'autopilot.work.detail.load',
  LoadAutopilotWorkEvents: 'autopilot.work.events.load',
  LoadAutopilotWorkList: 'autopilot.work.list.load',
  LoadArtanisOperatorConsole: 'operator.artanis.console.load',
  LoadArtanisOperatorDashboard: 'operator.artanis.dashboard.load',
  LoadArtanisOperatorGoal: 'operator.artanis.goal.load',
  LoadMulletBootstrap: 'mullet.bootstrap.load',
  LoadTokenUsageStats: 'admin.token-usage.stats.load',
  LoadCustomerOrder: 'customer.order.active.load',
  LoadCustomerOrders: 'customer.orders.load',
  LoadCustomerSiteBuilderEvents: 'customer.order.site-builder.events.load',
  LoadCustomerSiteBuilderFile: 'customer.order.site-builder.file.load',
  LoadCustomerSiteBuilderFiles: 'customer.order.site-builder.files.load',
  LoadCustomerSiteBuilderSession: 'customer.order.site-builder.session.load',
  LoadCustomerSiteFeedback: 'customer.order.site-feedback.load',
  LoadCustomerOneCohort: 'forge.customer-one.cohort.load',
  LoadCustomerFulfillmentArtifacts: 'customer.order.fulfillment-artifacts.load',
  LoadCustomerSiteRevisions: 'customer.order.site-revisions.load',
  LoadExternal: 'navigation.external.load',
  LoadOnboardingRepositories: 'onboarding.repositories.load',
  LoadProviderAccountPool: 'providers.account-pool.load',
  LoadProAgentDashboard: 'pro.agent-dashboard.load',
  ResetProviderAccountPoolAccount: 'providers.account-pool.reset-account',
  LoadPrefilledWorkspace: 'workspace.prefilled.load',
  LoadPublicActivityTimeline: 'public.activity-timeline.load',
  LoadPublicAdjutantActivity: 'public.adjutant.activity.load',
  LoadPublicAgentGoal: 'public.agent.goal.load',
  LoadPublicArtanisReport: 'public.artanis.report.load',
  LoadPublicForumLaunchStatus: 'public.forum.launch-status.load',
  LoadPublicForumTipLeaderboards: 'public.forum.tip-leaderboards.load',
  LoadPublicKhalaTokensServed: 'public.khala.tokens-served.load',
  LoadKhalaTokensServedSnapshot: 'public.khala.tokens-served.snapshot.load',
  LoadPublicKhalaTokensServedHistory: 'public.khala.tokens-served.history.load',
  LoadPublicKhalaTokensServedModelMix:
    'public.khala.tokens-served.model-mix.load',
  LoadPublicKhalaTokensServedChannelMix:
    'public.khala.tokens-served.channel-mix.load',
  LoadPublicGymRunProgress: 'public.gym.run-progress.load',
  LoadGymRunProgressSnapshot: 'public.gym.run-progress.snapshot.load',
  LoadMirrorCodeRuns: 'public.mirrorcode.runs.load',
  LoadPublicProductPromises: 'public.product-promises.load',
  LoadPublicPromiseTransitions: 'public.product-promises.load-transitions',
  LoadPublicPylonStats: 'public.pylon.stats.load',
  LoadPublicTrainingRuns: 'public.training.runs.load',
  LoadSettledFeedSnapshot: 'public.settled-feed.snapshot.load',
  LoadShareProjection: 'share.projection.load',
  LoadSyncSnapshot: 'sync.workspace.snapshot.load',
  LoadTeamChatMessages: 'workroom.chat.team.messages.load',
  LoadThreadFileDetail: 'files.thread.detail.load',
  LoadThreadFiles: 'files.thread.index.load',
  LoadTrace: 'trace.public.load',
  LoadWorkroomLifecycle: 'workroom.delivery.lifecycle.load',
  LoadWorkroomSurface: 'workroom.delivery.surface.load',
  LogError: 'telemetry.error.log',
  LogoutFromLanding: 'public.landing.logout',
  NavigateInternal: 'navigation.internal.navigate',
  PollProviderDeviceLogin: 'providers.chatgpt-device-login.poll',
  PostTeamChatMessage: 'workroom.chat.team.message.post',
  RaiseBrowserNotifications: 'notifications.browser.raise',
  RedeemBillingCoupon: 'billing.coupon.redeem',
  RedirectToChat: 'navigation.redirect.chat',
  RedirectToDefaultLoggedInRoute: 'navigation.redirect.default-logged-in',
  RedirectToHome: 'navigation.redirect.home',
  RedirectToInvite: 'navigation.redirect.invite',
  RedirectToOnboarding: 'navigation.redirect.onboarding',
  RedirectToOrder: 'navigation.redirect.order',
  RequestNotificationPermission: 'notifications.permission.request',
  ReviewAdminAdjutantResearchBrief: 'admin.adjutant.research-brief.review',
  ReviewAdminAdjutantSourceCard: 'admin.adjutant.source-card.review',
  RunAdminAdjutantEnrichment: 'admin.adjutant.enrichment.run',
  RunAdminSiteDeploymentAction: 'admin.sites.deployment.action',
  ScrollChatTimelineToEnd: 'workroom.timeline.scroll-end',
  SelectOnboardingRepository: 'onboarding.repository.select',
  SetAutopilotThreadUrl: 'workroom.thread.url.set',
  SaveAgentGoal: 'autopilot.goal.save',
  SaveArtanisOperatorGoal: 'operator.artanis.goal.save',
  SkipOnboardingBilling: 'onboarding.billing.skip',
  SkipOnboardingRepository: 'onboarding.repository.skip',
  StartProviderDeviceLogin: 'providers.chatgpt-device-login.start',
  OpenCustomerSiteBuilderSession: 'customer.order.site-builder.session.open',
  SubmitAutopilotDecisionAction: 'autopilot.decisions.act',
  SubmitAutopilotWorkComposer: 'autopilot.work.composer.submit',
  SubmitAutopilotWorkReview: 'autopilot.work.review.submit',
  SubmitOnboardingGoal: 'onboarding.goal.submit',
  SubmitCustomerOrder: 'customer.order.submit',
  SubmitCustomerSiteFeedback: 'customer.order.site-feedback.submit',
  SubmitWorkroomLifecycleDecision:
    'workroom.delivery.lifecycle.decision.submit',
  UpdateOnboardingRepository: 'onboarding.repository.update',
  UpdateThreadFileDownload: 'files.thread.download-policy.update',
  UpdateAgentGoalAction: 'autopilot.goal.action',
  UpdateArtanisOperatorApprovalAction: 'operator.artanis.approval.action',
  UpdateArtanisOperatorGoalAction: 'operator.artanis.goal.action',
  UploadThreadFile: 'files.thread.upload',
} as const

export type BrowserCommandName = keyof typeof browserCommandProductIntents

export const browserCommandName = <Name extends BrowserCommandName>(
  name: Name,
): Name => name
