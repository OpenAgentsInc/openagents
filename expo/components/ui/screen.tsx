import React from 'react'
import { ScrollView, View, type StyleProp, type ViewStyle } from 'react-native'
import { SafeAreaView } from 'react-native-safe-area-context'
import { Colors } from '@/constants/theme'

export type ScreenPreset = 'fixed' | 'scroll'

export type ScreenProps = {
  preset?: ScreenPreset
  safeTop?: boolean
  safeBottom?: boolean
  style?: StyleProp<ViewStyle>
  contentContainerStyle?: StyleProp<ViewStyle>
  children?: React.ReactNode
  testID?: string
}

export function Screen({
  preset = 'fixed',
  safeTop = true,
  safeBottom = true,
  style,
  contentContainerStyle,
  children,
  testID,
}: ScreenProps) {
  const inner = (
    preset === 'scroll' ? (
      <ScrollView
        testID={testID}
        style={{ flex: 1 }}
        contentContainerStyle={[{ padding: 16 }, contentContainerStyle]}
      >
        {children}
      </ScrollView>
    ) : (
      <View testID={testID} style={[{ flex: 1, padding: 16 }, contentContainerStyle]}>
        {children}
      </View>
    )
  )

  return (
    <SafeAreaView style={[{ flex: 1, backgroundColor: Colors.background }, style]} edges={[safeTop ? 'top' : undefined, safeBottom ? 'bottom' : undefined].filter(Boolean) as ('top'|'bottom')[]}>
      {inner}
    </SafeAreaView>
  )
}

export default Screen

