import React from 'react'
import { Text as RNText, type TextProps as RNTextProps, type TextStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export type TextVariant = 'body' | 'heading' | 'subheading' | 'label' | 'caption' | 'mono'
export type TextTone = 'default' | 'secondary' | 'tertiary' | 'success' | 'warning' | 'danger'

export interface TextProps extends Omit<RNTextProps, 'style'> {
  variant?: TextVariant
  tone?: TextTone
  style?: TextStyle | TextStyle[]
}

export function Text({ variant = 'body', tone = 'default', style, children, ...rest }: TextProps) {
  const base: TextStyle = {
    color: toneColor(tone),
    fontFamily: fontFor(variant),
    fontSize: sizeFor(variant),
    lineHeight: lineFor(variant),
  }
  return (
    <RNText {...rest} style={[base, style as any]}> {/* style cast for RN array union typing */}
      {children}
    </RNText>
  )
}

function toneColor(tone: TextTone): string {
  switch (tone) {
    case 'secondary':
      return Colors.secondary
    case 'tertiary':
      return Colors.tertiary
    case 'success':
      return Colors.success
    case 'warning':
      return Colors.warning
    case 'danger':
      return Colors.danger
    case 'default':
    default:
      return Colors.foreground
  }
}

function fontFor(variant: TextVariant): string {
  switch (variant) {
    case 'heading':
      return Typography.bold
    case 'subheading':
    case 'label':
      return Typography.bold
    case 'caption':
      return Typography.primary
    case 'mono':
      return Typography.primary
    case 'body':
    default:
      return Typography.primary
  }
}

function sizeFor(variant: TextVariant): number {
  switch (variant) {
    case 'heading':
      return 18
    case 'subheading':
      return 16
    case 'label':
      return 13
    case 'caption':
      return 11
    case 'mono':
    case 'body':
    default:
      return 14
  }
}

function lineFor(variant: TextVariant): number {
  switch (variant) {
    case 'heading':
      return 24
    case 'subheading':
      return 20
    case 'label':
      return 18
    case 'caption':
      return 14
    case 'mono':
    case 'body':
    default:
      return 18
  }
}

