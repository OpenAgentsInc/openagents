import React from "react"

const host = (name: string) => ({ children, ...props }: Record<string, any>) =>
  React.createElement(name, props, children)

export const Blur = host("Blur")
export const BlurMask = host("BlurMask")
export const Canvas = host("Canvas")
export const Circle = host("Circle")
export const Group = host("Group")
export const Image = host("SkiaImage")
export const Line = host("Line")
export const Path = host("Path")
export const RadialGradient = host("RadialGradient")
export const Rect = host("Rect")
export const RoundedRect = host("RoundedRect")
export const SweepGradient = host("SweepGradient")
const pathBuilder = {
  arcToOval: () => pathBuilder,
  close: () => pathBuilder,
  detach: () => ({}),
  transform: () => pathBuilder,
}
const matrix = { translate: () => matrix }
export const Skia = {
  Matrix: () => matrix,
  Path: { Make: () => ({}) },
  PathBuilder: { Make: () => pathBuilder },
}
export const interpolateColors = (_value: number, _input: readonly number[], output: readonly unknown[]) => output[0]
export const makeImageFromView = async () => undefined
export const rect = (x: number, y: number, width: number, height: number) => ({ height, width, x, y })
export const vec = (x: number, y: number) => ({ x, y })
