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
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>URL: {status?.url || '-'}</Text>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>DB: {status?.db || '-'}</Text>
        </View>
      )}
      <View style={{ height: 8 }} />
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
