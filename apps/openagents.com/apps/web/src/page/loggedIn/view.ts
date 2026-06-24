import { Match as M } from 'effect'
import { Submodel } from 'foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { notFoundView } from '../../notFoundView'
import { browserRouteIsEnabled } from '../../product-policy'
import {
  adminRouter,
  activityRouter,
  animationsRouter,
  autopilotWorkDetailRouter,
  autopilotWorkRouter,
  billingRouter,
  blogPostRouter,
  blogRouter,
  businessRouter,
  chatRouter,
  clientsPreviewRouter,
  componentsFamilyRouter,
  componentsRouter,
  decisionsRouter,
  demoLegalRouter,
  docsPageRouter,
  docsRouter,
  forgeRouter,
  forumForumRouter,
  forumReceiptRouter,
  forumRouter,
  forumTopicRouter,
  gymOssRouter,
  imagesRouter,
  inviteRouter,
  loginRouter,
  mulletRouter,
  onboardingRouter,
  orderDetailRouter,
  orderRouter,
  personalFileRouter,
  proEvalRouter,
  proEvalsRouter,
  proRouter,
  proRunRouter,
  proRunsRouter,
  publicAgentRouter,
  traceRouter,
  publicTrainingRunRouter,
  publicTrainingRunsRouter,
  runRouter,
  settingsRouter,
  siteCheckoutDemoReturnRouter,
  siteCheckoutDemoRouter,
  statsRouter,
  tassadarReplayRouter,
  tassadarRouter,
  teamChatRouter,
  teamFileRouter,
  teamFilesRouter,
  teamProjectChatRouter,
  threadRouter,
  usageRouter,
  workroomRouter,
  workroomTabRouter,
  workspaceRouter,
} from '../../route'
import * as Ui from '../../ui'
import * as Activity from '../activity'
import * as ClientsPreview from '../clientsPreview'
import * as Forum from '../forum'
import * as SiteCheckoutDemo from '../siteCheckoutDemo'
import { ClickedLogout, ClickedNewChat, Message } from './message'
import { type Model, type SidebarModel, teamRouteRef } from './model'
import * as Mullet from './mullet/view'
import { notificationsPanel } from './notifications/view'
import * as Admin from './page/admin'
import * as AutopilotWork from './page/autopilot-work'
import * as Billing from './page/billing'
import * as Chat from './page/chat'
import * as Dashboard from './page/dashboard'
import * as Decisions from './page/decisions'
import * as Files from './page/files'
import * as Forge from './page/forge'
import * as GymOss from './page/gymOss'
import * as Images from './page/images'
import * as Invite from './page/invite'
import * as Onboarding from './page/onboarding'
import * as Order from './page/order'
import * as Pro from './page/pro'
import * as ProEvals from './page/pro-evals'
import * as ProRuns from './page/pro-runs'
import * as Settings from './page/settings'
import * as Stats from './page/stats'
import * as Usage from './page/usage'
import * as Workroom from './page/workroom'
import * as Workspace from './page/workspace'

