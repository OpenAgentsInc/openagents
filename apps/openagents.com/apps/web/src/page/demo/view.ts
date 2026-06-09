import { Match as M } from 'effect'
import { Submodel } from 'foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../ui'
import * as LoggedIn from '../loggedIn'
import * as Chat from '../loggedIn/page/chat'
import * as Files from '../loggedIn/page/files'
import {
  DEMO_FILE_PLAN_ID,
  DEMO_PROJECT_REF,
  DEMO_RUN_ID,
  DEMO_TEAM_REF,
} from './fixtures'
import {
  ClickedNextDemoStep,
  ClickedPreviousDemoStep,
  GotLoggedInDemoMessage,
  Message,
} from './message'
import { Model } from './model'

const demoProjectHref = `/demo/teams/${DEMO_TEAM_REF}/projects/${DEMO_PROJECT_REF}/chat`
const demoThreadHref = `/demo/t/${DEMO_RUN_ID}`
const demoFilesHref = `/demo/teams/${DEMO_TEAM_REF}/files`
const demoFileHref = `/demo/teams/${DEMO_TEAM_REF}/files/${DEMO_FILE_PLAN_ID}`

const currentDemoHref = (model: LoggedIn.Model): string =>
  M.value(model.route).pipe(
    M.tags({
      Thread: () => demoThreadHref,
      TeamFiles: () => demoFilesHref,
      TeamFile: () => demoFileHref,
      TeamProjectChat: () => demoProjectHref,
    }),
    M.orElse(() => demoProjectHref),
  )

const sidebar = (model: LoggedIn.Model): Html =>
  Ui.workroomSidebar<LoggedIn.Message>({
    product: 'OpenAgents',
    workspace: 'OpenAgents',
    userName: model.session.name,
    userEmail: model.session.email,
    navSections: [],
    sessionSections: [
      {
        title: 'Threads',
        items: [
          {
            active:
              currentDemoHref(model) === demoProjectHref ||
              currentDemoHref(model) === demoThreadHref,
            attention:
              model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.status !== 'completed',
            detail: 'Artanis project',
            href: demoProjectHref,
            status:
              model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.status === 'completed'
                ? 'complete'
                : 'active',
            title: 'Pylon briefing',
          },
          {
            active: false,
            attention: false,
            detail: 'Run thread',
            href: demoThreadHref,
            status:
              model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.status === 'completed'
                ? 'complete'
                : 'queued',
            title: 'Autopilot run',
          },
          {
            active:
              currentDemoHref(model) === demoFilesHref ||
              currentDemoHref(model) === demoFileHref,
            attention: false,
            detail: '2 files',
            href: demoFilesHref,
            status: 'active',
            title: 'Team files',
          },
        ],
      },
    ],
    accountMenuItems: [],
    footerRows: [{ label: 'Playback', value: '15s' }],
    navDensity: 'compact',
    headerActions: [
      Ui.workroomSidebarActionLink<LoggedIn.Message>({
        href: demoProjectHref,
        icon: 'ChatCompose',
        label: 'New thread',
      }),
      Ui.workroomSidebarActionLink<LoggedIn.Message>({
        href: demoFilesHref,
        icon: 'Folder',
        label: 'Files',
      }),
    ],
  })

const mobileSidebar = (model: LoggedIn.Model): Html =>
  Ui.workroomMobileSidebar<LoggedIn.Message>({
    product: 'OpenAgents',
    userName: model.session.name,
    navSections: [],
    sessionSections: [
      {
        title: 'Threads',
        items: [
          {
            active:
              currentDemoHref(model) === demoProjectHref ||
              currentDemoHref(model) === demoThreadHref,
            attention:
              model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.status !== 'completed',
            detail: 'Artanis project',
            href: demoProjectHref,
            status:
              model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.status === 'completed'
                ? 'complete'
                : 'active',
            title: 'Pylon briefing',
          },
          {
            active:
              currentDemoHref(model) === demoFilesHref ||
              currentDemoHref(model) === demoFileHref,
            attention: false,
            detail: '2 files',
            href: demoFilesHref,
            status: 'active',
            title: 'Team files',
          },
        ],
      },
    ],
    headerActions: [
      Ui.workroomSidebarActionLink<LoggedIn.Message>({
        href: demoProjectHref,
        icon: 'ChatCompose',
        label: 'New thread',
      }),
      Ui.workroomSidebarActionLink<LoggedIn.Message>({
        href: demoFilesHref,
        icon: 'Folder',
        label: 'Files',
      }),
    ],
  })

