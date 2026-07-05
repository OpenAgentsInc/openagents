import type { ReactNode } from "react"
import { useEffect, useState } from "react"
import type { LayoutChangeEvent, StyleProp, ViewStyle } from "react-native"
import { StyleSheet, View } from "react-native"

import type { FrameProps as SkiaFrameProps } from "./frame"
import { Frame as SkiaFrame } from "./frame"

export type { FrameProps as SkiaFrameProps } from "./frame"

type FrameProps = Omit<SkiaFrameProps, "height" | "width"> & Readonly<{
  children?: ReactNode
  style?: StyleProp<ViewStyle>
}>

/** Ported from Arcade's `Frame` wrapper (`app/components/Frame/index.tsx`) —
 * places the Skia-drawn `Frame` canvas as an absolute-fill background behind
 * normal layout `children`.
 *
 * Deviation from Arcade's verbatim port: Arcade's wrapper required a
 * pre-supplied numeric `height`/`width` (thrown if `style` didn't carry one),
 * because every Arcade call site was a `DemoScreen` with a literal fixed
 * size. Khala's real call sites include content-driven cards (settings.tsx
 * fleet/account rows) whose size isn't known upfront, so this version
 * measures itself via `onLayout` instead of requiring a literal size — the
 * Skia canvas mounts once the first layout pass reports a size, then stays in
 * sync with any later resize. This still covers the fixed-size case (e.g. a
 * 44x44 `ArwesButton`) identically, since a fixed-size `View` also reports
 * its exact size on `onLayout`. */
export const Frame = ({ children, style, ...skiaFrameProps }: FrameProps) => {
  const [size, setSize] = useState<{ height: number; width: number } | undefined>(undefined)

  const onLayout = (event: LayoutChangeEvent) => {
    const { height, width } = event.nativeEvent.layout
    setSize(previous => (previous?.height === height && previous?.width === width ? previous : { height, width }))
  }

  return (
    <View onLayout={onLayout} style={style}>
      {size === undefined ? null : (
        <View pointerEvents="none" style={StyleSheet.absoluteFill}>
          <SkiaFrame {...skiaFrameProps} height={size.height} width={size.width} />
        </View>
      )}
      {children}
    </View>
  )
}

/** Small ergonomic helper for the "power on" mount effect the harvest issue
 * calls for (settings.tsx fleet/account cards): starts `false`, flips to
 * `true` one frame after mount (optionally staggered by `delayMs`, e.g.
 * `MOTION_STAGGER_MS * index`) so `Frame`'s `visible` prop animates its
 * corner-unfold/border-grow reveal instead of rendering already-unfolded. */
export const usePowerOnVisible = (delayMs = 0) => {
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    const timeoutId = setTimeout(() => setVisible(true), delayMs)
    return () => clearTimeout(timeoutId)
  }, [delayMs])

  return visible
}
