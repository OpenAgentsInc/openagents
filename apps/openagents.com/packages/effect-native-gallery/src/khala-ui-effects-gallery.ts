import { Effect } from "effect"
import {
  checkKhalaChoreographyModel,
  khalaEasingNames,
  khalaSvgIlluminationGradientId,
  makeKhalaIlluminationNativePlan,
  makeKhalaTextDecipherFrames,
  makeKhalaTextSequenceFrames,
  planKhalaChoreography,
  resolveKhalaMotionKeyframes,
  sampleKhalaEasing
} from "@effect-native/khala-ui"
import { makeKhalaBackgroundFrame } from "@effect-native/render-canvas"
import {
  khalaMotifIds,
  khalaTheme,
  resolveKhalaFrameScene,
  resolveKhalaSeparatorPaint,
  resolveKhalaStepsPaint,
  resolveKhalaStripPaint
} from "@effect-native/tokens"
import { khalaUiVisualParity, type KhalaUiParityCapability } from "./khala-ui-parity.js"

export const khalaUiEffectRendererIds = [
  "headless",
  "dom",
  "react-dom",
  "react-native",
  "svg",
  "canvas",
  "electron-react"
] as const
export type KhalaUiEffectRendererId = (typeof khalaUiEffectRendererIds)[number]
export type KhalaUiEffectDisposition = "supported" | "static-degradation" | "not-applicable"

const variants: Readonly<Record<string, ReadonlyArray<string>>> = {
  "foundation.theme": ["default", "forced-colors", "zoom-200", "text-200"],
  "foundation.steps": ["horizontal", "vertical", "forced-colors"],
  "foundation.strip": ["two-role", "four-role", "vertical"],
  "foundation.separator": ["start", "end", "both", "vertical"],
  "motion.easing": ["31-easings", "among", "steps", "reduced-motion"],
  "motion.css-properties": ["opacity", "translate", "scale", "rotate", "stroke"],
  "motion.driver": ["enter", "exit", "reverse-interruption", "cancel", "reduced-motion"],
  "motion.element": ["entered", "entering", "exiting", "exited", "ssr"],
  "motion.presets": ["property", "fade", "flicker", "stroke-draw"],
  "choreography.animator": ["parallel", "sequence", "sequence-reverse", "stagger", "stagger-reverse", "switch", "merge", "combine"],
  "frame.generic": ["groups", "clip", "mask", "pattern", "forced-colors"],
  "frame.clipping": ["octagonal", "asymmetric", "focus-clearance"],
  "frame.assembly": ["background", "line", "deco", "reversed", "reduced-motion"],
  "text.sequence": ["enter", "exit", "caret", "grapheme", "reduced-motion"],
  "text.decipher": ["seed-42", "seed-43", "recipher", "grapheme", "reduced-motion"],
  "illumination.html": ["pointer", "keyboard-focus", "coarse-pointer", "reduced-motion"],
  "illumination.svg": ["pointer", "deterministic-id", "keyboard-focus", "reduced-motion"],
  "background.dots": ["box", "circle", "cross", "origin", "inverted", "reduced-motion"],
  "background.grid-lines": ["axis-dashes", "enter", "exit", "forced-colors", "reduced-motion"],
  "background.moving-lines": ["up", "down", "left", "right", "glow", "seed-42", "reduced-motion"],
  "background.puffs": ["seed-42", "radius-growth", "padding", "quality-tiers", "reduced-motion"]
}

for (const motif of ["underline", "lines", "corners", "octagon", "nero", "nefrex", "kranox", "header", "circle"]) {
  ;(variants as Record<string, ReadonlyArray<string>>)[`frame.${motif}`] = ["compact", "comfortable", "spacious", "forced-colors", "zoom-200"]
}

const dispositionFor = (
  capability: KhalaUiParityCapability,
  renderer: KhalaUiEffectRendererId
): KhalaUiEffectDisposition => {
  if (renderer === "headless") return "supported"
  if (capability.family === "background") {
    if (renderer === "canvas" || renderer === "dom" || renderer === "react-dom" || renderer === "electron-react") return "supported"
    return renderer === "react-native" ? "static-degradation" : "not-applicable"
  }
  if (capability.family === "illumination") {
    if (renderer === "dom" || renderer === "react-dom" || renderer === "electron-react") return "supported"
    if (renderer === "svg") return capability.id === "illumination.svg" ? "supported" : "not-applicable"
    return renderer === "react-native" ? "static-degradation" : "not-applicable"
  }
  if (capability.family === "frame") {
    if (renderer === "dom" || renderer === "react-dom" || renderer === "svg" || renderer === "electron-react") return "supported"
    return renderer === "react-native" ? "static-degradation" : "not-applicable"
  }
  if (capability.family === "text") {
    if (renderer === "dom" || renderer === "react-dom" || renderer === "electron-react") return "supported"
    return renderer === "react-native" ? "static-degradation" : "not-applicable"
  }
  if (renderer === "canvas" || renderer === "svg") return capability.family === "motion" ? "supported" : "not-applicable"
  return renderer === "react-native" && capability.family === "choreography" ? "supported" : "supported"
}

