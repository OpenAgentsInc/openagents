import React from 'react'
import { ConvexProvider, ConvexReactClient } from 'convex/react'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'

function sanitizeHost(raw: string): string {
  try {
    const val = String(raw || '').trim()
    if (!val) return '127.0.0.1:8787'
    const stripped = val
      .replace(/^ws:\/\//i, '')
      .replace(/^wss:\/\//i, '')
      .replace(/^http:\/\//i, '')
      .replace(/^https:\/\//i, '')
      .replace(/\/$/, '')
      .replace(/\/ws$/i, '')
      .replace(/\/$/, '')
    return stripped || '127.0.0.1:8787'
  } catch {
    return '127.0.0.1:8787'
  }
}

export function ConvexProviderLocal({ children }: { children: React.ReactNode }) {
  const { bridgeHost } = useBridge()
  const convexOverride = useSettings((s) => s.convexUrl)
  const convexUrl = React.useMemo(() => {
    const ov = String(convexOverride || '').trim()
    if (ov) {
      try {
        const u = new URL(ov)
        if (u.protocol === 'http:' || u.protocol === 'https:') {
          const norm = u.toString().replace(/\/$/, '')
          return norm
        }
      } catch {
        // ignore invalid while user is typing; fall back below
      }
    }
    const hostPort = sanitizeHost(bridgeHost)
    const hostOnly = hostPort.split(':')[0] || '127.0.0.1'
    return `http://${hostOnly}:7788`
  }, [bridgeHost, convexOverride])

  const client = React.useMemo(() => {
    try { console.log('[convex] client url =', convexUrl) } catch {}
    return new ConvexReactClient(convexUrl, { verbose: true })
  }, [convexUrl])

  return <ConvexProvider client={client}>{children}</ConvexProvider>
}
