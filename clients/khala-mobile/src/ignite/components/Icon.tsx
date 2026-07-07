import {
  StyleProp,
  TextStyle,
  TouchableOpacity,
  TouchableOpacityProps,
  View,
  ViewProps,
  ViewStyle,
} from "react-native"

import { useAppTheme } from "../theme/context"

import { Text } from "./Text"

/**
 * Minimal Icon for the port. The full Ignite `Icon` maps a fixed registry of
 * named keys to bundled PNG image assets. This port keeps the same
 * `Icon` / `PressableIcon` surface but renders the `icon` string itself as a
 * unicode glyph via `<Text>` — no image assets required. `IconTypes` is a plain
 * `string`, so callers pass the glyph directly (e.g. `icon="☰"`).
 */
export type IconTypes = string

type BaseIconProps = {
  /**
   * The glyph to render.
   */
  icon: IconTypes
  /**
   * An optional tint color for the icon.
   */
  color?: string
  /**
   * An optional size (font size) for the glyph.
   */
  size?: number
  /**
   * Style overrides for the glyph text.
   */
  style?: StyleProp<TextStyle>
  /**
   * Style overrides for the icon container.
   */
  containerStyle?: StyleProp<ViewStyle>
}

type PressableIconProps = Omit<TouchableOpacityProps, "style"> & BaseIconProps
type IconProps = Omit<ViewProps, "style"> & BaseIconProps

/**
 * A pressable glyph icon, wrapped in a <TouchableOpacity />.
 */
export function PressableIcon(props: PressableIconProps) {
  const {
    icon,
    color,
    size = 24,
    style: $styleOverride,
    containerStyle: $containerStyleOverride,
    ...pressableProps
  } = props

  const { theme } = useAppTheme()

  return (
    <TouchableOpacity {...pressableProps} style={$containerStyleOverride}>
      <Text
        style={[
          { color: color ?? theme.colors.text, fontSize: size, lineHeight: size + 2 },
          $styleOverride,
        ]}
      >
        {icon}
      </Text>
    </TouchableOpacity>
  )
}

/**
 * A glyph icon, wrapped in a <View />. Use `PressableIcon` to react to input.
 */
export function Icon(props: IconProps) {
  const {
    icon,
    color,
    size = 24,
    style: $styleOverride,
    containerStyle: $containerStyleOverride,
    ...viewProps
  } = props

  const { theme } = useAppTheme()

  return (
    <View {...viewProps} style={$containerStyleOverride}>
      <Text
        style={[
          { color: color ?? theme.colors.text, fontSize: size, lineHeight: size + 2 },
          $styleOverride,
        ]}
      >
        {icon}
      </Text>
    </View>
  )
}
