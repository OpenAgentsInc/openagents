import { Rect } from "@shopify/react-native-skia"

/** Ported near-verbatim from Arcade's `FrameSquare`
 * (`app/components/Frame/FrameSquare.tsx`). One stroked corner square of the
 * Arwes frame. */
export type CornerType = "bottomLeft" | "bottomRight" | "topLeft" | "topRight"

type FrameSquareProps = Readonly<{
  size: number
  color: string
  strokeWidth: number
  x: number
  y: number
}>

export const FrameSquare = ({ color, size, strokeWidth, x, y }: FrameSquareProps) => (
  <Rect
    color={color}
    height={size}
    strokeWidth={strokeWidth}
    style="stroke"
    width={size}
    x={x}
    y={y}
  />
)
