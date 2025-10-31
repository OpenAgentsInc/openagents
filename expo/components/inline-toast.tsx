import React from 'react'
import { View, Text, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function InlineToast({ text, position = 'bottom', align = 'right' }: { text: string; position?: 'top'|'bottom'; align?: 'left'|'right' }) {
  const top = position === 'top'
  const right = align === 'right'
  const pos: ViewStyle = ({ position: 'absolute', [top ? 'top' : 'bottom']: 2, [right ? 'right' : 'left']: 2 } as unknown) as ViewStyle
  return (
    <View pointerEvents="none" style={pos}>
      <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, paddingVertical: 4, paddingHorizontal: 8 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{text}</Text>
      </View>
    </View>
  )
}
