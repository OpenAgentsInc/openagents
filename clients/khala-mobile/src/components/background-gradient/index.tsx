import type { ReactNode } from "react"
import { useState } from "react"
import type { LayoutChangeEvent, StyleProp, ViewStyle } from "react-native"
import { StyleSheet, View } from "react-native"

import type { BackgroundGradientProps as SkiaBackgroundGradientProps } from "./background-gradient"
import { BackgroundGradient as SkiaBackgroundGradient } from "./background-gradient"

export type { BackgroundGradientProps as SkiaBackgroundGradientProps } from "./background-gradient"

type BackgroundGradientProps = Omit<SkiaBackgroundGradientProps, "height" | "width"> &
  Readonly<{
    children?: ReactNode
    style?: StyleProp<ViewStyle>
  }>

/** Self-measuring wrapper around the raw Skia `BackgroundGradient` canvas
 * (`./background-gradient.tsx`), following the same `onLayout`-driven pattern
 * as `../frame/index.tsx`: places the animated breathing glow as an
 * absolute-fill background behind normal layout `children`, sized to
 * whatever box the children (or an explicit `style`) end up occupying, so
 * call sites don't need to pre-compute a literal size. */
export const BackgroundGradient = ({ children, style, ...skiaProps }: BackgroundGradientProps) => {
  const [size, setSize] = useState<{ height: number; width: number } | undefined>(undefined)

  const onLayout = (event: LayoutChangeEvent) => {
    const { height, width } = event.nativeEvent.layout
    setSize(previous => (previous?.height === height && previous?.width === width ? previous : { height, width }))
  }

  return (
    <View onLayout={onLayout} style={style}>
      {size === undefined ? null : (
        <View pointerEvents="none" style={StyleSheet.absoluteFill}>
          <SkiaBackgroundGradient {...skiaProps} height={size.height} width={size.width} />
        </View>
      )}
      {children}
    </View>
  )
}
