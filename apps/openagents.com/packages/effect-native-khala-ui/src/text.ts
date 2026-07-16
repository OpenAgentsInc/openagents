import { Duration, Effect } from "effect"
import type { KhalaEasingName } from "./motion.js"

export const khalaCipherCharacters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" as const

export interface KhalaTextEffectFrame {
  readonly offset: number
  readonly visualText: string
  readonly accessibleText: string
}

export interface KhalaTextEffectOptions {
  readonly direction?: "enter" | "exit"
  readonly maxGraphemes?: number
  readonly frames?: number
}

const graphemes = (text: string): ReadonlyArray<string> => {
  if (typeof Intl.Segmenter === "function") {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" })
    return [...segmenter.segment(text)].map((entry) => entry.segment)
  }
  return Array.from(text)
}

const boundedFrameCount = (value: number | undefined, length: number): number =>
  Math.min(128, Math.max(2, Math.round(value ?? Math.min(32, Math.max(2, length + 1)))))

export const khalaTextDurationMillis = (text: string, perGraphemeMillis = 28): number =>
  Math.min(1_600, Math.max(100, graphemes(text).length * Math.min(100, Math.max(10, perGraphemeMillis))))

export const makeKhalaTextSequenceFrames = (
  text: string,
  options: KhalaTextEffectOptions & { readonly caret?: boolean } = {}
): ReadonlyArray<KhalaTextEffectFrame> => {
  const values = graphemes(text).slice(0, Math.min(128, Math.max(1, options.maxGraphemes ?? 128)))
  if (values.length !== graphemes(text).length) return [{ offset: 1, visualText: text, accessibleText: text }]
  const count = boundedFrameCount(options.frames, values.length)
  const frames = Array.from({ length: count }, (_, index) => {
    const offset = index / (count - 1)
    const visible = Math.round(offset * values.length)
    const content = values.slice(0, visible).join("")
    const caret = options.caret === true && index < count - 1 ? "▌" : ""
    return { offset, visualText: `${content}${caret}`, accessibleText: text }
  })
  return options.direction === "exit"
    ? frames.map((_, index) => ({ ...frames[frames.length - 1 - index]!, offset: index / (count - 1) }))
    : frames
}

const seededRandom = (seed: number): (() => number) => {
  let value = (Number.isFinite(seed) ? Math.round(seed) : 1) | 0
  if (value === 0) value = 0x6d2b79f5
  return () => {
    value ^= value << 13
    value ^= value >>> 17
    value ^= value << 5
    return (value >>> 0) / 0x1_0000_0000
  }
}

const shuffledIndexes = (length: number, random: () => number): ReadonlyArray<number> => {
  const indexes = Array.from({ length }, (_, index) => index)
  for (let index = indexes.length - 1; index > 0; index -= 1) {
    const swap = Math.floor(random() * (index + 1))
    const current = indexes[index]!
    indexes[index] = indexes[swap]!
    indexes[swap] = current
  }
  return indexes
}

export const makeKhalaTextDecipherFrames = (
  text: string,
  seed: number,
  options: KhalaTextEffectOptions & { readonly characters?: string } = {}
): ReadonlyArray<KhalaTextEffectFrame> => {
  const values = graphemes(text)
  const maximum = Math.min(128, Math.max(1, options.maxGraphemes ?? 128))
  if (values.length > maximum) return [{ offset: 1, visualText: text, accessibleText: text }]
  const characters = graphemes(options.characters ?? khalaCipherCharacters).filter((value) => value.trim().length > 0).slice(0, 128)
  if (characters.length === 0) return [{ offset: 1, visualText: text, accessibleText: text }]
  const random = seededRandom(seed)
  const order = shuffledIndexes(values.length, random)
  const rank = new Map(order.map((value, index) => [value, index]))
  const count = boundedFrameCount(options.frames, values.length)
  const frames = Array.from({ length: count }, (_, frameIndex) => {
    const offset = frameIndex / (count - 1)
    const revealed = Math.round(offset * values.length)
    const visualText = values
      .map((value, index) => {
        if (/^\s+$/u.test(value) || (rank.get(index) ?? 0) < revealed) return value
        return characters[Math.floor(random() * characters.length)]!
      })
      .join("")
    return { offset, visualText, accessibleText: text }
  })
  const stable = { offset: 1, visualText: text, accessibleText: text }
  frames[frames.length - 1] = stable
  return options.direction === "exit"
    ? frames.map((_, index) => ({ ...frames[frames.length - 1 - index]!, offset: index / (count - 1) }))
    : frames
}

export interface KhalaDomTextEffectOptions {
  readonly durationMillis: number
  readonly reducedMotion?: boolean
  readonly easing?: KhalaEasingName
}

interface TextLayer {
  readonly visual: HTMLElement
  readonly semanticStyle: string
  readonly rootPosition: string
}

const setupTextLayer = (root: HTMLElement, semantic: HTMLElement): TextLayer => {
  const visual = root.ownerDocument.createElement("span")
  visual.setAttribute("aria-hidden", "true")
  visual.setAttribute("data-en-khala-text-visual", "true")
  visual.style.position = "absolute"
  visual.style.inset = "0"
  visual.style.pointerEvents = "none"
  const semanticStyle = semantic.getAttribute("style") ?? ""
  const rootPosition = root.style.position
  if (root.style.position === "") root.style.position = "relative"
  semantic.setAttribute("data-en-khala-text-semantic", "true")
  semantic.style.position = "absolute"
  semantic.style.width = "1px"
  semantic.style.height = "1px"
  semantic.style.padding = "0"
  semantic.style.margin = "-1px"
  semantic.style.overflow = "hidden"
  semantic.style.clip = "rect(0, 0, 0, 0)"
  semantic.style.whiteSpace = "nowrap"
  semantic.style.border = "0"
  root.appendChild(visual)
  return { visual, semanticStyle, rootPosition }
}

const teardownTextLayer = (root: HTMLElement, semantic: HTMLElement, layer: TextLayer): void => {
  layer.visual.remove()
  if (layer.semanticStyle === "") semantic.removeAttribute("style")
  else semantic.setAttribute("style", layer.semanticStyle)
  semantic.removeAttribute("data-en-khala-text-semantic")
  root.style.position = layer.rootPosition
}

/**
 * DOM text driver with one stable semantic node and one short-lived inert
 * visual duplicate. Interruption always restores the semantic node.
 */
export const runKhalaDomTextEffect = (
  root: HTMLElement,
  semantic: HTMLElement,
  frames: ReadonlyArray<KhalaTextEffectFrame>,
  options: KhalaDomTextEffectOptions
): Effect.Effect<void> => {
  const stable = frames.at(-1)?.accessibleText ?? semantic.textContent ?? ""
  if (options.reducedMotion === true || frames.length <= 1) {
    return Effect.sync(() => {
      semantic.textContent = stable
    })
  }
  const duration = Math.min(4_000, Math.max(0, options.durationMillis))
  const frameMillis = duration / Math.max(1, frames.length - 1)
  return Effect.acquireUseRelease(
    Effect.sync(() => {
      semantic.textContent = stable
      return setupTextLayer(root, semantic)
    }),
    (layer) =>
      Effect.forEach(
        frames,
        (frame, index) =>
          Effect.sync(() => {
            layer.visual.textContent = frame.visualText
          }).pipe(index === frames.length - 1 ? Effect.asVoid : Effect.andThen(Effect.sleep(Duration.millis(frameMillis)))),
        { discard: true }
      ),
    (layer) => Effect.sync(() => teardownTextLayer(root, semantic, layer))
  )
}
