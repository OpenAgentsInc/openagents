import React from 'react'
import { ScrollView, View, type ViewProps, type StyleProp, type ViewStyle } from 'react-native'
import { SafeAreaView } from 'react-native-safe-area-context'
import { Colors } from '@/constants/theme'

export type ScreenPreset = 'fixed' | 'scroll'

export interface ScreenProps extends Omit<ViewProps, 'style'> {
  preset?: ScreenPreset
  contentContainerStyle?: StyleProp<ViewStyle>
  style?: StyleProp<ViewStyle>
}

export function Screen({ preset = 'fixed', style, contentContainerStyle, children, ...rest }: ScreenProps) {
  if (preset === 'scroll') {
    return (
      <SafeAreaView style={[{ flex: 1, backgroundColor: Colors.background }, style] as any}>
        <ScrollView contentContainerStyle={contentContainerStyle}>{children}</ScrollView>
      </SafeAreaView>
    )
  }
  return (
    <SafeAreaView style={[{ flex: 1, backgroundColor: Colors.background }, style] as any} {...rest}>
      <View style={contentContainerStyle as any}>{children}</View>
    </SafeAreaView>
  )
}