const currentHref = (model: Model): string =>
  M.value(model.route).pipe(
    M.tagsExhaustive({
      Onboarding: () => onboardingRouter(),
      Order: () => orderRouter(),
      OrderDetail: ({ orderId }) => orderDetailRouter({ orderId }),
      AutopilotWork: () => autopilotWorkRouter(),
      AutopilotWorkDetail: ({ workOrderRef }) =>
        autopilotWorkDetailRouter({ workOrderRef }),
      Forge: () => forgeRouter(),
      Decisions: () => decisionsRouter(),
      Workspace: ({ workspaceId }) => workspaceRouter({ workspaceId }),
      Workroom: ({ workroomId }) => workroomRouter({ workroomId }),
      WorkroomTab: ({ tab, workroomId }) =>
        workroomTabRouter({ tab, workroomId }),
      Invite: () => inviteRouter(),
      Chat: () => chatRouter(),
      TeamChat: ({ teamRef }) => teamChatRouter({ teamRef }),
      TeamProjectChat: ({ projectRef, teamRef }) =>
        teamProjectChatRouter({ projectRef, teamRef }),
      TeamFiles: ({ teamRef }) => teamFilesRouter({ teamRef }),
      TeamFile: ({ teamRef, fileId }) => teamFileRouter({ teamRef, fileId }),
      PersonalFile: ({ fileId }) => personalFileRouter({ fileId }),
      Thread: ({ threadId }) => threadRouter({ threadId }),
      Docs: () => docsRouter(),
      DocsPage: ({ slug }) => docsPageRouter({ slug }),
      Forum: () => forumRouter(),
      ForumForum: ({ forumRef }) => forumForumRouter({ forumRef }),
      ForumTopic: ({ topicId }) => forumTopicRouter({ topicId }),
      ForumReceipt: ({ receiptRef }) => forumReceiptRouter({ receiptRef }),
      SiteCheckoutDemo: () => siteCheckoutDemoRouter(),
      SiteCheckoutDemoReturn: ({ returnAction }) =>
        siteCheckoutDemoReturnRouter({ returnAction }),
      ClientsPreview: () => clientsPreviewRouter(),
      Components: () => componentsRouter(),
      ComponentsFamily: ({ family }) => componentsFamilyRouter({ family }),
      Business: () => businessRouter(),
      Animations: () => animationsRouter(),
      Activity: () => activityRouter(),
      DemoLegal: () => demoLegalRouter(),
      Run: () => runRouter(),
      GymOss: () => gymOssRouter(),
      Tassadar: () => tassadarRouter(),
      TassadarReplay: ({ replaySlug }) => tassadarReplayRouter({ replaySlug }),
      Login: () => loginRouter(),
      Blog: () => blogRouter(),
      BlogPost: ({ slug }) => blogPostRouter({ slug }),
      PublicAgent: ({ agentRef }) => publicAgentRouter({ agentRef }),
      Trace: ({ uuid }) => traceRouter({ uuid }),
      PublicTrainingRuns: () => publicTrainingRunsRouter(),
      PublicTrainingRun: ({ runId }) => publicTrainingRunRouter({ runId }),
      Dashboard: () => '',
      Pro: () => proRouter(),
      ProRuns: () => proRunsRouter(),
      ProRun: ({ runId }) => proRunRouter({ runId }),
      ProEvals: () => proEvalsRouter(),
      ProEval: ({ evalId }) => proEvalRouter({ evalId }),
      Billing: () => billingRouter(),
      Usage: () => usageRouter(),
      Stats: () => statsRouter(),
      Admin: () => adminRouter(),
      Mullet: () => mulletRouter(),
      Images: () => imagesRouter(),
      NotFound: () => '',
      Settings: () => settingsRouter(),
      SettingsSection: ({ section }) =>
        Settings.settingsSectionHref(Settings.normalizeSection(section)),
    }),
  )

