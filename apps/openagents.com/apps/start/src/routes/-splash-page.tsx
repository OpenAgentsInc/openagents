import { makeKhalaTextSequenceFrames } from '@effect-native/khala-ui'
import { khalaTheme } from '@effect-native/tokens'
import {
  Check,
  ChevronDown,
  FileCode2,
  GitBranch,
  Menu,
  PanelLeft,
  RotateCcw,
  Search,
  Send,
  Square,
  SquarePen,
  X,
} from 'lucide-react'
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
} from 'react'

import { InternalLink } from '@/components/internal-link'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import '../splash.css'

type ConversationItem = Readonly<{
  id: string
  kind: 'assistant' | 'tool' | 'user'
  text: string
  detail?: string
  status?: 'done' | 'running'
}>

type DemoSession = Readonly<{
  id: string
  title: string
  meta: string
  items: ReadonlyArray<ConversationItem>
  reply?: string
}>

type Playback = Readonly<{ id: number; text: string }>
type PlaybackPhase = 'complete' | 'idle' | 'preparing' | 'stopped' | 'streaming'

const firstReply =
  'I’ve mounted the session rail, conversation timeline, review state, and composer as live components. This response is streaming inside the page now—there is no screenshot to swap later.'

const sessions: ReadonlyArray<DemoSession> = [
  {
    id: 'splash',
    title: 'Live product splash',
    meta: 'Streaming · now',
    items: [
      {
        id: 'splash-user-1',
        kind: 'user',
        text: 'Build a new /splash page from the real Desktop workroom. No screenshot. Show the product actually working.',
      },
      {
        id: 'splash-assistant-1',
        kind: 'assistant',
        text: 'I found the shared Khala tokens and the Desktop session, timeline, composer, and review patterns. I’m rebuilding the scene as a live route.',
      },
      {
        id: 'splash-tool-1',
        kind: 'tool',
        status: 'done',
        text: 'Inspected Desktop renderer',
        detail: 'react-primitive-adapters.tsx · react-timeline.tsx · react-composer.tsx',
      },
      {
        id: 'splash-user-2',
        kind: 'user',
        text: 'Good. Make the reconstruction itself the whole page, and let the response stream.',
      },
    ],
    reply: firstReply,
  },
  {
    id: 'navigation',
    title: 'Instant public navigation',
    meta: 'Complete · 8m',
    items: [
      {
        id: 'navigation-user',
        kind: 'user',
        text: 'The OpenAgents logo on the blog is doing a full page reload. Keep public navigation fast.',
      },
      {
        id: 'navigation-tool',
        kind: 'tool',
        status: 'done',
        text: 'Converted shared document links',
        detail: 'Client routing · route preloading · native modified-click behavior',
      },
      {
        id: 'navigation-assistant',
        kind: 'assistant',
        text: 'Public website, blog, install, and docs links now stay inside the TanStack router and preload their next route.',
      },
    ],
  },
  {
    id: 'desktop',
    title: 'Desktop release acceptance',
    meta: 'Complete · 43m',
    items: [
      {
        id: 'desktop-user',
        kind: 'user',
        text: 'Run the Desktop release acceptance and keep the evidence attached to the work.',
      },
      {
        id: 'desktop-tool',
        kind: 'tool',
        status: 'done',
        text: 'Release evidence collected',
        detail: 'Typecheck · renderer tests · Electron smoke · macOS package',
      },
      {
        id: 'desktop-assistant',
        kind: 'assistant',
        text: 'The candidate passed the bounded acceptance gates and the evidence remains linked to this session.',
      },
    ],
  },
]

const followupReplies = [
  'The composer is live too. Your message was added to the timeline, and this mocked Codex turn is using the same bounded Khala text choreography as the product surface.',
  'That interaction stayed on this page. The workroom can keep the active turn, review state, and session context together without flattening the product into an image.',
] as const

