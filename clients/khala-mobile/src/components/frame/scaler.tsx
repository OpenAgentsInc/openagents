import { Group } from "@shopify/react-native-skia"
import type { ReactNode } from "react"
import type { SharedValue } from "react-native-reanimated"
import { useDerivedValue } from "react-native-reanimated"

/** Ported from Arcade's `Scaler` (`app/components/Frame/Scaler.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.1). Scales its
 * children around a fixed origin point via a Skia `<Group>` transform.
 *
 * Arcade's version read `scale.current` off a Skia `SkiaValue` inside Skia's
 * own `useComputedValue`. The Skia version pinned in this repo (2.6.2, see
 * `src/animation/use-shared-value-effect.ts`) accepts Reanimated
 * `SharedValue`/`DerivedValue` props directly, so `scale` here is a plain
 * Reanimated `SharedValue<number>` and the transform array is built with
 * Reanimated's own `useDerivedValue` instead of a Skia bridge. */
type ScalerProps = Readonly<{
  scaleOrigin: { x: number; y: number }
  scale: SharedValue<number>
  children: ReactNode
  type?: "scale" | "scaleX" | "scaleY"
}>

export const Scaler = ({ children, scale, scaleOrigin, type = "scale" }: ScalerProps) => {
  const transform = useDerivedValue(() => [{ [type]: scale.value } as { scale: number }])

  return (
    <Group origin={scaleOrigin} transform={transform}>
      {children}
    </Group>
  )
}
