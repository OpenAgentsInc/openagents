import { makeKhalaTextSequenceFrames } from '@effect-native/khala-ui'
import { khalaTheme } from '@effect-native/tokens'
import {
  DesktopComposerBar,
  DesktopComposerFrame,
  DesktopComposerInput,
  DesktopConversation,
  DesktopConversationHeader,
  DesktopRailScrim,
  DesktopSessionRail,
  DesktopSidebarExpand,
  DesktopTimeline,
  DesktopTimelineMessage,
  DesktopWorkbench,
  DesktopWorkEntry,
  DesktopWorkGroup,
  desktopThemeCssVariables,
  type DesktopRailDestination,
} from '@openagentsinc/ui/desktop-workbench'
import { ArrowUp, Command, Folder, ImagePlus, Square, Zap } from 'lucide-react'
import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react'

import '../splash.css'

type DemoItem = Readonly<{
  id: string
  kind: 'assistant' | 'tool' | 'user' | 'work-group'
  text: string
  detail?: string
  count?: number
}>

type DemoSession = Readonly<{
  id: string
  title: string
  meta: string
  items: ReadonlyArray<DemoItem>
  reply?: string
}>

type Playback = Readonly<{ id: number; text: string }>
type PlaybackPhase = 'complete' | 'idle' | 'preparing' | 'stopped' | 'streaming'

const firstReply =
  'The Desktop renderer and this web route now consume the same workbench package. The rail, conversation frame, timeline rows, work entries, and composer shell are shared—the page is no longer a parallel reconstruction.'

const sessions: ReadonlyArray<DemoSession> = [
  {
    id: 't3code-yoink',
    title: 'T3CODE YOINK',
    meta: '⌘1',
    items: [
      {
        id: 'intro',
        kind: 'assistant',
        text: 'I found the actual Desktop workbench boundary. I’m lifting its controlled visual primitives and canonical CSS into packages/ui, while keeping Electron state and intents in the Desktop adapter.',
      },
      {
        id: 'inspect',
        kind: 'tool',
        text: 'Inspected Desktop renderer',
        detail: 'react-primitive-adapters.tsx · react-timeline.tsx · react-composer.tsx',
      },
      {
        id: 'request',
        kind: 'user',
        text: 'Use the actual Desktop components on web so the two surfaces cannot drift.',
      },
      { id: 'worked', kind: 'work-group', text: 'Shared workbench extraction', count: 3 },
    ],
    reply: firstReply,
  },
  {
    id: 'arwes',
    title: 'ARWES',
    meta: '⌘2',
    items: [
      { id: 'arwes-user', kind: 'user', text: 'Keep the Khala color and motion language restrained.' },
      { id: 'arwes-work', kind: 'work-group', text: 'Token audit', count: 2 },
      { id: 'arwes-assistant', kind: 'assistant', text: 'The shared workbench consumes the same Effect Native token sheet in both hosts.' },
    ],
  },
  {
    id: 'website',
    title: 'WEBSITE',
    meta: '⌘3',
    items: [
      { id: 'website-user', kind: 'user', text: 'Make public navigation instant and keep the landing page focused.' },
      { id: 'website-assistant', kind: 'assistant', text: 'The public surface stays in TanStack Start; this route embeds the shared Desktop workbench presentation.' },
    ],
  },
  {
    id: 'docs',
    title: 'DOCS',
    meta: '⌘4',
    items: [
      { id: 'docs-user', kind: 'user', text: 'Keep the documentation inside the same fast application shell.' },
      { id: 'docs-assistant', kind: 'assistant', text: 'Docs, blog, website, and app routes now share the TanStack application authority.' },
    ],
  },
  { id: 'appserver', title: 'APPSERVER', meta: '⌘5', items: [] },
  { id: 'untitled-6', title: 'Untitled Codex chat', meta: '⌘6', items: [] },
  { id: 'untitled-7', title: 'Untitled Codex chat', meta: '⌘7', items: [] },
  { id: 'grokcli', title: 'GrokCLI', meta: '⌘8', items: [] },
  { id: 'untitled-9', title: 'Untitled Codex chat', meta: '⌘9', items: [] },
]

const destinations: ReadonlyArray<DesktopRailDestination> = [
  { id: 'new', icon: 'new-session', label: 'New session' },
  { id: 'chat', icon: 'chat', label: 'Chat' },
  { id: 'home', icon: 'home', label: 'Project home' },
  { id: 'settings', icon: 'settings', label: 'Settings' },
]

const followupReplies = [
  'That message stayed in the controlled web fixture, but its rail, timeline row, and composer presentation are the exact shared components the Desktop host renders.',
  'One package now owns the visual contract. Product state remains host-specific, so the web demo cannot accidentally become a second Desktop runtime.',
] as const

const cleanStreamingText = (value: string): string => value.replace(/▌$/u, '')

const Item = ({ item, sequence }: Readonly<{ item: DemoItem; sequence: number }>) => {
  if (item.kind === 'tool') return <DesktopWorkEntry
    body={<pre><code>{item.detail}</code></pre>}
    itemKey={item.id}
    label={item.text}
    preview={item.detail ?? item.text}
  />
  if (item.kind === 'work-group') return <DesktopWorkGroup count={item.count ?? 1}>
    <DesktopWorkEntry body={<pre><code>{item.text}</code></pre>} itemKey={`${item.id}:entry`} label="FileChange" preview={item.text} />
  </DesktopWorkGroup>
  return <DesktopTimelineMessage itemKey={item.id} label={item.kind === 'user' ? 'You' : 'Assistant'} sequence={sequence} tone={item.kind === 'user' ? 'user' : 'assistant'}>
    <p>{item.text}</p>
  </DesktopTimelineMessage>
}

