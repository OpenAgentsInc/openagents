import { BlurMask, Canvas, LinearGradient, Rect, vec } from "@shopify/react-native-skia"
import { useEffect, useState } from "react"
import type { LayoutChangeEvent } from "react-native"
import { StyleSheet, View } from "react-native"
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withTiming,
} from "react-native-reanimated"

import { MOTION_AMBIENT } from "../../theme/motion"
import { khalaMobileTheme } from "../../theme/tokens"
import { KhalaText } from "../khala-text"

/** Decorative "Nexus Beam" sign-in backdrop — ported from the owner-picked
 * wireframe direction 1d ("Nexus Beam") in
 * `~/Downloads/Khala Mobile landing wireframe/Khala Mobile Landing
 * Wireframes.dc.html` (`id="1d"`, roughly lines 154-189). The wireframe's CSS
 * is a full-height vertical energy column (a wide soft band plus a thin glowy
 * core line) with small code glyphs materializing near the top.
 *
 * Ported as a self-measuring absolute-fill layer (same `onLayout` pattern as
 * `../background-gradient/index.tsx` and `../frame/index.tsx`): a Skia
 * `Canvas` draws the beam band + core line with `LinearGradient`s built from
 * the accent token's own RGB components (matching the existing
 * `rgba(79, 208, 255, ...)` convention in `../touchable-feedback.tsx` /
 * `../khala-list-item.tsx` rather than inventing a new palette entry), and a
 * plain RN glyph column sits on top. Only the wireframe's `pulseGlow`
 * keyframe is ported as real motion (the "◇" glyph breathing 0.45 <-> 0.9
 * opacity over `MOTION_AMBIENT`); `psiFall` doesn't apply here (1d's glyphs
 * materialize in place, they don't fall) and would be redundant motion beside
 * the fleet-status/ambient-glow conventions this app already reserves for
 * "alive" surfaces. */

const BEAM_WIDTH = 130

const BEAM_BAND_COLORS = [
  "transparent",
  "rgba(58, 123, 255, 0.10)", // borderStrong-family blue, matching the wireframe's beam edge
  "rgba(79, 208, 255, 0.16)", // accent (#4fd0ff)
  "rgba(58, 123, 255, 0.10)",
  "transparent",
]
const BEAM_BAND_POSITIONS = [0, 0.3, 0.5, 0.7, 1]

const BEAM_CORE_COLORS = [
  "rgba(79, 208, 255, 0.7)", // accent (#4fd0ff)
  "rgba(79, 208, 255, 0.15)",
  "rgba(79, 208, 255, 0.5)",
]
const BEAM_CORE_POSITIONS = [0, 0.55, 1]

const glyphs: ReadonlyArray<{ color: string; text: string }> = [
  { color: khalaMobileTheme.borderMuted, text: "10011" },
  { color: khalaMobileTheme.borderStrong, text: "{ intent }" },
  { color: khalaMobileTheme.accentSoft, text: "fn resolve()" },
  { color: khalaMobileTheme.borderMuted, text: "await khala" },
  { color: khalaMobileTheme.borderStrong, text: "=> merge" },
]

const PulsingDiamondGlyph = () => {
  const progress = useSharedValue(0)

  useEffect(() => {
    progress.value = withRepeat(withTiming(1, { duration: MOTION_AMBIENT }), -1, true)
  }, [progress])

  const style = useAnimatedStyle(() => ({ opacity: 0.45 + progress.value * 0.45 }))

  return (
    <Animated.Text style={[{ color: khalaMobileTheme.accent, fontSize: 13 }, style]}>{"◇"}</Animated.Text>
  )
}

export const NexusBeamBackdrop = () => {
  const [size, setSize] = useState<{ height: number; width: number } | undefined>(undefined)

  const onLayout = (event: LayoutChangeEvent) => {
    const { height, width } = event.nativeEvent.layout
    setSize(previous => (previous?.height === height && previous?.width === width ? previous : { height, width }))
  }

  return (
    <View onLayout={onLayout} pointerEvents="none" style={StyleSheet.absoluteFill}>
      {size === undefined ? null : (
        <Canvas style={{ height: size.height, width: size.width }}>
          <Rect
            color={khalaMobileTheme.accent}
            height={size.height}
            width={BEAM_WIDTH}
            x={size.width / 2 - BEAM_WIDTH / 2}
            y={0}
          >
            <LinearGradient
              colors={BEAM_BAND_COLORS}
              end={vec(size.width / 2 + BEAM_WIDTH / 2, 0)}
              positions={BEAM_BAND_POSITIONS}
              start={vec(size.width / 2 - BEAM_WIDTH / 2, 0)}
            />
          </Rect>
          <Rect color={khalaMobileTheme.accent} height={size.height} width={2} x={size.width / 2 - 1} y={0}>
            <LinearGradient
              colors={BEAM_CORE_COLORS}
              end={vec(0, size.height)}
              positions={BEAM_CORE_POSITIONS}
              start={vec(0, 0)}
            />
            <BlurMask blur={5} style="solid" />
          </Rect>
        </Canvas>
      )}
      <View className="absolute left-0 right-0 top-16 items-center gap-6">
        {glyphs.map(glyph => (
          <KhalaText key={glyph.text} className="text-[10px]" style={{ color: glyph.color }} variant="mono">
            {glyph.text}
          </KhalaText>
        ))}
        <PulsingDiamondGlyph />
      </View>
    </View>
  )
}
