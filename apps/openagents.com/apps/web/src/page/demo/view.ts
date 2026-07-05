import {
  type TrainingRunNodeDefinition,
  type TrainingRunVisualizationOptions,
  defaultTrainingRunNodes,
  trainingRunVisualizationOptionsFromSnapshot,
} from '@openagentsinc/three-effect/core'
import { trainingRunView } from '@openagentsinc/three-effect/foldkit'
import { Match as M, Option } from 'effect'
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
  SelectedTrainingSceneNode,
} from './message'
import { Model } from './model'

const demoProjectHref = `/demo2/teams/${DEMO_TEAM_REF}/projects/${DEMO_PROJECT_REF}/chat`
const demoThreadHref = `/demo2/t/${DEMO_RUN_ID}`
const demoFilesHref = `/demo2/teams/${DEMO_TEAM_REF}/files`
const demoFileHref = `/demo2/teams/${DEMO_TEAM_REF}/files/${DEMO_FILE_PLAN_ID}`

const trainingVisualization = trainingRunVisualizationOptionsFromSnapshot({
  activeWindowCount: 2,
  assignedContributorCount: 7,
  blockerRefCount: 0,
  closeoutSatisfied: true,
  deviceObserved: 4,
  deviceRequired: 4,
  finalValidationLoss: 2.74,
  freivaldsRefCount: 5,
  gradientCloseoutRefCount: 4,
  maxAllowedStaleSteps: 5,
  maxValidationLoss: 3.1,
  pendingPayoutCount: 2,
  plannedWindowCount: 1,
  promiseEvidenceRefCount: 17,
  promiseGreenCount: 3,
  promiseYellowCount: 1,
  receiptRefCount: 12,
  reconciledWindowCount: 1,
  runDetail: 'distributed training authority',
  runLabel: 'CS336 A1 public run',
  runState: 'active',
  sealedWindowCount: 3,
  settledPayoutSats: 2100,
  verifiedWorkCount: 9,
  operatorSignals: [
    {
      id: 'operator.window',
      label: 'window',
      state: 'success',
      detail: '2 active',
    },
    {
      id: 'operator.freivalds',
      label: 'Freivalds',
      state: 'success',
      detail: '5 refs',
    },
    {
      id: 'operator.payout',
      label: 'ledger',
      state: 'info',
      detail: '2 pending',
    },
  ],
}) satisfies TrainingRunVisualizationOptions

const trainingNodes =
  trainingVisualization.nodes === undefined
    ? defaultTrainingRunNodes
    : trainingVisualization.nodes

type TrainingStat = Readonly<{
  label: string
  value: string
  detail: string
}>

const trainingStats: ReadonlyArray<TrainingStat> = [
  { label: 'Active windows', value: '2', detail: '3 sealed' },
  { label: 'Pylons online', value: '4/4', detail: '7 assigned' },
  { label: 'Receipts', value: '12', detail: '17 promise refs' },
  { label: 'Freivalds', value: '5', detail: '9 verified work refs' },
]

const selectedTrainingNode = (model: Model): TrainingRunNodeDefinition | null =>
  Option.match(model.maybeSelectedTrainingSceneNodeId, {
    onNone: () =>
      trainingNodes.find(node => node.id === 'run') ?? trainingNodes[0] ?? null,
    onSome: nodeId =>
      trainingNodes.find(node => node.id === nodeId) ??
      trainingNodes.find(node => node.id === 'run') ??
      trainingNodes[0] ??
      null,
  })

