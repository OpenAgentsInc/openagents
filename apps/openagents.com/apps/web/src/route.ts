import { Schema as S, pipe } from 'effect'
import { Route } from 'foldkit'
import { literal, r, slash, string } from 'foldkit/route'
import type { Url } from 'foldkit/url'

export const HomeRoute = r('Home')
export const InviteRoute = r('Invite')
export const OnboardingRoute = r('Onboarding')
export const OrderRoute = r('Order')
export const OrderDetailRoute = r('OrderDetail', { orderId: S.String })
export const AutopilotWorkRoute = r('AutopilotWork')
export const AutopilotWorkDetailRoute = r('AutopilotWorkDetail', {
  workOrderRef: S.String,
})
export const ForgeRoute = r('Forge')
export const DecisionsRoute = r('Decisions')
export const WorkspaceRoute = r('Workspace', { workspaceId: S.String })
export const WorkroomRoute = r('Workroom', { workroomId: S.String })
export const WorkroomTabRoute = r('WorkroomTab', {
  tab: S.String,
  workroomId: S.String,
})
export const ChatRoute = r('Chat')
export const TeamChatRoute = r('TeamChat', { teamRef: S.String })
export const TeamProjectChatRoute = r('TeamProjectChat', {
  projectRef: S.String,
  teamRef: S.String,
})
export const TeamFilesRoute = r('TeamFiles', { teamRef: S.String })
export const TeamFileRoute = r('TeamFile', {
  teamRef: S.String,
  fileId: S.String,
})
export const PersonalFileRoute = r('PersonalFile', { fileId: S.String })
export const ThreadRoute = r('Thread', { threadId: S.String })
export const DocsRoute = r('Docs')
export const DocsPageRoute = r('DocsPage', { slug: S.String })
export const ProductPromisesRoute = r('ProductPromises')
export const PublicTrainingRunsRoute = r('PublicTrainingRuns')
export const PublicTrainingRunRoute = r('PublicTrainingRun', {
  runId: S.String,
})
export const ForumRoute = r('Forum')
export const ForumForumRoute = r('ForumForum', { forumRef: S.String })
export const ForumTopicRoute = r('ForumTopic', { topicId: S.String })
export const ForumReceiptRoute = r('ForumReceipt', { receiptRef: S.String })
export const SiteCheckoutDemoRoute = r('SiteCheckoutDemo')
export const SiteCheckoutDemoReturnRoute = r('SiteCheckoutDemoReturn', {
  returnAction: S.String,
})
export const ClientsPreviewRoute = r('ClientsPreview')
export const ComponentsRoute = r('Components')
export const ComponentsFamilyRoute = r('ComponentsFamily', {
  family: S.String,
})
export const BusinessRoute = r('Business')
export const AnimationsRoute = r('Animations')
export const ActivityRoute = r('Activity')
export const RunRoute = r('Run')
export const TassadarRoute = r('Tassadar')
export const TassadarReplayRoute = r('TassadarReplay', {
  replaySlug: S.String,
})
export const LoginRoute = r('Login')
export const BlogRoute = r('Blog')
export const BlogPostRoute = r('BlogPost', { slug: S.String })
export const PublicAgentRoute = r('PublicAgent', { agentRef: S.String })
export const ShareRoute = r('Share', { shareId: S.String })
export const MokshaRoute = r('Moksha')
export const Moksha2Route = r('Moksha2')
export const LandingRoute = r('Landing')
export const TermsRoute = r('Terms')
export const PrivacyRoute = r('Privacy')
export const PylonRoute = r('Pylon')
export const DownloadRoute = r('Download')
export const DashboardRoute = r('Dashboard')
export const BillingRoute = r('Billing')
export const UsageRoute = r('Usage')
export const StatsRoute = r('Stats')
export const PublicStatsArchiveRoute = r('PublicStatsArchive')
export const AdminRoute = r('Admin')
export const MulletRoute = r('Mullet')
export const ImagesRoute = r('Images')
export const SettingsRoute = r('Settings')
export const SettingsSectionRoute = r('SettingsSection', { section: S.String })
export const DemoRoute = r('Demo')
export const DemoLegalRoute = r('DemoLegal')
export const DemoOrderRoute = r('DemoOrder')
export const DemoThreadRoute = r('DemoThread', { threadId: S.String })
export const DemoTeamProjectChatRoute = r('DemoTeamProjectChat', {
  projectRef: S.String,
  teamRef: S.String,
})
export const DemoTeamFilesRoute = r('DemoTeamFiles', { teamRef: S.String })
export const DemoTeamFileRoute = r('DemoTeamFile', {
  teamRef: S.String,
  fileId: S.String,
})
export const Demo2Route = r('Demo2')
export const Demo2OrderRoute = r('Demo2Order')
export const Demo2ThreadRoute = r('Demo2Thread', { threadId: S.String })
export const Demo2TeamProjectChatRoute = r('Demo2TeamProjectChat', {
  projectRef: S.String,
  teamRef: S.String,
})
export const Demo2TeamFilesRoute = r('Demo2TeamFiles', { teamRef: S.String })
export const Demo2TeamFileRoute = r('Demo2TeamFile', {
  teamRef: S.String,
  fileId: S.String,
})
export const NotFoundRoute = r('NotFound', { path: S.String })

