import React from 'react'
import { useTricoder } from '../hooks/useTricoder'
import { renderAnsi } from '../lib/ansi'
import { renderJsonSyntax } from '../lib/jsonSyntax'
import { useEffect, useRef } from 'react'
import { TinyvexProvider, useTinyvexThreads, useTinyvexThread } from 'tinyvex/react'
import { ChatMessageBubble } from '@openagentsinc/core'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import type { MessageRowTs } from 'tricoder/types'

// Minimal desktop entrypoint view using our global theme
// Theme CSS (fonts, colors) is imported by main.tsx via App.css

function ThreadsList({ onSelect, selectedId }: { onSelect: (id: string) => void; selectedId?: string }) {
  const { threads } = useTinyvexThreads(50)
  return (
    <ScrollArea className="flex-1 min-h-0">
      {threads.map((t) => (
        <button
          key={t.id}
          className={[
            'w-full text-left px-3 py-2 border-b border-[var(--border)] cursor-pointer',
            'bg-transparent border-0 border-b',
            'text-[var(--foreground)] hover:bg-white/5',
            'focus:outline-none focus:ring-0 shadow-none',
            selectedId === String(t.id) ? 'bg-white/10' : 'bg-transparent',
          ].join(' ')}
          onClick={() => onSelect(String(t.id))}
        >
          <div className="text-sm truncate">{t.title || 'Thread'}</div>
          <div className="text-[11px] text-[var(--tertiary)]">{new Date(Number(t.updated_at || 0)).toLocaleString()}</div>
        </button>
      ))}
    </ScrollArea>
  )
}

function ChatThread({ id }: { id: string }) {
  const { history, status } = useTinyvexThread({ idOrAlias: id })
  const chatRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    const el = chatRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [history])
  const messages = (history || []).filter((m: MessageRowTs) => m.kind === 'message' && !!m.role)
  return (
    <ScrollArea className="flex-1 min-h-0 p-3" ref={chatRef as any}>
      {status !== 'ready' && (
        <div className="text-xs text-[var(--tertiary)] mb-2">Loading…</div>
      )}
      {messages.map((m) => (
        <div key={`${m.item_id ?? ''}:${m.ts}`}>
          <ChatMessageBubble role={(m.role as 'assistant' | 'user') ?? 'assistant'} text={String(m.text || '')} />
        </div>
      ))}
    </ScrollArea>
  )}

export default function HelloDesktop() {
  const { status, wsUrl, token, logs, sidecarLogs } = useTricoder()
  const wsRef = useRef<HTMLDivElement | null>(null)
  const bridgeRef = useRef<HTMLDivElement | null>(null)
  const [view, setView] = React.useState<'dev' | 'chat'>('dev')
  const [selected, setSelected] = React.useState<string>('')

  const copyWsEvents = async () => {
    try { await navigator.clipboard.writeText((logs || []).join('\n')) } catch {}
  }
  const copyBridgeLogs = async () => {
    try { await navigator.clipboard.writeText((sidecarLogs || []).join('\n')) } catch {}
  }

  // Auto-scroll to bottom when new items arrive
  useEffect(() => {
    const el = wsRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [logs])
  useEffect(() => {
    const el = bridgeRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [sidecarLogs])

  return (
    <div className="h-screen w-full flex flex-col bg-[var(--background)] text-[var(--foreground)] font-mono">
      <header className="px-4 py-4 border-b border-[var(--border)] flex items-center justify-between">
        <div className="text-[20px]">Hello world</div>
        <div className="flex items-center gap-3">
          {view === 'chat' && selected && (
            <Badge className="max-w-[520px] truncate" title={selected}>Thread {selected}</Badge>
          )}
          {view === 'chat' && (
            <Button variant="outline" size="sm" onClick={() => setView('dev')}>Developer</Button>
          )}
          <div className="text-xs text-[var(--tertiary)]">
            <span className="mr-3">status: <span className="text-[var(--secondary)]">{status}</span></span>
            <span>ws: <span className="text-[var(--secondary)]">{wsUrl || '—'}</span></span>
          </div>
        </div>
      </header>
      <main className="flex-1 min-h-0 flex gap-3 p-3">
        {/* Leftmost threads column */}
        <Card className="w-[280px] min-w-[240px] flex flex-col">
          <CardHeader>Recent threads</CardHeader>
          {wsUrl ? (
            <TinyvexProvider config={{ url: wsUrl, token }}>
              <ThreadsList selectedId={selected} onSelect={(id) => { setSelected(id); setView('chat') }} />
            </TinyvexProvider>
          ) : (
            <div className="p-3 text-[var(--tertiary)] text-xs">Connecting…</div>
          )}
        </Card>

        {view === 'dev' ? (
          <Card className="flex-1 min-w-0 flex flex-col">
            <CardHeader>
              <span>WS events</span>
              <Button variant="outline" size="sm" onClick={copyWsEvents}>Copy</Button>
            </CardHeader>
            <ScrollArea ref={wsRef as any} className="flex-1 min-h-0 text-xs leading-[18px] p-2.5">
              {logs.length === 0 ? (
                <div className="text-[var(--tertiary)]">No events yet.</div>
              ) : (
                logs.map((l, i) => (
                  <div key={i} className="whitespace-pre-wrap break-words">
                    {renderJsonSyntax(l) ?? renderAnsi(l)}
                  </div>
                ))
              )}
            </ScrollArea>
          </Card>
        ) : (
          <Card className="flex-1 min-w-0 flex flex-col">
            <CardHeader>Thread</CardHeader>
            {wsUrl ? (
              <TinyvexProvider config={{ url: wsUrl, token }}>
                {selected ? (
                  <ChatThread id={selected} />
                ) : (
                  <div className="p-3 text-xs text-[var(--tertiary)]">Select a thread</div>
                )}
              </TinyvexProvider>
            ) : (
              <div className="p-3 text-xs text-[var(--tertiary)]">Connecting…</div>
            )}
          </Card>
        )}

        {view === 'dev' && (
          <Card className="w-[420px] min-w-[300px] flex flex-col">
            <CardHeader>
              <span>Bridge logs</span>
              <Button variant="outline" size="sm" onClick={copyBridgeLogs}>Copy</Button>
            </CardHeader>
            <ScrollArea ref={bridgeRef as any} className="flex-1 min-h-0 text-xs leading-[18px] p-2.5">
              {sidecarLogs.length === 0 ? (
                <div className="text-[var(--tertiary)]">Waiting for bridge…</div>
              ) : (
                sidecarLogs.map((l, i) => (
                  <div key={i} className="whitespace-pre-wrap break-words">{renderAnsi(l)}</div>
                ))
              )}
            </ScrollArea>
          </Card>
        )}
      </main>
    </div>
  )
}
