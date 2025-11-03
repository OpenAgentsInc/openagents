import React from 'react'
import { View, ScrollView, Text, Pressable } from 'react-native'
import * as Clipboard from 'expo-clipboard'
import { useHeaderTitle } from '@/lib/header-store'
import { useAppLogStore } from '@openagentsinc/core'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { AnsiText } from '@/components/ansi-text'
import { useIsDevEnv } from '@/lib/env'

export default function DeveloperLogsScreen() {
  useHeaderTitle('Logs')
  const isDev = useIsDevEnv()
  const logs = useAppLogStore((s) => s.logs)
  const clear = useAppLogStore((s) => s.clear)
  const lines = React.useMemo(() => {
    const list = Array.isArray(logs) ? logs : []
    // Render latest 1000 to avoid pathological growth during long sessions
    return list.slice(-1000).map((l, idx) => {
      let text = ''
      try {
        const details = l.details != null ? (typeof l.details === 'string' ? l.details : JSON.stringify(l.details)) : ''
        if (l.event === 'bridge.sidecar' && details) {
          const obj = l.details as any
          text = String(obj && obj.line ? obj.line : details)
        } else {
          text = `[${l.level}] ${l.event}${details ? ' ' + details : ''}`
        }
      } catch { text = `${l.event}` }
      const anyL: any = l as any
      return { id: (anyL.id as string | undefined) ?? `${l.ts}-${idx}`, text }
    })
  }, [logs])
  const scrollRef = React.useRef<ScrollView>(null)
  React.useEffect(() => { try { scrollRef.current?.scrollToEnd({ animated: false }) } catch {} }, [lines.length])

  if (!isDev) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>Logs are available in dev mode.</Text>
      </View>
    )
  }

  const copyAll = React.useCallback(async () => {
    try {
      const blob = lines.map((l) => l.text).join('\n')
      await Clipboard.setStringAsync(blob)
      // Use toast to confirm
      try { (await import('@openagentsinc/core')).useToastStore.getState().enqueue({ text: 'Logs copied to clipboard', type: 'success', duration: 1600 }) } catch {}
    } catch {}
  }, [lines])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 12 }}>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8, flexWrap: 'wrap', gap: 8 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Live Console</Text>
        <View style={{ flexDirection: 'row', gap: 8 }}>
          <Pressable
            testID="developer-logs-copy"
            onPress={copyAll}
            accessibilityRole="button"
            style={{ paddingHorizontal: 10, paddingVertical: 6, borderWidth: 1, borderColor: Colors.border, borderRadius: 6 }}
          >
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 12 }}>Copy</Text>
          </Pressable>
          <Pressable
            testID="developer-logs-clear"
            onPress={() => { try { clear() } catch {} }}
            accessibilityRole="button"
            style={{ paddingHorizontal: 10, paddingVertical: 6, borderWidth: 1, borderColor: Colors.border, borderRadius: 6 }}
          >
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 12 }}>Clear</Text>
          </Pressable>
        </View>
      </View>
      <ScrollView
        ref={scrollRef}
        style={{ flex: 1, borderWidth: 1, borderColor: Colors.border, borderRadius: 6, backgroundColor: '#0e0e0e', maxWidth: '100%', width: '100%', overflow: 'hidden' }}
        contentContainerStyle={{ padding: 10, gap: 2, width: '100%' }}
      >
        {lines.map((l) => (
          <AnsiText key={l.id} line={l.text} />
        ))}
      </ScrollView>
    </View>
  )
}