const routeBody = (model: LoggedIn.Model): Html =>
  M.value(model.route).pipe(
    M.tags({
      Thread: () => Ui.workroomChatRoute<LoggedIn.Message>(Chat.view(model)),
      TeamFiles: () =>
        Ui.workroomScrollableRoute<LoggedIn.Message>([
          Files.view(model, DEMO_TEAM_REF),
        ]),
      TeamFile: () =>
        Ui.workroomScrollableRoute<LoggedIn.Message>([
          Files.detailView(model, {
            fileId: DEMO_FILE_PLAN_ID,
            teamRef: DEMO_TEAM_REF,
            variant: 'team',
          }),
        ]),
      TeamProjectChat: () =>
        Ui.workroomChatRoute<LoggedIn.Message>(
          Chat.teamProjectView(model, DEMO_TEAM_REF, DEMO_PROJECT_REF),
        ),
    }),
    M.orElse(() =>
      Ui.workroomChatRoute<LoggedIn.Message>(
        Chat.teamProjectView(model, DEMO_TEAM_REF, DEMO_PROJECT_REF),
      ),
    ),
  )

const loggedInDemoView = Submodel.defineView<LoggedIn.Model, LoggedIn.Message>(
  model =>
    Ui.workroomShell<LoggedIn.Message>(
      [
        sidebar(model),
        Ui.workroomRouteMain<LoggedIn.Message>({
          key: currentDemoHref(model),
          variant:
            model.route._tag === 'TeamFiles' || model.route._tag === 'TeamFile'
              ? 'scroll'
              : 'chat',
          mobileSidebar: mobileSidebar(model),
          children: [routeBody(model)],
        }),
      ],
      [
        html<LoggedIn.Message>().Key('demo-workroom-shell'),
        html<LoggedIn.Message>().DataAttribute(
          'component',
          'demo-workroom-shell',
        ),
      ],
    ),
)

const stepControls = (): Html => {
  const h = html<Message>()
  const buttonClass =
    'fixed top-1/2 z-50 flex size-11 -translate-y-1/2 items-center justify-center rounded-full border border-white/15 bg-black/75 font-mono text-xl text-white/70 shadow-[0_0_24px_rgba(0,0,0,0.45)] backdrop-blur hover:border-white/35 hover:text-white focus:outline-none focus:ring-2 focus:ring-white/30'

  return h.div(
    [Ui.className<Message>('pointer-events-none fixed inset-0 z-50 font-mono')],
    [
      h.button(
        [
          Ui.className<Message>(
            `${buttonClass} pointer-events-auto left-3 md:left-5`,
          ),
          h.AriaLabel('Previous demo step'),
          h.OnClick(ClickedPreviousDemoStep()),
        ],
        ['<'],
      ),
      h.button(
        [
          Ui.className<Message>(
            `${buttonClass} pointer-events-auto right-3 md:right-5`,
          ),
          h.AriaLabel('Next demo step'),
          h.OnClick(ClickedNextDemoStep()),
        ],
        ['>'],
      ),
    ],
  )
}

export const view = Submodel.defineView<Model, Message>((model): Html => {
  const h = html<Message>()

  if (model.mode === 'order') {
    return h.div(
      [Ui.className<Message>('min-h-screen bg-black text-white')],
      [
        stepControls(),
        h.submodel({
          slotId: 'demo-order-flow',
          model: model.loggedIn,
          view: LoggedIn.view,
          toParentMessage: message => GotLoggedInDemoMessage({ message }),
        }),
      ],
    )
  }

  return h.div(
    [Ui.className<Message>('min-h-screen bg-black text-white')],
    [
      stepControls(),
      h.submodel({
        slotId: 'demo-logged-in-workroom',
        model: model.loggedIn,
        view: loggedInDemoView,
        toParentMessage: message => GotLoggedInDemoMessage({ message }),
      }),
    ],
  )
})