export type HomeRoute = typeof HomeRoute.Type
export type InviteRoute = typeof InviteRoute.Type
export type OnboardingRoute = typeof OnboardingRoute.Type
export type OrderRoute = typeof OrderRoute.Type
export type OrderDetailRoute = typeof OrderDetailRoute.Type
export type AutopilotWorkRoute = typeof AutopilotWorkRoute.Type
export type AutopilotWorkDetailRoute = typeof AutopilotWorkDetailRoute.Type
export type ForgeRoute = typeof ForgeRoute.Type
export type DecisionsRoute = typeof DecisionsRoute.Type
export type WorkspaceRoute = typeof WorkspaceRoute.Type
export type WorkroomRoute = typeof WorkroomRoute.Type
export type WorkroomTabRoute = typeof WorkroomTabRoute.Type
export type ChatRoute = typeof ChatRoute.Type
export type TeamChatRoute = typeof TeamChatRoute.Type
export type TeamProjectChatRoute = typeof TeamProjectChatRoute.Type
export type TeamFilesRoute = typeof TeamFilesRoute.Type
export type TeamFileRoute = typeof TeamFileRoute.Type
export type PersonalFileRoute = typeof PersonalFileRoute.Type
export type ThreadRoute = typeof ThreadRoute.Type
export type DocsRoute = typeof DocsRoute.Type
export type DocsPageRoute = typeof DocsPageRoute.Type
export type ProductPromisesRoute = typeof ProductPromisesRoute.Type
export type PublicTrainingRunsRoute = typeof PublicTrainingRunsRoute.Type
export type PublicTrainingRunRoute = typeof PublicTrainingRunRoute.Type
export type ForumRoute = typeof ForumRoute.Type
export type ForumForumRoute = typeof ForumForumRoute.Type
export type ForumTopicRoute = typeof ForumTopicRoute.Type
export type ForumReceiptRoute = typeof ForumReceiptRoute.Type
export type SiteCheckoutDemoRoute = typeof SiteCheckoutDemoRoute.Type
export type SiteCheckoutDemoReturnRoute =
  typeof SiteCheckoutDemoReturnRoute.Type
