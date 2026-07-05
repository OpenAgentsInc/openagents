import type { ReactNode } from "react"
import type { StyleProp, ViewStyle } from "react-native"
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
  accessibilityRole?: "button" | "link" | "none"
  /** Translucent highlight color shown while pressed. Defaults to the
   * app's `accent/10` opacity-modifier convention (see `Pill` in
   * `src/components/shell.tsx`). */
  highlightColor?: string
  defaultColor?: string
}>

const DEFAULT_HIGHLIGHT_COLOR = "rgba(79, 208, 255, 0.1)" // accent/10 (accent = #4fd0ff)
const DEFAULT_COLOR = "transparent"

export const TouchableFeedback = ({
  accessibilityRole,
  children,
  className,
  defaultColor = DEFAULT_COLOR,
  highlightColor = DEFAULT_HIGHLIGHT_COLOR,
  onPress,
  style
}: TouchableFeedbackProps) => {
  const active = useSharedValue(false)

  const gesture = Gesture.Tap()
    .maxDuration(4000)
    .onBegin(() => {
      active.value = true
    })
    .onTouchesUp(() => {
      if (onPress !== undefined) runOnJS(onPress)()
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
        accessibilityRole={accessibilityRole}
        className={className}
        style={[style, rAnimatedStyle]}
      >
        {children}
      </Animated.View>
    </GestureDetector>
  )
}