const themeStyle = {
  '--splash-accent': khalaTheme.color.accent,
  '--splash-background': khalaTheme.color.background,
  '--splash-border': khalaTheme.color.border,
  '--splash-border-strong': khalaTheme.color.borderStrong,
  '--splash-code': khalaTheme.color.codeBackground,
  '--splash-focus': khalaTheme.color.focus,
  '--splash-info': khalaTheme.color.info,
  '--splash-raised': khalaTheme.color.surfaceRaised,
  '--splash-surface': khalaTheme.color.surface,
  '--splash-text': khalaTheme.color.textPrimary,
  '--splash-text-faint': khalaTheme.color.textFaint,
  '--splash-text-muted': khalaTheme.color.textMuted,
} as CSSProperties & Readonly<Record<`--splash-${string}`, string>>

const cleanStreamingText = (value: string): string => value.replace(/▌$/u, '')

function SessionRail({
  activeId,
  onDismiss,
  onNewChat,
  onSelect,
  open,
}: Readonly<{
  activeId: string
  onDismiss: () => void
  onNewChat: () => void
  onSelect: (session: DemoSession) => void
  open: boolean
}>) {
  const [query, setQuery] = useState('')
  const normalizedQuery = query.trim().toLocaleLowerCase()
  const visibleSessions = sessions.filter(session =>
    session.title.toLocaleLowerCase().includes(normalizedQuery),
  )

  return (
    <aside aria-label="Sessions" className="splash-session-rail" data-open={open}>
      <div className="splash-rail-topline">
        <Button aria-label="Close sessions" className="splash-mobile-close" onClick={onDismiss} size="sm" variant="ghost">
          <X aria-hidden="true" />
        </Button>
        <InternalLink aria-label="OpenAgents home" className="splash-brand" href="/" preload="render">
          <span aria-hidden="true" className="splash-brand-mark" />
          OpenAgents
        </InternalLink>
      </div>
      <Button className="splash-new-chat" onClick={onNewChat} size="sm" variant="secondary">
        <SquarePen aria-hidden="true" />
        New session
      </Button>
      <label className="splash-session-search">
        <span className="sr-only">Search sessions</span>
        <Search aria-hidden="true" />
        <input
          onChange={event => setQuery(event.target.value)}
          placeholder="Search sessions"
          type="search"
          value={query}
        />
      </label>
      <div className="splash-session-group">
        <p>Recent</p>
        {visibleSessions.map(session => (
          <button
            className="splash-session-row"
            data-selected={activeId === session.id}
            key={session.id}
            onClick={() => onSelect(session)}
            type="button"
          >
            <strong>{session.title}</strong>
            <span>{session.meta}</span>
          </button>
        ))}
        {visibleSessions.length === 0 ? <span className="splash-no-sessions">No matching sessions.</span> : null}
      </div>
      <div className="splash-rail-footer">
        <span aria-hidden="true" className="splash-avatar">OA</span>
        <span><strong>Local workspace</strong><small>openagents · main</small></span>
        <Menu aria-hidden="true" />
      </div>
    </aside>
  )
}

function TimelineItemView({ item }: Readonly<{ item: ConversationItem }>) {
  if (item.kind === 'user') {
    return <article className="splash-user-message"><span className="sr-only">You: </span>{item.text}</article>
  }
  if (item.kind === 'tool') {
    return (
      <details className="splash-tool-entry">
        <summary>
          <span aria-hidden="true" className="splash-tool-icon"><Check /></span>
          <span><strong>{item.text}</strong><small>{item.detail}</small></span>
          <Badge variant={item.status === 'running' ? 'running' : 'ready'}>{item.status === 'running' ? 'Running' : 'Done'}</Badge>
          <ChevronDown aria-hidden="true" />
        </summary>
        <pre><code>{item.detail}</code></pre>
      </details>
    )
  }
  return (
    <article className="splash-assistant-message">
      <header><span className="splash-agent-mark" aria-hidden="true">K</span><strong>Codex</strong></header>
      <p>{item.text}</p>
    </article>
  )
}

function StreamingReply({ phase, text, semanticText }: Readonly<{
  phase: PlaybackPhase
  semanticText: string
  text: string
}>) {
  if (phase === 'idle') return null
  return (
    <article aria-busy={phase === 'preparing' || phase === 'streaming'} className="splash-assistant-message splash-streaming-message">
      <header>
        <span className="splash-agent-mark" aria-hidden="true">K</span>
        <strong>Codex</strong>
        <Badge variant={phase === 'complete' ? 'ready' : phase === 'stopped' ? 'warning' : 'running'}>
          {phase === 'complete' ? 'Complete' : phase === 'stopped' ? 'Stopped' : phase === 'preparing' ? 'Reading' : 'Streaming'}
        </Badge>
      </header>
      <p aria-hidden="true">{phase === 'preparing' ? 'Reading the workroom state…' : text}</p>
      <span className="sr-only">{semanticText}</span>
    </article>
  )
}

