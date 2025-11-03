import React from 'react'
import { useTricoder } from '../hooks/useTricoder'

// Minimal desktop entrypoint view using our global theme
// Theme CSS (fonts, colors) is imported by main.tsx via App.css

export default function HelloDesktop() {
  const { status, wsUrl, logs, sidecarLogs } = useTricoder()

  return (
    <div style={{ height: '100vh', width: '100%', display: 'flex', flexDirection: 'column', background: 'var(--background)', color: 'var(--foreground)' }}>
      <header style={{ padding: 16, borderBottom: '1px solid var(--border)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ fontSize: 20 }}>Hello world</div>
        <div style={{ fontSize: 12, color: 'var(--tertiary)' }}>
          <span style={{ marginRight: 12 }}>status: <span style={{ color: 'var(--secondary)' }}>{status}</span></span>
          <span>ws: <span style={{ color: 'var(--secondary)' }}>{wsUrl || '—'}</span></span>
        </div>
      </header>
      <main style={{ flex: 1, minHeight: 0, display: 'flex', gap: 12, padding: 12 }}>
        <section style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', border: '1px solid var(--border)', borderRadius: 4 }}>
          <div style={{ padding: '8px 10px', borderBottom: '1px solid var(--border)', fontSize: 12, color: 'var(--tertiary)' }}>WS events</div>
          <div style={{ flex: 1, minHeight: 0, overflow: 'auto', fontSize: 12, lineHeight: '18px', padding: 10 }}>
            {logs.length === 0 ? (
              <div style={{ color: 'var(--tertiary)' }}>No events yet.</div>
            ) : (
              logs.map((l, i) => (
                <div key={i} style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{l}</div>
              ))
            )}
          </div>
        </section>
        <section style={{ width: 420, minWidth: 300, display: 'flex', flexDirection: 'column', border: '1px solid var(--border)', borderRadius: 4 }}>
          <div style={{ padding: '8px 10px', borderBottom: '1px solid var(--border)', fontSize: 12, color: 'var(--tertiary)' }}>Bridge logs</div>
          <div style={{ flex: 1, minHeight: 0, overflow: 'auto', fontSize: 12, lineHeight: '18px', padding: 10 }}>
            {sidecarLogs.length === 0 ? (
              <div style={{ color: 'var(--tertiary)' }}>Waiting for bridge…</div>
            ) : (
              sidecarLogs.map((l, i) => (
                <div key={i} style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{l}</div>
              ))
            )}
          </div>
        </section>
      </main>
    </div>
  )
}
