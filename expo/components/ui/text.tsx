import React from 'react'
import { Text as RNText, type TextProps as RNTextProps, type TextStyle, type StyleProp } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export type TextVariant = 'body' | 'heading' | 'subheading' | 'label' | 'caption' | 'mono'
export type TextTone = 'default' | 'secondary' | 'tertiary' | 'danger' | 'success' | 'warning'

export type TextProps = Omit<RNTextProps, 'style'> & {
  variant?: TextVariant
  tone?: TextTone
  style?: StyleProp<TextStyle>
}

export function Text({ variant = 'body', tone = 'default', style, children, ...rest }: TextProps) {
  const variantStyle: TextStyle = (() => {
    switch (variant) {
      case 'heading':
        return { fontFamily: Typography.bold, fontSize: 20 }
      case 'subheading':
        return { fontFamily: Typography.bold, fontSize: 16 }
      case 'label':
        return { fontFamily: Typography.bold, fontSize: 12, textTransform: 'uppercase', letterSpacing: 0.4 }
      case 'caption':
        return { fontFamily: Typography.primary, fontSize: 12 }
      case 'mono':
        return { fontFamily: Typography.primary, fontSize: 14 }
      case 'body':
      default:
        return { fontFamily: Typography.primary, fontSize: 14 }
    }
  })()

  const toneColor = (() => {
    switch (tone) {
      case 'secondary':
        return Colors.secondary
      case 'tertiary':
        return Colors.tertiary
      case 'danger':
        return Colors.danger
      case 'success':
        return Colors.success
      case 'warning':
        return Colors.warning
      case 'default':
      default:
        return Colors.foreground
    }
  })()

  return (
    <RNText {...rest} style={[{ color: toneColor }, variantStyle, style]}>
      {children}
    </RNText>
  )
}

export default Text

