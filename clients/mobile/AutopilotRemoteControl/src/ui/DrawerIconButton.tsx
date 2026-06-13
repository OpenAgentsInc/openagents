import { Pressable, type PressableProps, type ViewStyle } from "react-native"
import { useDrawerProgress } from "react-native-drawer-layout"
import Animated, { interpolate, interpolateColor, useAnimatedStyle } from "react-native-reanimated"

import { CANONICAL_DARK } from "@openagentsinc/autopilot-control-protocol"

// Faithful port of infinitered/ignite's DrawerIconButton — three bars that
// morph from a hamburger into an arrow as the drawer opens, animated on the UI
// thread via reanimated `useDrawerProgress`. Adapted to our dark palette and
// LTR-only (ignite's RTL branch dropped). The interpolation constants match
// ignite's exactly.
export type DrawerIconButtonProps = PressableProps

const AnimatedPressable = Animated.createAnimatedComponent(Pressable)

const C = CANONICAL_DARK

export function DrawerIconButton(props: DrawerIconButtonProps) {
  const { ...pressableProps } = props
  const progress = useDrawerProgress()

  const animatedContainerStyles = useAnimatedStyle(() => {
    const translateX = interpolate(progress.value, [0, 1], [0, -60])
    return { transform: [{ translateX }] }
  })

  const animatedTopBarStyles = useAnimatedStyle(() => {
    const backgroundColor = interpolateColor(progress.value, [0, 1], [C.text, C.primary])
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, -45])
    const marginBottom = interpolate(progress.value, [0, 1], [0, -2])
    const width = interpolate(progress.value, [0, 1], [18, 12])
    return { backgroundColor, marginBottom, marginLeft: marginStart, width, transform: [{ rotate: `${rotate}deg` }] }
  })

  const animatedMiddleBarStyles = useAnimatedStyle(() => {
    const backgroundColor = interpolateColor(progress.value, [0, 1], [C.text, C.primary])
    const width = interpolate(progress.value, [0, 1], [18, 16])
    return { backgroundColor, width }
  })

  const animatedBottomBarStyles = useAnimatedStyle(() => {
    const marginTop = interpolate(progress.value, [0, 1], [4, 2])
    const backgroundColor = interpolateColor(progress.value, [0, 1], [C.text, C.primary])
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, 45])
    const width = interpolate(progress.value, [0, 1], [18, 12])
    return { backgroundColor, marginLeft: marginStart, marginTop, width, transform: [{ rotate: `${rotate}deg` }] }
  })

  return (
    <AnimatedPressable {...pressableProps} style={[$container, animatedContainerStyles]}>
      <Animated.View style={[$bar, animatedTopBarStyles]} />
      <Animated.View style={[$bar, animatedMiddleBarStyles]} />
      <Animated.View style={[$bar, animatedBottomBarStyles]} />
    </AnimatedPressable>
  )
}

const $container: ViewStyle = {
  alignItems: "center",
  height: 56,
  justifyContent: "center",
  width: 56,
}

const $bar: ViewStyle = {
  height: 2,
}