function Composer({
  draft,
  onDraftChange,
  onStop,
  onSubmit,
  phase,
}: Readonly<{
  draft: string
  onDraftChange: (value: string) => void
  onStop: () => void
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  phase: PlaybackPhase
}>) {
  const active = phase === 'preparing' || phase === 'streaming'
  return (
    <form className="splash-composer" onSubmit={onSubmit}>
      <label className="sr-only" htmlFor="splash-composer-input">Message Codex</label>
      <textarea
        id="splash-composer-input"
        onChange={event => onDraftChange(event.target.value)}
        onKeyDown={event => {
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault()
            event.currentTarget.form?.requestSubmit()
          }
        }}
        placeholder={active ? 'Steer the current turn…' : 'Message Codex…'}
        rows={2}
        value={draft}
      />
      <div className="splash-composer-meta">
        <span><GitBranch aria-hidden="true" /> openagents / main</span>
        <span className="splash-model">Codex · GPT-5</span>
        {active ? (
          <Button aria-label="Stop current turn" className="splash-send-button" onClick={onStop} size="sm" type="button">
            <Square aria-hidden="true" fill="currentColor" />
          </Button>
        ) : (
          <Button aria-label="Send message" className="splash-send-button" disabled={draft.trim() === ''} size="sm" type="submit">
            <Send aria-hidden="true" />
          </Button>
        )}
      </div>
    </form>
  )
}

function ReviewPanel({ onClose, open }: Readonly<{ onClose: () => void; open: boolean }>) {
  return (
    <aside aria-label="Changes" aria-hidden={!open} className="splash-review-panel" data-open={open}>
      <header>
        <div><strong>Review changes</strong><span>3 files · +412 −18</span></div>
        <Button aria-label="Close review" onClick={onClose} size="sm" variant="ghost"><X aria-hidden="true" /></Button>
      </header>
      <div className="splash-review-summary"><span aria-hidden="true" /><p>Live workroom reconstruction ready for review.</p></div>
      <ul>
        <li><FileCode2 aria-hidden="true" /><span><strong>-splash-page.tsx</strong><small>Interactive workroom surface</small></span><b>+286</b></li>
        <li><FileCode2 aria-hidden="true" /><span><strong>splash.css</strong><small>Responsive Khala presentation</small></span><b>+126</b></li>
        <li><FileCode2 aria-hidden="true" /><span><strong>splash.tsx</strong><small>TanStack route</small></span><b>+18</b></li>
      </ul>
      <footer><Button onClick={onClose} size="sm" variant="secondary">Back to conversation</Button></footer>
    </aside>
  )
}