export type ClientsPreviewRoute = typeof ClientsPreviewRoute.Type
export type ComponentsRoute = typeof ComponentsRoute.Type
export type ComponentsFamilyRoute = typeof ComponentsFamilyRoute.Type
export type BusinessRoute = typeof BusinessRoute.Type
export type AnimationsRoute = typeof AnimationsRoute.Type
export type ActivityRoute = typeof ActivityRoute.Type
export type RunRoute = typeof RunRoute.Type
export type TassadarRoute = typeof TassadarRoute.Type
export type TassadarReplayRoute = typeof TassadarReplayRoute.Type
export type LoginRoute = typeof LoginRoute.Type
export type BlogRoute = typeof BlogRoute.Type
export type BlogPostRoute = typeof BlogPostRoute.Type
export type PublicAgentRoute = typeof PublicAgentRoute.Type
export type ShareRoute = typeof ShareRoute.Type
export type MokshaRoute = typeof MokshaRoute.Type
export type Moksha2Route = typeof Moksha2Route.Type
export type LandingRoute = typeof LandingRoute.Type
export type TermsRoute = typeof TermsRoute.Type
export type PrivacyRoute = typeof PrivacyRoute.Type
export type PylonRoute = typeof PylonRoute.Type
export type DownloadRoute = typeof DownloadRoute.Type
export type DashboardRoute = typeof DashboardRoute.Type
export type BillingRoute = typeof BillingRoute.Type
export type UsageRoute = typeof UsageRoute.Type
export type StatsRoute = typeof StatsRoute.Type
export type PublicStatsArchiveRoute = typeof PublicStatsArchiveRoute.Type
export type AdminRoute = typeof AdminRoute.Type
export type MulletRoute = typeof MulletRoute.Type
export type ImagesRoute = typeof ImagesRoute.Type
export type SettingsRoute = typeof SettingsRoute.Type
export type SettingsSectionRoute = typeof SettingsSectionRoute.Type
export type DemoRoute = typeof DemoRoute.Type
export type DemoLegalRoute = typeof DemoLegalRoute.Type
export type DemoOrderRoute = typeof DemoOrderRoute.Type
export type DemoThreadRoute = typeof DemoThreadRoute.Type
export type DemoTeamProjectChatRoute = typeof DemoTeamProjectChatRoute.Type
export type DemoTeamFilesRoute = typeof DemoTeamFilesRoute.Type
export type DemoTeamFileRoute = typeof DemoTeamFileRoute.Type
export type Demo2Route = typeof Demo2Route.Type
export type Demo2OrderRoute = typeof Demo2OrderRoute.Type
export type Demo2ThreadRoute = typeof Demo2ThreadRoute.Type
export type Demo2TeamProjectChatRoute = typeof Demo2TeamProjectChatRoute.Type
export type Demo2TeamFilesRoute = typeof Demo2TeamFilesRoute.Type
export type Demo2TeamFileRoute = typeof Demo2TeamFileRoute.Type
export type NotFoundRoute = typeof NotFoundRoute.Type

