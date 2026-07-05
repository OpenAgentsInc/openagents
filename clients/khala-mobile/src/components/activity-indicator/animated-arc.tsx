import { BlurMask, Circle, Group, Path, rect, Skia } from "@shopify/react-native-skia"
import type { DerivedValue, SharedValue } from "react-native-reanimated"
import { useDerivedValue } from "react-native-reanimated"

/** Ported from Arcade's `AnimatedArc`
 * (`app/components/ActivityIndicator/AnimatedArc.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.12 and issue #8402).
 * Draws a static two-quarter-arc Skia `Path` (90-degree arcs at 0 degrees and
 * 180 degrees, forming a broken ring) inside a `<Group>` continuously rotated
 * via a transform, plus a soft `BlurMask` glow halo behind it.
 *
 * The broken-ring path has 180-degree rotational symmetry (the shape at angle
 * theta is identical to the shape at theta + 180deg), so the driving
 * `rotation` value only ever needs to sweep 0..PI per loop (not a full
 * 0..2*PI turn) for a seamless jump-back-to-start repeat — see the
 * `withRepeat` calls in `./index.tsx`.
 *
 * Per #8390/#8392's finding, the Skia version pinned in this repo (2.6.2)
 * accepts Reanimated `SharedValue`/`DerivedValue` instances directly as
 * props, so `rotation` here is a plain Reanimated derived value read on the
 * UI thread — no `useSharedValueEffect`-style bridge, unlike Arcade's
 * original (which copied a value out of Skia's own `useComputedValue` /
 * `useTiming`/`useSpring` reactive system).
 *
 * Deviation from Arcade: the original also drew a second, non-blurred, plain
 * `<Circle>` with no `color` prop (defaulting to opaque black) directly under
 * the arc — apparently meant to mask the glow circle's solid center down to a
 * thin outer halo. Rendered small inside a themed dark button, an opaque
 * black disc reads as a UI bug rather than a glow, so this port drops that
 * circle and instead renders the glow circle itself at partial opacity —
 * still a soft ambient halo per the issue, without punching an opaque hole in
 * the middle or fully filling the broken ring's gaps.
 *
 * Also builds the path via `Skia.PathBuilder.Make().arcToOval(...).detach()`
 * rather than the (deprecated-on-device, confirmed via on-device console
 * warnings during this issue's verification) mutable `Skia.Path.Make()` +
 * `addArc`/`transform` pair Arcade originally used. `forceMoveTo: true` on
 * each `arcToOval` call reproduces `addArc`'s "always starts a new contour"
 * behavior, so the two arcs stay visually disconnected (the broken-ring
 * look), not joined into one continuous stroke. */
export type AnimatedArcProps = Readonly<{
  cx: number
  cy: number
  internalRadius: number
  color: string
  strokeWidth: number
  /** Rotation in radians, driven by a looping Reanimated animation in the
   * parent (linear `withTiming` for the small arc, organic `withSpring` for
   * the large variant's counter-rotating outer arc). */
  rotation: DerivedValue<number> | SharedValue<number>
}>

export const AnimatedArc = ({ color, cx, cy, internalRadius, rotation, strokeWidth }: AnimatedArcProps) => {
  const r = internalRadius / 2 + strokeWidth / 2

  const path = useDerivedValue(() => {
    const bounds = rect(0, 0, internalRadius, internalRadius)
    return Skia.PathBuilder.Make()
      .arcToOval(bounds, 0, 90, true)
      .arcToOval(bounds, 180, 90, true)
      .transform(Skia.Matrix().translate(cx - internalRadius / 2, cy - internalRadius / 2))
      .detach()
  }, [cx, cy, internalRadius])

  const transform = useDerivedValue(() => [{ rotate: rotation.value }])

  return (
    <Group>
      <Circle color={color} cx={cx} cy={cy} opacity={0.35} r={r}>
        <BlurMask blur={r / 4} style="solid" />
      </Circle>
      <Group origin={{ x: cx, y: cy }} transform={transform}>
        <Path color={color} path={path} strokeWidth={strokeWidth} style="stroke" />
      </Group>
    </Group>
  )
}
