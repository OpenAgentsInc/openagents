import React from 'react'
import { FlatList, Text, View } from 'react-native'
import { useHeaderTitle } from '@/lib/header-store'
import { useAppLogStore } from '@/lib/app-log'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function AppLogs() {
  useHeaderTitle('Logs')
  const logs = useAppLogStore((s) => s.logs)

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <FlatList
        data={logs}
        keyExtractor={(_, i) => String(i)}
        contentContainerStyle={{ padding: 12, gap: 10 }}
        renderItem={({ item }) => (
          <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 8 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11 }}>
              {new Date(item.ts).toLocaleTimeString()} · {item.level.toUpperCase()} · {item.event}
            </Text>
            {!!item.details && (
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>
                {safeStringify(item.details)}
              </Text>
            )}
          </View>
        )}
      />
    </View>
  )
}

function safeStringify(x: any): string {
  try { return typeof x === 'string' ? x : JSON.stringify(x) } catch { return String(x) }
}

