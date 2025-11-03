import React from 'react'
import { useTricoder } from '../hooks/useTricoder'
import { renderAnsi } from '../lib/ansi'
import { renderJsonSyntax } from '../lib/jsonSyntax'
import { useEffect, useRef } from 'react'

// Minimal desktop entrypoint view using our global theme
// Theme CSS (fonts, colors) is imported by main.tsx via App.css

export default function HelloDesktop() {
  const { status, wsUrl, logs, sidecarLogs } = useTricoder()
  const wsRef = useRef<HTMLDivElement | null>(null)
  const bridgeRef = useRef<HTMLDivElement | null>(null)

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
        <div className="text-xs text-[var(--tertiary)]">
          <span className="mr-3">status: <span className="text-[var(--secondary)]">{status}</span></span>
          <span>ws: <span className="text-[var(--secondary)]">{wsUrl || '—'}</span></span>
        </div>
      </header>
      <main className="flex-1 min-h-0 flex gap-3 p-3">
        <section className="flex-1 min-w-0 flex flex-col border border-[var(--border)] rounded">
          <div className="px-2.5 py-2 border-b border-[var(--border)] text-xs text-[var(--tertiary)]">WS events</div>
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
        <section className="w-[420px] min-w-[300px] flex flex-col border border-[var(--border)] rounded">
          <div className="px-2.5 py-2 border-b border-[var(--border)] text-xs text-[var(--tertiary)]">Bridge logs</div>
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
      </main>
    </div>
  )
}
