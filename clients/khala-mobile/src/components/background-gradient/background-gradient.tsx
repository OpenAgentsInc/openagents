import { BlurMask, Canvas, RoundedRect, SweepGradient, vec } from "@shopify/react-native-skia"
import { useEffect } from "react"
import { useDerivedValue, useSharedValue, withRepeat, withTiming } from "react-native-reanimated"

import { MOTION_AMBIENT } from "../../theme/motion"
import { khalaMobileTheme } from "../../theme/tokens"

export type BackgroundGradientProps = Readonly<{
  height: number
  width: number
  cornerRadius?: number
  /** Max blur radius (dp) the pulse oscillates up to, from 0. */
  maxBlur?: number
  colors?: Array<string>
}>

/** Ported from Arcade's `BackgroundGradient`
 * (`app/components/BackgroundGradient.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.9 and issue #8399).
 * A Skia `RoundedRect` filled with an angular `SweepGradient`, given a soft
 * "breathing" pulse by oscillating its `BlurMask` radius between 0 and
 * `maxBlur` via `withRepeat(withTiming(...), -1, true)` (infinite yoyo) — a
 * cheap "this surface is alive/thinking" ambient loop. Per
 * `docs/design/starcraft.md` and this session's motion-token doc
 * (`../../theme/motion.ts`), ambient/breathing loops like this are reserved
 * for genuinely in-progress/"live" surfaces, not settled content or snappy
 * interactive feedback.
 *
 * Recolored per the harvest issue: Arcade's `cyan`/`rgb(0,248,248)` stops ->
 * `accent`/`accentSoft` (`packages/ui/src/react/nativewind-tokens.cjs` via
 * `../../theme/tokens`). Duration comes from `MOTION_AMBIENT` instead of a
 * hardcoded 2000ms literal.
 *
 * Per #8390/#8392/#8402's finding, the Skia version pinned in this repo
 * (2.6.2) accepts Reanimated `SharedValue`/`DerivedValue` instances directly
 * as props, so `BlurMask`'s `blur` below is a plain derived value read on the
 * UI thread — no `useSharedValueEffect`/Skia-`useValue` bridge like Arcade's
 * original.
 *
 * Deviation from Arcade: the original grew its own `Canvas` by a fixed
 * `canvasPadding` (40dp) beyond the requested box so the blurred edge would
 * not be hard-clipped, which meant every call site had to independently
 * reserve extra layout space for it. Following this session's `Frame`
 * precedent (`../frame/frame.tsx`), this port instead draws exactly within
 * the given `width`/`height` with no implicit bleed — callers that want a
 * clean edge can add rounded corners plus `overflow: "hidden"` via the
 * wrapping `index.tsx`, and any minor edge softening from `maxBlur` is
 * imperceptible at the small text-row/card sizes this is used at. */
export const BackgroundGradient = ({
  colors = [
    khalaMobileTheme.accent,
    khalaMobileTheme.accentSoft,
    khalaMobileTheme.accentSoft,
    khalaMobileTheme.accent
  ],
  cornerRadius = 12,
  height,
  maxBlur = 10,
  width
}: BackgroundGradientProps) => {
  const blurProgress = useSharedValue(0)

  useEffect(() => {
    blurProgress.value = withRepeat(withTiming(1, { duration: MOTION_AMBIENT }), -1, true)
  }, [blurProgress])

  const blur = useDerivedValue(() => blurProgress.value * maxBlur)

  return (
    <Canvas style={{ height, width }}>
      <RoundedRect color={khalaMobileTheme.surfaceRaised} height={height} r={cornerRadius} width={width} x={0} y={0}>
        <SweepGradient c={vec(width / 2, height / 2)} colors={colors} />
        <BlurMask blur={blur} style="solid" />
      </RoundedRect>
    </Canvas>
  )
}
