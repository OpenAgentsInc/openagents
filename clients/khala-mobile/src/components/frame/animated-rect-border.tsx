import { Group, Line, vec } from "@shopify/react-native-skia"
import type { SharedValue } from "react-native-reanimated"

import { Scaler } from "./scaler"

/** Ported near-verbatim from Arcade's `AnimatedRectBorder`
 * (`app/components/Frame/AnimatedRectBorder.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.1). Four independent
 * border lines, each wrapped in its own `Scaler` anchored at the line's own
 * midpoint, so the rectangle's border visibly grows out from each edge's
 * center rather than clipping or fading in as one shape. */
type AnimatedRectBorderProps = Readonly<{
  x: number
  y: number
  strokeWidth: number
  height: number
  width: number
  color?: string
  scale: SharedValue<number>
}>

const getScaleOrigin = (
  lineType: "top" | "right" | "bottom" | "left",
  x: number,
  y: number,
  width: number,
  height: number
) => {
  const centerX = (width + x) / 2
  const centerY = (height + y) / 2

  switch (lineType) {
    case "top":
      return { x: centerX, y }
    case "right":
      return { x: x + width, y: centerY }
    case "bottom":
      return { x: centerX, y: y + height }
    case "left":
      return { x, y: centerY }
  }
}

export const AnimatedRectBorder = ({ color, height, scale, strokeWidth, width, x, y }: AnimatedRectBorderProps) => (
  <Group>
    <Scaler scale={scale} scaleOrigin={getScaleOrigin("top", x, y, width, height)} type="scaleX">
      <Line color={color} p1={vec(x, y)} p2={vec(x + width, y)} strokeWidth={strokeWidth} />
    </Scaler>
    <Scaler scale={scale} scaleOrigin={getScaleOrigin("left", x, y, width, height)} type="scaleY">
      <Line color={color} p1={vec(x, y)} p2={vec(x, y + height)} strokeWidth={strokeWidth} />
    </Scaler>
    <Scaler scale={scale} scaleOrigin={getScaleOrigin("right", x, y, width, height)} type="scaleY">
      <Line
        color={color}
        p1={vec(x + width, y)}
        p2={vec(x + width, y + height)}
        strokeWidth={strokeWidth}
      />
    </Scaler>
    <Scaler scale={scale} scaleOrigin={getScaleOrigin("bottom", x, y, width, height)} type="scaleX">
      <Line
        color={color}
        p1={vec(x, y + height)}
        p2={vec(x + width, y + height)}
        strokeWidth={strokeWidth}
      />
    </Scaler>
  </Group>
)
