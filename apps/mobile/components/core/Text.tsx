import { ReactNode, forwardRef, ForwardedRef } from "react"
import { StyleProp, Text as RNText, TextProps as RNTextProps, TextStyle, Platform } from "react-native"

type Sizes = keyof typeof $sizeStyles
type Weights = "normal" | "medium" | "bold"
type Presets = "default" | "bold" | "heading" | "subheading" | "formLabel" | "formHelper"

export interface TextProps extends RNTextProps {
  /**
   * The text to display if not using nested components.
   */
  text?: string
  /**
   * An optional style override useful for padding & margin.
   */
  style?: StyleProp<TextStyle>
  /**
   * One of the different types of text presets.
   */
  preset?: Presets
  /**
   * Text weight modifier.
   */
  weight?: Weights
  /**
   * Text size modifier.
   */
  size?: Sizes
  /**
   * Children components.
   */
  children?: ReactNode
}

/**
 * For your text displaying needs.
 * This component is a HOC over the built-in React Native one.
 * Uses our black/gray theme with Berkeley Mono font.
 */
export const Text = forwardRef(function Text(props: TextProps, ref: ForwardedRef<RNText>) {
  const { weight, size, text, children, style: $styleOverride, ...rest } = props

  const content = text || children
  const preset: Presets = props.preset ?? "default"
  
  const $styles: StyleProp<TextStyle> = [
    $presets[preset],
    weight && $fontWeightStyles[weight],
    size && $sizeStyles[size],
    $styleOverride,
  ]

  return (
    <RNText {...rest} style={$styles} ref={ref}>
      {content}
    </RNText>
  )
})

const $sizeStyles = {
  xxl: { fontSize: 36, lineHeight: 44 } satisfies TextStyle,
  xl: { fontSize: 24, lineHeight: 34 } satisfies TextStyle,
  lg: { fontSize: 20, lineHeight: 32 } satisfies TextStyle,
  md: { fontSize: 18, lineHeight: 26 } satisfies TextStyle,
  sm: { fontSize: 16, lineHeight: 24 } satisfies TextStyle,
  xs: { fontSize: 14, lineHeight: 21 } satisfies TextStyle,
  xxs: { fontSize: 12, lineHeight: 18 } satisfies TextStyle,
}

const $fontWeightStyles: Record<Weights, TextStyle> = {
  normal: { 
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
    fontWeight: 'normal' 
  },
  medium: { 
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
    fontWeight: '500' 
  },
  bold: { 
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
    fontWeight: 'bold' 
  },
}

const $baseStyle: TextStyle = {
  ...$sizeStyles.sm,
  ...$fontWeightStyles.normal,
  color: '#f4f4f5', // Zinc-100 text for dark zinc theme
}

const $presets: Record<Presets, TextStyle> = {
  default: $baseStyle,
  bold: { ...$baseStyle, ...$fontWeightStyles.bold },
  heading: {
    ...$baseStyle,
    ...$sizeStyles.xxl,
    ...$fontWeightStyles.bold,
  },
  subheading: { ...$baseStyle, ...$sizeStyles.lg, ...$fontWeightStyles.medium },
  formLabel: { ...$baseStyle, ...$fontWeightStyles.medium },
  formHelper: {
    ...$baseStyle,
    ...$sizeStyles.sm,
    ...$fontWeightStyles.normal,
    color: '#a1a1aa', // Zinc-400 dimmed text for helpers
  },
}