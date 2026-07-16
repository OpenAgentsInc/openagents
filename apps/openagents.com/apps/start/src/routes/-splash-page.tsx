import { InternalLink } from '@/components/internal-link'
import GithubMark from '@/components/launch-ui/logos/github'
import { PublicFooter } from '@/components/public-footer'
import { PublicHeader } from '@/components/public-header'
import {
  DOCS_URL,
  DOWNLOAD_URL,
  GITHUB_REPOSITORY_URL,
} from '@/lib/public-site'
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
  'The full app-server turn is now projected as a coherent workroom instead of a pile of logs. Three delegated agents completed in parallel—including a nested accessibility child—while the parent streamed plans, commands, patches, approvals, tool progress, context events, and token-aware completion. The final checks are green: 62 focused tests, all shared UI typechecks, and a clean production build.'

const appServerItems: ReadonlyArray<DemoItem> = [
  {
    id: 'brief',
    kind: 'user',
    text: 'Show the whole Codex app-server workflow in the product: plans, tool progress, approvals, patches, subagents, steering, and a real streaming result. Make it feel like one coherent workroom.',
  },
  {
    id: 'acknowledge',
    kind: 'assistant',
    text: 'I’ll trace the generated protocol and the Desktop projections first, then split the implementation into independent lanes. I’ll keep the parent turn responsive while the child threads work.',
  },
  {
    id: 'plan-start',
    kind: 'plan',
    entries: [
      {
        step: 'Trace the current ThreadItem and notification unions',
        status: 'completed',
      },
      {
        step: 'Audit Desktop plan, tool, approval, and child projections',
        status: 'completed',
      },
      {
        step: 'Build shared app-server presentation components',
        status: 'in_progress',
      },
      {
        step: 'Run renderer, accessibility, and route verification',
        status: 'pending',
      },
      { step: 'Review the finished workroom end to end', status: 'pending' },
    ],
  },
  {
    id: 'reasoning-map',
    kind: 'work',
    label: 'Reasoning summary',
    status: 'completed',
    detail:
      'The current source exposes plan, reasoning, commandExecution, fileChange, mcpToolCall, dynamicToolCall, collabAgentToolCall, subAgentActivity, webSearch, imageView, approval, and contextCompaction events. The UI should preserve those distinctions without exposing raw protocol envelopes.',
  },
  {
    id: 'inspect-protocol',
    kind: 'command',
    command:
      'rg -n "collabAgentToolCall|contextCompaction|requestApproval" packages apps/openagents-desktop',
    cwd: '/work/openagents',
    output:
      'current-source-thread-items.json:62  collabAgentToolCall\ncodex-app-server-turn.ts:396       registerChild(receiver, threadId, prompt)\nmeta.gen.ts:2140                   item/commandExecution/requestApproval\nmeta.gen.ts:2684                   item/reasoning/textDelta\n18 protocol projections matched',
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
        detail: 'Inventory app-server items and lifecycle notifications',
        status: 'completed',
        transcript: [
          {
            label: 'spawnAgent',
            text: 'Trace the current generated protocol and report every UI-relevant item.',
          },
          {
            label: 'Update',
            text: 'Mapped 18 ThreadItem variants and the corresponding delta/progress notifications.',
          },
          {
            label: 'Result',
            text: 'Command, patch, MCP, collaboration, search, image, approval, and compaction are all first-class.',
          },
        ],
      },
      {
        agentKey: 'timeline-builder',
        name: 'timeline-builder',
        role: 'Frontend',
        detail: 'Build dense Khala timeline cards in the shared package',
        status: 'running',
        transcript: [
          {
            label: 'spawnAgent',
            text: 'Own the shared workbench components and preserve Desktop geometry.',
          },
          {
            label: 'FileChange',
            text: 'Added plan, command, patch, tool, approval, queue, and delegated-agent cards.',
          },
          {
            label: 'Result',
            text: 'Web and Desktop can now consume one controlled component vocabulary.',
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
        path: 'packages/ui/src/desktop-workbench.tsx',
        additions: 284,
        deletions: 12,
      },
      {
        kind: 'modified',
        path: 'packages/ui/src/desktop-workbench.css',
        additions: 191,
        deletions: 8,
      },
      {
        kind: 'modified',
        path: 'apps/openagents.com/apps/start/src/routes/-splash-page.tsx',
        additions: 326,
        deletions: 48,
      },
      {
        kind: 'modified',
        path: 'apps/openagents-desktop/src/renderer/react-timeline.tsx',
        additions: 18,
        deletions: 13,
      },
    ],
    status: 'completed',
  },
  {
    id: 'focused-tests',
    kind: 'command',
    command: 'pnpm exec vitest run src/routes/-splash.test.tsx',
    cwd: '/work/openagents/apps/openagents.com/apps/start',
    output:
      '✓ src/routes/-splash.test.tsx (2 tests) 41ms\n\nTest Files  1 passed (1)\nTests       2 passed (2)',
    status: 'completed',
  },
  {
    id: 'build-approval',
    kind: 'approval',
    decision: 'approved',
    title: 'Command approval',
    description:
      'Run the bounded production build to verify the shared UI package in both hosts.',
    resource: 'pnpm --dir apps/openagents-desktop run build',
  },
  {
    id: 'desktop-build',
    kind: 'command',
    command: 'pnpm --dir apps/openagents-desktop run build',
    cwd: '/work/openagents',
    output:
      'renderer bundle     412.7 kB │ gzip: 121.9 kB\nmain process         188.4 kB │ gzip:  52.1 kB\n✓ built in 2.14s',
    status: 'completed',
  },
  {
    id: 'browser-inspect',
    kind: 'tool',
    toolKind: 'mcp',
    label: 'browser · inspect',
    summary: 'Checked the live /splash workroom',
    meta: 'mcpToolCall',
    status: 'completed',
    body: 'Conversation timeline: 17 items\nDelegated agents: 3 rows, 3 expandable transcripts\nComposer actions: 24 × 24 px\nHorizontal overflow: none',
  },
  {
    id: 'web-search',
    kind: 'tool',
    toolKind: 'web',
    label: 'Web search',
    summary: 'TanStack Start route preloading and navigation',
    meta: 'webSearch',
    status: 'completed',
    body: '2 primary documentation results inspected. The route remains client-navigated and preloaded.',
  },
  {
    id: 'image-view',
    kind: 'tool',
    toolKind: 'image',
    label: 'Image view',
    summary: 'Compared Desktop and web workroom geometry',
    meta: 'imageView',
    status: 'completed',
    body: 'Desktop reference: 1920 × 1280\nLocal /splash: 1512 × 982\nRail, timeline, and composer density remain proportional.',
  },
  {
    id: 'compaction',
    kind: 'notice',
    label: 'Context compacted',
    noticeKind: 'contextCompaction',
    body: 'Preserved the active plan, child topology, approval decision, changed files, and verification receipts.',
  },
  {
    id: 'promoted-followup',
    kind: 'user',
    text: 'Measure the route bundle after the visual pass.',
  },
  {
    id: 'bundle-analysis',
    kind: 'command',
    command: 'pnpm run build && pnpm run typecheck',
    cwd: '/work/openagents/apps/openagents.com/apps/start',
    output:
      'client route chunk    31.8 kB │ gzip: 9.7 kB\nshared workbench      18.6 kB │ gzip: 5.4 kB\n✓ typecheck passed\n✓ production build passed',
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
        detail: '18 protocol projections inventoried',
        status: 'completed',
      },
      {
        agentKey: 'timeline-builder-final',
        name: 'timeline-builder',
        role: 'Frontend',
        detail: 'Shared workbench components and host adapters complete',
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
        step: 'Trace the current ThreadItem and notification unions',
        status: 'completed',
      },
      {
        step: 'Audit Desktop plan, tool, approval, and child projections',
        status: 'completed',
      },
      {
        step: 'Build shared app-server presentation components',
        status: 'completed',
      },
      {
        step: 'Run renderer, accessibility, and route verification',
        status: 'completed',
      },
      { step: 'Review the finished workroom end to end', status: 'completed' },
    ],
  },
]

