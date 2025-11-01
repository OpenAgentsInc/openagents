import React from 'react'
import { Pressable, ActivityIndicator, type GestureResponderEvent, type PressableProps, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from '@/components/ui/text'

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'destructive'
export type ButtonSize = 'sm' | 'md' | 'lg'

export interface ButtonProps extends Omit<PressableProps, 'onPress' | 'style'> {
  title?: string
  onPress?: (e: GestureResponderEvent) => void
  variant?: ButtonVariant
  size?: ButtonSize
  loading?: boolean
  left?: React.ReactNode
  right?: React.ReactNode
  style?: StyleProp<ViewStyle>
}

export function Button({
  title,
  onPress,
  variant = 'primary',
  size = 'md',
  loading = false,
  disabled,
  left,
  right,
  style,
  children,
  ...rest
}: ButtonProps) {
  const [bg, fg, border] = colorsFor(variant, disabled || loading)
  const [padV, padH, fontSize] = metricsFor(size)
  return (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      disabled={disabled || loading}
      {...rest}
      style={[{ backgroundColor: bg, borderColor: border, borderWidth: 1, paddingVertical: padV, paddingHorizontal: padH, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8 }, style]}
    >
      {left}
      {loading ? (
        <ActivityIndicator color={fg} />
      ) : (
        <Text style={{ color: fg, fontSize }} variant="label">
          {title ?? children}
        </Text>
      )}
      {right}
    </Pressable>
  )
}

function colorsFor(variant: ButtonVariant, disabled?: boolean): [string, string, string] {
  if (disabled) return [Colors.border, Colors.tertiary, Colors.border]
  switch (variant) {
    case 'secondary':
      return [Colors.card, Colors.foreground, Colors.border]
    case 'ghost':
      return [Colors.transparent, Colors.foreground, Colors.border]
    case 'destructive':
      return [Colors.destructive, Colors.destructiveForeground, Colors.destructive]
    case 'primary':
    default:
      return [Colors.foreground, Colors.primaryForeground, Colors.foreground]
  }
}

function metricsFor(size: ButtonSize): [number, number, number] {
  switch (size) {
    case 'sm':
      return [6, 10, 12]
    case 'lg':
      return [12, 16, 16]
    case 'md':
    default:
      return [10, 14, 14]
  }
}

