export const khalaEasingNames = [
  "linear",
  "inQuad",
  "outQuad",
  "inOutQuad",
  "inCubic",
  "outCubic",
  "inOutCubic",
  "inQuart",
  "outQuart",
  "inOutQuart",
  "inQuint",
  "outQuint",
  "inOutQuint",
  "inSine",
  "outSine",
  "inOutSine",
  "inExpo",
  "outExpo",
  "inOutExpo",
  "inCirc",
  "outCirc",
  "inOutCirc",
  "inBack",
  "outBack",
  "inOutBack",
  "inElastic",
  "outElastic",
  "inOutElastic",
  "inBounce",
  "outBounce",
  "inOutBounce"
] as const

export type KhalaEasingName = (typeof khalaEasingNames)[number]
export type KhalaEasing = (progress: number) => number

const clamp01 = (value: number): number => Math.min(1, Math.max(0, Number.isFinite(value) ? value : 0))
const powerIn = (power: number): KhalaEasing => (value) => clamp01(value) ** power
const powerOut = (power: number): KhalaEasing => (value) => 1 - (1 - clamp01(value)) ** power
const powerInOut = (power: number): KhalaEasing => (value) => {
  const x = clamp01(value)
  return x < 0.5 ? 2 ** (power - 1) * x ** power : 1 - ((-2 * x + 2) ** power) / 2
}
const bounceOut: KhalaEasing = (value) => {
  let x = clamp01(value)
  const scale = 7.5625
  const divisor = 2.75
  if (x < 1 / divisor) return scale * x * x
  if (x < 2 / divisor) {
    x -= 1.5 / divisor
    return scale * x * x + 0.75
  }
  if (x < 2.5 / divisor) {
    x -= 2.25 / divisor
    return scale * x * x + 0.9375
  }
  x -= 2.625 / divisor
  return scale * x * x + 0.984375
}

const back = 1.70158

export const khalaEasings: Readonly<Record<KhalaEasingName, KhalaEasing>> = {
  linear: clamp01,
  inQuad: powerIn(2),
  outQuad: powerOut(2),
  inOutQuad: powerInOut(2),
  inCubic: powerIn(3),
  outCubic: powerOut(3),
  inOutCubic: powerInOut(3),
  inQuart: powerIn(4),
  outQuart: powerOut(4),
  inOutQuart: powerInOut(4),
  inQuint: powerIn(5),
  outQuint: powerOut(5),
  inOutQuint: powerInOut(5),
  inSine: (value) => 1 - Math.cos((clamp01(value) * Math.PI) / 2),
  outSine: (value) => Math.sin((clamp01(value) * Math.PI) / 2),
  inOutSine: (value) => -(Math.cos(Math.PI * clamp01(value)) - 1) / 2,
  inExpo: (value) => {
    const x = clamp01(value)
    return x === 0 ? 0 : 2 ** (10 * x - 10)
  },
  outExpo: (value) => {
    const x = clamp01(value)
    return x === 1 ? 1 : 1 - 2 ** (-10 * x)
  },
  inOutExpo: (value) => {
    const x = clamp01(value)
    return x === 0 ? 0 : x === 1 ? 1 : x < 0.5 ? 2 ** (20 * x - 10) / 2 : (2 - 2 ** (-20 * x + 10)) / 2
  },
  inCirc: (value) => 1 - Math.sqrt(1 - clamp01(value) ** 2),
  outCirc: (value) => Math.sqrt(1 - (clamp01(value) - 1) ** 2),
  inOutCirc: (value) => {
    const x = clamp01(value)
    return x < 0.5 ? (1 - Math.sqrt(1 - (2 * x) ** 2)) / 2 : (Math.sqrt(1 - (-2 * x + 2) ** 2) + 1) / 2
  },
  inBack: (value) => {
    const x = clamp01(value)
    return (back + 1) * x ** 3 - back * x ** 2
  },
  outBack: (value) => {
    const x = clamp01(value) - 1
    return 1 + (back + 1) * x ** 3 + back * x ** 2
  },
  inOutBack: (value) => {
    const x = clamp01(value)
    const amount = back * 1.525
    return x < 0.5
      ? ((2 * x) ** 2 * ((amount + 1) * 2 * x - amount)) / 2
      : (((2 * x - 2) ** 2 * ((amount + 1) * (2 * x - 2) + amount) + 2) / 2)
  },
  inElastic: (value) => {
    const x = clamp01(value)
    return x === 0 || x === 1 ? x : -(2 ** (10 * x - 10)) * Math.sin(((10 * x - 10.75) * 2 * Math.PI) / 3)
  },
  outElastic: (value) => {
    const x = clamp01(value)
    return x === 0 || x === 1 ? x : 2 ** (-10 * x) * Math.sin(((10 * x - 0.75) * 2 * Math.PI) / 3) + 1
  },
  inOutElastic: (value) => {
    const x = clamp01(value)
    if (x === 0 || x === 1) return x
    const angle = ((20 * x - 11.125) * 2 * Math.PI) / 4.5
    return x < 0.5 ? -(2 ** (20 * x - 10) * Math.sin(angle)) / 2 : (2 ** (-20 * x + 10) * Math.sin(angle)) / 2 + 1
  },
  inBounce: (value) => 1 - bounceOut(1 - clamp01(value)),
  outBounce: bounceOut,
  inOutBounce: (value) => {
    const x = clamp01(value)
    return x < 0.5 ? (1 - bounceOut(1 - 2 * x)) / 2 : (1 + bounceOut(2 * x - 1)) / 2
  }
}