const sessions: ReadonlyArray<DemoSession> = [
  {
    id: 'appserver',
    title: 'APPSERVER',
    meta: '⌘1',
    items: appServerItems,
    reply: initialReply,
  },
  {
    id: 't3code-yoink',
    title: 'T3CODE YOINK',
    meta: '⌘2',
    items: [
      {
        id: 'extract-user',
        kind: 'user',
        text: 'Use the actual Desktop components on web so the two surfaces cannot drift.',
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
        text: 'The controlled primitives now live in packages/ui and both hosts consume the same geometry.',
      },
    ],
  },
  {
    id: 'arwes',
    title: 'ARWES',
    meta: '⌘3',
    items: [
      {
        id: 'arwes-user',
        kind: 'user',
        text: 'Keep the Khala color and motion language restrained.',
      },
      { id: 'arwes-work', kind: 'work-group', text: 'Token audit', count: 2 },
      {
        id: 'arwes-assistant',
        kind: 'assistant',
        text: 'The shared workbench consumes the same Effect Native token sheet in both hosts.',
      },
    ],
  },
  {
    id: 'website',
    title: 'WEBSITE',
    meta: '⌘4',
    items: [
      {
        id: 'website-user',
        kind: 'user',
        text: 'Make public navigation instant and keep the landing page focused.',
      },
      {
        id: 'website-assistant',
        kind: 'assistant',
        text: 'The public surface stays in TanStack Start; this route embeds the shared Desktop workbench presentation.',
      },
    ],
  },
  {
    id: 'docs',
    title: 'DOCS',
    meta: '⌘5',
    items: [
      {
        id: 'docs-user',
        kind: 'user',
        text: 'Keep the documentation inside the same fast application shell.',
      },
      {
        id: 'docs-assistant',
        kind: 'assistant',
        text: 'Docs, blog, website, and app routes share the TanStack application authority.',
      },
    ],
  },
  { id: 'untitled-6', title: 'Untitled Codex chat', meta: '⌘6', items: [] },
  { id: 'grokcli', title: 'GrokCLI', meta: '⌘7', items: [] },
  { id: 'reposync', title: 'reposync', meta: '⌘8', items: [] },
  { id: 'untitled-9', title: 'Untitled Codex chat', meta: '⌘9', items: [] },
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
  'Steering landed in the active turn without replacing the parent context. The fixture can now add a queued follow-up, update a child transcript, or interrupt a running delegate while the main response continues.',
  'The event stays capability-truthful: app-server owns lifecycle and identity, while the shared workbench only projects typed state into plan, tool, approval, patch, and agent components.',
] as const

