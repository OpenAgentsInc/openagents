import type { ReactNode } from "react"
import type { AccessibilityState, StyleProp, ViewStyle } from "react-native"
import { Gesture, GestureDetector } from "react-native-gesture-handler"
import Animated, { runOnJS, useAnimatedStyle, useSharedValue, withTiming } from "react-native-reanimated"

import { MOTION_FAST } from "../theme/motion"

// `Animated.View`'s `className` support is registered once, globally, via
// `src/native/animated-view-css-interop.ts`, imported from the root layout
// (`app/_layout.tsx`) before any screen mounts.

/** Ported from Arcade's `TouchableFeedback` (see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.7). Drop-in
 * `Pressable` replacement whose press-highlight state lives entirely on the
 * UI thread — a `Gesture.Tap()` flips a shared `active` boolean and
 * `useAnimatedStyle` cross-fades `backgroundColor` between `defaultColor`
 * and `highlightColor` over `MOTION_FAST`, with no React state (and no
 * NativeWind `active:` class recalculation) in the visual feedback path. */
type TouchableFeedbackProps = Readonly<{
  children: ReactNode
  onPress?: () => void
  style?: StyleProp<ViewStyle>
  className?: string
  accessibilityLabel?: string
  accessibilityRole?: "button" | "link" | "none"
  accessibilityState?: AccessibilityState
  disabled?: boolean
  testID?: string
  /** Translucent highlight color shown while pressed. Defaults to the
   * app's `accent/10` opacity-modifier convention. */
  highlightColor?: string
  defaultColor?: string
  /** Expands the tap-recognition area beyond the rendered bounds — the
   * `Gesture.Tap()` equivalent of `Pressable`'s `hitSlop`. Not part of
   * arcade's original `TouchableFeedback`; added when swapping this in as
   * a drop-in `Pressable` replacement for small icon buttons that relied on
   * `hitSlop` to meet a comfortable touch-target size. */
  hitSlop?: number | Readonly<{ top?: number; bottom?: number; left?: number; right?: number }>
}>

const DEFAULT_HIGHLIGHT_COLOR = "rgba(79, 208, 255, 0.1)" // accent/10 (accent = #4fd0ff)
const DEFAULT_COLOR = "transparent"

export const TouchableFeedback = ({
  accessibilityRole,
  accessibilityLabel,
  accessibilityState,
  children,
  className,
  defaultColor = DEFAULT_COLOR,
  disabled = false,
  highlightColor = DEFAULT_HIGHLIGHT_COLOR,
  hitSlop,
  onPress,
  style,
  testID
}: TouchableFeedbackProps) => {
  const active = useSharedValue(false)

  let gesture = Gesture.Tap()
    .enabled(!disabled)
    .maxDuration(4000)

  if (hitSlop !== undefined) gesture = gesture.hitSlop(hitSlop)

  gesture = gesture
    .onBegin(() => {
      active.value = true
    })
    .onTouchesUp(() => {
      if (!disabled && onPress !== undefined) runOnJS(onPress)()
    })
    .onFinalize(() => {
      active.value = false
    })

  const rAnimatedStyle = useAnimatedStyle(() => ({
    backgroundColor: withTiming(active.value ? highlightColor : defaultColor, {
      duration: MOTION_FAST
    })
  }), [highlightColor, defaultColor])

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View
        accessibilityLabel={accessibilityLabel}
        accessibilityRole={accessibilityRole}
        accessibilityState={{ ...accessibilityState, disabled }}
        className={className}
        style={[style, rAnimatedStyle]}
        testID={testID}
      >
        {children}
      </Animated.View>
    </GestureDetector>
  )
}
