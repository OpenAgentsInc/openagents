import { Effect } from "effect"
import {
  resolveKhalaMotionKeyframes,
  type KhalaEasingName,
  type KhalaMotionDirection,
  type KhalaMotionKeyframe,
  type KhalaMotionPreset
} from "./motion.js"

export interface KhalaDomMotionOptions {
  readonly durationMillis: number
  readonly delayMillis?: number
  readonly easing?: KhalaEasingName
  readonly reducedMotion?: boolean
}

export type KhalaAnimatableElement = (HTMLElement | SVGElement) & { readonly style: CSSStyleDeclaration }

const transformValue = (property: string, value: number): string =>
  property === "x"
    ? `translateX(${value}px)`
    : property === "y"
      ? `translateY(${value}px)`
      : property === "scale"
        ? `scale(${value})`
        : property === "rotate"
          ? `rotate(${value}deg)`
          : `skew(${value}deg)`

export const khalaKeyframeToDom = (frame: KhalaMotionKeyframe): Record<string, string | number> => {
  const output: Record<string, string | number> = { offset: frame.offset }
  for (const [property, value] of Object.entries(frame.values)) {
    if (property === "x" || property === "y" || property === "scale" || property === "rotate" || property === "skew") {
      output.transform = transformValue(property, value)
    } else if (property === "strokeDasharray") {
      output.strokeDasharray = String(value)
    } else if (property === "strokeDashoffset") {
      output.strokeDashoffset = String(value)
    } else {
      output[property] = value
    }
  }
  return output
}

const applyDomFrame = (element: KhalaAnimatableElement, frame: Record<string, string | number>): void => {
  for (const [property, value] of Object.entries(frame)) {
    if (property === "offset") continue
    element.style.setProperty(
      property.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`),
      String(value)
    )
  }
}

/**
 * Interruptible DOM/Web Animations driver. Stable output is applied directly
 * when motion is reduced or WAAPI is unavailable; neither path allocates a
 * fallback timer.
 */
export const runKhalaDomMotion = (
  element: KhalaAnimatableElement,
  preset: KhalaMotionPreset,
  direction: KhalaMotionDirection,
  options: KhalaDomMotionOptions
): Effect.Effect<void> => {
  const frames = resolveKhalaMotionKeyframes(preset, direction).map(khalaKeyframeToDom)
  const finalFrame = frames.at(-1) ?? {}
  if (options.reducedMotion === true || typeof element.animate !== "function") {
    return Effect.sync(() => applyDomFrame(element, finalFrame))
  }

  return Effect.acquireUseRelease(
    Effect.sync(() =>
      element.animate([...frames] as Keyframe[], {
        duration: Math.min(60_000, Math.max(0, options.durationMillis)),
        delay: Math.min(60_000, Math.max(0, options.delayMillis ?? 0)),
        easing: "linear",
        fill: "both"
      })
    ),
    (animation) =>
      animation.finished === undefined
        ? Effect.sync(() => applyDomFrame(element, finalFrame))
        : Effect.promise(() => animation.finished!.then(() => undefined, () => undefined)).pipe(
            Effect.andThen(Effect.sync(() => applyDomFrame(element, finalFrame)))
          ),
    (animation) => Effect.sync(() => animation.cancel())
  )
}

export interface KhalaNativeMotionPlan {
  readonly keyframes: ReadonlyArray<KhalaMotionKeyframe>
  readonly durationMillis: number
  readonly delayMillis: number
  readonly easing: KhalaEasingName
  readonly static: boolean
}

/** Renderer-neutral plan for React Native/native drivers and static fallback. */
export const makeKhalaNativeMotionPlan = (
  preset: KhalaMotionPreset,
  direction: KhalaMotionDirection,
  options: KhalaDomMotionOptions
): KhalaNativeMotionPlan => ({
  keyframes: options.reducedMotion === true
    ? resolveKhalaMotionKeyframes(preset, direction).slice(-1)
    : resolveKhalaMotionKeyframes(preset, direction),
  durationMillis: options.reducedMotion === true ? 0 : Math.min(60_000, Math.max(0, options.durationMillis)),
  delayMillis: options.reducedMotion === true ? 0 : Math.min(60_000, Math.max(0, options.delayMillis ?? 0)),
  easing: options.easing ?? "linear",
  static: options.reducedMotion === true
})
