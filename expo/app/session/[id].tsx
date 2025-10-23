import React from 'react'
import { useLocalSearchParams, useRouter, Stack } from 'expo-router'
import { ActivityIndicator, FlatList, Pressable, Text, View } from 'react-native'
import { useSessions } from '@/lib/sessions-store'
import { useWs } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'

export default function SessionHistoryView() {
  const { id, path } = useLocalSearchParams<{ id: string; path?: string }>()
  const { wsUrl, setResumeNextId } = useWs()
  const router = useRouter()
  const loadSession = useSessions((s) => s.loadSession)
  const session = useSessions((s) => (id ? s.session[id] : undefined))
  const loadingMap = useSessions((s) => s.loadingSession)
  const loading = Boolean(id && loadingMap[id])

  React.useEffect(() => { if (id) loadSession(wsUrl, id, typeof path === 'string' ? path : undefined).catch(()=>{}) }, [id, path, wsUrl, loadSession])
  useHeaderTitle(session?.title || 'Session')

  const items = session?.items || []

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ title: '', headerBackTitle: '' }} />
      {loading && items.length === 0 ? (
        <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
          <ActivityIndicator color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginTop: 8 }}>Loading…</Text>
        </View>
      ) : (
        <FlatList
          data={items}
          keyExtractor={(_, i) => String(i)}
          contentContainerStyle={{ padding: 12, gap: 10, paddingBottom: 80 }}
          renderItem={({ item }) => <Row it={item} />}
          ListFooterComponent={id ? (
            <View style={{ marginTop: 12 }}>
              <Pressable
                onPress={() => { setResumeNextId(id); router.push('/(tabs)/session') }}
                style={{ alignSelf: 'flex-start', backgroundColor: Colors.card, borderColor: Colors.border, borderWidth: 1, paddingHorizontal: 10, paddingVertical: 8 }}
                accessibilityRole="button"
              >
                <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Continue chat</Text>
              </Pressable>
            </View>
          ) : null}
        />
      )}
    </View>
  )
}

function Row({ it }: { it: { ts: number; kind: 'message'|'reason'|'cmd'; role?: 'assistant'|'user'; text: string } }) {
  if (it.kind === 'message' && it.role === 'assistant') {
    return (
      <View style={{ borderColor: Colors.border, borderWidth: 1, backgroundColor: Colors.card, padding: 8 }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{it.text}</Text>
      </View>
    )
  }
  if (it.kind === 'message' && it.role === 'user') {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>You: {it.text}</Text>
  }
  if (it.kind === 'reason') {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
  }
  if (it.kind === 'cmd') {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
  }
  return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
}
