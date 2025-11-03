import React, { useContext, useEffect, useRef, useState } from 'react'
import { useTricoder } from '../hooks/useTricoder'
import { renderAnsi } from '../lib/ansi'
import { renderJsonSyntax } from '../lib/jsonSyntax'
import { TinyvexProvider, TinyvexContext, useTinyvexThreads, useTinyvexThread } from 'tinyvex/react'
import { ChatMessageBubble } from '@openagentsinc/core'
import { Button } from '@/components/ui/button'
import { Card, CardHeader } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import type { MessageRowTs } from 'tricoder/types'
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarInset,
  SidebarRail,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import { invoke } from '@tauri-apps/api/core'

function ChatThread({ id }: { id: string }) {
  const { history, status } = useTinyvexThread({ idOrAlias: id })
  const chatRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => { const el = chatRef.current; if (el) el.scrollTop = el.scrollHeight }, [history])
  const messages = (history || []).filter((m: MessageRowTs) => m.kind === 'message' && !!m.role)
  return (
    <ScrollArea className="flex-1 min-h-0 p-3" ref={chatRef as any}>
      {status !== 'ready' && (<div className="text-xs text-[var(--tertiary)] mb-2">Loading…</div>)}
      {messages.map((m) => (
        <div key={`${m.item_id ?? ''}:${m.ts}`}>
          <ChatMessageBubble role={(m.role as 'assistant' | 'user') ?? 'assistant'} text={String(m.text || '')} />
        </div>
      ))}
    </ScrollArea>
  )
}

function SidebarThreadsInner({ selectedId, onSelect }: { selectedId?: string; onSelect: (id: string) => void }) {
  const { threads } = useTinyvexThreads(50)
  return (
    <>
      {threads.map((t) => (
        <SidebarMenuItem key={t.id}>
          <SidebarMenuButton isActive={selectedId === String(t.id)} onClick={() => onSelect(String(t.id))}>
            <span className="truncate">{t.title || 'Thread'}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
      ))}
    </>
  )
}

function SidebarThreads({ wsUrl, selectedId, onSelect }: { wsUrl: string; selectedId?: string; onSelect: (id: string) => void }) {
  if (!wsUrl) return <div className="px-3 py-2 text-[var(--tertiary)] text-xs">Connecting…</div>
  return (
    <SidebarMenu>
      <SidebarThreadsInner selectedId={selectedId} onSelect={onSelect} />
    </SidebarMenu>
  )
}

function WsEventsPanel() {
  const wsRef = useRef<HTMLDivElement | null>(null)
  const client = useContext(TinyvexContext as unknown as React.Context<any>) as any
  const [lines, setLines] = useState<string[]>([])
  useEffect(() => {
    if (!client) return
    const off = client.onRaw((evt: unknown) => {
      try {
        const line = typeof evt === 'string' ? evt : JSON.stringify(evt)
        setLines((prev) => {
          const next = [...prev, line]
          return next.length > 200 ? next.slice(next.length - 200) : next
        })
      } catch {}
    })
    return () => { try { off?.() } catch {} }
  }, [client])
  useEffect(() => { const el = wsRef.current; if (el) el.scrollTop = el.scrollHeight }, [lines])
  const copy = async () => { try { await navigator.clipboard.writeText(lines.join('\n')) } catch {} }
  return (
    <Card className="flex-1 min-w-0 flex flex-col">
      <CardHeader>
        <span>WS events</span>
        <Button variant="outline" size="sm" onClick={copy}>Copy</Button>
      </CardHeader>
      <ScrollArea ref={wsRef as any} className="flex-1 min-h-0 text-xs leading-[18px] p-2.5">
        {lines.length === 0 ? (
          <div className="text-[var(--tertiary)]">No events yet.</div>
        ) : (
          lines.map((l, i) => (
            <div key={i} className="whitespace-pre-wrap break-words">{renderJsonSyntax(l) ?? renderAnsi(l)}</div>
          ))
        )}
      </ScrollArea>
    </Card>
  )
}

