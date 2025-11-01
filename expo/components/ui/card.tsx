import React from 'react'
import { View, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from './text'

export type CardProps = {
  title?: string
  children?: React.ReactNode
  style?: StyleProp<ViewStyle>
  headerRight?: React.ReactNode
}

export function Card({ title, children, style, headerRight }: CardProps) {
  return (
    <View style={[{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }, style]}>
      {(title || headerRight) ? (
        <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
          {!!title && <Text variant="subheading">{title}</Text>}
          {headerRight}
        </View>
      ) : null}
      {children}
    </View>
  )
}

export default Card

