import { Canvas, Group, Path, Skia } from "@shopify/react-native-skia"
import { useMemo } from "react"
import { StyleSheet } from "react-native"
import type { DerivedValue, SharedValue } from "react-native-reanimated"

import { khalaMobileTheme, khalaMobileTokens } from "../../theme/tokens"

export type SwipeQuoteDonutProps = Readonly<{
  size: number
  strokeWidth?: number
  /** 0..1 fill progress, driven by the parent `SwipeableItem`'s swipe
   * gesture (already squared for an eased-not-linear fill — see
   * `swipeable-item.tsx`). */
  progress: DerivedValue<number> | SharedValue<number>
  color?: string
  trackColor?: string
}>

/** Ported from Arcade's `AnimatedDonut`/`DonutChart`
 * (`app/components/AnimatedDonut/index.tsx`; see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.3 and issue #8393).
 * A circular-progress "pull to confirm" ring: a static full-circle Skia
 * `Path` whose stroke is trimmed by the `start`/`end` props to draw only the
 * `progress` fraction of the circle, over a full-circle track underneath.
 *
 * Reskinned per the harvest issue: `accent` (`#4fd0ff`) stroke over a
 * `surfaceMuted` track (both from
 * `packages/ui/src/react/nativewind-tokens.cjs` via `../../theme/tokens`) —
 * Arcade's original drew a translucent-white ring with no visible track at
 * all, so this port adds one for a clearer "how far until it completes"
 * affordance.
 *
 * `progress` is a plain Reanimated `SharedValue`/`DerivedValue` passed
 * straight into the Skia `<Path>`'s `end` prop. Per #8390's finding, the
 * Skia version pinned in this repo (2.6.2) reads Reanimated values directly
 * with no bridge — unlike Arcade's original, which copied the value into a
 * Skia-native `SkiaMutableValue` every frame via `useSharedValueEffect`. */
export const SwipeQuoteDonut = ({
  color = khalaMobileTheme.accent,
  progress,
  size,
  strokeWidth = 3,
  trackColor = khalaMobileTokens.colors.surfaceMuted
}: SwipeQuoteDonutProps) => {
  const radius = size / 2
  const innerRadius = radius - strokeWidth / 2

  // Static geometry — only the stroke `end` prop below animates per frame,
  // so the path itself only needs to rebuild if the ring's own size changes.
  const path = useMemo(
    () => Skia.PathBuilder.Make().addCircle(radius, radius, innerRadius).detach(),
    [innerRadius, radius]
  )

  return (
    <Canvas style={[styles.container, { height: size, width: size }]}>
      <Group>
        <Path color={trackColor} path={path} strokeCap="round" strokeWidth={strokeWidth} style="stroke" />
        <Path
          color={color}
          end={progress}
          path={path}
          start={0}
          strokeCap="round"
          strokeJoin="round"
          strokeWidth={strokeWidth}
          style="stroke"
        />
      </Group>
    </Canvas>
  )
}

const styles = StyleSheet.create({
  container: { alignItems: "center", justifyContent: "center" }
})