export function SplashPage() {
  const initial = sessions[0]!
  const [activeId, setActiveId] = useState(initial.id)
  const [title, setTitle] = useState(initial.title)
  const [items, setItems] = useState<ReadonlyArray<DemoItem>>(initial.items)
  const [draft, setDraft] = useState('')
  const [fullAuto, setFullAuto] = useState(true)
  const [phase, setPhase] = useState<PlaybackPhase>('preparing')
  const [playback, setPlayback] = useState<Playback | null>({ id: 0, text: initial.reply! })
  const [streamingText, setStreamingText] = useState('')
  const [railOpen, setRailOpen] = useState(false)
  const [searchOpen, setSearchOpen] = useState(false)
  const [query, setQuery] = useState('')
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
    const frames = makeKhalaTextSequenceFrames(playback.text, { caret: true, frames: 72 })
    const startTimer = window.setTimeout(() => {
      setPhase('streaming')
      frames.forEach((frame, index) => {
        const frameTimer = window.setTimeout(() => {
          setStreamingText(frame.visualText)
          if (index === frames.length - 1) setPhase('complete')
        }, index * 22)
        timerIds.current = [...timerIds.current, frameTimer]
      })
    }, 350)
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
    window.setTimeout(() => document.querySelector<HTMLTextAreaElement>('#splash-composer-input')?.focus(), 0)
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
      ...(priorReply === '' ? [] : [{ id: `assistant-${playbackId.current}`, kind: 'assistant' as const, text: priorReply }]),
      { id: `user-${playbackId.current + 1}`, kind: 'user', text: value },
    ])
    setDraft('')
    setStreamingText('')
    const reply = followupReplies[followupIndex.current % followupReplies.length]!
    followupIndex.current += 1
    beginPlayback(reply)
  }

  const visibleSessions = sessions.filter(session => session.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()))
  const active = phase === 'preparing' || phase === 'streaming'

  return <main className="splash-page" data-route="splash" style={desktopThemeCssVariables(khalaTheme)}>
    <DesktopWorkbench aria-label="OpenAgents Desktop live product preview">
      <DesktopSidebarExpand aria-expanded={railOpen} aria-label="Expand sidebar" onClick={() => setRailOpen(true)} title="Expand sidebar" />
      <DesktopSessionRail
        canGoBack
        canGoForward={false}
        destinations={destinations}
        footer={<section aria-label="Coding workspaces" className="oa-react-workspaces splash-workspace-footer">
          <h2><Folder aria-hidden="true" /><span>Workspaces</span></h2>
          <div className="oa-react-workspace-row"><button type="button"><span>openagents-desktop</span><small>Active</small></button></div>
        </section>}
        onBack={() => undefined}
        onCollapse={() => setRailOpen(false)}
        onDestinationSelect={destination => destination.id === 'new' ? newSession() : undefined}
        onForward={() => undefined}
        onSearchOpenChange={setSearchOpen}
        onSearchQueryChange={setQuery}
        onSessionSelect={session => selectSession(session.id)}
        open={railOpen}
        searchOpen={searchOpen}
        searchQuery={query}
        sessions={visibleSessions.map(session => ({ ...session, selected: session.id === activeId }))}
        stageLabel="DEV"
      />
      {railOpen ? <DesktopRailScrim aria-label="Close sessions" onClick={() => setRailOpen(false)} /> : null}
      <DesktopConversation
        composer={<DesktopComposerFrame aria-label="Message composer">
          <DesktopComposerInput>
            <textarea
              aria-label={active ? 'Steer a Codex message' : 'Message Codex'}
              id="splash-composer-input"
              onChange={event => setDraft(event.currentTarget.value)}
              onKeyDown={event => {
                if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return
                event.preventDefault()
                submitMessage()
              }}
              placeholder={active ? 'Steer the current turn…' : 'Message Codex…'}
              rows={2}
              value={draft}
            />
            <DesktopComposerBar>
              <button aria-label="Attach image" className="splash-composer-action" type="button"><ImagePlus aria-hidden="true" /></button>
              <button aria-label="Open commands" className="splash-composer-action" type="button"><Command aria-hidden="true" /></button>
              <button aria-label={fullAuto ? 'Turn off Full Auto' : 'Turn on Full Auto'} aria-pressed={fullAuto} className="splash-full-auto" onClick={() => setFullAuto(value => !value)} type="button"><Zap aria-hidden="true" />Full Auto</button>
              <span className="oa-react-composer-spacer" />
              {active ? <button aria-label="Stop current turn" className="oa-react-stop splash-submit" onClick={stopPlayback} type="button"><Square aria-hidden="true" /></button> : null}
              <button aria-label="Send" className="oa-react-submit splash-submit" disabled={draft.trim() === ''} onClick={() => submitMessage()} type="button"><ArrowUp aria-hidden="true" /></button>
            </DesktopComposerBar>
          </DesktopComposerInput>
        </DesktopComposerFrame>}
        header={<DesktopConversationHeader lifecycle={active ? 'Running' : 'Ready'} secondary="openagents / main" title={title} />}
        timeline={<DesktopTimeline working={phase === 'preparing'}>
          {items.length === 0 ? <div className="splash-empty"><h2>Start a conversation with Codex</h2><p>Choose a workspace, then send a message.</p></div> : items.map((item, index) => <Item item={item} key={item.id} sequence={index} />)}
          {playback === null || phase === 'preparing' ? null : <DesktopTimelineMessage itemKey={`playback-${playback.id}`} label="Assistant" sequence={items.length} tone="assistant">
            <p aria-hidden="true">{streamingText}</p><span className="oa-react-sr-only">{playback.text}</span>
          </DesktopTimelineMessage>}
        </DesktopTimeline>}
      />
    </DesktopWorkbench>
  </main>
}
