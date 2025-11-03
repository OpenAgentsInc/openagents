import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { WsTransport } from 'tinyvex/client/WsTransport'
import type { BridgeEvent } from 'tricoder/types'

export type TricoderStatus = 'idle' | 'connecting' | 'open' | 'closed' | 'error'

export type UseTricoderResult = {
  status: TricoderStatus
  wsUrl: string
  host: string
  token: string
  logs: string[]
  sidecarLogs: string[]
}

export function useTricoder(): UseTricoderResult {
  const [host, setHost] = useState<string>('')
  const [token, setToken] = useState<string>('')
  const [status, setStatus] = useState<TricoderStatus>('idle')
  const [logs, setLogs] = useState<string[]>([])
  const [sidecarLogs, setSidecarLogs] = useState<string[]>([])
  const transportRef = useRef<WsTransport | null>(null)
  const unsubRef = useRef<(() => void) | null>(null)

  const wsUrl = useMemo(() => {
    if (!host) return ''
    const scheme = 'ws'
    return `${scheme}://${host}/ws`
  }, [host])

  // Load token once
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const t = await invoke<string | null>('get_bridge_token')
        if (!cancelled && t) setToken(t)
      } catch {
        // ignore
      }
    })()
    return () => { cancelled = true }
  }, [])

  // Start local bridge and poll status/logs
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const h = await invoke<string>('bridge_start', { bind: null, token: token || null })
        if (!cancelled && h) setHost(h)
      } catch {
        // ignore; UI can still connect to remote if provided later
      }
      const id = window.setInterval(() => { refreshBridgeStatus().catch(() => {}) }, 1500)
      ;(window as any).__bridgePoll = id
    })()
    return () => {
      cancelled = true
      try { window.clearInterval((window as any).__bridgePoll) } catch {}
    }
  }, [token])

  const refreshBridgeStatus = useCallback(async () => {
    try {
      const s = await invoke<{ running: boolean; bind?: string; logs: string[] }>('bridge_status')
      if (s?.bind) setHost(s.bind)
      setSidecarLogs(Array.isArray(s?.logs) ? s.logs.slice().reverse() : [])
    } catch {
      // ignore
    }
  }, [])

  // Connect to WS once host/token known
  useEffect(() => {
    if (!host || !token) return
    // Clean up any previous
    try { unsubRef.current?.() } catch {}
    try { transportRef.current?.close() } catch {}
    setStatus('connecting')
    const t = new WsTransport({ url: `${wsUrl}?token=${encodeURIComponent(token)}` })
    transportRef.current = t
    ;(async () => {
      try {
        await t.connect()
        setStatus('open')
        unsubRef.current = t.onMessage((evt: unknown) => {
          try {
            const line = JSON.stringify(evt as BridgeEvent)
            setLogs((prev) => {
              const next = [...prev, line]
              return next.length > 200 ? next.slice(next.length - 200) : next
            })
          } catch {
            // ignore
          }
        })
      } catch {
        setStatus('error')
      }
    })()
    return () => {
      try { unsubRef.current?.() } catch {}
      unsubRef.current = null
      try { transportRef.current?.close() } catch {}
      transportRef.current = null
      setStatus('closed')
    }
  }, [host, token, wsUrl])

  return { status, wsUrl, host, token, logs, sidecarLogs }
}