export const LoggedOutRoute = S.Union([
  HomeRoute,
  StatsRoute,
  PublicStatsArchiveRoute,
  InviteRoute,
  OnboardingRoute,
  DocsRoute,
  DocsPageRoute,
  ProductPromisesRoute,
  PublicTrainingRunsRoute,
  PublicTrainingRunRoute,
  ForumRoute,
  ForumForumRoute,
  ForumTopicRoute,
  ForumReceiptRoute,
  SiteCheckoutDemoRoute,
  SiteCheckoutDemoReturnRoute,
  ClientsPreviewRoute,
  ComponentsRoute,
  ComponentsFamilyRoute,
  BusinessRoute,
  AnimationsRoute,
  ActivityRoute,
  RunRoute,
  TassadarRoute,
  TassadarReplayRoute,
  LoginRoute,
  BlogRoute,
  BlogPostRoute,
  PublicAgentRoute,
  ShareRoute,
  MokshaRoute,
  Moksha2Route,
  LandingRoute,
  TermsRoute,
  PrivacyRoute,
  PylonRoute,
  DownloadRoute,
  WorkspaceRoute,
  DemoLegalRoute,
  NotFoundRoute,
])
export const LoggedInRoute = S.Union([
  InviteRoute,
  OnboardingRoute,
  OrderRoute,
  OrderDetailRoute,
  AutopilotWorkRoute,
  AutopilotWorkDetailRoute,
  ForgeRoute,
  DecisionsRoute,
  WorkspaceRoute,
  WorkroomRoute,
  WorkroomTabRoute,
  ChatRoute,
  TeamChatRoute,
  TeamProjectChatRoute,
  TeamFilesRoute,
  TeamFileRoute,
  PersonalFileRoute,
  ThreadRoute,
  DocsRoute,
  DocsPageRoute,
  ForumRoute,
  PublicTrainingRunsRoute,
  PublicTrainingRunRoute,
  ForumForumRoute,
  ForumTopicRoute,
  ForumReceiptRoute,
  SiteCheckoutDemoRoute,
  SiteCheckoutDemoReturnRoute,
  ClientsPreviewRoute,
  ComponentsRoute,
  ComponentsFamilyRoute,
  BusinessRoute,
  AnimationsRoute,
  ActivityRoute,
  RunRoute,
  TassadarRoute,
  TassadarReplayRoute,
  LoginRoute,
  BlogRoute,
  BlogPostRoute,
  PublicAgentRoute,
  DashboardRoute,
  BillingRoute,
  UsageRoute,
  StatsRoute,
  AdminRoute,
  MulletRoute,
  ImagesRoute,
  SettingsRoute,
  SettingsSectionRoute,
  DemoLegalRoute,
  NotFoundRoute,
])
export const AppRoute = S.Union([
  HomeRoute,
  InviteRoute,
  OnboardingRoute,
  OrderRoute,
  OrderDetailRoute,
  AutopilotWorkRoute,
  AutopilotWorkDetailRoute,
  ForgeRoute,
  DecisionsRoute,
  WorkspaceRoute,
  WorkroomRoute,
  WorkroomTabRoute,
  ChatRoute,
  TeamChatRoute,
  TeamProjectChatRoute,
  TeamFilesRoute,
  TeamFileRoute,
  PersonalFileRoute,
  ThreadRoute,
  DocsRoute,
  DocsPageRoute,
  ProductPromisesRoute,
  PublicTrainingRunsRoute,
  PublicTrainingRunRoute,
  ForumRoute,
  ForumForumRoute,
  ForumTopicRoute,
  ForumReceiptRoute,
  SiteCheckoutDemoRoute,
  SiteCheckoutDemoReturnRoute,
  ClientsPreviewRoute,
  ComponentsRoute,
  ComponentsFamilyRoute,
  BusinessRoute,
  AnimationsRoute,
  ActivityRoute,
  RunRoute,
  TassadarRoute,
  TassadarReplayRoute,
  LoginRoute,
  BlogRoute,
  BlogPostRoute,
  PublicAgentRoute,
  ShareRoute,
  MokshaRoute,
  Moksha2Route,
  LandingRoute,
  TermsRoute,
  PrivacyRoute,
  PylonRoute,
  DownloadRoute,
  DashboardRoute,
  BillingRoute,
  UsageRoute,
  StatsRoute,
  PublicStatsArchiveRoute,
  AdminRoute,
  MulletRoute,
  ImagesRoute,
  SettingsRoute,
  SettingsSectionRoute,
  DemoRoute,
  DemoLegalRoute,
  DemoOrderRoute,
  DemoThreadRoute,
  DemoTeamProjectChatRoute,
  DemoTeamFilesRoute,
  DemoTeamFileRoute,
  Demo2Route,
  Demo2OrderRoute,
  Demo2ThreadRoute,
  Demo2TeamProjectChatRoute,
  Demo2TeamFilesRoute,
  Demo2TeamFileRoute,
  NotFoundRoute,
])

export type LoggedOutRoute = typeof LoggedOutRoute.Type
export type LoggedInRoute = typeof LoggedInRoute.Type
export type AppRoute = typeof AppRoute.Type

