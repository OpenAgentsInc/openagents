import { useEffect } from "react"
import type { PressableProps, ViewStyle } from "react-native"
import { I18nManager, Pressable } from "react-native"
import Animated, { interpolate, interpolateColor, useAnimatedStyle, useSharedValue, withTiming } from "react-native-reanimated"

import { khalaMobileTheme } from "../theme/tokens"

type DrawerIconButtonProps = Readonly<
  PressableProps & {
    /** Whether the drawer this button controls is currently open. Drives a
     * shared `progress` value (0=closed, 1=open) that morphs the three bars
     * from a hamburger into an X. */
    open: boolean
  }
>

const AnimatedPressable = Animated.createAnimatedComponent(Pressable)
const isRTL = I18nManager.isRTL

/**
 * Hamburger-to-X drawer toggle icon. Three bars animated off one shared
 * `progress` value (0=closed, 1=open): the top/bottom bars rotate ±45°,
 * shrink width, shift margins, and cross-fade color; the middle bar shrinks
 * and fades to form the X's gap.
 *
 * Ported from Arcade's `DrawerIconButton` (`app/components/DrawerIconButton.tsx`).
 */
export const DrawerIconButton = (props: DrawerIconButtonProps) => {
  const { open, ...pressableProps } = props
  const progress = useSharedValue(open ? 1 : 0)

  const animatedTopBarStyles = useAnimatedStyle(() => {
    const backgroundColor = interpolateColor(progress.value, [0, 1], [khalaMobileTheme.text, khalaMobileTheme.accent])
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, isRTL ? 45 : -45])
    // Arcade's source only closes this gap to -2, which leaves the top and
    // bottom bars' rotated pivots ~8pt apart vertically — not enough for
    // their diagonals to cross near each segment's middle, so it renders as
    // a lopsided arrow/chevron instead of a centered X. Closing the gap all
    // the way (bars fully overlapping vertically, differentiated only by
    // rotation) is what actually produces a clean X; verified on-device.
    const marginBottom = interpolate(progress.value, [0, 1], [0, -6])
    const width = interpolate(progress.value, [0, 1], [18, 12])

    return {
      backgroundColor,
      marginStart,
      marginBottom,
      width,
      transform: [{ rotate: `${rotate}deg` }]
    }
  })

  const animatedMiddleBarStyles = useAnimatedStyle(() => {
    const backgroundColor = interpolateColor(progress.value, [0, 1], [khalaMobileTheme.text, khalaMobileTheme.accent])
    // Arcade's source only shrinks this 18->16 (barely at all), which left a
    // visible horizontal stub poking out of the X once the top/bottom bars'
    // pivots were corrected to converge (see above) — collapsing it fully is
    // what actually makes it "become the X's gap" as the component's own
    // doc comment describes.
    const width = interpolate(progress.value, [0, 1], [18, 0])
    const opacity = interpolate(progress.value, [0, 1], [1, 0])

    return {
      backgroundColor,
      opacity,
      width
    }
  })

  const animatedBottomBarStyles = useAnimatedStyle(() => {
    // See the comment on the top bar's `marginBottom` above — this needs to
    // close all the way (and slightly past 0) for the bottom bar's pivot to
    // meet the top bar's, forming a centered X rather than an arrow.
    const marginTop = interpolate(progress.value, [0, 1], [4, -2])
    const backgroundColor = interpolateColor(progress.value, [0, 1], [khalaMobileTheme.text, khalaMobileTheme.accent])
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, isRTL ? -45 : 45])
    const width = interpolate(progress.value, [0, 1], [18, 12])

    return {
      backgroundColor,
      marginStart,
      width,
      marginTop,
      transform: [{ rotate: `${rotate}deg` }]
    }
  })

  useEffect(() => {
    progress.value = withTiming(open ? 1 : 0)
  }, [open, progress])

  return (
    <AnimatedPressable {...pressableProps} style={$container}>
      <Animated.View style={[$topBar, animatedTopBarStyles]} />
      <Animated.View style={[$middleBar, animatedMiddleBarStyles]} />
      <Animated.View style={[$bottomBar, animatedBottomBarStyles]} />
    </AnimatedPressable>
  )
}

const barHeight = 2

// Container size and the bar width/margin constants below are load-bearing
// together: the top/bottom bars' `marginStart` shift must land their rotated
// pivots at the same point for the bars to cross into a clean X (the middle
// bar's un-shifted center marks that point). These are Arcade's original
// 56x56 values, kept verbatim — shrinking the container without rescaling
// the margins breaks the convergence and produces a lopsided chevron instead
// of an X.
const $container: ViewStyle = {
  alignItems: "center",
  height: 56,
  justifyContent: "center",
  width: 56
}

const $topBar: ViewStyle = {
  height: barHeight
}

const $middleBar: ViewStyle = {
  height: barHeight,
  marginTop: 4
}

const $bottomBar: ViewStyle = {
  height: barHeight
}