const routeKey = (model: Model): string =>
  M.value(model.route).pipe(
    M.tagsExhaustive({
      Onboarding: () => 'Onboarding',
      Order: () => 'Order',
      OrderDetail: ({ orderId }) => `OrderDetail:${orderId}`,
      AutopilotWork: () => 'AutopilotWork',
      AutopilotWorkDetail: ({ workOrderRef }) =>
        `AutopilotWorkDetail:${workOrderRef}`,
      Forge: () => 'Forge',
      Decisions: () => 'Decisions',
      Workspace: ({ workspaceId }) => `Workspace:${workspaceId}`,
      Workroom: ({ workroomId }) => `Workroom:${workroomId}`,
      WorkroomTab: ({ tab, workroomId }) => `WorkroomTab:${workroomId}:${tab}`,
      Invite: () => 'Invite',
      Chat: () => 'Chat',
      TeamChat: ({ teamRef }) => `TeamChat:${teamRef}`,
      TeamProjectChat: ({ projectRef, teamRef }) =>
        `TeamProjectChat:${teamRef}:${projectRef}`,
      TeamFiles: ({ teamRef }) => `TeamFiles:${teamRef}`,
      TeamFile: ({ teamRef, fileId }) => `TeamFile:${teamRef}:${fileId}`,
      PersonalFile: ({ fileId }) => `PersonalFile:${fileId}`,
      Thread: ({ threadId }) => `Thread:${threadId}`,
      Docs: () => 'Docs',
      DocsPage: ({ slug }) => `DocsPage:${slug}`,
      Forum: () => 'Forum',
      ForumForum: ({ forumRef }) => `ForumForum:${forumRef}`,
      ForumTopic: ({ topicId }) => `ForumTopic:${topicId}`,
      ForumReceipt: ({ receiptRef }) => `ForumReceipt:${receiptRef}`,
      SiteCheckoutDemo: () => 'SiteCheckoutDemo',
      SiteCheckoutDemoReturn: ({ returnAction }) =>
        `SiteCheckoutDemoReturn:${returnAction}`,
      ClientsPreview: () => 'ClientsPreview',
      Components: () => 'Components',
      ComponentsFamily: () => 'Components',
      Business: () => 'Business',
      Animations: () => 'Animations',
      Activity: () => 'Activity',
      DemoLegal: () => 'DemoLegal',
      Run: () => 'Run',
      GymOss: () => 'GymOss',
      Tassadar: () => 'Tassadar',
      TassadarReplay: ({ replaySlug }) => `TassadarReplay:${replaySlug}`,
      Login: () => 'Login',
      Blog: () => 'Blog',
      BlogPost: ({ slug }) => `BlogPost:${slug}`,
      PublicAgent: ({ agentRef }) => `PublicAgent:${agentRef}`,
      Trace: ({ uuid }) => `Trace:${uuid}`,
      PublicTrainingRuns: () => 'PublicTrainingRuns',
      PublicTrainingRun: ({ runId }) => `PublicTrainingRun:${runId}`,
      Dashboard: () => 'Dashboard',
      Pro: () => 'Pro',
      ProRuns: () => 'ProRuns',
      ProRun: ({ runId }) => `ProRun:${runId}`,
      ProEvals: () => 'ProEvals',
      ProEval: ({ evalId }) => `ProEval:${evalId}`,
      Billing: () => 'Billing',
      Usage: () => 'Usage',
      Stats: () => 'Stats',
      Admin: () => 'Admin',
      Mullet: () => 'Mullet',
      Images: () => 'Images',
      NotFound: ({ path }) => `NotFound:${path}`,
      Settings: () => 'Settings',
      SettingsSection: ({ section }) =>
        `SettingsSection:${Settings.normalizeSection(section)}`,
    }),
  )

const sidebarNavSections = (
  sidebar: SidebarModel,
  activeHref: string,
): ReadonlyArray<Ui.WorkroomSidebarNavSection> => {
  const items = sidebar.primaryItems.map(item => ({
    href: item.href,
    label: item.label,
    ...(item.meta === undefined ? {} : { meta: item.meta }),
    active: item.href === activeHref,
  }))

  return items.length === 0
    ? []
    : [
        {
          title: 'Primary',
          items,
        },
      ]
}

const sidebarSessionSections = (
  sidebar: SidebarModel,
  activeHref: string,
): ReadonlyArray<Ui.WorkroomSidebarSessionSection> =>
  sidebar.sessionSections.map(section => ({
    title: section.title,
    items: section.items.map(item => ({
      active: item.href === activeHref,
      attention: item.attention,
      detail: item.detail,
      href: item.href,
      status: item.status,
      title: item.title,
    })),
  }))

const isSettingsSurface = (model: Model): boolean =>
  model.route._tag === 'Settings' ||
  model.route._tag === 'SettingsSection' ||
  model.route._tag === 'Billing' ||
  model.route._tag === 'Usage'