export const homeRouter = pipe(Route.root, Route.mapTo(PylonRoute))
export const chatRouter = pipe(literal('autopilot'), Route.mapTo(ChatRoute))
export const inviteRouter = pipe(literal('invite'), Route.mapTo(InviteRoute))
export const onboardingRouter = pipe(
  literal('onboarding'),
  Route.mapTo(OnboardingRoute),
)
export const orderRouter = pipe(literal('order'), Route.mapTo(OrderRoute))
export const orderDetailRouter = pipe(
  literal('orders'),
  slash(string('orderId')),
  Route.mapTo(OrderDetailRoute),
)
export const autopilotWorkRouter = pipe(
  literal('autopilot'),
  slash(literal('work')),
  Route.mapTo(AutopilotWorkRoute),
)
export const autopilotWorkDetailRouter = pipe(
  literal('autopilot'),
  slash(literal('work')),
  slash(string('workOrderRef')),
  Route.mapTo(AutopilotWorkDetailRoute),
)
export const forgeRouter = pipe(literal('forge'), Route.mapTo(ForgeRoute))
export const decisionsRouter = pipe(
  literal('decisions'),
  Route.mapTo(DecisionsRoute),
)
export const workspaceRouter = pipe(
  literal('workspaces'),
  slash(string('workspaceId')),
  Route.mapTo(WorkspaceRoute),
)
export const workroomRouter = pipe(
  literal('workrooms'),
  slash(string('workroomId')),
  Route.mapTo(WorkroomRoute),
)
export const workroomTabRouter = pipe(
  literal('workrooms'),
  slash(string('workroomId')),
  slash(string('tab')),
  Route.mapTo(WorkroomTabRoute),
)
export const teamChatRouter = pipe(
  literal('teams'),
  slash(string('teamRef')),
  slash(literal('chat')),
  Route.mapTo(TeamChatRoute),
)
export const teamProjectChatRouter = pipe(
  literal('teams'),
  slash(string('teamRef')),
  slash(literal('projects')),
  slash(string('projectRef')),
  slash(literal('chat')),
  Route.mapTo(TeamProjectChatRoute),
)
export const teamFilesRouter = pipe(
  literal('teams'),
  slash(string('teamRef')),
  slash(literal('files')),
  Route.mapTo(TeamFilesRoute),
)
export const teamFileRouter = pipe(
  literal('teams'),
  slash(string('teamRef')),
  slash(literal('files')),
  slash(string('fileId')),
  Route.mapTo(TeamFileRoute),
)
export const personalFileRouter = pipe(
  literal('files'),
  slash(string('fileId')),
  Route.mapTo(PersonalFileRoute),
)
export const threadRouter = pipe(
  literal('t'),
  slash(string('threadId')),
  Route.mapTo(ThreadRoute),
)
export const docsRouter = pipe(literal('docs'), Route.mapTo(DocsRoute))
export const docsPageRouter = pipe(
  literal('docs'),
  slash(string('slug')),
  Route.mapTo(DocsPageRoute),
)
export const productPromisesRouter = pipe(
  literal('promises'),
  Route.mapTo(ProductPromisesRoute),
)
export const publicTrainingRunsRouter = pipe(
  literal('training'),
  slash(literal('runs')),
  Route.mapTo(PublicTrainingRunsRoute),
)
export const publicTrainingRunRouter = pipe(
  literal('training'),
  slash(literal('runs')),
  slash(string('runId')),
  Route.mapTo(PublicTrainingRunRoute),
)
export const forumRouter = pipe(literal('forum'), Route.mapTo(ForumRoute))
export const forumForumRouter = pipe(
  literal('forum'),
  slash(literal('f')),
  slash(string('forumRef')),
  Route.mapTo(ForumForumRoute),
)
export const forumTopicRouter = pipe(
  literal('forum'),
  slash(literal('t')),
  slash(string('topicId')),
  Route.mapTo(ForumTopicRoute),
)
export const forumReceiptRouter = pipe(
  literal('forum'),
  slash(literal('receipts')),
  slash(string('receiptRef')),
  Route.mapTo(ForumReceiptRoute),
)
export const siteCheckoutDemoRouter = pipe(
  literal('sites'),
  slash(literal('demo-checkout')),
  Route.mapTo(SiteCheckoutDemoRoute),
)
export const siteCheckoutDemoReturnRouter = pipe(
  literal('sites'),
  slash(literal('demo-checkout')),
  slash(string('returnAction')),
  Route.mapTo(SiteCheckoutDemoReturnRoute),
)
export const clientsPreviewRouter = pipe(
  literal('clients-preview'),
  Route.mapTo(ClientsPreviewRoute),
)
export const componentsRouter = pipe(
  literal('components'),
  Route.mapTo(ComponentsRoute),
)
export const componentsFamilyRouter = pipe(
  literal('components'),
  slash(string('family')),
  Route.mapTo(ComponentsFamilyRoute),
)
export const businessRouter = pipe(
  literal('business'),
  Route.mapTo(BusinessRoute),
)
export const animationsRouter = pipe(
  literal('animations'),
  Route.mapTo(AnimationsRoute),
)
export const activityRouter = pipe(
  literal('activity'),
  Route.mapTo(ActivityRoute),
)
export const runRouter = pipe(literal('run'), Route.mapTo(RunRoute))
export const tassadarRouter = pipe(
  literal('tassadar'),
  Route.mapTo(TassadarRoute),
)
export const tassadarReplayRouter = pipe(
  literal('tassadar'),
  slash(literal('replay')),
  slash(string('replaySlug')),
  Route.mapTo(TassadarReplayRoute),
)
export const loginRouter = pipe(literal('login'), Route.mapTo(LoginRoute))
export const blogRouter = pipe(literal('blog'), Route.mapTo(BlogRoute))
export const blogPostRouter = pipe(
  literal('blog'),
  slash(string('slug')),
  Route.mapTo(BlogPostRoute),
)
export const publicAgentRouter = pipe(
  literal('agents'),
  slash(string('agentRef')),
  Route.mapTo(PublicAgentRoute),
)
export const shareRouter = pipe(
  literal('share'),
  slash(string('shareId')),
  Route.mapTo(ShareRoute),
)
export const mokshaRouter = pipe(literal('moksha'), Route.mapTo(MokshaRoute))
export const moksha2Router = pipe(literal('moksha2'), Route.mapTo(Moksha2Route))
export const landingRouter = pipe(literal('landing'), Route.mapTo(LandingRoute))
export const termsRouter = pipe(literal('terms'), Route.mapTo(TermsRoute))
export const privacyRouter = pipe(literal('privacy'), Route.mapTo(PrivacyRoute))
export const pylonRouter = pipe(literal('pylon'), Route.mapTo(PylonRoute))
export const downloadRouter = pipe(
  literal('download'),
  Route.mapTo(DownloadRoute),
)
export const billingRouter = pipe(literal('billing'), Route.mapTo(BillingRoute))
export const usageRouter = pipe(literal('usage'), Route.mapTo(UsageRoute))
export const statsRouter = pipe(literal('stats'), Route.mapTo(StatsRoute))
export const publicStatsArchiveRouter = pipe(
  literal('stats-old'),
  Route.mapTo(PublicStatsArchiveRoute),
)
export const adminRouter = pipe(literal('admin'), Route.mapTo(AdminRoute))
export const mulletRouter = pipe(literal('mullet'), Route.mapTo(MulletRoute))
export const imagesRouter = pipe(literal('images'), Route.mapTo(ImagesRoute))
export const settingsRouter = pipe(
  literal('settings'),
  Route.mapTo(SettingsRoute),
)
export const settingsSectionRouter = pipe(
  literal('settings'),
  slash(string('section')),
  Route.mapTo(SettingsSectionRoute),
)
export const demoRouter = pipe(literal('demo'), Route.mapTo(DemoRoute))
export const demoLegalRouter = pipe(
  literal('demo'),
  slash(literal('legal')),
  Route.mapTo(DemoLegalRoute),
)
export const demoOrderRouter = pipe(
  literal('demo'),
  slash(literal('order')),
  Route.mapTo(DemoOrderRoute),
)
export const demoThreadRouter = pipe(
  literal('demo'),
  slash(literal('t')),
  slash(string('threadId')),
  Route.mapTo(DemoThreadRoute),
)
export const demoTeamProjectChatRouter = pipe(
  literal('demo'),
  slash(literal('teams')),
  slash(string('teamRef')),
  slash(literal('projects')),
  slash(string('projectRef')),
  slash(literal('chat')),
  Route.mapTo(DemoTeamProjectChatRoute),
)
export const demoTeamFilesRouter = pipe(
  literal('demo'),
  slash(literal('teams')),
  slash(string('teamRef')),
  slash(literal('files')),
  Route.mapTo(DemoTeamFilesRoute),
)
export const demoTeamFileRouter = pipe(
  literal('demo'),
  slash(literal('teams')),
  slash(string('teamRef')),
  slash(literal('files')),
  slash(string('fileId')),
  Route.mapTo(DemoTeamFileRoute),
)
export const demo2Router = pipe(literal('demo2'), Route.mapTo(Demo2Route))
export const demo2OrderRouter = pipe(
  literal('demo2'),
  slash(literal('order')),
  Route.mapTo(Demo2OrderRoute),
)
export const demo2ThreadRouter = pipe(
  literal('demo2'),
  slash(literal('t')),
  slash(string('threadId')),
  Route.mapTo(Demo2ThreadRoute),
)
export const demo2TeamProjectChatRouter = pipe(
  literal('demo2'),
  slash(literal('teams')),
  slash(string('teamRef')),
  slash(literal('projects')),
  slash(string('projectRef')),
  slash(literal('chat')),
  Route.mapTo(Demo2TeamProjectChatRoute),
)
export const demo2TeamFilesRouter = pipe(
  literal('demo2'),
  slash(literal('teams')),
  slash(string('teamRef')),
  slash(literal('files')),
  Route.mapTo(Demo2TeamFilesRoute),
)
export const demo2TeamFileRouter = pipe(
  literal('demo2'),
  slash(literal('teams')),
  slash(string('teamRef')),
  slash(literal('files')),
  slash(string('fileId')),
  Route.mapTo(Demo2TeamFileRoute),
)

