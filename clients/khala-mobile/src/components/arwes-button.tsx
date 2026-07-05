import type { ReactNode } from "react"
import type { AccessibilityRole, StyleProp, ViewStyle } from "react-native"
import { Gesture, GestureDetector } from "react-native-gesture-handler"
import Animated, { runOnJS, useSharedValue } from "react-native-reanimated"

import type { SkiaFrameProps } from "./frame"
import { Frame } from "./frame"

/** Ported from Arcade's `ArwesButton` (`app/components/ArwesButton/index.tsx`,
 * see `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.2). A
 * `Gesture.Tap()` wrapper that flips a shared boolean driving `Frame`'s
 * `highlighted` glow-fill state (opacity 0.2 -> 0.7) directly from a gesture
 * worklet — press feedback never round-trips through React state.
 *
 * Arcade mutated a Skia `useValue(false)` (needing `runOnJS` just to write
 * it). This repo's Skia consumes Reanimated `SharedValue`s directly (see
 * `../animation/use-shared-value-effect.ts`), so `highlighted` here is a
 * plain Reanimated `SharedValue<boolean>` mutated straight from the worklet —
 * `runOnJS` is only needed for the JS-thread `onPress`/`onPressIn` callbacks
 * themselves. */
type ArwesButtonProps = Omit<SkiaFrameProps, "height" | "width" | "highlighted"> &
  Readonly<{
    children?: ReactNode
    style?: StyleProp<ViewStyle>
    onPress?: () => void
    onPressIn?: () => void
    accessibilityLabel?: string
    accessibilityRole?: AccessibilityRole
    disabled?: boolean
  }>

export const ArwesButton = ({
  accessibilityLabel,
  accessibilityRole = "button",
  children,
  disabled = false,
  onPress,
  onPressIn,
  style,
  ...frameProps
}: ArwesButtonProps) => {
  const highlighted = useSharedValue(false)

  const gesture = Gesture.Tap()
    .maxDuration(5000)
    .onBegin(() => {
      if (disabled) return
      highlighted.value = true
      if (onPressIn !== undefined) runOnJS(onPressIn)()
    })
    .onTouchesUp(() => {
      if (disabled) return
      highlighted.value = false
      if (onPress !== undefined) runOnJS(onPress)()
    })
    .onFinalize(() => {
      highlighted.value = false
    })

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View accessibilityLabel={accessibilityLabel} accessibilityRole={accessibilityRole} style={style}>
        {/* `style` (the button's own explicit size, e.g. a fixed 44x44 icon
         * button) has to reach `Frame`'s own View too, not just this
         * `Animated.View` — `Frame` measures ITS OWN `onLayout` to size the
         * Skia canvas (see `./frame/index.tsx`), so without this it renders
         * a 0-height canvas (visually invisible; logs
         * "RNSKIA: Could not retrieve drawable from CAMetalLayer"). */}
        <Frame {...frameProps} highlighted={highlighted} style={style}>
          {children}
        </Frame>
      </Animated.View>
    </GestureDetector>
  )
}