const splashQuestions = [
  [
    'Does OpenAgents replace Codex?',
    'No. Codex remains the engine and source of truth. OpenAgents Desktop adds a durable workroom around the session you already use.',
  ],
  [
    'Do I need an OpenAgents account?',
    'Not for the Desktop MVP. It uses your ordinary logged-in Codex session and keeps the core workroom local-first.',
  ],
  [
    'Can the review UI change my files?',
    'No. Repository status and diff views are deliberately read-only. Changes still happen through the active agent turn, where cause and result remain visible.',
  ],
  [
    'What happens after a restart or interrupted turn?',
    'OpenAgents restores stable session identity, then reconciles the latest known turn state. It does not silently replay tools or pretend interrupted work completed.',
  ],
  [
    'What is available today?',
    'The current release candidate is available for Apple silicon Macs; the download page shows the exact version and platform availability from the signed release feed. OpenAgents Desktop is still an MVP, so the download and documentation describe the supported boundary precisely.',
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
        <InternalLink
          className="splash-release-link"
          href="/blog/introducing-openagents-desktop"
          preload="render"
        >
          Introducing OpenAgents Desktop
          <ArrowRight aria-hidden="true" />
        </InternalLink>
        <h1 id="splash-heading">Your last agent IDE.</h1>
        <p>
          Plan, delegate, review, and steer coding work from one local-first
          desktop workroom.
        </p>
        <InternalLink
          className="splash-primary-action"
          href={DOWNLOAD_URL}
          preload="intent"
        >
          Download for Mac
          <ArrowRight aria-hidden="true" />
        </InternalLink>
        <a
          className="splash-source-link"
          href={GITHUB_REPOSITORY_URL}
          rel="noreferrer"
          target="_blank"
        >
          <GithubMark aria-hidden="true" />
          Or build from source
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
          <span>OpenAgents Desktop</span>
          <span className="splash-window-status">
            <i />
            Codex connected
          </span>
        </div>
        <div
          className="splash-demo-frame"
          data-interactive={demoInteractive ? 'true' : 'false'}
          onWheelCapture={demoInteractive ? releaseDemoWheel : undefined}
        >
          {demoInteractive ? null : (
            <button
              aria-label="Activate the OpenAgents Desktop demo"
              className="splash-demo-activation"
              onClick={() => setDemoInteractive(true)}
              type="button"
            >
              <span>Click to interact</span>
            </button>
          )}
          <DesktopWorkbench aria-label="OpenAgents Desktop live product preview">
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
              stageLabel="ALPHA"
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
                        active ? 'Steer a Codex message' : 'Message Codex'
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
                        active ? 'Steer the current turn…' : 'Message Codex…'
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
                  secondary="openagents / main · gpt-5.6-sol"
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
                      <h2>Start a conversation with Codex</h2>
                      <p>Choose a workspace, then send a message.</p>
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
          A live, interactive OpenAgents Desktop workroom rendered with the
          shared production components.
        </figcaption>
      </figure>

      <section aria-labelledby="splash-faq-title" className="splash-faq">
        <div className="splash-faq-intro">
          <p>Questions and answers</p>
          <h2 id="splash-faq-title">The important boundaries, plainly.</h2>
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