export const sampleKhalaEasing = (name: KhalaEasingName, progress: number): number => khalaEasings[name](progress)

export const khalaEaseAmong = <Value>(values: readonly [Value, ...ReadonlyArray<Value>], progress: number): Value =>
  values[Math.min(values.length - 1, Math.floor(clamp01(progress) * values.length))]!

export const khalaEaseSteps = (steps: number, progress: number): number => {
  const count = Math.min(64, Math.max(1, Math.round(steps)))
  return Math.min(1, Math.floor(clamp01(progress) * count) / count)
}

export type KhalaMotionDirection = "enter" | "exit"

export type KhalaMotionPreset =
  | { readonly _tag: "Property"; readonly property: "opacity" | "x" | "y" | "scale" | "rotate" | "skew"; readonly from: number; readonly to: number }
  | { readonly _tag: "Fade" }
  | { readonly _tag: "Flicker" }
  | { readonly _tag: "StrokeDraw"; readonly length: number }
  | { readonly _tag: "FrameAssembly"; readonly phase: "background" | "line" | "deco" }

export interface KhalaMotionKeyframe {
  readonly offset: number
  readonly values: Readonly<Record<string, number>>
}

export const resolveKhalaMotionKeyframes = (
  preset: KhalaMotionPreset,
  direction: KhalaMotionDirection
): ReadonlyArray<KhalaMotionKeyframe> => {
  const reverse = direction === "exit"
  switch (preset._tag) {
    case "Property":
      return [
        { offset: 0, values: { [preset.property]: reverse ? preset.to : preset.from } },
        { offset: 1, values: { [preset.property]: reverse ? preset.from : preset.to } }
      ]
    case "Fade":
      return [
        { offset: 0, values: { opacity: reverse ? 1 : 0 } },
        { offset: 1, values: { opacity: reverse ? 0 : 1 } }
      ]
    case "Flicker":
      return reverse
        ? [
            { offset: 0, values: { opacity: 1 } },
            { offset: 0.35, values: { opacity: 0 } },
            { offset: 0.7, values: { opacity: 0.5 } },
            { offset: 1, values: { opacity: 0 } }
          ]
        : [
            { offset: 0, values: { opacity: 0 } },
            { offset: 0.35, values: { opacity: 1 } },
            { offset: 0.7, values: { opacity: 0.5 } },
            { offset: 1, values: { opacity: 1 } }
          ]
    case "StrokeDraw":
      return [
        { offset: 0, values: { strokeDasharray: preset.length, strokeDashoffset: reverse ? 0 : preset.length } },
        { offset: 1, values: { strokeDasharray: preset.length, strokeDashoffset: reverse ? preset.length : 0 } }
      ]
    case "FrameAssembly": {
      const interval = preset.phase === "background" ? [0, 0.45] : preset.phase === "line" ? [0.2, 0.8] : [0.55, 1]
      return [
        { offset: reverse ? 1 - interval[1]! : interval[0]!, values: { opacity: reverse ? 1 : 0 } },
        { offset: reverse ? 1 - interval[0]! : interval[1]!, values: { opacity: reverse ? 0 : 1 } }
      ]
    }
  }
}