export interface KhalaUiEffectStory {
  readonly id: string
  readonly capabilityId: string
  readonly sourcePaths: ReadonlyArray<string>
  readonly variants: ReadonlyArray<string>
  readonly renderers: Readonly<Record<KhalaUiEffectRendererId, KhalaUiEffectDisposition>>
  readonly baseline: string
  readonly reducedMotion: "zero-continuous-work"
}

const stableHash = (value: string): string => {
  let hash = 2166136261
  for (const character of value) { hash ^= character.codePointAt(0) ?? 0; hash = Math.imul(hash, 16777619) }
  return (hash >>> 0).toString(36)
}

export const khalaUiEffectStories = khalaUiVisualParity.map((capability): KhalaUiEffectStory => {
  const storyVariants = variants[capability.id]
  if (storyVariants === undefined || storyVariants.length === 0) throw new Error(`Missing Khala UI variants for ${capability.id}`)
  return {
    id: `khala-effect-${capability.id.replaceAll(".", "-")}`,
    capabilityId: capability.id,
    sourcePaths: capability.sourcePaths,
    variants: storyVariants,
    renderers: Object.fromEntries(khalaUiEffectRendererIds.map((renderer) => [renderer, dispositionFor(capability, renderer)])) as Record<KhalaUiEffectRendererId, KhalaUiEffectDisposition>,
    baseline: `khala-v1-${stableHash(JSON.stringify([capability.id, storyVariants]))}`,
    reducedMotion: "zero-continuous-work"
  }
})

/** Executable, deterministic receipts from each implementation family. */
export const makeKhalaUiEffectReceipts = () => {
  const frameScenes = khalaMotifIds.map((motif) => Effect.runSync(resolveKhalaFrameScene({
    motif, width: 320, height: 140, zoom: 1, density: "comfortable", forcedColors: false
  }, khalaTheme.khalaUi)))
  const backgrounds = [
    makeKhalaBackgroundFrame({ kind: "dots", shape: "cross", color: "cyan" }, 320, 180, 1),
    makeKhalaBackgroundFrame({ kind: "grid-lines", color: "cyan" }, 320, 180, 1),
    makeKhalaBackgroundFrame({ kind: "moving-lines", color: "cyan", seed: 42 }, 320, 180, 0.5),
    makeKhalaBackgroundFrame({ kind: "puffs", color: "cyan", seed: 42 }, 320, 180, 0.5)
  ]
  return {
    foundation: {
      theme: khalaTheme.khalaUi,
      steps: resolveKhalaStepsPaint(8),
      strip: resolveKhalaStripPaint(["quiet", "signal"]),
      separator: resolveKhalaSeparatorPaint("both")
    },
    motion: {
      easingSamples: khalaEasingNames.map((name) => sampleKhalaEasing(name, 0.5)),
      fade: resolveKhalaMotionKeyframes({ _tag: "Fade" }, "enter"),
      flicker: resolveKhalaMotionKeyframes({ _tag: "Flicker" }, "enter"),
      assembly: resolveKhalaMotionKeyframes({ _tag: "FrameAssembly", phase: "line" }, "enter")
    },
    choreography: {
      model: checkKhalaChoreographyModel(),
      sequence: planKhalaChoreography({ manager: "sequence", target: "entered", children: [
        { id: "a", enterMillis: 100, exitMillis: 80 }, { id: "b", enterMillis: 120, exitMillis: 90 }
      ] })
    },
    frames: frameScenes,
    text: {
      sequence: makeKhalaTextSequenceFrames("A👩🏽‍💻B", { caret: true }),
      decipher: makeKhalaTextDecipherFrames("KHALA READY", 42)
    },
    illumination: {
      svgId: khalaSvgIlluminationGradientId("gallery-primary"),
      native: makeKhalaIlluminationNativePlan({ color: "cyan", radius: 120 })
    },
    backgrounds
  } as const
}

export const khalaUiFinalParityReceipt = {
  sourceRevision: "bdbaa0324900ee978d42036d1304a053c1fe54b5",
  nonAudioRows: khalaUiVisualParity.length,
  shippedRows: khalaUiVisualParity.filter((row) => row.status === "shipped").length,
  plannedRows: khalaUiVisualParity.filter((row) => row.status === "planned").length,
  stories: khalaUiEffectStories.length,
  audio: "excluded",
  reactAuthority: "host-only",
  lifecycleAuthority: "effect-scope"
} as const
