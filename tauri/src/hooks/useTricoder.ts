import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

export type UseTricoderResult = {
  bridgeRunning: boolean
  wsUrl: string
  host: string
  token: string
}

export function useTricoder(): UseTricoderResult {
  const [host, setHost] = useState<string>('')
  const [token, setToken] = useState<string>('')
  const [bridgeRunning, setBridgeRunning] = useState<boolean>(false)

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
      const id = window.setInterval(() => { refreshBridgeStatus().catch(() => {}) }, 2000)
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
      setBridgeRunning(!!s?.running)
    } catch {
      // ignore
    }
  }, [])

  return { bridgeRunning, wsUrl, host, token }
}