const settingsNavSections = (
  activeHref: string,
): ReadonlyArray<Ui.WorkroomSidebarNavSection> => [
  {
    title: 'Settings',
    items: [
      { href: Settings.settingsSectionHref('general'), label: 'General' },
      {
        href: Settings.settingsSectionHref('connections'),
        label: 'Connections',
      },
      {
        href: Settings.settingsSectionHref('organization'),
        label: 'Organization',
      },
      { href: Settings.settingsSectionHref('members'), label: 'Members' },
      { href: billingRouter(), label: 'Billing' },
      { href: usageRouter(), label: 'Usage' },
    ].map(item => ({
      ...item,
      active: item.href === activeHref,
    })),
  },
]

const avatarInput = (model: Model): { userAvatarUrl?: string } =>
  model.session.avatarUrl === null ||
  model.session.avatarUrl === undefined ||
  model.session.avatarUrl === ''
    ? {}
    : { userAvatarUrl: model.session.avatarUrl }

const accountMenuItems = (): ReadonlyArray<
  Ui.WorkroomAccountMenuItem<Message>
> => [
  { href: settingsRouter(), label: 'Settings' },
  { href: billingRouter(), label: 'Billing' },
  { href: usageRouter(), label: 'Usage' },
  {
    attrs: [html<Message>().OnClick(ClickedLogout())],
    label: 'Log out',
    tone: 'danger',
  },
]

const settingsSidebarView = (model: Model): Html =>
  Ui.workroomSidebar<Message>({
    product: 'OpenAgents',
    workspace: 'Settings',
    userName: model.session.name,
    userEmail: model.session.email,
    ...avatarInput(model),
    navSections: settingsNavSections(currentHref(model)),
    sessionSections: [],
    accountMenuItems: accountMenuItems(),
    footerRows: [],
    navDensity: 'compact',
    headerActions: [
      Ui.workroomSidebarActionLink<Message>({
        icon: 'ArrowLeft',
        href: chatRouter(),
        label: 'Back to App',
      }),
    ],
  })

const sidebarFilesHref = (model: Model): string | undefined => {
  const team = model.auth.teams[0]

  return team === undefined
    ? undefined
    : teamFilesRouter({ teamRef: teamRouteRef(team) })
}

const sidebarHeaderActions = (model: Model): ReadonlyArray<Html> => {
  const filesHref = sidebarFilesHref(model)

  return [
    Ui.workroomSidebarActionButton<Message>({
      icon: 'ChatCompose',
      label: 'New thread',
      attrs: [html<Message>().OnClick(ClickedNewChat())],
    }),
    ...(filesHref === undefined
      ? []
      : [
          Ui.workroomSidebarActionLink<Message>({
            href: filesHref,
            icon: 'Folder',
            label: 'Files',
          }),
        ]),
  ]
}

const sidebarView = (model: Model): Html =>
  isSettingsSurface(model)
    ? settingsSidebarView(model)
    : Ui.workroomSidebar<Message>({
        product: 'OpenAgents',
        productAttrs: [html<Message>().OnClick(ClickedNewChat())],
        workspace: 'OpenAgents',
        userName: model.session.name,
        userEmail: model.session.email,
        ...avatarInput(model),
        navSections: sidebarNavSections(model.sidebar, currentHref(model)),
        sessionSections: sidebarSessionSections(
          model.sidebar,
          currentHref(model),
        ),
        accountMenuItems: accountMenuItems(),
        footerRows: model.sidebar.footerRows,
        navDensity: 'compact',
        headerActions: sidebarHeaderActions(model),
      })

const mobileSettingsSidebarView = (model: Model): Html =>
  Ui.workroomMobileSidebar<Message>({
    product: 'OpenAgents settings',
    userName: model.session.name,
    navSections: settingsNavSections(currentHref(model)),
    sessionSections: [],
    headerActions: [
      Ui.workroomSidebarActionLink<Message>({
        icon: 'ArrowLeft',
        href: chatRouter(),
        label: 'Back to App',
      }),
    ],
  })

