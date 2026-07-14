import React from "react"
import { Text, TextInput, View } from "./react-native-vitest"

const animated = {
  Text,
  TextInput,
  View,
  createAnimatedComponent: <T,>(component: T): T => component,
}

export default animated
export const Easing = { linear: (value: number) => value }
export const FadeIn = { delay: () => FadeIn, duration: () => FadeIn }
export const interpolate = (value: number) => value
export const interpolateColor = (_value: number, _input: readonly number[], output: readonly unknown[]) => output[0]
export const measure = () => ({ height: 0, pageX: 0, pageY: 0, width: 0, x: 0, y: 0 })
export const runOnJS = <T extends (...args: any[]) => any>(fn: T): T => fn
export const useAnimatedProps = (factory: () => unknown) => factory()
export const useAnimatedReaction = () => undefined
export const useAnimatedRef = () => React.createRef()
export const useAnimatedStyle = (factory: () => unknown) => factory()
export const useDerivedValue = (factory: () => unknown) => ({ value: factory() })
export const useSharedValue = (value: unknown) => ({ value })
export const withRepeat = (value: unknown) => value
export const withSpring = (value: unknown) => value
export const withTiming = (value: unknown) => value
