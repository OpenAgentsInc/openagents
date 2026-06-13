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
export const DecisionsRoute = r('Decisions')
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
export const BlogRoute = r('Blog')
export const BlogPostRoute = r('BlogPost', { slug: S.String })
export const PublicAgentRoute = r('PublicAgent', { agentRef: S.String })
export const ShareRoute = r('Share', { shareId: S.String })
export const DashboardRoute = r('Dashboard')
export const BillingRoute = r('Billing')
export const UsageRoute = r('Usage')
export const StatsRoute = r('Stats')
export const AdminRoute = r('Admin')
export const MulletRoute = r('Mullet')
export const ImagesRoute = r('Images')
export const SettingsRoute = r('Settings')
export const SettingsSectionRoute = r('SettingsSection', { section: S.String })
export const DemoRoute = r('Demo')
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
export const NotFoundRoute = r('NotFound', { path: S.String })

export type HomeRoute = typeof HomeRoute.Type
export type InviteRoute = typeof InviteRoute.Type
export type OnboardingRoute = typeof OnboardingRoute.Type
export type OrderRoute = typeof OrderRoute.Type
export type OrderDetailRoute = typeof OrderDetailRoute.Type
export type AutopilotWorkRoute = typeof AutopilotWorkRoute.Type
export type AutopilotWorkDetailRoute =
  typeof AutopilotWorkDetailRoute.Type
export type DecisionsRoute = typeof DecisionsRoute.Type
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
export type BlogRoute = typeof BlogRoute.Type
export type BlogPostRoute = typeof BlogPostRoute.Type
export type PublicAgentRoute = typeof PublicAgentRoute.Type
export type ShareRoute = typeof ShareRoute.Type
export type DashboardRoute = typeof DashboardRoute.Type
export type BillingRoute = typeof BillingRoute.Type
export type UsageRoute = typeof UsageRoute.Type
export type StatsRoute = typeof StatsRoute.Type
export type AdminRoute = typeof AdminRoute.Type
export type MulletRoute = typeof MulletRoute.Type
export type ImagesRoute = typeof ImagesRoute.Type
export type SettingsRoute = typeof SettingsRoute.Type
export type SettingsSectionRoute = typeof SettingsSectionRoute.Type
export type DemoRoute = typeof DemoRoute.Type
export type DemoOrderRoute = typeof DemoOrderRoute.Type
export type DemoThreadRoute = typeof DemoThreadRoute.Type
export type DemoTeamProjectChatRoute = typeof DemoTeamProjectChatRoute.Type
export type DemoTeamFilesRoute = typeof DemoTeamFilesRoute.Type
export type DemoTeamFileRoute = typeof DemoTeamFileRoute.Type
export type NotFoundRoute = typeof NotFoundRoute.Type

export const LoggedOutRoute = S.Union([
  HomeRoute,
  StatsRoute,
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
  BlogRoute,
  BlogPostRoute,
  PublicAgentRoute,
  ShareRoute,
  NotFoundRoute,
])
export const LoggedInRoute = S.Union([
  InviteRoute,
  OnboardingRoute,
  OrderRoute,
  OrderDetailRoute,
  AutopilotWorkRoute,
  AutopilotWorkDetailRoute,
  DecisionsRoute,
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
  DecisionsRoute,
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
  BlogRoute,
  BlogPostRoute,
  PublicAgentRoute,
  ShareRoute,
  DashboardRoute,
  BillingRoute,
  UsageRoute,
  StatsRoute,
  AdminRoute,
  MulletRoute,
  ImagesRoute,
  SettingsRoute,
  SettingsSectionRoute,
  DemoRoute,
  DemoOrderRoute,
  DemoThreadRoute,
  DemoTeamProjectChatRoute,
  DemoTeamFilesRoute,
  DemoTeamFileRoute,
  NotFoundRoute,
])

export type LoggedOutRoute = typeof LoggedOutRoute.Type
export type LoggedInRoute = typeof LoggedInRoute.Type
export type AppRoute = typeof AppRoute.Type

export const homeRouter = pipe(Route.root, Route.mapTo(HomeRoute))
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
export const decisionsRouter = pipe(
  literal('decisions'),
  Route.mapTo(DecisionsRoute),
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
export const billingRouter = pipe(literal('billing'), Route.mapTo(BillingRoute))
export const usageRouter = pipe(literal('usage'), Route.mapTo(UsageRoute))
export const statsRouter = pipe(literal('stats'), Route.mapTo(StatsRoute))
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

const routeParser = Route.oneOf(
  demoTeamProjectChatRouter,
  demoTeamFileRouter,
  demoTeamFilesRouter,
  demoThreadRouter,
  demoOrderRouter,
  demoRouter,
  productPromisesRouter,
  publicTrainingRunRouter,
  publicTrainingRunsRouter,
  docsPageRouter,
  siteCheckoutDemoReturnRouter,
  siteCheckoutDemoRouter,
  clientsPreviewRouter,
  forumReceiptRouter,
  forumTopicRouter,
  forumForumRouter,
  blogPostRouter,
  publicAgentRouter,
  shareRouter,
  inviteRouter,
  onboardingRouter,
  autopilotWorkDetailRouter,
  autopilotWorkRouter,
  decisionsRouter,
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
