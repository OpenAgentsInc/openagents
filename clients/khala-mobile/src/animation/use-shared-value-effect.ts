/** Reanimated `SharedValue` <-> Skia bridge helper for `clients/khala-mobile`.
 *
 * Arcade's own Skia components (`Frame`, `AnimatedDonut`, `ActivityIndicator`,
 * `BackgroundGradient`, ...) were built against an OLD `@shopify/react-native-skia`
 * (~0.1.x, targeting RN 0.71) whose reactive value system (`useValue`,
 * `useComputedValue`) was a SEPARATE runtime from Reanimated's `SharedValue`.
 * Bridging the two required an explicit `useSharedValueEffect` hook that
 * copied a Reanimated value into a Skia `useValue` on every UI-thread frame
 * (`progress.current = rProgress.value`) — see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.1/2.3/2.9/2.12.
 *
 * The `@shopify/react-native-skia` version pinned in THIS repo (`2.6.2`,
 * matching Expo SDK 57's bundled-native-modules pin) dropped that separate
 * reactive system entirely. Skia components now accept Reanimated
 * `SharedValue`/`DerivedValue` instances DIRECTLY as props, read on the UI
 * thread, with no bridge call and no `useSharedValueEffect`-equivalent hook:
 *
 * ```tsx
 * import { useSharedValue } from "react-native-reanimated"
 * import { Circle } from "@shopify/react-native-skia"
 *
 * const progress = useSharedValue(0)
 * // progress.value = withTiming(1, { duration: MOTION_MEDIUM }) elsewhere
 * <Circle cx={progress} cy={progress} r={progress} color="cyan" />
 * ```
 *
 * See https://shopify.github.io/react-native-skia/docs/animations/animations
 * ("React Native Skia supports the direct usage of Reanimated's shared and
 * derived values as properties... no need for `useAnimatedProps`").
 *
 * DO NOT port Arcade's `useSharedValueEffect` hook verbatim into this repo —
 * against the pinned Skia version it would be dead, unnecessary indirection.
 * Every subsequent Skia-based harvest issue (Frame/ArwesButton,
 * SwipeableItem/AnimatedDonut, BlurredPopup, BackgroundGradient,
 * ActivityIndicator — see the harvest audit §2.1/2.2/2.3/2.5/2.9/2.12) should
 * pass its Reanimated `SharedValue`s straight into Skia component props.
 *
 * The ONE place Skia still genuinely diverges from Reanimated: colors. Skia
 * stores color components in its own float-array format, so Reanimated's own
 * `interpolateColor` does not produce a value Skia can read directly. Use
 * `useSkiaAnimatedColor` below — a thin wrapper over Skia's own worklet-safe
 * `interpolateColors` — instead of Reanimated's `interpolateColor` whenever a
 * Skia paint/color prop needs to animate off a Reanimated progress
 * `SharedValue`. */

import { interpolateColors } from "@shopify/react-native-skia"
import { useDerivedValue, type SharedValue } from "react-native-reanimated"

/** Derive a Skia-consumable animated color from a Reanimated progress
 * `SharedValue`. Wraps Skia's own worklet-safe `interpolateColors` (NOT
 * Reanimated's `interpolateColor`, whose output format Skia cannot read).
 * Returns a `DerivedValue<number[]>` (RGBA float components) that can be
 * passed straight into a Skia `color` / paint prop. */
export function useSkiaAnimatedColor(
  progress: SharedValue<number>,
  inputRange: number[],
  outputRange: string[]
) {
  return useDerivedValue(
    () => interpolateColors(progress.value, inputRange, outputRange),
    [inputRange, outputRange]
  )
}
