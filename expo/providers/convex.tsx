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
  const { bridgeHost, connected } = useBridge()
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
    if (!hostPort) return ''
    const hostOnly = hostPort.split(':')[0] || ''
    if (!hostOnly) return ''
    return `http://${hostOnly}:7788`
  }, [bridgeHost, convexOverride])

  // Maintain a stable inert client and swap in a live client when available.
  // Do not proactively close clients to avoid "ConvexReactClient has already been closed" during re-renders.
  const inertRef = React.useRef<ConvexReactClient | null>(null)
  if (!inertRef.current) {
    inertRef.current = new ConvexReactClient('http://127.0.0.1:1', { verbose: false })
  }
  const [client, setClient] = React.useState<ConvexReactClient>(() => inertRef.current!)

  React.useEffect(() => {
    if (!connected || !convexUrl) {
      // Revert to inert without closing the prior client to prevent hook errors
      setClient(inertRef.current!)
      return
    }
    try { console.log('[convex] client url =', convexUrl) } catch {}
    const live = new ConvexReactClient(convexUrl, { verbose: false })
    setClient(live)
    // Intentionally skip closing the previous client here to avoid races with consumers.
  }, [connected, convexUrl])

  return <ConvexProvider client={client}>{children}</ConvexProvider>
}
