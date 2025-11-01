import React from 'react'
import { Pressable, View, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from './text'
import { Ionicons } from '@expo/vector-icons'

export type ListItemProps = {
  title: string
  subtitle?: string
  left?: React.ReactNode
  right?: React.ReactNode
  onPress?: () => void
  chevron?: boolean
  testID?: string
  style?: StyleProp<ViewStyle>
}

export function ListItem({ title, subtitle, left, right, onPress, chevron = false, testID, style }: ListItemProps) {
  return (
    <Pressable onPress={onPress} disabled={!onPress} accessibilityRole={onPress ? 'button' : undefined} testID={testID}>
      <View style={[{ flexDirection: 'row', alignItems: 'center', paddingVertical: 10, gap: 12 }, style]}>
        {left}
        <View style={{ flex: 1 }}>
          <Text variant="body">{title}</Text>
          {!!subtitle && <Text variant="caption" tone="tertiary" style={{ marginTop: 2 }}>{subtitle}</Text>}
        </View>
        {right}
        {chevron && <Ionicons name="chevron-forward" size={16} color={Colors.tertiary} />}
      </View>
    </Pressable>
  )
}

export default ListItem

