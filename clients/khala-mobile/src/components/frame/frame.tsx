import { Canvas, Rect } from "@shopify/react-native-skia"
import type { SharedValue } from "react-native-reanimated"
import { useDerivedValue, withTiming } from "react-native-reanimated"

import { MOTION_FAST, MOTION_MEDIUM } from "../../theme/motion"
import { khalaMobileTheme } from "../../theme/tokens"
import { AnimatedRectBorder } from "./animated-rect-border"
import type { CornerType } from "./frame-square"
import { FrameSquare } from "./frame-square"
import { Scaler } from "./scaler"

/** Ported from Arcade's `Frame` (`app/components/Frame/Frame.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.1). A Skia-drawn
 * rectangular frame: 4 stroked corner squares + 4 independent border lines,
 * each unfolding outward from its own corner/midpoint via a `Scaler` Group
 * transform, plus a `highlighted` glow-fill state (opacity 0.2 -> 0.7) fed
 * from `ArwesButton` (`../arwes-button.tsx`).
 *
 * Recolored per the harvest issue: `colors.palette.cyan400` -> `accent`
 * (`#4fd0ff`), background fill `almostBlack` -> `surfaceRaised` (both from
 * `packages/ui/src/react/nativewind-tokens.cjs` via `../../theme/tokens`).
 *
 * Arcade bridged its `visible`/`highlighted` transitions into Skia's own
 * reactive value system via `useSharedValueEffect`. The Skia version pinned
 * in this repo (2.6.2) accepts Reanimated `SharedValue`/`DerivedValue`
 * instances directly as props (see `../../animation/use-shared-value-effect.ts`),
 * so every animated prop below is a plain Reanimated derived value with no
 * bridge call. */
export type FrameProps = Readonly<{
  height: number
  width: number
  color?: string
  borderColor?: string
  backgroundColor?: string
  internalSquareBorderWidth?: number
  strokeWidth?: number
  visible?: boolean
  /** Reanimated shared boolean, mutated from a gesture worklet (see
   * `ArwesButton`), driving the glow-fill opacity. Undefined = never
   * highlighted. */
  highlighted?: SharedValue<boolean>
  alwaysShowBackground?: boolean
  alwaysShowBorder?: boolean
  internalSquareSize?: number
}>

const getCornerScaleOrigin = (cornerType: CornerType, width: number, height: number) => {
  switch (cornerType) {
    case "bottomLeft":
      return { x: 0, y: height }
    case "bottomRight":
      return { x: width, y: height }
    case "topLeft":
      return { x: 0, y: 0 }
    case "topRight":
      return { x: width, y: 0 }
  }
}

export const Frame = ({
  alwaysShowBackground = false,
  alwaysShowBorder = false,
  backgroundColor = khalaMobileTheme.surfaceRaised,
  borderColor = khalaMobileTheme.accent,
  color = khalaMobileTheme.accent,
  height,
  highlighted,
  internalSquareBorderWidth = 3,
  internalSquareSize: maxInternalSquareSize,
  strokeWidth: rectStrokeWidth = 1.5,
  visible = true,
  width
}: FrameProps) => {
  const containerWidth = width - internalSquareBorderWidth * 2
  const containerHeight = height - internalSquareBorderWidth * 2
  const offsetWidth = (width - containerWidth) / 2
  const offsetHeight = (height - containerHeight) / 2
  const squareSize = maxInternalSquareSize ?? width * 0.1

  // 0 = fully unfolded/visible, 1 = folded away. Drives every corner/border
  // `Scaler` below so the whole frame assembles from its corners rather than
  // fading or clipping in.
  const rProgress = useDerivedValue(() => withTiming(visible ? 0 : 1, { duration: MOTION_MEDIUM }), [visible])
  const scale = useDerivedValue(() => 1 - rProgress.value)
  const alwaysVisibleScale = useDerivedValue(() => 1)

  const highlightedProgress = useDerivedValue(() => {
    const isHighlighted = highlighted?.value ?? false
    return withTiming(isHighlighted ? 0.7 : 0.2, { duration: MOTION_FAST })
  })

  const highlightedBackgroundOpacity = useDerivedValue(() =>
    Math.max(scale.value * highlightedProgress.value, alwaysShowBackground ? 0.1 : 0)
  )

  return (
    <Canvas style={{ height, width }}>
      <Scaler scale={scale} scaleOrigin={getCornerScaleOrigin("topLeft", width, height)}>
        <FrameSquare
          color={color}
          size={squareSize}
          strokeWidth={internalSquareBorderWidth}
          x={offsetWidth / 2}
          y={offsetHeight / 2}
        />
      </Scaler>
      <Scaler scale={scale} scaleOrigin={getCornerScaleOrigin("topRight", width, height)}>
        <FrameSquare
          color={color}
          size={squareSize}
          strokeWidth={internalSquareBorderWidth}
          x={width - squareSize - offsetWidth / 2}
          y={offsetHeight / 2}
        />
      </Scaler>
      <Scaler scale={scale} scaleOrigin={getCornerScaleOrigin("bottomLeft", width, height)}>
        <FrameSquare
          color={color}
          size={squareSize}
          strokeWidth={internalSquareBorderWidth}
          x={offsetWidth / 2}
          y={height - squareSize - offsetHeight / 2}
        />
      </Scaler>
      <Scaler scale={scale} scaleOrigin={getCornerScaleOrigin("bottomRight", width, height)}>
        <FrameSquare
          color={color}
          size={squareSize}
          strokeWidth={internalSquareBorderWidth}
          x={width - squareSize - offsetWidth / 2}
          y={height - squareSize - offsetHeight / 2}
        />
      </Scaler>
      <AnimatedRectBorder
        color={borderColor}
        height={containerHeight}
        scale={alwaysShowBorder ? alwaysVisibleScale : scale}
        strokeWidth={rectStrokeWidth}
        width={containerWidth}
        x={offsetWidth}
        y={offsetHeight}
      />
      <Rect color={backgroundColor} height={containerHeight} width={containerWidth} x={offsetWidth} y={offsetHeight} />
      <Rect
        color={borderColor}
        height={containerHeight}
        opacity={highlightedBackgroundOpacity}
        width={containerWidth}
        x={offsetWidth}
        y={offsetHeight}
      />
    </Canvas>
  )
}