const trainingNodeFacts = (
  node: TrainingRunNodeDefinition,
): ReadonlyArray<TrainingStat> => {
  if (node.id === 'run') {
    return [
      { label: 'Run state', value: 'active', detail: 'public projection live' },
      { label: 'Promise gates', value: '3 green', detail: '1 yellow' },
      { label: 'Settled', value: '2,100 sats', detail: '2 pending payouts' },
    ]
  }

  if (node.id === 'freivalds') {
    return [
      { label: 'Challenge refs', value: '5', detail: 'all public-safe' },
      { label: 'Verified work', value: '9', detail: 'sealed windows' },
      { label: 'Loss', value: '2.74', detail: 'under 3.10 budget' },
    ]
  }

  if (node.id === 'training_window' || node.id === 'sealed_window') {
    return [
      { label: 'Active', value: '2', detail: '1 planned' },
      { label: 'Durable', value: '4', detail: '3 sealed + 1 reconciled' },
      { label: 'Closeout', value: 'ready', detail: '4 gradient refs' },
    ]
  }

  if (node.role === 'receipt' || node.role === 'rung') {
    return [
      { label: 'Receipts', value: '12', detail: 'ledger projection' },
      { label: 'Payouts', value: '2', detail: 'pending settlement' },
      { label: 'Settled', value: '2,100', detail: 'sats recorded' },
    ]
  }

  return [
    { label: 'Role', value: node.role, detail: node.status },
    { label: 'Detail', value: node.detail, detail: 'selected node' },
    { label: 'Staleness', value: '<= 5', detail: 'bounded steps' },
  ]
}

const statusToneClass = (
  status: TrainingRunNodeDefinition['status'],
): string =>
  status === 'verified' || status === 'sealed'
    ? 'border-[#00c853]/35 text-[#00c853]'
    : status === 'blocked'
      ? 'border-[#d32f2f]/35 text-[#d32f2f]'
      : status === 'active'
        ? 'border-[#ffb400]/35 text-[#ffb400]'
        : 'border-white/15 text-white/65'

const trainingStatView = <Message>(stat: TrainingStat): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-t border-white/10 pt-2 first:border-t-0 first:pt-0 sm:border-t-0 sm:border-l sm:border-white/10 sm:pt-0 sm:pl-3 sm:first:border-l-0 sm:first:pl-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('text-[0.6875rem] uppercase text-white/40')],
        [stat.label],
      ),
      h.div(
        [Ui.className<Message>('text-xl leading-7 text-white')],
        [stat.value],
      ),
      h.div(
        [Ui.className<Message>('text-base/6 text-white/55 sm:text-xs/5')],
        [stat.detail],
      ),
    ],
  )
}

const trainingNodePanel = (node: TrainingRunNodeDefinition): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('selected-training-node', node.id),
      Ui.className<Message>(
        'pointer-events-auto w-full border border-white/15 bg-black/80 p-3 text-white shadow-[0_0_28px_rgba(0,0,0,0.45)] backdrop-blur md:max-w-[24rem]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-start justify-between gap-3')],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'text-[0.6875rem] uppercase text-white/40',
                  ),
                ],
                ['Selected node'],
              ),
              h.h2(
                [Ui.className<Message>('m-0 text-lg leading-6 text-white')],
                [node.label],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-base/6 text-white/60')],
                [node.detail],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                `shrink-0 border px-2 py-1 text-[0.6875rem] uppercase ${statusToneClass(
                  node.status,
                )}`,
              ),
            ],
            [node.status],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'mt-3 grid gap-3 border-t border-white/10 pt-3 sm:grid-cols-3',
          ),
        ],
        trainingNodeFacts(node).map(trainingStatView),
      ),
    ],
  )
}

const trainingSidebarLink = (
  label: string,
  href: string,
  active: boolean,
): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        active
          ? 'block border border-white/20 bg-white/[0.07] px-3 py-2 text-base/6 text-white sm:text-xs/5'
          : 'block border border-white/10 px-3 py-2 text-base/6 text-white/55 hover:border-white/25 hover:text-white sm:text-xs/5',
      ),
    ],
    [label],
  )
}

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

