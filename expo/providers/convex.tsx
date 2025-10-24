import React from 'react'
import { ConvexProvider, ConvexReactClient } from 'convex/react'
import { useBridge } from '@/providers/ws'

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
  const convexUrl = React.useMemo(() => {
    const hostPort = sanitizeHost(bridgeHost)
    const hostOnly = hostPort.split(':')[0] || '127.0.0.1'
    return `http://${hostOnly}:7788`
  }, [bridgeHost])

  const client = React.useMemo(() => new ConvexReactClient(convexUrl, { verbose: false }), [convexUrl])

  return <ConvexProvider client={client}>{children}</ConvexProvider>
}
