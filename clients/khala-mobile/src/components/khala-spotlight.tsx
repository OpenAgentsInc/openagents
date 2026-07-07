import { Canvas, Circle, RadialGradient, vec } from "@shopify/react-native-skia"
import { StyleSheet, useWindowDimensions } from "react-native"

export type KhalaSpotlightProps = Readonly<{
  color?: string
  falloffColor?: string
}>

export const KhalaSpotlight = ({
  color = "rgba(79, 208, 255, 0.26)",
  falloffColor = "rgba(2, 6, 13, 1)",
}: KhalaSpotlightProps) => {
  const { height, width } = useWindowDimensions()
  const radius = width * 0.68
  const center = vec(width / 2, height / 3.2)

  return (
    <Canvas pointerEvents="none" style={StyleSheet.absoluteFill}>
      <Circle cx={center.x} cy={center.y} r={radius}>
        <RadialGradient c={center} colors={[color, falloffColor]} r={radius} />
      </Circle>
    </Canvas>
  )
}