const trainingFullscreenDemoView = (model: Model): Html => {
  const h = html<Message>()
  const selectedNode = selectedTrainingNode(model)

  return h.div(
    [
      h.Key('demo-training-fullscreen'),
      h.DataAttribute('component', 'demo-training-fullscreen'),
      Ui.className<Message>(
        'grid h-dvh min-h-[560px] bg-black font-mono text-[#f1efe8] md:grid-cols-[15rem_minmax(0,1fr)]',
      ),
    ],
    [
      h.aside(
        [
          Ui.className<Message>(
            'z-20 grid content-start gap-4 border-b border-white/10 bg-black/95 p-3 md:border-r md:border-b-0',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'text-[0.6875rem] uppercase text-white/40',
                  ),
                ],
                ['OpenAgents'],
              ),
              h.div(
                [Ui.className<Message>('text-base/6 text-white sm:text-sm/5')],
                ['Training Live'],
              ),
            ],
          ),
          h.nav(
            [Ui.className<Message>('grid gap-2')],
            [
              trainingSidebarLink('Fullscreen demo', '/demo', true),
              trainingSidebarLink('Workroom playback', '/demo2', false),
              trainingSidebarLink('Order playback', '/demo2/order', false),
              // Training runs sidebar link intentionally removed — the
              // training-runs/gym feature is deprecated-for-now (owner
              // decision, restorable later); the page and its route stay in
              // git history rather than being deleted. See
              // docs/fable/2026-07-04-ts-6-start-khala-tassadar-route-slice.md.
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-2 border-t border-white/10 pt-3 text-base/6 text-white/60 sm:text-xs/5',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('flex justify-between gap-3')],
                [
                  h.span([], ['state']),
                  h.span([Ui.className<Message>('text-[#00c853]')], ['active']),
                ],
              ),
              h.div(
                [Ui.className<Message>('flex justify-between gap-3')],
                [
                  h.span([], ['loss']),
                  h.span([Ui.className<Message>('text-white')], ['2.74']),
                ],
              ),
              h.div(
                [Ui.className<Message>('flex justify-between gap-3')],
                [
                  h.span([], ['settled']),
                  h.span([Ui.className<Message>('text-white')], ['2,100 sats']),
                ],
              ),
            ],
          ),
        ],
      ),
      h.main(
        [Ui.className<Message>('relative min-h-0 overflow-hidden bg-black')],
        [
          trainingRunView<Message>(
            [
              Ui.className<Message>(
                'absolute inset-0 block h-full min-h-full w-full',
              ),
            ],
            trainingVisualization,
            node => SelectedTrainingSceneNode({ nodeId: node.id }),
          ),
          h.div(
            [
              Ui.className<Message>(
                'pointer-events-none absolute inset-x-3 top-3 z-10 grid gap-3 lg:grid-cols-[minmax(0,0.72fr)_minmax(24rem,1fr)]',
              ),
            ],
            [
              h.section(
                [
                  Ui.className<Message>(
                    'pointer-events-auto border border-white/15 bg-black/75 p-3 backdrop-blur',
                  ),
                ],
                [
                  h.div(
                    [
                      Ui.className<Message>(
                        'text-[0.6875rem] uppercase text-white/40',
                      ),
                    ],
                    ['Public training projection'],
                  ),
                  h.h1(
                    [
                      Ui.className<Message>(
                        'mt-1 mb-0 text-xl leading-7 text-white sm:text-lg/6',
                      ),
                    ],
                    ['CS336 A1 public run'],
                  ),
                  h.p(
                    [
                      Ui.className<Message>(
                        'mt-1 mb-0 text-base/6 text-white/60',
                      ),
                    ],
                    [
                      'Lifecycle, proof, receipt, payout, and promise state in one live surface.',
                    ],
                  ),
                ],
              ),
              h.section(
                [
                  Ui.className<Message>(
                    'pointer-events-auto grid gap-3 border border-white/15 bg-black/75 p-3 backdrop-blur sm:grid-cols-4',
                  ),
                ],
                trainingStats.map(trainingStatView),
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'pointer-events-none absolute right-3 bottom-3 left-3 z-10 flex justify-end',
              ),
            ],
            [selectedNode === null ? h.empty : trainingNodePanel(selectedNode)],
          ),
        ],
      ),
    ],
  )
}

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

  if (model.mode === 'training') {
    return trainingFullscreenDemoView(model)
  }

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
