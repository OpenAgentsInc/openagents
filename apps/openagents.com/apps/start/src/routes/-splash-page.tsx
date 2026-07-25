import { InternalLink } from '@/components/internal-link'
import { PublicFooter } from '@/components/public-footer'
import { PublicHeader } from '@/components/public-header'
import { DOCS_URL, OMEGA_REPOSITORY_URL } from '@/lib/public-site'
import { trackNamedAnalyticsEvent } from '@/lib/web-analytics'
import { makeKhalaTextSequenceFrames } from '@effect-native/khala-ui'
import { khalaTheme } from '@effect-native/tokens'
import {
  type DesktopActivityStatus,
  type DesktopAgentActivity,
  DesktopAgentGroup,
  DesktopApprovalCard,
  type DesktopApprovalDecision,
  DesktopCommandCard,
  DesktopComposerBar,
  DesktopComposerButton,
  DesktopComposerFrame,
  DesktopComposerInput,
  DesktopConversation,
  DesktopConversationHeader,
  type DesktopFileChange,
  DesktopFileChangeCard,
  DesktopPlanCard,
  type DesktopPlanEntry,
  DesktopQueuedFollowup,
  type DesktopRailDestination,
  DesktopRailScrim,
  DesktopSessionRail,
  DesktopSidebarExpand,
  DesktopTimeline,
  DesktopTimelineMessage,
  DesktopTimelineNotice,
  DesktopToolCallCard,
  type DesktopToolKind,
  DesktopWorkEntry,
  DesktopWorkGroup,
  DesktopWorkbench,
  desktopThemeCssVariables,
} from '@openagentsinc/ui/desktop-workbench'
import {
  ArrowRight,
  ArrowUp,
  ArrowUpRight,
  Command,
  ImagePlus,
  Square,
  Zap,
} from 'lucide-react'
import {
  type FormEvent,
  type WheelEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'

import '../splash.css'
import { SplashHeroCanvas } from './-splash-khala-canvas'

export const splashPageDescription =
  'Omega is the native, Zed-based OpenAgents IDE now in active development.'

type DemoMessageItem = Readonly<{
  id: string
  kind: 'assistant' | 'user'
  text: string
}>

type DemoWorkItem = Readonly<{
  detail: string
  id: string
  kind: 'work'
  label: string
  status: DesktopActivityStatus
}>

type DemoPlanItem = Readonly<{
  entries: ReadonlyArray<DesktopPlanEntry>
  id: string
  kind: 'plan'
  title?: string
}>

type DemoCommandItem = Readonly<{
  command: string
  cwd: string
  defaultOpen?: boolean
  id: string
  kind: 'command'
  output: string
  status: DesktopActivityStatus
}>

type DemoFilesItem = Readonly<{
  changes: ReadonlyArray<DesktopFileChange>
  defaultOpen?: boolean
  id: string
  kind: 'files'
  status: DesktopActivityStatus
}>

type DemoToolItem = Readonly<{
  body: string
  defaultOpen?: boolean
  id: string
  kind: 'tool'
  label: string
  meta?: string
  status: DesktopActivityStatus
  summary: string
  toolKind: DesktopToolKind
}>

type DemoAgentsItem = Readonly<{
  agents: ReadonlyArray<DesktopAgentActivity>
  id: string
  kind: 'agents'
  title?: string
}>

type DemoApprovalItem = Readonly<{
  decision: DesktopApprovalDecision
  description: string
  id: string
  kind: 'approval'
  resource: string
  title: string
}>

type DemoNoticeItem = Readonly<{
  body: string
  danger?: boolean
  id: string
  kind: 'notice'
  label: string
  noticeKind: string
}>

type DemoQueueItem = Readonly<{
  id: string
  kind: 'queue'
  position: number
  text: string
}>

type DemoWorkGroupItem = Readonly<{
  count: number
  id: string
  kind: 'work-group'
  text: string
}>

type DemoItem =
  | DemoMessageItem
  | DemoWorkItem
  | DemoPlanItem
  | DemoCommandItem
  | DemoFilesItem
  | DemoToolItem
  | DemoAgentsItem
  | DemoApprovalItem
  | DemoNoticeItem
  | DemoQueueItem
  | DemoWorkGroupItem

type DemoSession = Readonly<{
  id: string
  title: string
  meta: string
  items: ReadonlyArray<DemoItem>
  reply?: string
}>

type Playback = Readonly<{ id: number; text: string }>
type PlaybackPhase = 'complete' | 'idle' | 'preparing' | 'stopped' | 'streaming'

const initialReply =
  'The onboarding change is ready for review. Three agents handled the interface, accessibility, and verification in parallel. The workroom connects the request to exact files, tests, approvals, and receipts, so the result can be inspected before it is accepted.'

const appServerItems: ReadonlyArray<DemoItem> = [
  {
    id: 'brief',
    kind: 'user',
    text: 'Prepare identity-first onboarding for Omega. Keep the editor, project, and agent setup intact. Split implementation from verification, then show me the exact changes and evidence.',
  },
  {
    id: 'acknowledge',
    kind: 'assistant',
    text: 'I’ll inspect the project and current onboarding flow first, then split the interface and verification work into independent lanes. You can review or steer the work while they run.',
  },
  {
    id: 'plan-start',
    kind: 'plan',
    entries: [
      {
        step: 'Inspect the current identity and onboarding flow',
        status: 'completed',
      },
      {
        step: 'Map the change onto native project and editor state',
        status: 'completed',
      },
      {
        step: 'Implement the identity-first onboarding slice',
        status: 'in_progress',
      },
      {
        step: 'Run accessibility and journey verification',
        status: 'pending',
      },
      { step: 'Review the evidence and accept the result', status: 'pending' },
    ],
  },
  {
    id: 'reasoning-map',
    kind: 'work',
    label: 'Reasoning summary',
    status: 'completed',
    detail:
      'The editor already owns project, buffer, task, terminal, and Git truth. The onboarding change should add identity without creating a second project graph or copying an external agent’s configuration.',
  },
  {
    id: 'inspect-protocol',
    kind: 'command',
    command:
      'rg -n "Identity|Agent Setup|onboarding" crates/onboarding crates/agent_ui',
    cwd: '/work/omega',
    output:
      'crates/onboarding/src/identity.rs       identity-first step\ncrates/onboarding/src/agent_setup.rs    external-agent setup\ncrates/agent_ui/src/agent_panel.rs      native agent thread\n3 product seams matched',
    status: 'completed',
  },
  {
    id: 'skills-tool',
    kind: 'tool',
    toolKind: 'dynamic',
    label: 'skills/list',
    summary: 'Loaded repository and interface guidance',
    meta: 'dynamicToolCall',
    status: 'completed',
    body: 'impeccable · active\nrepository AGENTS.md · active\nopenagents.com constraints · active',
  },
  {
    id: 'spawn-agents',
    kind: 'agents',
    title: 'spawnAgent · implementation swarm',
    agents: [
      {
        agentKey: 'protocol-scout',
        name: 'protocol-scout',
        role: 'Explorer',
        detail: 'Trace identity, project, and agent ownership boundaries',
        status: 'completed',
        transcript: [
          {
            label: 'spawnAgent',
            text: 'Trace the current onboarding flow and report each authority boundary.',
          },
          {
            label: 'Update',
            text: 'Mapped identity, project, agent setup, and application-state owners.',
          },
          {
            label: 'Result',
            text: 'Identity can enter the existing flow without replacing editor or agent state.',
          },
        ],
      },
      {
        agentKey: 'timeline-builder',
        name: 'timeline-builder',
        role: 'Frontend',
        detail: 'Build the native identity-first onboarding step',
        status: 'running',
        transcript: [
          {
            label: 'spawnAgent',
            text: 'Own the onboarding interface and preserve the native application structure.',
          },
          {
            label: 'FileChange',
            text: 'Added identity creation and recovery states before theme and agent setup.',
          },
          {
            label: 'Result',
            text: 'The first-run journey now keeps identity, project, and agent setup distinct.',
          },
        ],
      },
      {
        agentKey: 'a11y-oracle',
        name: 'a11y-oracle',
        role: 'Nested reviewer',
        parent: 'timeline-builder',
        depth: 1,
        detail:
          'Verify disclosure semantics, keyboard targets, and status names',
        status: 'completed',
        transcript: [
          {
            label: 'spawnAgent',
            text: 'Nested child spawned by timeline-builder for an independent accessibility pass.',
          },
          {
            label: 'Review',
            text: 'Details use native disclosures, status is exposed as text, and controls retain accessible names.',
          },
          {
            label: 'Result',
            text: 'No icon-only ambiguity or inaccessible status color dependency found.',
          },
        ],
      },
    ],
  },
  {
    id: 'wait-agents',
    kind: 'tool',
    toolKind: 'dynamic',
    label: 'waitAgent',
    summary: 'Collected 3 delegated results',
    meta: 'collabAgentToolCall',
    status: 'completed',
    body: 'protocol-scout       completed · 38s\ntimeline-builder      completed · 1m 14s\n└─ a11y-oracle        completed · 24s',
  },
  {
    id: 'steer-request',
    kind: 'user',
    text: 'Also make sure the child work is inspectable, and keep the composer usable while the agents are still running.',
  },
  {
    id: 'steer-agent',
    kind: 'tool',
    toolKind: 'dynamic',
    label: 'sendMessage',
    summary: 'Steered timeline-builder',
    meta: 'collabAgentToolCall',
    status: 'completed',
    body: 'Delivered: “Expose the nested child transcript and preserve mid-turn steering in the composer.”',
  },
  {
    id: 'queued-followup',
    kind: 'queue',
    position: 1,
    text: 'Measure the route bundle after the visual pass.',
  },
  {
    id: 'patch',
    kind: 'files',
    defaultOpen: true,
    changes: [
      {
        kind: 'modified',
        path: 'crates/onboarding/src/identity.rs',
        additions: 96,
        deletions: 8,
      },
      {
        kind: 'modified',
        path: 'crates/onboarding/src/onboarding.rs',
        additions: 42,
        deletions: 12,
      },
      {
        kind: 'modified',
        path: 'crates/omega_deltas/src/omega_deltas.rs',
        additions: 18,
        deletions: 2,
      },
      {
        kind: 'modified',
        path: 'crates/onboarding/src/onboarding_tests.rs',
        additions: 74,
        deletions: 4,
      },
    ],
    status: 'completed',
  },
  {
    id: 'focused-tests',
    kind: 'command',
    command: 'cargo test -p onboarding -p omega_deltas',
    cwd: '/work/omega',
    output:
      'test onboarding::identity_first ... ok\ntest onboarding::recovery_path ... ok\ntest omega_deltas::identity_boundary ... ok\n\n3 focused tests passed',
    status: 'completed',
  },
  {
    id: 'build-approval',
    kind: 'approval',
    decision: 'approved',
    title: 'Command approval',
    description:
      'Run the bounded native build to verify the onboarding journey.',
    resource: 'cargo build -p omega',
  },
  {
    id: 'desktop-build',
    kind: 'command',
    command: 'cargo build -p omega',
    cwd: '/work/omega',
    output:
      'Compiling onboarding\nCompiling omega\nFinished release profile\n✓ native build passed',
    status: 'completed',
  },
  {
    id: 'browser-inspect',
    kind: 'tool',
    toolKind: 'mcp',
    label: 'browser · inspect',
    summary: 'Checked the onboarding journey',
    meta: 'mcpToolCall',
    status: 'completed',
    body: 'Identity appears before theme and agent setup\nRecovery remains explicit\nKeyboard traversal completes\nHorizontal overflow: none',
  },
  {
    id: 'web-search',
    kind: 'tool',
    toolKind: 'web',
    label: 'Web search',
    summary: 'Reviewed identity and recovery guidance',
    meta: 'webSearch',
    status: 'completed',
    body: '2 primary references inspected. Recovery language matches the product contract.',
  },
  {
    id: 'image-view',
    kind: 'tool',
    toolKind: 'image',
    label: 'Image view',
    summary: 'Compared default and large-text layouts',
    meta: 'imageView',
    status: 'completed',
    body: 'Default: 1512 × 982\nCompact: 360 × 780\nLarge text: 24 px\nNo clipped controls.',
  },
  {
    id: 'compaction',
    kind: 'notice',
    label: 'Context compacted',
    noticeKind: 'contextCompaction',
    body: 'Preserved the objective, plan, child work, approval, changed files, and verification receipts.',
  },
  {
    id: 'promoted-followup',
    kind: 'user',
    text: 'Measure the route bundle after the visual pass.',
  },
  {
    id: 'bundle-analysis',
    kind: 'command',
    command: './script/clippy && cargo test -p omega_deltas',
    cwd: '/work/omega',
    output:
      '✓ clippy passed\n✓ divergence checks passed\n✓ onboarding journey passed',
    status: 'completed',
  },
  {
    id: 'agents-settled',
    kind: 'agents',
    title: 'Agent lifecycle · settled',
    agents: [
      {
        agentKey: 'protocol-scout-final',
        name: 'protocol-scout',
        role: 'Explorer',
        detail: 'Identity and agent ownership boundaries verified',
        status: 'completed',
      },
      {
        agentKey: 'timeline-builder-final',
        name: 'timeline-builder',
        role: 'Frontend',
        detail: 'Identity-first onboarding journey complete',
        status: 'completed',
      },
      {
        agentKey: 'a11y-oracle-final',
        name: 'a11y-oracle',
        role: 'Nested reviewer',
        parent: 'timeline-builder',
        depth: 1,
        detail: 'Keyboard and status semantics verified',
        status: 'completed',
      },
    ],
  },
  {
    id: 'plan-complete',
    kind: 'plan',
    title: 'Plan updated',
    entries: [
      {
        step: 'Inspect the current identity and onboarding flow',
        status: 'completed',
      },
      {
        step: 'Map the change onto native project and editor state',
        status: 'completed',
      },
      {
        step: 'Implement the identity-first onboarding slice',
        status: 'completed',
      },
      {
        step: 'Run accessibility and journey verification',
        status: 'completed',
      },
      {
        step: 'Review the evidence and accept the result',
        status: 'completed',
      },
    ],
  },
]

const sessions: ReadonlyArray<DemoSession> = [
  {
    id: 'appserver',
    title: 'IDENTITY ONBOARDING',
    meta: '⌘1',
    items: appServerItems,
    reply: initialReply,
  },
  {
    id: 't3code-yoink',
    title: 'PROJECT REVIEW',
    meta: '⌘2',
    items: [
      {
        id: 'extract-user',
        kind: 'user',
        text: 'Connect this agent run to the exact project files, diff, and review.',
      },
      {
        id: 'extract-work',
        kind: 'work-group',
        text: 'Shared workbench extraction',
        count: 3,
      },
      {
        id: 'extract-assistant',
        kind: 'assistant',
        text: 'The workroom now opens the native project diff without creating a second file or Git record.',
      },
    ],
  },
  {
    id: 'arwes',
    title: 'FULL AUTO',
    meta: '⌘3',
    items: [
      {
        id: 'arwes-user',
        kind: 'user',
        text: 'Keep this run moving under hard limits, then bring me the evidence.',
      },
      { id: 'arwes-work', kind: 'work-group', text: 'Token audit', count: 2 },
      {
        id: 'arwes-assistant',
        kind: 'assistant',
        text: 'The run is bounded by its objective, turn cap, workspace, and typed stop conditions.',
      },
    ],
  },
  {
    id: 'website',
    title: 'AGENT HANDOFF',
    meta: '⌘4',
    items: [
      {
        id: 'website-user',
        kind: 'user',
        text: 'Use the best available agent for this task without moving its credentials.',
      },
      {
        id: 'website-assistant',
        kind: 'assistant',
        text: 'The selected agent keeps its own account, configuration, and runtime. This thread records which executor produced the result.',
      },
    ],
  },
  {
    id: 'docs',
    title: 'RELEASE CHECK',
    meta: '⌘5',
    items: [
      {
        id: 'docs-user',
        kind: 'user',
        text: 'Tell me what is proven before we describe this build as ready.',
      },
      {
        id: 'docs-assistant',
        kind: 'assistant',
        text: 'The build stays in development until its installed journey, brand, accessibility, and independent review gates all pass.',
      },
    ],
  },
  { id: 'untitled-6', title: 'Untitled work thread', meta: '⌘6', items: [] },
  { id: 'agent-computer', title: 'AGENT COMPUTER', meta: '⌘7', items: [] },
  { id: 'community', title: 'COMMUNITY WORKROOM', meta: '⌘8', items: [] },
  { id: 'untitled-9', title: 'Untitled work thread', meta: '⌘9', items: [] },
]

const destinations: ReadonlyArray<DesktopRailDestination> = [
  { id: 'workspace-new-chat', icon: 'new-session', label: 'New session' },
]

const settingsDestination: DesktopRailDestination = {
  accessibilityLabel: 'Open Settings',
  id: 'shell-settings-toggle',
  icon: 'settings',
  label: 'Settings',
}

const followupReplies = [
  'The new direction is queued without replacing the original objective. The active agent can finish its bounded step while the next instruction waits visibly.',
  'The work record stays honest: the native editor owns files and project state, while OpenAgents records the objective, executor, policy, evidence, and receipt.',
] as const

const splashQuestions = [
  [
    'What is Omega?',
    'Omega is the Zed-based OpenAgents IDE now in active development. It brings projects, editing, terminals, agents, review, and work records into one native application.',
  ],
  [
    'Which agents will Omega work with?',
    'Omega is designed to work with native and external agents, including agents you already use. Each external agent keeps its own account, credentials, configuration, and runtime.',
  ],
  [
    'What makes the work verifiable?',
    'Omega is being built to connect an objective to the agents, changes, tests, approvals, and receipts behind the result. You can inspect the evidence instead of trusting a confident summary.',
  ],
  [
    'What stays local?',
    'The native editor, project, and direct agent paths are designed to keep working without an OpenAgents cloud connection. Hosted routing, community workrooms, and cross-device features need services and must stop clearly when those services are unavailable.',
  ],
  [
    'Is Omega available yet?',
    'Not yet. Omega is in active development, and the latest candidate has not passed every release-readiness gate. The current Electron-based OpenAgents Desktop remains the supported application until Omega earns the cutover.',
  ],
] as const

const cleanStreamingText = (value: string): string => value.replace(/▌$/u, '')

const DemoTimelineItem = ({
  item,
  sequence,
}: Readonly<{ item: DemoItem; sequence: number }>) => {
  if (item.kind === 'work')
    return (
      <DesktopWorkEntry
        body={
          <pre>
            <code>{item.detail}</code>
          </pre>
        }
        itemKey={item.id}
        kind="reasoning"
        label={item.label}
        preview={item.detail}
        status={item.status}
      />
    )
  if (item.kind === 'work-group')
    return (
      <DesktopWorkGroup count={item.count}>
        <DesktopWorkEntry
          body={
            <pre>
              <code>{item.text}</code>
            </pre>
          }
          itemKey={`${item.id}:entry`}
          label="FileChange"
          preview={item.text}
        />
      </DesktopWorkGroup>
    )
  if (item.kind === 'plan')
    return (
      <DesktopPlanCard
        entries={item.entries}
        itemKey={item.id}
        title={item.title}
      />
    )
  if (item.kind === 'command')
    return <DesktopCommandCard {...item} itemKey={item.id} />
  if (item.kind === 'files')
    return <DesktopFileChangeCard {...item} itemKey={item.id} />
  if (item.kind === 'tool')
    return (
      <DesktopToolCallCard
        body={
          <pre>
            <code>{item.body}</code>
          </pre>
        }
        defaultOpen={item.defaultOpen}
        itemKey={item.id}
        label={item.label}
        meta={item.meta}
        status={item.status}
        summary={item.summary}
        toolKind={item.toolKind}
      />
    )
  if (item.kind === 'agents')
    return (
      <DesktopAgentGroup
        agents={item.agents}
        itemKey={item.id}
        title={item.title}
      />
    )
  if (item.kind === 'approval')
    return <DesktopApprovalCard {...item} itemKey={item.id} />
  if (item.kind === 'queue')
    return <DesktopQueuedFollowup {...item} itemKey={item.id} />
  if (item.kind === 'notice')
    return (
      <DesktopTimelineNotice
        body={item.body}
        danger={item.danger}
        itemKey={item.id}
        kind={item.noticeKind}
        label={item.label}
      />
    )
  return (
    <DesktopTimelineMessage
      itemKey={item.id}
      label={item.kind === 'user' ? 'You' : 'Assistant'}
      sequence={sequence}
      tone={item.kind === 'user' ? 'user' : 'assistant'}
    >
      <p>{item.text}</p>
    </DesktopTimelineMessage>
  )
}

export function SplashPage() {
  const initial = sessions[0]!
  const [activeId, setActiveId] = useState(initial.id)
  const [title, setTitle] = useState(initial.title)
  const [items, setItems] = useState<ReadonlyArray<DemoItem>>(initial.items)
  const [draft, setDraft] = useState('')
  const [fullAuto, setFullAuto] = useState(true)
  const [phase, setPhase] = useState<PlaybackPhase>('preparing')
  const [playback, setPlayback] = useState<Playback | null>({
    id: 0,
    text: initial.reply!,
  })
  const [streamingText, setStreamingText] = useState('')
  const [railOpen, setRailOpen] = useState(false)
  const [searchOpen, setSearchOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [demoInteractive, setDemoInteractive] = useState(false)
  const timerIds = useRef<ReadonlyArray<number>>([])
  const playbackId = useRef(1)
  const followupIndex = useRef(0)

  const clearPlayback = useCallback(() => {
    timerIds.current.forEach(timerId => window.clearTimeout(timerId))
    timerIds.current = []
  }, [])

  useEffect(() => {
    if (playback === null) return
    clearPlayback()
    setStreamingText('')
    setPhase('preparing')
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      setStreamingText(playback.text)
      setPhase('complete')
      return
    }
    const frames = makeKhalaTextSequenceFrames(playback.text, {
      caret: true,
      frames: 96,
    })
    const startTimer = window.setTimeout(() => {
      setPhase('streaming')
      frames.forEach((frame, index) => {
        const frameTimer = window.setTimeout(() => {
          setStreamingText(frame.visualText)
          if (index === frames.length - 1) setPhase('complete')
        }, index * 18)
        timerIds.current = [...timerIds.current, frameTimer]
      })
    }, 400)
    timerIds.current = [startTimer]
    return clearPlayback
  }, [clearPlayback, playback])

  const beginPlayback = (text: string): void => {
    playbackId.current += 1
    setPlayback({ id: playbackId.current, text })
  }

  const selectSession = (id: string): void => {
    const session = sessions.find(candidate => candidate.id === id)
    if (session === undefined) return
    clearPlayback()
    setActiveId(session.id)
    setTitle(session.title)
    setItems(session.items)
    setStreamingText('')
    setRailOpen(false)
    if (session.reply === undefined) {
      setPlayback(null)
      setPhase('idle')
    } else beginPlayback(session.reply)
  }

  const newSession = (): void => {
    clearPlayback()
    setActiveId('new')
    setTitle('New session')
    setItems([])
    setPlayback(null)
    setStreamingText('')
    setPhase('idle')
    setRailOpen(false)
    window.setTimeout(
      () =>
        document
          .querySelector<HTMLTextAreaElement>('#splash-composer-input')
          ?.focus(),
      0,
    )
  }

  const stopPlayback = (): void => {
    clearPlayback()
    setStreamingText(value => cleanStreamingText(value))
    setPhase('stopped')
  }

  const submitMessage = (event?: FormEvent): void => {
    event?.preventDefault()
    const value = draft.trim()
    if (value === '') return
    clearPlayback()
    const priorReply = cleanStreamingText(streamingText)
    setItems(current => [
      ...current,
      ...(priorReply === ''
        ? []
        : [
            {
              id: `assistant-${playbackId.current}`,
              kind: 'assistant' as const,
              text: priorReply,
            },
          ]),
      { id: `user-${playbackId.current + 1}`, kind: 'user', text: value },
    ])
    setDraft('')
    setStreamingText('')
    const reply =
      followupReplies[followupIndex.current % followupReplies.length]!
    followupIndex.current += 1
    beginPlayback(reply)
  }

  const visibleSessions = sessions.filter(session =>
    session.title
      .toLocaleLowerCase()
      .includes(query.trim().toLocaleLowerCase()),
  )
  const active = phase === 'preparing' || phase === 'streaming'
  const releaseDemoWheel = (event: WheelEvent<HTMLDivElement>): void => {
    const target = event.target
    if (!(target instanceof Element)) return
    const scroller = target.closest<HTMLElement>(
      '.oa-react-timeline-scroll, .oa-react-session-scroll',
    )
    if (scroller === null) return
    const atTop = scroller.scrollTop <= 1
    const atBottom =
      scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 1
    if ((event.deltaY < 0 && atTop) || (event.deltaY > 0 && atBottom)) {
      event.preventDefault()
      window.scrollBy({ behavior: 'auto', top: event.deltaY })
    }
  }

  return (
    <div
      className="splash-page"
      data-route="splash"
      style={desktopThemeCssVariables(khalaTheme)}
    >
      <PublicHeader />

      <section aria-labelledby="splash-heading" className="splash-hero">
        <SplashHeroCanvas />
        <span className="splash-development-status">
          <i aria-hidden="true" />
          In development
        </span>
        <h1 id="splash-heading">Your last agent IDE.</h1>
        <p>
          Omega brings your project, agents, code, review, and evidence into one
          fast, native workspace.
        </p>
        <a
          className="splash-primary-action"
          href={OMEGA_REPOSITORY_URL}
          onClick={() =>
            trackNamedAnalyticsEvent('github_view', window.location.pathname)
          }
          rel="noreferrer"
          target="_blank"
        >
          View on GitHub
          <ArrowUpRight aria-hidden="true" />
        </a>
      </section>

      <figure className="splash-product" id="product">
        <div aria-hidden="true" className="splash-window-bar">
          <span className="splash-window-dots">
            <i />
            <i />
            <i />
          </span>
          <span>Omega</span>
          <span className="splash-window-status">
            <i />
            In active development
          </span>
        </div>
        <div
          className="splash-demo-frame"
          data-interactive={demoInteractive ? 'true' : 'false'}
          onWheelCapture={demoInteractive ? releaseDemoWheel : undefined}
        >
          {demoInteractive ? null : (
            <button
              aria-label="Activate the Omega product direction demo"
              className="splash-demo-activation"
              onClick={() => setDemoInteractive(true)}
              type="button"
            >
              <span>Click to interact</span>
            </button>
          )}
          <DesktopWorkbench aria-label="Omega interactive product direction">
            <DesktopSidebarExpand
              aria-expanded={railOpen}
              aria-label="Expand sidebar"
              onClick={() => setRailOpen(true)}
              title="Expand sidebar"
            />
            <DesktopSessionRail
              canGoBack
              canGoForward={false}
              destinations={destinations}
              onBack={() => undefined}
              onCollapse={() => setRailOpen(false)}
              onDestinationSelect={destination =>
                destination.id === 'workspace-new-chat'
                  ? newSession()
                  : undefined
              }
              onForward={() => undefined}
              onSearchOpenChange={setSearchOpen}
              onSearchQueryChange={setQuery}
              onSessionSelect={session => selectSession(session.id)}
              open={railOpen}
              searchOpen={searchOpen}
              searchQuery={query}
              sessions={visibleSessions.map(session => ({
                ...session,
                selected: session.id === activeId,
              }))}
              settingsDestination={settingsDestination}
              stageLabel="IN DEV"
            />
            {railOpen ? (
              <DesktopRailScrim
                aria-label="Close sessions"
                onClick={() => setRailOpen(false)}
              />
            ) : null}
            <DesktopConversation
              composer={
                <DesktopComposerFrame aria-label="Message composer">
                  <DesktopComposerInput>
                    <textarea
                      aria-label={
                        active ? 'Steer the current work' : 'Describe the work'
                      }
                      id="splash-composer-input"
                      onChange={event => setDraft(event.currentTarget.value)}
                      onKeyDown={event => {
                        if (
                          event.key !== 'Enter' ||
                          event.shiftKey ||
                          event.nativeEvent.isComposing
                        )
                          return
                        event.preventDefault()
                        submitMessage()
                      }}
                      placeholder={
                        active
                          ? 'Steer the current work…'
                          : 'What should happen?'
                      }
                      rows={2}
                      value={draft}
                    />
                    <DesktopComposerBar>
                      <DesktopComposerButton
                        aria-label="Attach image"
                        kind="action"
                      >
                        <ImagePlus aria-hidden="true" />
                      </DesktopComposerButton>
                      <DesktopComposerButton
                        aria-label="Open commands"
                        kind="action"
                      >
                        <Command aria-hidden="true" />
                      </DesktopComposerButton>
                      <DesktopComposerButton
                        aria-label={
                          fullAuto ? 'Turn off Full Auto' : 'Turn on Full Auto'
                        }
                        aria-pressed={fullAuto}
                        kind="toggle"
                        onClick={() => setFullAuto(value => !value)}
                      >
                        <Zap aria-hidden="true" />
                        Full Auto
                      </DesktopComposerButton>
                      <span className="oa-react-composer-spacer" />
                      {active ? (
                        <DesktopComposerButton
                          aria-label="Stop current turn"
                          kind="stop"
                          onClick={stopPlayback}
                        >
                          <Square aria-hidden="true" />
                        </DesktopComposerButton>
                      ) : null}
                      <DesktopComposerButton
                        aria-label="Send"
                        disabled={draft.trim() === ''}
                        kind="submit"
                        onClick={() => submitMessage()}
                      >
                        <ArrowUp aria-hidden="true" />
                      </DesktopComposerButton>
                    </DesktopComposerBar>
                  </DesktopComposerInput>
                </DesktopComposerFrame>
              }
              header={
                <DesktopConversationHeader
                  lifecycle={active ? 'Running' : 'Ready'}
                  secondary="product direction · interactive preview"
                  title={title}
                />
              }
              timeline={
                <DesktopTimeline
                  followKey={`${items.length}:${streamingText.length}:${phase}`}
                  working={phase === 'preparing'}
                >
                  {items.length === 0 ? (
                    <div className="splash-empty">
                      <h2>Start a work thread</h2>
                      <p>Choose a project, then describe the outcome.</p>
                    </div>
                  ) : (
                    items.map((item, index) => (
                      <DemoTimelineItem
                        item={item}
                        key={item.id}
                        sequence={index}
                      />
                    ))
                  )}
                  {playback === null || phase === 'preparing' ? null : (
                    <DesktopTimelineMessage
                      itemKey={`playback-${playback.id}`}
                      label="Assistant"
                      sequence={items.length}
                      tone="assistant"
                    >
                      <p aria-hidden="true">{streamingText}</p>
                      <span className="oa-react-sr-only">{playback.text}</span>
                    </DesktopTimelineMessage>
                  )}
                </DesktopTimeline>
              }
            />
          </DesktopWorkbench>
        </div>
        <figcaption className="oa-react-sr-only">
          An interactive preview of Omega product direction, rendered with
          shared workroom components.
        </figcaption>
      </figure>

      <section aria-labelledby="splash-faq-title" className="splash-faq">
        <div className="splash-faq-intro">
          <p>Questions and answers</p>
          <h2 id="splash-faq-title">What Omega is—and where it’s going.</h2>
          <InternalLink href={DOCS_URL} preload="render">
            Read the full documentation <ArrowRight aria-hidden="true" />
          </InternalLink>
        </div>
        <div className="splash-question-list">
          {splashQuestions.map(([question, answer], index) => (
            <details key={question} open={index === 0}>
              <summary>
                {question}
                <span aria-hidden="true">＋</span>
              </summary>
              <p>{answer}</p>
            </details>
          ))}
        </div>
      </section>

      <PublicFooter />
    </div>
  )
}
