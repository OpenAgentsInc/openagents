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

export const routeAllowedForLoggedInAuth = (
  route: LoggedInRoute,
  auth: AuthBootstrap,
): boolean => {
  if (
    route._tag === 'Chat' ||
    route._tag === 'TeamChat' ||
    route._tag === 'TeamProjectChat' ||
    route._tag === 'TeamFiles' ||
    route._tag === 'TeamFile' ||
    route._tag === 'PersonalFile' ||
    route._tag === 'Thread' ||
    route._tag === 'Billing' ||
    route._tag === 'Usage' ||
    route._tag === 'Images' ||
    route._tag === 'Settings' ||
    route._tag === 'SettingsSection'
  ) {
    return loggedInWorkroomAllowed(auth)
  }

  if (route._tag === 'Admin') {
    return loggedInAdminAccessAllowed(auth)
  }

  if (route._tag === 'Stats') {
    return loggedInAdminAccessAllowed(auth)
  }

  if (route._tag === 'Mullet') {
    return loggedInMulletAccessAllowed(auth)
  }

  return true
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

export const routeRequiresAuthBootstrap = (route: AppRoute): boolean =>
  route._tag !== 'Docs' &&
  route._tag !== 'DocsPage' &&
  route._tag !== 'Forum' &&
  route._tag !== 'ForumForum' &&
  route._tag !== 'ForumTopic' &&
  route._tag !== 'ForumReceipt' &&
  route._tag !== 'SiteCheckoutDemo' &&
  route._tag !== 'SiteCheckoutDemoReturn' &&
  route._tag !== 'ClientsPreview' &&
  route._tag !== 'Components' &&
  route._tag !== 'ComponentsFamily' &&
  route._tag !== 'Business' &&
  route._tag !== 'Animations' &&
  route._tag !== 'Activity' &&
  route._tag !== 'DemoLegal' &&
  route._tag !== 'Run' &&
  route._tag !== 'Tassadar' &&
  route._tag !== 'TassadarReplay' &&
  route._tag !== 'Login' &&
  route._tag !== 'Blog' &&
  route._tag !== 'BlogPost' &&
  route._tag !== 'PublicAgent' &&
  route._tag !== 'PublicTrainingRuns' &&
  route._tag !== 'PublicTrainingRun' &&
  route._tag !== 'PublicStatsArchive' &&
  route._tag !== 'Share' &&
  route._tag !== 'Moksha' &&
  route._tag !== 'Moksha2' &&
  route._tag !== 'Landing' &&
  route._tag !== 'Terms' &&
  route._tag !== 'Privacy' &&
  route._tag !== 'Khala' &&
  route._tag !== 'Pylon' &&
  route._tag !== 'Download' &&
  route._tag !== 'Demo' &&
  route._tag !== 'DemoOrder' &&
  route._tag !== 'DemoThread' &&
  route._tag !== 'DemoTeamProjectChat' &&
  route._tag !== 'DemoTeamFiles' &&
  route._tag !== 'DemoTeamFile' &&
  route._tag !== 'Demo2' &&
  route._tag !== 'Demo2Order' &&
  route._tag !== 'Demo2Thread' &&
  route._tag !== 'Demo2TeamProjectChat' &&
  route._tag !== 'Demo2TeamFiles' &&
  route._tag !== 'Demo2TeamFile' &&
  route._tag !== 'NotFound' &&
  route._tag !== 'Home'
    ? true
    : route._tag === 'Home'

export const browserRouteProductIntents = {
  Admin: 'admin.overview',
  AutopilotWork: 'autopilot.work.index',
  AutopilotWorkDetail: 'autopilot.work.detail',
  Billing: 'billing.credits',
  Business: 'public.business.landing',
  Autopilot: 'public.autopilot.onboarding',
  AutopilotVertical: 'public.autopilot.onboarding.vertical',
  Animations: 'internal.animations.playground',
  Activity: 'public.activity.timeline',
  Login: 'public.login',
  Blog: 'public.blog.index',
  BlogPost: 'public.blog.post',
  Chat: 'workroom.chat.personal',
  ClientsPreview: 'public.clients-preview',
  Components: 'internal.components.gallery',
  ComponentsFamily: 'internal.components.gallery',
  Dashboard: 'disabled.dashboard',
  Decisions: 'autopilot.decisions.index',
  Forge: 'forge.factory.index',
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
  Moksha: 'public.moksha',
  Moksha2: 'public.moksha2',
  Landing: 'public.landing',
  Terms: 'public.terms',
  Privacy: 'public.privacy',
  Khala: 'public.khala',
  Pylon: 'public.pylon',
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
  ProductPromises: 'public.product-promises',
  PublicAgent: 'public.agent.profile',
  PublicStatsArchive: 'public.stats.archive',
  PublicTrainingRun: 'public.training.run',
  PublicTrainingRuns: 'public.training.runs',
  Run: 'public.tassadar.run',
  GymOss: 'gym.oss.playground',
  Tassadar: 'public.tassadar.run',
  TassadarReplay: 'public.tassadar.replay',
  Share: 'share.projection',
  SiteCheckoutDemo: 'public.sites.demo-checkout',
  SiteCheckoutDemoReturn: 'public.sites.demo-checkout-return',
  Settings: 'account.settings',
  SettingsSection: 'account.settings.section',
  Stats: 'admin.token-usage.stats',
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
  SubmitAutopilotOnboardingTurn: 'public.autopilot.onboarding.turn.submit',
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
  LoadPrefilledWorkspace: 'workspace.prefilled.load',
  LoadPublicAdjutantActivity: 'public.adjutant.activity.load',
  LoadPublicAgentGoal: 'public.agent.goal.load',
  LoadPublicArtanisReport: 'public.artanis.report.load',
  LoadPublicForumLaunchStatus: 'public.forum.launch-status.load',
  LoadPublicForumTipLeaderboards: 'public.forum.tip-leaderboards.load',
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
  LoadWorkroomLifecycle: 'workroom.delivery.lifecycle.load',
  LoadWorkroomSurface: 'workroom.delivery.surface.load',
  LogError: 'telemetry.error.log',
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
