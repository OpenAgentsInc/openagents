import { useEffect } from "react"
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withTiming,
} from "react-native-reanimated"

import { MOTION_AMBIENT } from "../../theme/motion"
import { khalaMobileTheme } from "../../theme/tokens"

/** "Warp aperture" — the two concentric rotated-45deg diamond outlines behind
 * the hero title in wireframe direction 1d ("Nexus Beam", `id="1d"`, lines
 * 169-171 of the wireframe HTML). Plain rotated `View` borders rather than
 * Skia: these are two static-shape squares, no fills/paths complex enough to
 * need a canvas.
 *
 * The wireframe's `slowSpin` keyframe (`rotate(45deg)` oscillating to
 * `rotate(48deg)`) is ported onto the outer diamond, and `pulseGlow` is
 * ported onto the inner diamond's glow via an animated `shadowOpacity`, both
 * looping over `MOTION_AMBIENT` per this app's "ambient/breathing loops are
 * for alive surfaces" motion convention (`../../theme/motion.ts`). */
export const WarpAperture = () => {
  const spin = useSharedValue(0)
  const glow = useSharedValue(0)

  useEffect(() => {
    spin.value = withRepeat(withTiming(1, { duration: MOTION_AMBIENT * 2 }), -1, true)
    glow.value = withRepeat(withTiming(1, { duration: MOTION_AMBIENT }), -1, true)
  }, [glow, spin])

  const outerStyle = useAnimatedStyle(() => ({
    transform: [{ rotate: `${45 + spin.value * 3}deg` }],
  }))

  const innerStyle = useAnimatedStyle(() => ({
    opacity: 0.85 + glow.value * 0.15,
    shadowOpacity: 0.3 + glow.value * 0.3,
  }))

  return (
    <Animated.View className="items-center justify-center" style={{ height: 76, width: 76 }}>
      <Animated.View
        style={[
          {
            borderColor: "rgba(79, 208, 255, 0.25)", // accent (#4fd0ff) at low opacity
            borderWidth: 1,
            height: 58,
            position: "absolute",
            width: 58,
          },
          outerStyle,
        ]}
      />
      <Animated.View
        style={[
          {
            backgroundColor: "rgba(79, 208, 255, 0.09)", // accent (#4fd0ff) at low opacity
            borderColor: khalaMobileTheme.accent,
            borderWidth: 1.5,
            height: 38,
            position: "absolute",
            shadowColor: khalaMobileTheme.accent,
            shadowOffset: { height: 0, width: 0 },
            shadowRadius: 18,
            transform: [{ rotate: "45deg" }],
            width: 38,
          },
          innerStyle,
        ]}
      />
    </Animated.View>
  )
}
