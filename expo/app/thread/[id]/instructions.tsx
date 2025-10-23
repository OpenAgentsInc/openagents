import React from 'react'
import { useLocalSearchParams, Stack } from 'expo-router'
import { ScrollView, Text, View } from 'react-native'
import { useThreads } from '@/lib/threads-store'
import { useWs } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function ThreadInstructions() {
  const { id, path } = useLocalSearchParams<{ id: string; path?: string }>()
  const { wsUrl } = useWs()
  const loadThread = useThreads((s) => s.loadThread)
  const thread = useThreads((s) => (id ? s.thread[id] : undefined))
  React.useEffect(() => { if (id) loadThread(wsUrl, id, typeof path === 'string' ? path : undefined).catch(()=>{}) }, [id, path, wsUrl, loadThread])
  const body = thread?.instructions || '(no instructions)'
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ title: 'Instructions', headerBackTitle: '' }} />
      <ScrollView contentContainerStyle={{ padding: 16 }}>
        <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary, lineHeight: 18 }}>{body}</Text>
      </ScrollView>
    </View>
  )
}

