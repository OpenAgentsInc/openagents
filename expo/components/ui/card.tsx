import React from 'react'
import { View, type ViewProps, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from '@/components/ui/text'

export interface CardProps extends Omit<ViewProps, 'style'> {
  title?: string
  headerRight?: React.ReactNode
  style?: StyleProp<ViewStyle>
  contentStyle?: StyleProp<ViewStyle>
}

export function Card({ title, headerRight, style, contentStyle, children, ...rest }: CardProps) {
  return (
    <View {...rest} style={[{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }, style]}>
      {(title || headerRight) ? (
        <View style={{ flexDirection: 'row', alignItems: 'center', marginBottom: 6 }}>
          {title ? <Text variant="label">{title}</Text> : null}
          <View style={{ marginLeft: 'auto' }} />
          {headerRight}
        </View>
      ) : null}
      <View style={contentStyle}>{children}</View>
    </View>
  )
}