const routeParser = Route.oneOf(
  demo2TeamProjectChatRouter,
  demo2TeamFileRouter,
  demo2TeamFilesRouter,
  demo2ThreadRouter,
  demo2OrderRouter,
  demo2Router,
  demoTeamProjectChatRouter,
  demoTeamFileRouter,
  demoTeamFilesRouter,
  demoThreadRouter,
  demoOrderRouter,
  demoLegalRouter,
  demoRouter,
  productPromisesRouter,
  publicTrainingRunRouter,
  publicTrainingRunsRouter,
  docsPageRouter,
  siteCheckoutDemoReturnRouter,
  siteCheckoutDemoRouter,
  clientsPreviewRouter,
  componentsFamilyRouter,
  componentsRouter,
  businessRouter,
  animationsRouter,
  activityRouter,
  tassadarReplayRouter,
  tassadarRouter,
  runRouter,
  forumReceiptRouter,
  forumTopicRouter,
  forumForumRouter,
  blogPostRouter,
  publicAgentRouter,
  shareRouter,
  moksha2Router,
  mokshaRouter,
  landingRouter,
  termsRouter,
  privacyRouter,
  pylonRouter,
  downloadRouter,
  inviteRouter,
  onboardingRouter,
  autopilotWorkDetailRouter,
  autopilotWorkRouter,
  forgeRouter,
  decisionsRouter,
  workspaceRouter,
  workroomTabRouter,
  workroomRouter,
  orderDetailRouter,
  orderRouter,
  teamProjectChatRouter,
  teamChatRouter,
  teamFileRouter,
  teamFilesRouter,
  personalFileRouter,
  threadRouter,
  docsRouter,
  forumRouter,
  blogRouter,
  billingRouter,
  usageRouter,
  publicStatsArchiveRouter,
  statsRouter,
  adminRouter,
  mulletRouter,
  imagesRouter,
  settingsSectionRouter,
  settingsRouter,
  chatRouter,
  homeRouter,
)

const parseUrlToAppRoute = Route.parseUrlWithFallback(
  routeParser,
  NotFoundRoute,
)

export const urlToAppRoute = (url: Url): AppRoute =>
  url.pathname === '/artanis'
    ? PublicAgentRoute({ agentRef: 'artanis' })
    : url.pathname === '/adjutant'
      ? PublicAgentRoute({ agentRef: 'adjutant' })
      : parseUrlToAppRoute(url)