const mobileSidebarView = (model: Model): Html => {
  if (isSettingsSurface(model)) {
    return mobileSettingsSidebarView(model)
  }

  return Ui.workroomMobileSidebar<Message>({
    product: 'OpenAgents',
    userName: model.session.name,
    navSections: sidebarNavSections(model.sidebar, currentHref(model)),
    sessionSections: sidebarSessionSections(model.sidebar, currentHref(model)),
    headerActions: sidebarHeaderActions(model),
  })
}

const routeView = (model: Model): Html => {
  return Ui.workroomRouteMain<Message>({
    key: routeKey(model),
    variant:
      model.route._tag === 'Chat' ||
      model.route._tag === 'TeamChat' ||
      model.route._tag === 'TeamProjectChat' ||
      model.route._tag === 'Thread'
        ? 'chat'
        : 'scroll',
    mobileSidebar: mobileSidebarView(model),
    children: [
      ...(model.notifications.items.length > 0
        ? [notificationsPanel(model)]
        : []),
      M.value(model.route).pipe(
        M.tagsExhaustive({
          Invite: () =>
            Ui.workroomScrollableRoute<Message>([Invite.view(model)]),
          Chat: () => Ui.workroomChatRoute<Message>(Chat.view(model)),
          Onboarding: () =>
            Ui.workroomScrollableRoute<Message>([Onboarding.view(model)]),
          Order: () => Ui.workroomScrollableRoute<Message>([Order.view(model)]),
          OrderDetail: () =>
            Ui.workroomScrollableRoute<Message>([Order.view(model)]),
          AutopilotWork: () =>
            Ui.workroomScrollableRoute<Message>([
              AutopilotWork.listView(model),
            ]),
          AutopilotWorkDetail: () =>
            Ui.workroomScrollableRoute<Message>([
              AutopilotWork.detailView(model),
            ]),
          Forge: () => Ui.workroomScrollableRoute<Message>([Forge.view(model)]),
          Decisions: () =>
            Ui.workroomScrollableRoute<Message>([Decisions.view(model)]),
          Workspace: () =>
            Ui.workroomScrollableRoute<Message>([Workspace.view(model)]),
          Workroom: () =>
            Ui.workroomScrollableRoute<Message>([
              Workroom.view(model.workroom),
            ]),
          WorkroomTab: () =>
            Ui.workroomScrollableRoute<Message>([
              Workroom.view(model.workroom),
            ]),
          TeamChat: ({ teamRef }) =>
            Ui.workroomChatRoute<Message>(Chat.teamRoomView(model, teamRef)),
          TeamProjectChat: ({ projectRef, teamRef }) =>
            browserRouteIsEnabled(model.route)
              ? Ui.workroomChatRoute<Message>(
                  Chat.teamProjectView(model, teamRef, projectRef),
                )
              : Ui.workroomScrollableRoute<Message>([
                  notFoundView(
                    teamProjectChatRouter({ projectRef, teamRef }),
                    chatRouter(),
                    'Go to Chat',
                  ),
                ]),
          TeamFiles: ({ teamRef }) =>
            Ui.workroomScrollableRoute<Message>([Files.view(model, teamRef)]),
          TeamFile: ({ teamRef, fileId }) =>
            Ui.workroomScrollableRoute<Message>([
              Files.detailView(model, {
                fileId,
                teamRef,
                variant: 'team',
              }),
            ]),
          PersonalFile: ({ fileId }) =>
            Ui.workroomScrollableRoute<Message>([
              Files.detailView(model, {
                fileId,
                variant: 'personal',
              }),
            ]),
          Thread: () => Ui.workroomChatRoute<Message>(Chat.view(model)),
          Docs: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/docs', chatRouter(), 'Go to Chat'),
            ]),
          DocsPage: ({ slug }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(`/docs/${slug}`, chatRouter(), 'Go to Chat'),
            ]),
          Forum: route =>
            Ui.workroomScrollableRoute<Message>([
              Forum.view(route, {
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          ForumForum: route =>
            Ui.workroomScrollableRoute<Message>([
              Forum.view(route, {
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          ForumTopic: route =>
            Ui.workroomScrollableRoute<Message>([
              Forum.view(route, {
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          ForumReceipt: route =>
            Ui.workroomScrollableRoute<Message>([
              Forum.view(route, {
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          SiteCheckoutDemo: route =>
            Ui.workroomScrollableRoute<Message>([
              SiteCheckoutDemo.view(route, {
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          SiteCheckoutDemoReturn: route =>
            Ui.workroomScrollableRoute<Message>([
              SiteCheckoutDemo.view(route, {
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          ClientsPreview: () =>
            Ui.workroomScrollableRoute<Message>([ClientsPreview.view()]),
          Components: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/components', chatRouter(), 'Go to Chat'),
            ]),
          ComponentsFamily: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/components', chatRouter(), 'Go to Chat'),
            ]),
          Business: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/business', chatRouter(), 'Go to Chat'),
            ]),
          Animations: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/animations', chatRouter(), 'Go to Chat'),
            ]),
          Activity: () =>
            Ui.workroomScrollableRoute<Message>([
              Activity.view({
                _tag: 'LoggedIn',
                onLogout: ClickedLogout(),
              }),
            ]),
          DemoLegal: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/demo/legal', chatRouter(), 'Go to Chat'),
            ]),
          Run: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/run', chatRouter(), 'Go to Chat'),
            ]),
          GymOss: () =>
            Ui.workroomScrollableRoute<Message>([GymOss.view(model)]),
          Tassadar: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/tassadar', chatRouter(), 'Go to Chat'),
            ]),
          TassadarReplay: ({ replaySlug }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(
                tassadarReplayRouter({ replaySlug }),
                chatRouter(),
                'Go to Chat',
              ),
            ]),
          Login: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/login', chatRouter(), 'Go to Chat'),
            ]),
          Blog: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView('/blog', chatRouter(), 'Go to Chat'),
            ]),
          BlogPost: ({ slug }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(`/blog/${slug}`, chatRouter(), 'Go to Chat'),
            ]),
          PublicAgent: ({ agentRef }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(
                publicAgentRouter({ agentRef }),
                chatRouter(),
                'Go to Chat',
              ),
            ]),
          // /trace/{uuid} is a public route resolved by the top-level
          // `publicRouteBody` in view.ts before the logged-in submodel; this
          // branch is unreachable but kept exhaustive (matches PublicAgent).
          Trace: ({ uuid }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(traceRouter({ uuid }), chatRouter(), 'Go to Chat'),
            ]),
          PublicTrainingRuns: () =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(
                publicTrainingRunsRouter(),
                chatRouter(),
                'Go to Chat',
              ),
            ]),
          PublicTrainingRun: ({ runId }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(
                publicTrainingRunRouter({ runId }),
                chatRouter(),
                'Go to Chat',
              ),
            ]),
          Dashboard: () =>
            Ui.workroomScrollableRoute<Message>([
              Dashboard.view(model.session),
            ]),
          // /pro renders as a top-level page in the `view` function below
          // (its own top-strip + register + pane), so this workroom-shell case
          // is a defensive fallback only and never normally reached.
          Pro: () =>
            Ui.workroomScrollableRoute<Message>([Pro.view(model.session)]),
          // /pro/runs + /pro/evals render as top-level Pro console pages in the
          // `view` function below (their own shell), so these workroom-shell
          // cases are defensive fallbacks only and never normally reached.
          ProRuns: () =>
            Ui.workroomScrollableRoute<Message>([
              ProRuns.runsView(model.session),
            ]),
          ProRun: ({ runId }) =>
            Ui.workroomScrollableRoute<Message>([
              ProRuns.runDetailView(model.session, runId),
            ]),
          ProEvals: () =>
            Ui.workroomScrollableRoute<Message>([
              ProEvals.evalsView(model.session),
            ]),
          ProEval: ({ evalId }) =>
            Ui.workroomScrollableRoute<Message>([
              ProEvals.evalDetailView(model.session, evalId),
            ]),
          Billing: () =>
            Ui.workroomScrollableRoute<Message>([Billing.view(model)]),
          Usage: () => Ui.workroomScrollableRoute<Message>([Usage.view(model)]),
          Stats: () => Ui.workroomScrollableRoute<Message>([Stats.view(model)]),
          Admin: () => Ui.workroomScrollableRoute<Message>([Admin.view(model)]),
          Mullet: () =>
            Ui.workroomScrollableRoute<Message>([Mullet.view(model)]),
          Images: () =>
            Ui.workroomScrollableRoute<Message>([Images.view(model)]),
          Settings: () =>
            Ui.workroomScrollableRoute<Message>([
              Settings.view(model, 'general'),
            ]),
          SettingsSection: ({ section }) =>
            Ui.workroomScrollableRoute<Message>([
              Settings.view(model, Settings.normalizeSection(section)),
            ]),
          NotFound: ({ path }) =>
            Ui.workroomScrollableRoute<Message>([
              notFoundView(path, chatRouter(), 'Go to Chat'),
            ]),
        }),
      ),
    ],
  })
}

