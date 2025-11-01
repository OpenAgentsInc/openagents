import React from 'react'
import { ActivityIndicator, Pressable, View, type GestureResponderEvent, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from './text'

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'destructive'
export type ButtonSize = 'sm' | 'md' | 'lg'

export type ButtonProps = {
  label?: string
  variant?: ButtonVariant
  size?: ButtonSize
  disabled?: boolean
  loading?: boolean
  leftIcon?: React.ReactNode
  rightIcon?: React.ReactNode
  onPress?: (e: GestureResponderEvent) => void
  testID?: string
  style?: StyleProp<ViewStyle>
}

export function Button({
  label,
  variant = 'primary',
  size = 'md',
  disabled = false,
  loading = false,
  leftIcon,
  rightIcon,
  onPress,
  testID,
  style,
}: ButtonProps) {
  const [pressed, setPressed] = React.useState(false)

  const dims = (() => {
    switch (size) {
      case 'sm':
        return { padV: 8, padH: 12, font: 12 }
      case 'lg':
        return { padV: 14, padH: 18, font: 16 }
      case 'md':
      default:
        return { padV: 10, padH: 14, font: 14 }
    }
  })()

  const scheme = (() => {
    switch (variant) {
      case 'secondary':
        return { bg: Colors.card, fg: Colors.foreground, border: Colors.border }
      case 'ghost':
        return { bg: Colors.transparent, fg: Colors.foreground, border: Colors.border }
      case 'destructive':
        return { bg: Colors.destructive, fg: Colors.destructiveForeground, border: Colors.destructive }
      case 'primary':
      default:
        return { bg: Colors.foreground, fg: Colors.primaryForeground, border: Colors.foreground }
    }
  })()

  const opacity = disabled ? 0.5 : pressed ? 0.85 : 1

  return (
    <Pressable
      accessibilityRole="button"
      testID={testID}
      disabled={disabled || loading}
      onPress={onPress}
      onPressIn={() => setPressed(true)}
      onPressOut={() => setPressed(false)}
      style={[{ opacity }, style]}
    >
      <View
        style={{
          backgroundColor: scheme.bg,
          borderColor: scheme.border,
          borderWidth: 1,
          paddingHorizontal: dims.padH,
          paddingVertical: dims.padV,
          borderRadius: 0,
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 8,
        }}
      >
        {loading ? <ActivityIndicator color={scheme.fg} /> : leftIcon}
        {!!label && <Text style={{ color: scheme.fg, fontSize: dims.font }} variant="mono">{label}</Text>}
        {!loading && rightIcon}
      </View>
    </Pressable>
  )
}

export default Button

