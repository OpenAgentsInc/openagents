import { Canvas, Group } from "@shopify/react-native-skia"
import { useEffect } from "react"
import { StyleSheet } from "react-native"
import { Easing, useDerivedValue, useSharedValue, withRepeat, withSpring, withTiming } from "react-native-reanimated"

import { khalaMobileTheme } from "../../theme/tokens"
import { AnimatedArc } from "./animated-arc"

export type ActivityIndicatorType = "small" | "large"

export type ActivityIndicatorProps = Readonly<{
  /** `"small"`: one rotating broken-ring arc. `"large"`: adds a second,
   * bigger, counter-rotating arc driven by an organic looping spring, for a
   * secondary "wobble" layered on top of the steady linear rotation. */
  type?: ActivityIndicatorType
  color?: string
  strokeWidth?: number
  /** Canvas size (square) in dp. Defaults to a prominent app-loading size.
   * Compact controls should pass an explicit smaller size. */
  size?: number
}>

/** Ported from Arcade's Skia `ActivityIndicator`
 * (`app/components/ActivityIndicator/index.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.12 and issue #8402).
 * A static two-quarter-arc Skia `Path` (a "broken ring") continuously
 * rotated as a `<Group>` transform — not redrawn or re-stroked every frame —
 * reading distinctly more "psionic energy" than a generic RN
 * `<ActivityIndicator/>` spin.
 *
 * Deviation from Arcade: the harvest issue directs driving the rotation with
 * Reanimated (`withRepeat`/`withTiming`/`withSpring`) instead of Arcade's
 * original Skia-native `useTiming`/`useSpring` hooks, per #8390's finding
 * that the Skia version pinned in this repo (2.6.2) reads Reanimated
 * shared/derived values directly with no bridge needed — so this port skips
 * Skia's own animation-hook system entirely rather than re-implementing an
 * obsolete bridge for it. Canvas/arc sizing is also parameterized (`size`,
 * `strokeWidth`) instead of Arcade's fixed 240dp demo-screen constants, since
 * every real call site here is a small icon-button spinner, not a full-screen
 * demo. */
export const ActivityIndicator = ({
  color = khalaMobileTheme.accent,
  size = 112,
  strokeWidth = 7,
  type = "small"
}: ActivityIndicatorProps) => {
  const internalOffset = size * 0.125
  const internalRadius = (size / 2 - internalOffset) / 2
  const cx = size / 2
  const cy = size / 2

  // Small/inner arc: steady linear 500ms loop, matching Arcade's
  // `internalLoop` (`useTiming({ loop: true }, { duration: 500, easing:
  // Easing.linear })`). Sweeps 0..PI, not 0..2*PI — see `animated-arc.tsx`
  // for why that's still a full, seamless visual rotation.
  const internalProgress = useSharedValue(0)
  useEffect(() => {
    internalProgress.value = withRepeat(withTiming(1, { duration: 500, easing: Easing.linear }), -1, false)
  }, [internalProgress])
  const internalRotation = useDerivedValue(() => internalProgress.value * Math.PI)

  // Large/outer arc: organic looping spring, matching Arcade's
  // `externalLoop` (`useSpring({ loop: true }, { mass: 1.5, velocity: 100,
  // damping: 15 })`) verbatim. Counter-rotates relative to the inner arc.
  // Only driven while `type === "large"`, so the "small" variant never
  // schedules the extra animation.
  const externalProgress = useSharedValue(0)
  useEffect(() => {
    if (type !== "large") return
    externalProgress.value = withRepeat(withSpring(1, { damping: 15, mass: 1.5, velocity: 100 }), -1, false)
  }, [externalProgress, type])
  const externalRotation = useDerivedValue(() => -(externalProgress.value * Math.PI))

  return (
    <Canvas style={[{ height: size, width: size }, styles.container]}>
      {type === "large" ? (
        <Group origin={{ x: cx, y: cy }}>
          <AnimatedArc
            color={color}
            cx={cx}
            cy={cy}
            internalRadius={internalRadius * 1.8}
            rotation={externalRotation}
            strokeWidth={strokeWidth}
          />
        </Group>
      ) : null}
      <AnimatedArc
        color={color}
        cx={cx}
        cy={cy}
        internalRadius={internalRadius}
        rotation={internalRotation}
        strokeWidth={strokeWidth}
      />
    </Canvas>
  )
}

const styles = StyleSheet.create({
  container: { aspectRatio: 1 }
})
