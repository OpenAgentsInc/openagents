import React from 'react'
import { ScrollView, Text, View, Pressable, ActivityIndicator } from 'react-native'
import { useBridge } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle, useHeaderSubtitle } from '@/lib/header-store'

export default function ConvexScreen() {
  const ws = useBridge()
  const [loading, setLoading] = React.useState(false)
  const [status, setStatus] = React.useState<{ healthy: boolean; url: string; db: string; tables: string[] } | null>(null)
  useHeaderTitle('Convex')
  useHeaderSubtitle('local persistence')

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
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 20 }}>Tables</Text>
        <Pressable onPress={async () => { setLoading(true); try { const s = await ws.createConvexDemo(); setStatus({ healthy: !!s.healthy, url: s.url, db: s.db, tables: s.tables || [] }) } finally { setLoading(false) } }} accessibilityRole='button' style={{ borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 6 }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>Create demo</Text>
        </Pressable>
      </View>
      <View>
        {(status?.tables || []).length === 0 ? (
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No tables found.</Text>
        ) : (
          status!.tables.map((t) => (
            <Text key={t} style={{ color: Colors.foreground, fontFamily: Typography.primary, paddingVertical: 4 }}>{t}</Text>
          ))
        )}
      </View>
    </ScrollView>
  )
}

