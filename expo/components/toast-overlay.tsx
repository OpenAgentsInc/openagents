import React from 'react'
import { View, Text } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { useToastStore } from '@/lib/toast-store'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ToastOverlay() {
  const insets = useSafeAreaInsets()
  const toasts = useToastStore((s) => s.toasts)
  if (!Array.isArray(toasts) || toasts.length === 0) return null
  return (
    <View pointerEvents="none" style={{ position: 'absolute', right: 12, bottom: Math.max(insets.bottom, 8) + 8, gap: 8 }}>
      {toasts.map((t) => (
        <View key={t.id} style={{ maxWidth: 300, borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, paddingVertical: 8, paddingHorizontal: 10 }}>
          <Text style={{ color: colorFor(t.type), fontFamily: Typography.primary }}>{t.text}</Text>
        </View>
      ))}
    </View>
  )
}

function colorFor(type: 'info' | 'success' | 'error') {
  switch (type) {
    case 'success':
      return Colors.success
    case 'error':
      return Colors.danger
    case 'info':
    default:
      return Colors.foreground
  }
}