export const view = Submodel.defineView<Model, Message>((model): Html => {
  const h = html<Message>()

  if (model.route._tag === 'Onboarding') {
    return Ui.pageShell<Message>(
      [
        Ui.routeMain<Message>(
          [Onboarding.view(model)],
          [h.DataAttribute('component', 'logged-in-onboarding-route')],
        ),
      ],
      [
        h.Key('logged-in-onboarding-shell'),
        h.DataAttribute('component', 'logged-in-onboarding-shell'),
      ],
    )
  }

  if (model.route._tag === 'Pro') {
    return Ui.pageShell<Message>(
      [Pro.view(model.session)],
      [
        h.Key('logged-in-pro-shell'),
        h.DataAttribute('component', 'logged-in-pro-shell'),
      ],
    )
  }

  // /pro subpages render as top-level Pro console pages (their own shell), so
  // the URLs are stable + shareable (the PR-evidence loop links to them).
  if (model.route._tag === 'ProRuns') {
    return Ui.pageShell<Message>(
      [ProRuns.runsView(model.session)],
      [
        h.Key('logged-in-pro-runs-shell'),
        h.DataAttribute('component', 'logged-in-pro-runs-shell'),
      ],
    )
  }

  if (model.route._tag === 'ProRun') {
    return Ui.pageShell<Message>(
      [ProRuns.runDetailView(model.session, model.route.runId)],
      [
        h.Key('logged-in-pro-run-shell'),
        h.DataAttribute('component', 'logged-in-pro-run-shell'),
      ],
    )
  }

  if (model.route._tag === 'ProEvals') {
    return Ui.pageShell<Message>(
      [ProEvals.evalsView(model.session)],
      [
        h.Key('logged-in-pro-evals-shell'),
        h.DataAttribute('component', 'logged-in-pro-evals-shell'),
      ],
    )
  }

  if (model.route._tag === 'ProEval') {
    return Ui.pageShell<Message>(
      [ProEvals.evalDetailView(model.session, model.route.evalId)],
      [
        h.Key('logged-in-pro-eval-shell'),
        h.DataAttribute('component', 'logged-in-pro-eval-shell'),
      ],
    )
  }

  if (model.route._tag === 'Order' || model.route._tag === 'OrderDetail') {
    return Ui.pageShell<Message>(
      [
        Ui.routeMain<Message>(
          [Order.view(model)],
          [h.DataAttribute('component', 'logged-in-order-route')],
        ),
      ],
      [
        h.Key('logged-in-order-shell'),
        h.DataAttribute('component', 'logged-in-order-shell'),
      ],
    )
  }

  return Ui.workroomShell<Message>(
    [sidebarView(model), routeView(model)],
    [
      h.Key('logged-in-workroom-shell'),
      h.DataAttribute('component', 'logged-in-workroom-shell'),
    ],
  )
})
