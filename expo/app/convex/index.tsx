import React from 'react'
import { ScrollView, Text, View, Pressable, ActivityIndicator } from 'react-native'
import { useBridge } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useQuery, useMutation } from 'convex/react'
import { router } from 'expo-router'

export default function ConvexScreen() {
  const ws = useBridge()
  const [loading, setLoading] = React.useState(false)
  const [status, setStatus] = React.useState<{ healthy: boolean; url: string; db: string; tables: string[] } | null>(null)
  useHeaderTitle('Convex')

  const refresh = React.useCallback(async () => {
    setLoading(true)
    try { const s = await ws.requestConvexStatus(); setStatus({ healthy: !!s.healthy, url: s.url, db: s.db, tables: s.tables || [] }) } catch {}
    setLoading(false)
  }, [ws])

  React.useEffect(() => { refresh() }, [])

  // Ingestion progress (optional visual)
  const [ingest, setIngest] = React.useState<{ ok_batch: number; remaining: number } | null>(null)
  React.useEffect(() => {
    const unsub = ws.addSubscriber((line) => {
      try {
        const obj = JSON.parse(String(line || ''))
        if (obj?.type === 'bridge.ingest_progress') {
          setIngest({ ok_batch: Number(obj.ok_batch || 0), remaining: Number(obj.remaining || 0) })
        }
      } catch {}
    })
    return unsub
  }, [ws])

  // Derive the device-reachable Convex URL from Settings (bridgeHost)
  const deviceConvexUrl = React.useMemo(() => {
    try {
      const raw = String(ws.bridgeHost || '').trim()
      const stripped = raw
        .replace(/^ws:\/\//i, '')
        .replace(/^wss:\/\//i, '')
        .replace(/^http:\/\//i, '')
        .replace(/^https:\/\//i, '')
        .replace(/\/$/, '')
        .replace(/\/ws$/i, '')
        .replace(/\/$/, '')
      // If user supplied host:port, take the host portion; else default
      const hostOnly = (stripped.includes(':') ? stripped.split(':')[0] : stripped) || '127.0.0.1'
      return `http://${hostOnly}:7788`
    } catch { return 'http://127.0.0.1:7788' }
  }, [ws.bridgeHost])

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 20 }}>Connection</Text>
        <Pressable onPress={refresh} accessibilityRole='button' style={{ borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 6 }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>Refresh</Text>
        </Pressable>
      </View>
      {loading ? (
        <ActivityIndicator color={Colors.secondary} />
      ) : (
        <View>
          <Text style={{ color: status?.healthy ? Colors.success : Colors.danger, fontFamily: Typography.primary, marginBottom: 6 }}>
            {status?.healthy ? 'Connected' : 'Disconnected'}
          </Text>
          {/* Show the device-reachable URL derived from Settings, not the bridge's loopback */}
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>URL: {deviceConvexUrl}</Text>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>DB: {status?.db || '-'}</Text>
        </View>
      )}
      <View style={{ height: 8 }} />
      {ingest ? (
        <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 8 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 6 }}>Ingesting history… (batch {ingest.ok_batch}, remaining {ingest.remaining})</Text>
          <View style={{ height: 6, backgroundColor: Colors.border }}>
            {/* Simple indeterminate bar */}
            <View style={{ width: '35%', height: '100%', backgroundColor: Colors.secondary }} />
          </View>
        </View>
      ) : null}
      <View style={{ height: 16 }} />
      <CreateDemoConvexRow />
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 20 }}>Live Threads (Convex)</Text>
      <ThreadsList />
    </ScrollView>
  )
}

function ThreadsList() {
  // Subscribe to a generic Convex query named "threads:list" with no args
  // This will live-update if a Convex project with that query is deployed to the backend.
  const result = (useQuery as any)('threads:list', {}) as any
  if (result === undefined) {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Connecting…</Text>
  }
  if (result === null) {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No Convex project deployed or query missing (threads:list).</Text>
  }
  if (!Array.isArray(result)) {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Unexpected result.</Text>
  }
  return (
    <View style={{ gap: 6 }}>
      {result.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No threads yet.</Text>
      ) : result.map((row: any) => (
        <Pressable
          key={row._id || row.id}
          onPress={() => router.push(`/convex/thread/${encodeURIComponent(row._id || row.id)}`)}
          accessibilityRole='button'
          style={{ borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 8 }}
        >
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{row.title || String(row._id || row.id)}</Text>
        </Pressable>
      ))}
    </View>
  )
}

function CreateDemoConvexRow() {
  const createDemo = (useMutation as any)('threads:createDemo') as () => Promise<any>
  const [busy, setBusy] = React.useState(false)
  const onCreate = async () => {
    if (busy) return
    setBusy(true)
    try { await createDemo() } catch {} finally { setBusy(false) }
  }
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 16 }}>Actions</Text>
      <Pressable onPress={onCreate} accessibilityRole='button' disabled={busy} style={{ opacity: busy ? 0.6 : 1, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 6 }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>Create demo thread (Convex)</Text>
      </Pressable>
    </View>
  )
}
