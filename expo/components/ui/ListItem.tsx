import React from 'react'
import { Pressable, View, type PressableProps } from 'react-native'
import { Text } from '@/components/ui/Text'
import { Colors } from '@/constants/theme'
import { Ionicons } from '@expo/vector-icons'

export interface ListItemProps extends Omit<PressableProps, 'style'> {
  title: string
  subtitle?: string
  left?: React.ReactNode
  right?: React.ReactNode
  showChevron?: boolean
}

export function ListItem({ title, subtitle, left, right, showChevron = false, ...rest }: ListItemProps) {
  return (
    <Pressable accessibilityRole="button" {...rest} style={{ paddingVertical: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        {left}
        <View style={{ flex: 1 }}>
          <Text variant="body">{title}</Text>
          {subtitle ? <Text tone="secondary" variant="caption">{subtitle}</Text> : null}
        </View>
        {right}
        {showChevron ? <Ionicons name="chevron-forward" size={16} color={Colors.secondary} /> : null}
      </View>
    </Pressable>
  )
}