function BridgeLogsPanel() {
  const ref = useRef<HTMLDivElement | null>(null)
  const [lines, setLines] = useState<string[]>([])
  useEffect(() => {
    let mounted = true
    const id = window.setInterval(async () => {
      try {
        const s = await invoke<{ logs?: string[] }>('bridge_status')
        const arr = Array.isArray(s?.logs) ? s.logs.slice().reverse() : []
        if (mounted) setLines(arr)
      } catch {}
    }, 2000)
    return () => { mounted = false; try { window.clearInterval(id) } catch {} }
  }, [])
  useEffect(() => { const el = ref.current; if (el) el.scrollTop = el.scrollHeight }, [lines])
  const copy = async () => { try { await navigator.clipboard.writeText(lines.join('\n')) } catch {} }
  return (
    <Card className="w-[420px] min-w-[300px] flex flex-col">
      <CardHeader>
        <span>Bridge logs</span>
        <Button variant="outline" size="sm" onClick={copy}>Copy</Button>
      </CardHeader>
      <ScrollArea ref={ref as any} className="flex-1 min-h-0 text-xs leading-[18px] p-2.5">
        {lines.length === 0 ? (
          <div className="text-[var(--tertiary)]">Waiting for bridge…</div>
        ) : (
          lines.map((l, i) => (
            <div key={i} className="whitespace-pre-wrap break-words">{renderAnsi(l)}</div>
          ))
        )}
      </ScrollArea>
    </Card>
  )
}

export default function HelloDesktop() {
  const { bridgeRunning, wsUrl, token } = useTricoder()
  const [view, setView] = useState<'dev' | 'chat'>('dev')
  const [selected, setSelected] = useState<string>('')

  return (
    <div className="h-screen w-full flex flex-col bg-background text-foreground font-mono">
      {wsUrl ? (
        <TinyvexProvider config={{ url: wsUrl, token }}>
          <SidebarProvider>
            <Sidebar collapsible="icon">
              <SidebarHeader>Recent threads</SidebarHeader>
              <SidebarContent>
                <SidebarThreads wsUrl={wsUrl} selectedId={selected} onSelect={(id) => { setSelected(id); setView('chat') }} />
              </SidebarContent>
              <SidebarRail />
            </Sidebar>
            <SidebarInset>
              <header className="px-4 py-3 border-b border-[var(--border)] flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <SidebarTrigger />
                  <div className="text-[20px]">Hello world</div>
                </div>
                <div className="flex items-center gap-3">
                  {view === 'chat' && selected && (
                    <Badge className="max-w-[520px] truncate" title={selected}>Thread {selected}</Badge>
                  )}
                  {view === 'chat' && (
                    <Button variant="outline" size="sm" onClick={() => setView('dev')}>Developer</Button>
                  )}
                  <div className="text-xs text-[var(--tertiary)]">
                    <span className="mr-3">bridge: <span className="text-[var(--secondary)]">{bridgeRunning ? 'running' : 'starting'}</span></span>
                    <span>ws: <span className="text-[var(--secondary)]">{wsUrl || '—'}</span></span>
                  </div>
                </div>
              </header>
              <div className="flex-1 min-h-0 flex gap-3 p-3">
                {view === 'dev' ? (
                  <WsEventsPanel />
                ) : (
                  <Card className="flex-1 min-w-0 flex flex-col">
                    <CardHeader>Thread</CardHeader>
                    {selected ? (
                      <ChatThread id={selected} />
                    ) : (
                      <div className="p-3 text-xs text-[var(--tertiary)]">Select a thread</div>
                    )}
                  </Card>
                )}

                {view === 'dev' && <BridgeLogsPanel />}
              </div>
            </SidebarInset>
          </SidebarProvider>
        </TinyvexProvider>
      ) : (
        <div className="p-3 text-xs text-[var(--tertiary)]">{bridgeRunning ? 'Connecting…' : 'Starting bridge…'}</div>
      )}
    </div>
  )
}
