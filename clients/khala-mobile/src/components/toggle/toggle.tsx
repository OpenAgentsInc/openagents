import type { FC } from "react"
import { useCallback, useState } from "react"
import type { AccessibilityRole, LayoutChangeEvent } from "react-native"
import { Pressable, Text, View } from "react-native"
import Animated, { useAnimatedStyle, withTiming } from "react-native-reanimated"

import { toggleKnobTargetPosition } from "../../sync/toggle-position-core"
import { MOTION_FAST } from "../../theme/motion"

/** Ported from Arcade's `Toggle.tsx` (see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.8 and issue #8398).
 * Three variants — `checkbox`/`radio`/`switch` — recolored to the
 * `accent`/`surface*` token set. The `switch` variant is the interesting
 * one: its knob slides between two percentage anchors (`0%`/`100%`) offset by
 * its own measured width (via `onLayout`), computed by the pure
 * `toggleKnobTargetPosition` (`../../sync/toggle-position-core.ts`) rather
 * than hardcoding pixel positions against an assumed track width.
 *
 * Deviation from Arcade: dropped `TouchableOpacity`/i18n (`tx`)/helper-text
 * plumbing that has no current call site in this app — this is the scoped,
 * directly-useful subset (variant, controlled `value`/`onValueChange`,
 * `disabled`, an optional inline `label`), following the same "small,
 * correct, ready" posture the issue calls for. Currently unused —
 * `app/(drawer)/settings.tsx` has no toggle UI yet; land ready for the first
 * real settings toggle rather than inventing one. */
export type ToggleVariant = "checkbox" | "radio" | "switch"

export type ToggleProps = Readonly<{
  /** The variant of the toggle. Default: `"checkbox"`. */
  variant?: ToggleVariant
  /** Controlled value — the caller owns state and must update it from
   * `onValueChange` for the toggle to visually reflect user action. */
  value: boolean
  onValueChange?: (value: boolean) => void
  disabled?: boolean
  /** Optional inline label rendered to the toggle's right. */
  label?: string
  accessibilityLabel?: string
  className?: string
}>

type ToggleInputProps = Readonly<{
  on: boolean
  disabled: boolean
}>

const SWITCH_TRACK_WIDTH = 56
const SWITCH_TRACK_HEIGHT = 32
const SWITCH_TRACK_PADDING = 4
/** Rest-position fallback used only until the knob's first `onLayout` fires
 * (matches the knob's own fixed style width below, so there is no visible
 * jump once the real measurement arrives). */
const SWITCH_KNOB_FALLBACK_WIDTH = SWITCH_TRACK_HEIGHT - SWITCH_TRACK_PADDING * 2

const Checkbox = ({ disabled, on }: ToggleInputProps) => {
  const overlayStyle = useAnimatedStyle(() => ({
    opacity: withTiming(on ? 1 : 0, { duration: MOTION_FAST })
  }), [on])

  return (
    <View
      className={`h-6 w-6 items-center justify-center overflow-hidden rounded-md border ${
        disabled ? "border-borderMuted bg-surfaceMuted" : "border-border bg-surfaceMuted"
      }`}
    >
      <Animated.View
        className={`h-full w-full items-center justify-center ${disabled ? "bg-textFaint" : "bg-accent"}`}
        style={overlayStyle}
      >
        <Text className={`font-sans text-xs font-bold ${disabled ? "text-surfaceMuted" : "text-bg"}`}>
          ✓
        </Text>
      </Animated.View>
    </View>
  )
}

const Radio = ({ disabled, on }: ToggleInputProps) => {
  const overlayStyle = useAnimatedStyle(() => ({
    opacity: withTiming(on ? 1 : 0, { duration: MOTION_FAST })
  }), [on])

  return (
    <View
      className={`h-6 w-6 items-center justify-center overflow-hidden rounded-full border ${
        disabled ? "border-borderMuted bg-surfaceMuted" : "border-border bg-surfaceMuted"
      }`}
    >
      <Animated.View className="h-full w-full items-center justify-center" style={overlayStyle}>
        <View className={`h-3 w-3 rounded-full ${disabled ? "bg-textFaint" : "bg-accent"}`} />
      </Animated.View>
    </View>
  )
}

const Switch = ({ disabled, on }: ToggleInputProps) => {
  const [knobWidth, setKnobWidth] = useState(SWITCH_KNOB_FALLBACK_WIDTH)

  const onKnobLayout = useCallback((event: LayoutChangeEvent) => {
    const measuredWidth = event.nativeEvent.layout.width
    setKnobWidth(previous => (previous === measuredWidth ? previous : measuredWidth))
  }, [])

  const trackOverlayStyle = useAnimatedStyle(() => ({
    opacity: withTiming(on ? 1 : 0, { duration: MOTION_FAST })
  }), [on])

  const knobStyle = useAnimatedStyle(() => {
    const target = toggleKnobTargetPosition({
      knobWidth,
      offsetLeft: SWITCH_TRACK_PADDING,
      offsetRight: SWITCH_TRACK_PADDING,
      on
    })

    return {
      marginStart: withTiming(target.marginStart, { duration: MOTION_FAST }),
      start: withTiming(target.start, { duration: MOTION_FAST })
    }
  }, [on, knobWidth])

  return (
    <View
      className={`justify-center overflow-hidden rounded-full border ${
        disabled ? "border-borderMuted bg-surfaceMuted" : "border-border bg-surfaceMuted"
      }`}
      style={{ height: SWITCH_TRACK_HEIGHT, width: SWITCH_TRACK_WIDTH }}
    >
      <Animated.View
        className={`absolute inset-0 rounded-full ${disabled ? "bg-textFaint" : "bg-accent"}`}
        style={trackOverlayStyle}
      />
      <Animated.View
        className={`absolute rounded-full ${disabled ? "bg-surfaceMuted" : "bg-text"}`}
        onLayout={onKnobLayout}
        style={[
          { height: SWITCH_KNOB_FALLBACK_WIDTH, width: SWITCH_KNOB_FALLBACK_WIDTH },
          knobStyle
        ]}
      />
    </View>
  )
}

const TOGGLE_INPUTS: Record<ToggleVariant, FC<ToggleInputProps>> = {
  checkbox: Checkbox,
  radio: Radio,
  switch: Switch
}

export const Toggle = ({
  accessibilityLabel,
  className,
  disabled = false,
  label,
  onValueChange,
  value,
  variant = "checkbox"
}: ToggleProps) => {
  const ToggleInput = TOGGLE_INPUTS[variant]

  const handlePress = () => {
    if (disabled) return
    onValueChange?.(!value)
  }

  return (
    <Pressable
      accessibilityLabel={accessibilityLabel ?? label}
      accessibilityRole={variant as AccessibilityRole}
      accessibilityState={{ checked: value, disabled }}
      className={`flex-row items-center gap-3 ${disabled ? "opacity-40" : ""} ${className ?? ""}`}
      disabled={disabled}
      onPress={handlePress}
    >
      <ToggleInput disabled={disabled} on={value} />
      {label === undefined ? null : (
        <Text className="flex-1 font-sans text-sm text-text">{label}</Text>
      )}
    </Pressable>
  )
}
