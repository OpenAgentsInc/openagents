export const khalaUiArwesRevision = "bdbaa0324900ee978d42036d1304a053c1fe54b5" as const

export type KhalaUiParityFamily =
  | "foundation"
  | "motion"
  | "choreography"
  | "frame"
  | "text"
  | "illumination"
  | "background"

export type KhalaUiParityStatus = "shipped" | "planned"

export interface KhalaUiParityCapability {
  readonly id: string
  readonly family: KhalaUiParityFamily
  readonly arwesExports: ReadonlyArray<string>
  readonly sourcePaths: ReadonlyArray<string>
  readonly visualBehavior: string
  readonly khalaDestination: string
  readonly rendererContract: string
  readonly accessibilityCorrection: string
  readonly issue: `#${number}`
  readonly status: KhalaUiParityStatus
}

const capability = (
  value: KhalaUiParityCapability
): KhalaUiParityCapability => value

/**
 * Exhaustive non-audio visual parity ledger for the pinned Arwes v1 checkout.
 * Framework wrappers are listed beside the behavior they adapt; they are not
 * counted as separate effects. Infrastructure-only helpers are likewise
 * recorded with the visual primitive that consumes them.
 */
export const khalaUiVisualParity = [
  capability({
    id: "foundation.theme",
    family: "foundation",
    arwesExports: [
      "createThemeMultiplier",
      "createThemeUnit",
      "createThemeColor",
      "createThemeStyle",
      "createThemeBreakpoints",
      "createCreateTheme"
    ],
    sourcePaths: ["packages/theme/src/index.ts"],
    visualBehavior: "Derived multipliers, units, colors, styles, breakpoints, and theme factories.",
    khalaDestination: "@effect-native/tokens Theme/khalaTheme typed roles and resolvers.",
    rendererContract: "Headless authority projected identically into DOM, React DOM, RN, native, and Canvas.",
    accessibilityCorrection: "Semantic roles and contrast are bounded centrally instead of accepting arbitrary theme strings.",
    issue: "#90",
    status: "shipped"
  }),
  capability({
    id: "foundation.steps",
    family: "foundation",
    arwesExports: ["styleSteps"],
    sourcePaths: ["packages/styles/src/styleSteps/styleSteps.ts"],
    visualBehavior: "Alternating hard-stop color/transparent linear-gradient steps.",
    khalaDestination: "Typed Khala stepped-gradient paint descriptor.",
    rendererContract: "CSS gradient on DOM/React DOM; explicit band geometry on RN/Canvas.",
    accessibilityCorrection: "Decoration only; forced colors resolves to a solid structural line.",
    issue: "#98",
    status: "shipped"
  }),
  capability({
    id: "foundation.strip",
    family: "foundation",
    arwesExports: ["styleStrip"],
    sourcePaths: ["packages/styles/src/styleStrip/styleStrip.ts"],
    visualBehavior: "Hard-stop repeating multi-color strip gradient.",
    khalaDestination: "Typed Khala strip paint descriptor with bounded stops.",
    rendererContract: "CSS gradient on DOM/React DOM; explicit bands on RN/Canvas.",
    accessibilityCorrection: "No semantic meaning and no unbounded CSS input.",
    issue: "#98",
    status: "shipped"
  }),
  capability({
    id: "foundation.separator",
    family: "foundation",
    arwesExports: ["styleSeparator"],
    sourcePaths: ["packages/styles/src/styleSeparator/styleSeparator.ts"],
    visualBehavior: "Horizontal/vertical separator with active double blocks at either or both ends.",
    khalaDestination: "Signal-separator geometry and typed directional variants.",
    rendererContract: "Logical segments lower to DOM/SVG, RN views, or Canvas paths.",
    accessibilityCorrection: "Forced colors uses system stroke; never the sole state signal.",
    issue: "#98",
    status: "shipped"
  }),
  capability({
    id: "motion.easing",
    family: "motion",
    arwesExports: ["easing", "easeAmong", "easeSteps"],
    sourcePaths: [
      "packages/animated/src/easing/easing.ts",
      "packages/animated/src/easeAmong/easeAmong.ts",
      "packages/animated/src/easeSteps/easeSteps.ts"
    ],
    visualBehavior: "Linear plus in/out/inOut quad, cubic, quart, quint, sine, expo, circ, back, elastic, and bounce; stepped/list interpolation.",
    khalaDestination: "Pure bounded Khala easing functions and interpolation helpers.",
    rendererContract: "Headless sampling is canonical; renderer drivers consume sampled or native-compatible curves.",
    accessibilityCorrection: "Reduced motion bypasses the sampler and resolves the target at tick zero.",
    issue: "#92",
    status: "shipped"
  }),
  capability({
    id: "motion.css-properties",
    family: "motion",
    arwesExports: ["formatAnimatedCSSProps", "applyAnimatedCSSProps"],
    sourcePaths: [
      "packages/animated/src/formatAnimatedCSSProps/formatAnimatedCSSProps.ts",
      "packages/animated/src/applyAnimatedCSSProps/applyAnimatedCSSProps.ts"
    ],
    visualBehavior: "Transform shorthands and bounded animated style projection.",
    khalaDestination: "Closed typed opacity/transform/stroke animation values.",
    rendererContract: "DOM/React DOM WAAPI/CSS; RN native values; headless target-state snapshot.",
    accessibilityCorrection: "No arbitrary CSS property or string execution; decoration cannot move focus or semantic order.",
    issue: "#92",
    status: "shipped"
  }),
  capability({
    id: "motion.driver",
    family: "motion",
    arwesExports: ["createAnimation"],
    sourcePaths: ["packages/animated/src/createAnimation/createAnimation.ts"],
    visualBehavior: "Duration, delay, repeat, direction, easing, update, finish, and cancellation driver.",
    khalaDestination: "Effect Scope-owned Clock/scheduler animation driver.",
    rendererContract: "One lifecycle contract with renderer-specific paint adapters.",
    accessibilityCorrection: "Cancellation is total; reduced motion allocates no timer, frame, Fiber, or subscription.",
    issue: "#92",
    status: "shipped"
  }),
  capability({
    id: "motion.element",
    family: "motion",
    arwesExports: ["createAnimatedElement", "createAnimatedXElement", "Animated", "AnimatedX", "useAnimated", "useAnimatedX"],
    sourcePaths: ["packages/animated/src/createAnimatedElement", "packages/animated/src/createAnimatedXElement", "packages/react-animated/src"],
    visualBehavior: "State-keyed element attributes/styles and arbitrary named-state transitions.",
    khalaDestination: "Typed Khala visual-state descriptor attached below semantic views.",
    rendererContract: "Effect owns state; React/RN adapters only reconcile paint state.",
    accessibilityCorrection: "Stable semantic content precedes optional client animation and stays visible in SSR/no-JS.",
    issue: "#92",
    status: "shipped"
  }),
  capability({
    id: "motion.presets",
    family: "motion",
    arwesExports: ["transition", "fade", "flicker", "draw", "animateDraw"],
    sourcePaths: ["packages/animated/src/transitions/transitions.ts", "packages/animated/src/animateDraw/animateDraw.ts"],
    visualBehavior: "Property interpolation, opacity fade, multi-keyframe flicker, and SVG stroke progression.",
    khalaDestination: "Owned Khala property/fade/flicker/stroke-draw presets.",
    rendererContract: "DOM/React DOM/SVG supported; RN receives equivalent opacity/transform or static stroke state.",
    accessibilityCorrection: "Flicker is bounded and disabled for reduced motion; stroke drawing never hides semantic content.",
    issue: "#92",
    status: "shipped"
  }),
  capability({
    id: "choreography.animator",
    family: "choreography",
    arwesExports: ["createAnimatorSystem", "Animator", "AnimatorGeneralProvider", "useAnimator", "useAnimatorGeneral"],
    sourcePaths: ["packages/animator/src", "packages/react-animator/src"],
    visualBehavior: "Entered/entering/exiting/exited graph with parallel, sequence, reverse, stagger, reverse stagger, switch, merge, and combine semantics.",
    khalaDestination: "Modeled Effect-owned choreography service.",
    rendererContract: "Serializable transition plans drive DOM, React DOM, RN, SVG, and Canvas adapters.",
    accessibilityCorrection: "Convergence, interruption, deterministic hydration, Strict Mode replay, and zero-work reduced motion are invariants.",
    issue: "#92",
    status: "shipped"
  }),
  ...([
    ["underline", "createFrameUnderlineSettings", "FrameUnderline"],
    ["lines", "createFrameLinesSettings", "FrameLines"],
    ["corners", "createFrameCornersSettings", "FrameCorners"],
    ["octagon", "createFrameOctagonSettings", "FrameOctagon"],
    ["nero", "createFrameNeroSettings", "FrameNero"],
    ["nefrex", "createFrameNefrexSettings", "FrameNefrex"],
    ["kranox", "createFrameKranoxSettings", "FrameKranox"],
    ["header", "createFrameHeaderSettings", "FrameHeader"],
    ["circle", "createFrameCircleSettings", "FrameCircle"]
  ] as const).map(([name, factory, adapter]) =>
    capability({
      id: `frame.${name}`,
      family: "frame",
      arwesExports: [factory, adapter],
      sourcePaths: [`packages/frames/src/createFrame${name[0]!.toUpperCase()}${name.slice(1)}Settings`, `packages/react-frames/src/${adapter}`],
      visualBehavior: `Responsive ${name} background, structural line, and decoration geometry.`,
      khalaDestination: `Owned Khala ${name} frame motif and paint groups.`,
      rendererContract: "Exact inert SVG on DOM/React DOM; declared native geometry or static degradation on RN.",
      accessibilityCorrection: "Decoration is pointer-inert/aria-hidden, collapses before content, and preserves focus clearance.",
      issue: "#98",
      status: "shipped"
    })
  ),
  capability({
    id: "frame.generic",
    family: "frame",
    arwesExports: ["createFrame", "FrameBase"],
    sourcePaths: ["packages/frames/src/createFrame", "packages/react-frames/src/FrameBase"],
    visualBehavior: "Responsive SVG groups, paths, rectangles and circles with attributes, styles, classes, draw hooks, masks, clips, patterns, and transforms.",
    khalaDestination: "Closed Khala frame scene schema and deterministic geometry resolver.",
    rendererContract: "Headless scene is canonical; DOM/React DOM emit SVG; RN lowers supported nodes or documented static fallback.",
    accessibilityCorrection: "No eval, innerHTML, arbitrary selector, or unbounded markup/style escape hatch.",
    issue: "#98",
    status: "shipped"
  }),
  capability({
    id: "frame.clipping",
    family: "frame",
    arwesExports: ["styleFrameClipOctagon", "styleFrameClipKranox"],
    sourcePaths: ["packages/frames/src/styleFrameClipOctagon", "packages/frames/src/styleFrameClipKranox"],
    visualBehavior: "Responsive octagonal and asymmetric cut-corner clipping polygons.",
    khalaDestination: "Typed clip polygon geometry paired with the matching frame motif.",
    rendererContract: "CSS clip-path on DOM/React DOM; native mask where available; border-only fallback otherwise.",
    accessibilityCorrection: "Content and focus are never clipped; clipping applies only to decorative/background layers.",
    issue: "#98",
    status: "shipped"
  }),
  capability({
    id: "frame.assembly",
    family: "motion",
    arwesExports: ["animateFrameAssembler", "useFrameAssembler"],
    sourcePaths: ["packages/frames/src/animateFrameAssembler", "packages/react-frames/src/useFrameAssembler"],
    visualBehavior: "Ordered background fade, structural stroke drawing, and decoration reveal/conceal.",
    khalaDestination: "Khala frame-assembly transition plan on the choreography service.",
    rendererContract: "SVG/DOM supported, RN equivalent or stable assembled state, headless deterministic plan.",
    accessibilityCorrection: "Content is already complete; reduced motion begins assembled with no driver.",
    issue: "#92",
    status: "shipped"
  }),
  capability({
    id: "text.sequence",
    family: "text",
    arwesExports: ["animateTextSequence", "Text"],
    sourcePaths: ["packages/text/src/animateTextSequence", "packages/react-text/src/Text"],
    visualBehavior: "Character sequence reveal/conceal with optional blinking caret.",
    khalaDestination: "Bounded grapheme sequence effect over an aria-hidden duplicate.",
    rendererContract: "DOM/React DOM visual duplicate; RN bounded or static native degradation.",
    accessibilityCorrection: "One complete stable semantic string remains exposed and visible without motion.",
    issue: "#99",
    status: "shipped"
  }),
  capability({
    id: "text.decipher",
    family: "text",
    arwesExports: ["animateTextDecipher", "getAnimationTextDuration"],
    sourcePaths: ["packages/text/src/animateTextDecipher", "packages/text/src/getAnimationTextDuration"],
    visualBehavior: "Random-order cipher-to-real and real-to-cipher character transitions with content-derived duration.",
    khalaDestination: "Seeded, grapheme-aware Khala decipher effect with bounded vocabulary/duration.",
    rendererContract: "DOM/React DOM supported; RN explicit bounded/static degradation.",
    accessibilityCorrection: "Deterministic visual duplicate only; never mutates or hides the semantic text node.",
    issue: "#99",
    status: "shipped"
  }),
  capability({
    id: "illumination.html",
    family: "illumination",
    arwesExports: ["createEffectIlluminator", "Illuminator"],
    sourcePaths: ["packages/effects/src/createEffectIlluminator", "packages/react-effects/src/Illuminator"],
    visualBehavior: "Pointer-positioned radial light over an HTML surface.",
    khalaDestination: "Container-local Khala radial illumination descriptor and driver.",
    rendererContract: "DOM/React DOM supported; focus/static native degradation on RN.",
    accessibilityCorrection: "Local Pointer Events, cached bounds, frame coalescing, coarse-pointer/focus/reduced-motion fallbacks.",
    issue: "#100",
    status: "shipped"
  }),
  capability({
    id: "illumination.svg",
    family: "illumination",
    arwesExports: ["createEffectIlluminatorSVG", "IlluminatorSVG"],
    sourcePaths: ["packages/effects/src/createEffectIlluminatorSVG", "packages/react-effects/src/IlluminatorSVG"],
    visualBehavior: "Pointer-positioned radial gradient circle inside SVG geometry.",
    khalaDestination: "Deterministic SVG gradient/clip illumination driven by the same local service.",
    rendererContract: "SVG DOM/React DOM supported; stable structural highlight elsewhere.",
    accessibilityCorrection: "Deterministic IDs, inert nodes, local events, zero driver for reduced/coarse input.",
    issue: "#100",
    status: "shipped"
  }),
  ...([
    ["dots", "createBackgroundDots", "Dots", "box/circle/cross lattice with origin-progress reveal"],
    ["grid-lines", "createBackgroundGridLines", "GridLines", "dashed horizontal/vertical grid with opacity transition"],
    ["moving-lines", "createBackgroundMovingLines", "MovingLines", "seeded traveling luminous line sets"],
    ["puffs", "createBackgroundPuffs", "Puffs", "seeded moving radial-gradient puff sets"]
  ] as const).map(([name, factory, adapter, behavior]) =>
    capability({
      id: `background.${name}`,
      family: "background",
      arwesExports: [factory, adapter],
      sourcePaths: [`packages/bgs/src/${factory}`, `packages/react-bgs/src/${adapter}`],
      visualBehavior: behavior,
      khalaDestination: `Render-canvas Khala ${name} scene with typed quality and seed.`,
      rendererContract: "Canvas in DOM/React/Electron hosts; deterministic static image/geometry degradation on unsupported renderers.",
      accessibilityCorrection: "One Scope-owned scheduler, DPR cap, visibility/power suspension, deterministic seed, zero loop for reduced motion.",
      issue: "#93",
      status: "shipped"
    })
  )
] as const satisfies ReadonlyArray<KhalaUiParityCapability>

export const khalaUiAudioExclusions = [
  "packages/bleeps",
  "packages/react-bleeps",
  "packages/react-core/src/BleepsOnAnimator",
  "static/assets/sounds",
  "@arwes/bleeps",
  "@arwes/react-bleeps"
] as const

export const khalaUiExcludedVisualRuntimeDependencies = ["arwes", "@arwes/", "@motionone/", "motion"] as const