export function SplashPage() {
  const initial = sessions[0]!
  const [activeId, setActiveId] = useState(initial.id)
  const [title, setTitle] = useState(initial.title)
  const [items, setItems] = useState<ReadonlyArray<ConversationItem>>(initial.items)
  const [draft, setDraft] = useState('')
  const [phase, setPhase] = useState<PlaybackPhase>('preparing')
  const [playback, setPlayback] = useState<Playback | null>({ id: 0, text: initial.reply! })
  const [streamingText, setStreamingText] = useState('')
  const [railOpen, setRailOpen] = useState(false)
  const [reviewOpen, setReviewOpen] = useState(false)
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

    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    if (reducedMotion) {
      setStreamingText(playback.text)
      setPhase('complete')
      return
    }

    const frames = makeKhalaTextSequenceFrames(playback.text, { caret: true, frames: 72 })
    const startTimer = window.setTimeout(() => {
      setPhase('streaming')
      frames.forEach((frame, index) => {
        const frameTimer = window.setTimeout(() => {
          setStreamingText(frame.visualText)
          if (index === frames.length - 1) setPhase('complete')
        }, index * 24)
        timerIds.current = [...timerIds.current, frameTimer]
      })
    }, 420)
    timerIds.current = [startTimer]
    return clearPlayback
  }, [clearPlayback, playback])

  const beginPlayback = (text: string): void => {
    playbackId.current += 1
    setPlayback({ id: playbackId.current, text })
  }

  const selectSession = (session: DemoSession): void => {
    clearPlayback()
    setActiveId(session.id)
    setTitle(session.title)
    setItems(session.items)
    setStreamingText('')
    setRailOpen(false)
    setReviewOpen(false)
    if (session.reply === undefined) {
      setPlayback(null)
      setPhase('idle')
    } else {
      beginPlayback(session.reply)
    }
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
    window.setTimeout(() => document.querySelector<HTMLTextAreaElement>('#splash-composer-input')?.focus(), 0)
  }

  const stopPlayback = (): void => {
    clearPlayback()
    setStreamingText(value => cleanStreamingText(value))
    setPhase('stopped')
  }

  const submitMessage = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault()
    const value = draft.trim()
    if (value === '') return
    clearPlayback()
    const priorReply = cleanStreamingText(streamingText)
    setItems(current => [
      ...current,
      ...(priorReply === '' ? [] : [{ id: `assistant-${playbackId.current}`, kind: 'assistant' as const, text: priorReply }]),
      { id: `user-${playbackId.current + 1}`, kind: 'user', text: value },
    ])
    setDraft('')
    setStreamingText('')
    const reply = followupReplies[followupIndex.current % followupReplies.length]!
    followupIndex.current += 1
    beginPlayback(reply)
  }

  const replay = (): void => {
    if (playback !== null) beginPlayback(playback.text)
  }

  return (
    <main className="splash-page" data-route="splash" style={themeStyle}>
      <section aria-label="OpenAgents Desktop live product preview" className="splash-workbench">
        <header className="splash-window-bar">
          <div aria-hidden="true" className="splash-window-controls"><i /><i /><i /></div>
          <span>OpenAgents</span>
          <span className="splash-connection"><i aria-hidden="true" /> Codex connected</span>
        </header>
        <div className="splash-workbench-body">
          <button aria-label="Close sessions" className="splash-rail-scrim" data-open={railOpen} onClick={() => setRailOpen(false)} type="button" />
          <SessionRail
            activeId={activeId}
            onDismiss={() => setRailOpen(false)}
            onNewChat={newSession}
            onSelect={selectSession}
            open={railOpen}
          />
          <section className="splash-conversation" aria-label={title}>
            <header className="splash-conversation-header">
              <Button aria-label="Open sessions" className="splash-mobile-rail-button" onClick={() => setRailOpen(true)} size="sm" variant="ghost"><PanelLeft aria-hidden="true" /></Button>
              <div><h1>{title}</h1><p><span aria-hidden="true" /> {phase === 'preparing' || phase === 'streaming' ? 'Running' : 'Ready'} · openagents / main</p></div>
              <div className="splash-header-actions">
                <Button aria-label="Replay streamed response" disabled={playback === null} onClick={replay} size="sm" variant="ghost"><RotateCcw aria-hidden="true" /></Button>
                <Button aria-label="Review changes, 3 files" onClick={() => setReviewOpen(true)} size="sm" variant="secondary"><FileCode2 aria-hidden="true" /><span>Review changes</span><Badge variant="running">3</Badge></Button>
              </div>
            </header>
            <div className="splash-timeline" role="log">
              <div className="splash-timeline-inner">
                {items.length === 0 ? (
                  <div className="splash-empty-state"><span className="splash-agent-mark" aria-hidden="true">K</span><h2>What should we work on?</h2><p>Start a local Codex session. The timeline will keep the turn, tools, and review state together.</p></div>
                ) : items.map(item => <TimelineItemView item={item} key={item.id} />)}
                {playback === null ? null : <StreamingReply phase={phase} semanticText={playback.text} text={streamingText} />}
                <div aria-live="polite" className="sr-only" role="status">{phase === 'complete' ? 'Codex response complete.' : phase === 'stopped' ? 'Codex response stopped.' : ''}</div>
              </div>
            </div>
            <Composer draft={draft} onDraftChange={setDraft} onStop={stopPlayback} onSubmit={submitMessage} phase={phase} />
            <ReviewPanel onClose={() => setReviewOpen(false)} open={reviewOpen} />
          </section>
        </div>
      </section>
    </main>
  )
}
