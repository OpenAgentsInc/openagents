import { useEffect } from "react"
import { Pressable, PressableProps, ViewStyle } from "react-native"
import Animated, {
  interpolate,
  useAnimatedStyle,
  withSpring,
} from "react-native-reanimated"
import type { SharedValue } from "react-native-reanimated"
import { DARK_THEME } from "../constants/colors"

interface DrawerIconButtonProps extends PressableProps {
  open: boolean
  progress: SharedValue<number>
}

const AnimatedPressable = Animated.createAnimatedComponent(Pressable)

export function DrawerIconButton(props: DrawerIconButtonProps) {
  const { open, progress, ...PressableProps } = props

  const animatedTopBarStyles = useAnimatedStyle(() => {
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, -45])
    const marginBottom = interpolate(progress.value, [0, 1], [0, -2])
    const width = interpolate(progress.value, [0, 1], [18, 12])

    return {
      backgroundColor: DARK_THEME.text,
      marginStart,
      marginBottom,
      width,
      transform: [{ rotate: `${rotate}deg` }],
    }
  })

  const animatedMiddleBarStyles = useAnimatedStyle(() => {
    const width = interpolate(progress.value, [0, 1], [18, 16])

    return {
      backgroundColor: DARK_THEME.text,
      width,
    }
  })

  const animatedBottomBarStyles = useAnimatedStyle(() => {
    const marginTop = interpolate(progress.value, [0, 1], [4, 2])
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, 45])
    const width = interpolate(progress.value, [0, 1], [18, 12])

    return {
      backgroundColor: DARK_THEME.text,
      marginStart,
      width,
      marginTop,
      transform: [{ rotate: `${rotate}deg` }],
    }
  })

  useEffect(() => {
    progress.value = withSpring(open ? 1 : 0)
  }, [open, progress])

  return (
    <AnimatedPressable {...PressableProps} style={$container}>
      <Animated.View style={[$topBar, animatedTopBarStyles]} />
      <Animated.View style={[$middleBar, animatedMiddleBarStyles]} />
      <Animated.View style={[$bottomBar, animatedBottomBarStyles]} />
    </AnimatedPressable>
  )
}

const barHeight = 2

const $container: ViewStyle = {
  alignItems: "center",
  height: 56,
  justifyContent: "center",
  width: 56,
}

const $topBar: ViewStyle = {
  height: barHeight,
}

const $middleBar: ViewStyle = {
  height: barHeight,
  marginTop: 4,
}

const $bottomBar: ViewStyle = {
  height: barHeight,
}