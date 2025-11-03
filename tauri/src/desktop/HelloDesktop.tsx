import React from 'react'
import { useTricoder } from '../hooks/useTricoder'
import { renderAnsi } from '../lib/ansi'
import { renderJsonSyntax } from '../lib/jsonSyntax'
import { useEffect, useRef } from 'react'
import { TinyvexProvider, useTinyvexThreads, useTinyvexThread } from 'tinyvex/react'
import { ChatMessageBubble } from '@openagentsinc/core'
import type { MessageRowTs } from 'tricoder/types'

// Minimal desktop entrypoint view using our global theme
// Theme CSS (fonts, colors) is imported by main.tsx via App.css

function ThreadsList({ onSelect, selectedId }: { onSelect: (id: string) => void; selectedId?: string }) {
  const { threads } = useTinyvexThreads(50)
  return (
    <div className="flex-1 min-h-0 overflow-auto">
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
    </div>
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
    <div className="flex-1 min-h-0 overflow-auto p-3" ref={chatRef}>
      {status !== 'ready' && (
        <div className="text-xs text-[var(--tertiary)] mb-2">Loading…</div>
      )}
      {messages.map((m) => (
        <div key={`${m.item_id ?? ''}:${m.ts}`}>
          <ChatMessageBubble role={(m.role as 'assistant' | 'user') ?? 'assistant'} text={String(m.text || '')} />
        </div>
      ))}
    </div>
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
            <div
              className="max-w-[520px] truncate text-xs font-mono text-[var(--secondary)] bg-white/5 border border-[var(--border)] rounded px-2 py-1"
              title={selected}
            >
              Thread {selected}
            </div>
          )}
          {view === 'chat' && (
            <button
              className="text-xs border border-[var(--border)] rounded px-2 py-1 hover:bg-black/20"
              onClick={() => setView('dev')}
            >Developer</button>
          )}
          <div className="text-xs text-[var(--tertiary)]">
            <span className="mr-3">status: <span className="text-[var(--secondary)]">{status}</span></span>
            <span>ws: <span className="text-[var(--secondary)]">{wsUrl || '—'}</span></span>
          </div>
        </div>
      </header>
      <main className="flex-1 min-h-0 flex gap-3 p-3">
        {/* Leftmost threads column */}
        <section className="w-[280px] min-w-[240px] flex flex-col border border-[var(--border)] rounded">
          <div className="px-2.5 py-2 border-b border-[var(--border)] text-xs text-[var(--tertiary)]">Recent threads</div>
          {wsUrl ? (
            <TinyvexProvider config={{ url: wsUrl, token }}>
              <ThreadsList selectedId={selected} onSelect={(id) => { setSelected(id); setView('chat') }} />
            </TinyvexProvider>
          ) : (
            <div className="p-3 text-[var(--tertiary)] text-xs">Connecting…</div>
          )}
        </section>

        {view === 'dev' ? (
          <section className="flex-1 min-w-0 flex flex-col border border-[var(--border)] rounded">
            <div className="px-2.5 py-2 border-b border-[var(--border)] text-xs text-[var(--tertiary)] flex items-center justify-between">
              <span>WS events</span>
              <button onClick={copyWsEvents} className="text-[var(--secondary)] border border-[var(--border)] rounded px-2 py-0.5 hover:bg-black/20">Copy</button>
            </div>
            <div ref={wsRef} className="flex-1 min-h-0 overflow-auto text-xs leading-[18px] p-2.5">
              {logs.length === 0 ? (
                <div className="text-[var(--tertiary)]">No events yet.</div>
              ) : (
                logs.map((l, i) => (
                  <div key={i} className="whitespace-pre-wrap break-words">
                    {renderJsonSyntax(l) ?? renderAnsi(l)}
                  </div>
                ))
              )}
            </div>
          </section>
        ) : (
          <section className="flex-1 min-w-0 flex flex-col border border-[var(--border)] rounded">
            <div className="px-2.5 py-2 border-b border-[var(--border)] text-xs text-[var(--tertiary)]">Thread</div>
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
          </section>
        )}

        {view === 'dev' && (
          <section className="w-[420px] min-w-[300px] flex flex-col border border-[var(--border)] rounded">
            <div className="px-2.5 py-2 border-b border-[var(--border)] text-xs text-[var(--tertiary)] flex items-center justify-between">
              <span>Bridge logs</span>
              <button onClick={copyBridgeLogs} className="text-[var(--secondary)] border border-[var(--border)] rounded px-2 py-0.5 hover:bg-black/20">Copy</button>
            </div>
            <div ref={bridgeRef} className="flex-1 min-h-0 overflow-auto text-xs leading-[18px] p-2.5">
              {sidecarLogs.length === 0 ? (
                <div className="text-[var(--tertiary)]">Waiting for bridge…</div>
              ) : (
                sidecarLogs.map((l, i) => (
                  <div key={i} className="whitespace-pre-wrap break-words">{renderAnsi(l)}</div>
                ))
              )}
            </div>
          </section>
        )}
      </main>
    </div>
  )
}
