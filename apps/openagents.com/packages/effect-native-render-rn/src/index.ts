import { Deferred, Effect, Exit, FiberSet, Ref, Scope, Stream } from "effect"
import {
  type AriaRole,
  type BadgeView,
  type ButtonView,
  type CardView,
  type CheckboxView,
  type ChipView,
  type CodeBlockView,
  type CodeToken,
  type CodeTokenKind,
  codeBlockPlainText,
  type ColorToken,
  type DiffViewView,
  type GraphFigureView,
  type GraphStatus,
  graphStatusColorToken,
  type TimelineView,
  type SectionView,
  type HeroView,
  type AnnouncementBadgeView,
  type CtaSectionView,
  type FooterView,
  type NavBarView,
  type AccordionView,
  type PricingColumnView,
  type PricingTableView,
  type LogoRowView,
  type StatsBandView,
  type GlowView,
  type MockupFrameView,
  type PagerView,
  type SwipeableListItemView,
  type BackgroundGradientView,
  type WallpaperView,
  type SpotlightView,
  type FrameView,
  type BlurredPopupView,
  type IconButtonView,
  type ToolbarView,
  type ComboboxOption,
  type ComboboxView,
  type CommandPaletteView,
  type ComposerView,
  composerPlainText,
  type ContextMenuView,
  type DividerView,
  type DropdownMenuView,
  type FieldBinding,
  type FieldRowView,
  type MarkdownBlock,
  type MarkdownInline,
  type MarkdownView,
  type MenuItem,
  type MeterView,
  type NotificationModel,
  type NumberFieldView,
  type PopoverView,
  type RadioGroupView,
  type RecoveryOverlayView,
  type SelectView,
  type SliderView,
  type StatusBannerView,
  type ToastRegionView,
  type ToastView,
  type ToggleView,
  type TooltipView,
  type StatTileView,
  type TableView,
  type Tone,
  type HostKind,
  type HostView,
  type IconName,
  type IconSize,
  type IconView,
  type Dimension,
  type FlatStyle,
  FormFieldValueBinding,
  type ImageView,
  type IntentError,
  IntentRef,
  type IntentReporter,
  type JsonPayload,
  type LinkView,
  type ListView,
  type ModalView,
  type MountedSurface,
  type NavRailView,
  type PlatformVariant,
  type RendererAdapter,
  type SectionListView,
  type SheetView,
  type SpacerView,
  type SplitPaneView,
  type StackView,
  type TabsView,
  type TextFieldView,
  type TextView,
  type TranscriptView,
  type View,
  type WorkbenchView,
  type Viewport,
  type ViewportInput,
  StaticPayload,
  defaultViewportInput,
  defaultTheme,
  makeViewport,
  makeViewportService,
  makeNavigateIntent,
  resolveResponsiveValue,
  resolveView,
  resolveStyle
} from "@effect-native/core"
import {
  type DimensionToken,
  type RadiusToken,
  type SpacingToken,
  type Theme,
  type TypeScaleToken
} from "@effect-native/tokens"

export const packageName = "@effect-native/render-rn" as const

export type ReactNativePlatform = Extract<PlatformVariant, "ios" | "android">

export type ReactNativeStyleValue = string | number | boolean | undefined
export type ReactNativeStyle = Record<string, ReactNativeStyleValue>

export interface ReactElementLike {
  readonly type: unknown
  readonly key?: string | null
  readonly props: Record<string, unknown>
}

export type ReactNodeLike = ReactElementLike | string | number | boolean | null | undefined

export interface ReactRuntime {
  readonly createElement: (
    type: unknown,
    props?: Record<string, unknown> | null,
    ...children: ReadonlyArray<ReactNodeLike>
  ) => ReactElementLike
  readonly useEffect?: (
    effect: () => void | (() => void),
    dependencies?: ReadonlyArray<unknown>
  ) => void
  readonly useState?: <State>(
    initial: State | (() => State)
  ) => readonly [State, (value: State | ((current: State) => State)) => void]
}

export interface ReactNativeRuntime {
  readonly View: unknown
  readonly Text: unknown
  readonly Pressable: unknown
  readonly TextInput: unknown
  readonly FlatList: unknown
  readonly SectionList: unknown
  readonly Image: unknown
  readonly Modal: unknown
  /** Optional — present on real RN; headless tests may omit and still declare onRefresh. */
  readonly RefreshControl?: unknown
  // Optional — present on real RN. The @expo/ui glass lowering gates on
  // Platform.OS === "ios" and major Version >= 26 (real Liquid Glass); below
  // that the renderer keeps the honest RN material approximation.
  readonly Platform?: {
    readonly OS?: string
    readonly Version?: string | number
  }
  readonly Dimensions?: ReactNativeDimensions
  readonly StyleSheet?: {
    readonly create: <Styles extends Record<string, ReactNativeStyle>>(styles: Styles) => Styles
  }
}

export interface ReactNativeDimensionMetrics {
  readonly width: number
  readonly height: number
}

export interface ReactNativeDimensions {
  readonly get: (name: "window" | string) => ReactNativeDimensionMetrics
  readonly addEventListener?: (
    type: "change",
    listener: (event: { readonly window?: ReactNativeDimensionMetrics }) => void
  ) => { readonly remove: () => void } | (() => void)
}

export interface ReactNativeDependencies {
  readonly React: ReactRuntime
  readonly ReactNative: ReactNativeRuntime
}

export interface ReactNativeRenderOptions {
  readonly theme?: Theme
  readonly platform?: ReactNativePlatform
  readonly viewport?: ViewportInput | Viewport
  // Registered host drivers (issue #70 ask 2, GL-1 openagents#8647): the only
  // injection point through which native/imperative views mount behind the
  // typed `Host` catalog contract. `EffectNativeSurface` and
  // `makeReactNativeRenderer` build a Scope-bound instance runtime from this
  // list automatically; a bare `renderReactNativeView` call mounts transient
  // per-emission instances (unit-test posture — no retained native state).
  readonly hostDrivers?: ReadonlyArray<ReactNativeHostDriver>
  // Per-surface Scope-bound host-instance runtime. Created internally by the
  // surface entrypoints from `hostDrivers`; apps normally never construct one.
  readonly hostRuntime?: ReactNativeHostRuntime
  // GL-1 (openagents#8647) internal @expo/ui SwiftUI lowering seam. By default
  // the renderer require()s "@expo/ui/swift-ui" ITSELF when a glass component
  // renders on iOS 26+; this override exists for tests (inject a fake runtime)
  // — app code must never import @expo/ui.
  readonly expoUi?: ExpoUiSwiftUiRuntime
}

export type ReactNativeHostEffectRuntime = (effect: Effect.Effect<void, never>) => void

export interface ReactNativeRenderRuntimeOptions extends ReactNativeRenderOptions {
  readonly runEffect?: ReactNativeHostEffectRuntime
}

export interface EffectNativeSurfaceProps extends ReactNativeRenderOptions {
  readonly viewStream: Stream.Stream<View>
  readonly report: IntentReporter
  readonly initialView?: View
}

export interface ReactNativeContainer {
  readonly render?: (element: ReactNodeLike | undefined) => void
}

export interface ReactNativeRendererOptions extends ReactNativeRenderOptions {
  readonly dependencies?: ReactNativeDependencies
}

export interface ReactNativeMountedSurface extends MountedSurface {
  readonly current: Effect.Effect<View | undefined>
  readonly currentElement: Effect.Effect<ReactNodeLike | undefined>
  readonly serialize: Effect.Effect<ReactNativeStructure | undefined>
  readonly currentViewport: Effect.Effect<Viewport>
  readonly setViewport: (input: ViewportInput) => Effect.Effect<void>
}

export interface ReactNativeStructure {
  readonly tag: View["_tag"]
  readonly key?: string
  readonly text?: string
  readonly children?: ReadonlyArray<ReactNativeStructure>
}

const loadPeerDependencies = (): ReactNativeDependencies => {
  if (typeof require !== "function") {
    throw new Error("EffectNativeSurface requires react and react-native to be available from the host app")
  }

  return {
    React: require("react") as ReactRuntime,
    ReactNative: require("react-native") as ReactNativeRuntime
  }
}

// ── Scope-bound host-driver registry (issue #70 ask 2; mirrors render-dom's
// DomHostDriver contract for the declarative RN element tree) ────────────────
//
// A driver is the ONLY place imperative/native-module view code lives on RN
// (SwiftUI islands, native video, editors). Its lifecycle is owned by the
// renderer and bound to the surface Scope: `mount` when the Host node first
// appears, `render` on every typed prop emission, `unmount` on Scope exit or
// when the node leaves the tree. Props enter through `decodeProps` (Schema
// decode — throwing surfaces as a loud host error marker, never a silent
// no-op) and events leave only through `emit`, which dispatches the Host
// node's `onEvent` as a typed intent through the surface's IntentReporter.

export interface ReactNativeHostContext {
  readonly dependencies: ReactNativeDependencies
  readonly report: IntentReporter
  // Emit a typed host event outward as the Host node's `onEvent` intent. The
  // binding always targets the CURRENT view emission's `onEvent`, not the one
  // captured at mount.
  readonly emit: (payload: JsonPayload) => void
}

export interface ReactNativeHostInstance {
  // Produce the React element for the current decoded props. Called once per
  // view emission while the Host node stays mounted; React reconciles the
  // returned elements, the driver owns any imperative native state behind them.
  readonly render: (props: unknown) => ReactElementLike
  readonly unmount: () => void
}

export interface ReactNativeHostDriver {
  readonly kind: HostKind
  // Decode/validate the opaque props payload for this host kind. Throwing here
  // surfaces as a loud host error marker, never a silent no-op.
  readonly decodeProps: (props: JsonPayload) => unknown
  readonly mount: (props: unknown, context: ReactNativeHostContext) => ReactNativeHostInstance
}

interface HostInstanceRecord {
  readonly kind: HostKind
  instance: ReactNativeHostInstance
  // Latest emission's event binding — `emit` reads these at dispatch time.
  onEvent: IntentRef | undefined
  report: IntentReporter
  seen: boolean
}

export interface ReactNativeHostRuntime {
  readonly resolve: (kind: HostKind) => ReactNativeHostDriver | undefined
  // Get-or-mount the instance for this Host node (keyed kind + view key, the
  // same identity rule as render-dom) and render the current decoded props.
  readonly render: (
    view: HostView,
    driver: ReactNativeHostDriver,
    decoded: unknown,
    dependencies: ReactNativeDependencies,
    report: IntentReporter
  ) => ReactElementLike
  // Unmount every instance not rendered since the previous sweep (the node
  // left the tree). Surface entrypoints call this after each full render pass.
  readonly sweep: () => void
  // Unmount everything — bound to the surface Scope / React unmount cleanup.
  readonly dispose: () => void
}

const hostInstanceKey = (view: HostView): string => `${view.kind}:${view.key ?? ""}`

export const makeReactNativeHostRuntime = (
  drivers: ReadonlyArray<ReactNativeHostDriver>
): ReactNativeHostRuntime => {
  const byKind = new Map<HostKind, ReactNativeHostDriver>(drivers.map((driver) => [driver.kind, driver] as const))
  const instances = new Map<string, HostInstanceRecord>()

  const unmountRecord = (record: HostInstanceRecord): void => {
    try {
      record.instance.unmount()
    } catch {
      // Driver teardown must stay total: one faulty driver never breaks the
      // surface's Scope close or the other instances' teardown.
    }
  }

  return {
    resolve: (kind) => byKind.get(kind),
    render: (view, driver, decoded, dependencies, report) => {
      const key = hostInstanceKey(view)
      let record = instances.get(key)
      if (record !== undefined && record.kind !== view.kind) {
        unmountRecord(record)
        instances.delete(key)
        record = undefined
      }
      if (record === undefined) {
        const created: HostInstanceRecord = {
          kind: view.kind,
          instance: undefined as unknown as ReactNativeHostInstance,
          onEvent: view.onEvent,
          report,
          seen: true
        }
        const context: ReactNativeHostContext = {
          dependencies,
          report,
          emit: (payload) => {
            if (created.onEvent !== undefined) {
              runReportedIntent(created.report, created.onEvent, payload)
            }
          }
        }
        created.instance = driver.mount(decoded, context)
        instances.set(key, created)
        return created.instance.render(decoded)
      }
      record.onEvent = view.onEvent
      record.report = report
      record.seen = true
      return record.instance.render(decoded)
    },
    sweep: () => {
      for (const [key, record] of instances) {
        if (!record.seen) {
          unmountRecord(record)
          instances.delete(key)
        } else {
          record.seen = false
        }
      }
    },
    dispose: () => {
      for (const record of instances.values()) {
        unmountRecord(record)
      }
      instances.clear()
    }
  }
}

const fontWeightValue = (weight: string): number | string => {
  switch (weight) {
    case "regular":
      return 400
    case "medium":
      return 500
    case "semibold":
      return 600
    case "bold":
      return 700
    default:
      return weight
  }
}

const flexKeyword = (value: string): string => {
  switch (value) {
    case "start":
      return "flex-start"
    case "end":
      return "flex-end"
    case "between":
      return "space-between"
    case "around":
      return "space-around"
    default:
      return value
  }
}

const spacingValue = (theme: Theme, token: SpacingToken): number => theme.spacing[token]
const colorValue = (theme: Theme, token: ColorToken): string => theme.color[token]
const radiusValue = (theme: Theme, token: RadiusToken): number => theme.radius[token]
const dimensionValue = (theme: Theme, value: Dimension): number | string =>
  typeof value === "number" ? value : theme.dimension[value as DimensionToken]
const typeScaleValue = (theme: Theme, token: TypeScaleToken): ReactNativeStyle => {
  const value = theme.typeScale[token]
  return {
    fontSize: value.fontSize,
    lineHeight: value.lineHeight,
    fontWeight: value.fontWeight
  }
}

// GL-1 glass material. React Native core has no backdrop blur, so "glass"
// lowers to an HONEST approximation: the theme surface color at ~0.72 opacity
// plus a 1px hairline of the theme border color. High-fidelity iOS Liquid
// Glass lowering arrives via the @expo/ui native-island lane (openagents
// GL-1); this renderer stays dependency-free.
const translucentColor = (color: string, alpha: number): string => {
  const hex = /^#([0-9a-fA-F]{6})$/.exec(color)
  if (hex === null) {
    // Non-hex theme colors pass through untouched rather than guessing.
    return color
  }
  const value = hex[1]!
  const r = parseInt(value.slice(0, 2), 16)
  const g = parseInt(value.slice(2, 4), 16)
  const b = parseInt(value.slice(4, 6), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

const glassSurfaceStyle = (theme: Theme): ReactNativeStyle => ({
  backgroundColor: translucentColor(colorValue(theme, "surface"), 0.72),
  borderColor: colorValue(theme, "border"),
  borderWidth: 1
})

const styleDeclarations = (
  theme: Theme,
  key: string,
  value: unknown
): ReadonlyArray<readonly [string, ReactNativeStyleValue]> => {
  switch (key) {
    case "margin":
    case "marginTop":
    case "marginRight":
    case "marginBottom":
    case "marginLeft":
    case "padding":
    case "paddingTop":
    case "paddingRight":
    case "paddingBottom":
    case "paddingLeft":
    case "gap":
      return [[key, spacingValue(theme, value as SpacingToken)]]
    case "width":
    case "height":
    case "minWidth":
    case "minHeight":
    case "maxWidth":
    case "maxHeight":
      return [[key, dimensionValue(theme, value as Dimension)]]
    case "flex":
    case "opacity":
    case "borderWidth":
      return [[key, value as number]]
    case "alignSelf":
      return [["alignSelf", flexKeyword(String(value))]]
    case "backgroundColor":
    case "borderColor":
    case "color":
      return [[key, colorValue(theme, value as ColorToken)]]
    case "borderRadius":
      return [["borderRadius", radiusValue(theme, value as RadiusToken)]]
    case "fontWeight":
      return [["fontWeight", fontWeightValue(String(value))]]
    case "textAlign":
      return [["textAlign", String(value)]]
    case "typeScale":
      return Object.entries(typeScaleValue(theme, value as TypeScaleToken))
    case "surface":
      // GL-1 "glass": see glassSurfaceStyle — translucent theme surface plus
      // hairline border, the honest RN-core approximation of the material.
      return Object.entries(glassSurfaceStyle(theme))
    default:
      return []
  }
}

export const lowerStyle = (
  style: FlatStyle | undefined,
  options: ReactNativeRenderOptions = {}
): ReactNativeStyle => {
  const theme = options.theme ?? defaultTheme
  const lowered = new Map<string, ReactNativeStyleValue>()

  if (style !== undefined) {
    for (const [key, value] of Object.entries(style)) {
      for (const [property, nativeValue] of styleDeclarations(theme, key, value)) {
        lowered.set(property, nativeValue)
      }
    }
  }

  return Object.fromEntries(lowered)
}

const viewStyle = (view: View, options: ReactNativeRenderOptions): ReactNativeStyle => {
  if (!("style" in view) || view.style === undefined) {
    return {}
  }

  const viewport = options.viewport === undefined
    ? undefined
    : makeViewport(options.viewport, options.theme ?? defaultTheme)
  return lowerStyle(resolveStyle(view.style, {
    platform: options.platform ?? "ios",
    ...(viewport === undefined ? {} : { breakpoint: viewport.breakpoint })
  }), options)
}

const mergeNativeStyles = (...styles: ReadonlyArray<ReactNativeStyle | undefined>): ReactNativeStyle =>
  Object.assign({}, ...styles.filter((style): style is ReactNativeStyle => style !== undefined))

const nativeId = (view: View): string =>
  `effect-native:${view._tag}:${view.key === undefined ? "" : encodeURIComponent(view.key)}`

const parseNativeId = (value: unknown): { readonly tag: View["_tag"]; readonly key?: string } | undefined => {
  if (typeof value !== "string" || !value.startsWith("effect-native:")) {
    return undefined
  }

  const [, tag, encodedKey = ""] = value.split(":")
  if (tag === undefined || tag.length === 0) {
    return undefined
  }

  return {
    tag: tag as View["_tag"],
    ...(encodedKey.length === 0 ? {} : { key: decodeURIComponent(encodedKey) })
  }
}

const createElement = (
  dependencies: ReactNativeDependencies,
  type: unknown,
  props: Record<string, unknown>,
  ...children: ReadonlyArray<ReactNodeLike>
): ReactElementLike => dependencies.React.createElement(type, props, ...children)

const runReportedIntent = (
  report: IntentReporter,
  ref: IntentRef,
  runtimeValue: JsonPayload = null
): void => {
  const runEffect = reactNativeIntentReporterRuntimes.get(report)
  if (runEffect !== undefined) {
    runEffect((report(ref, runtimeValue) as Effect.Effect<void, IntentError>).pipe(Effect.ignoreCause))
  }
}

const reactNativeIntentReporterRuntimes = new WeakMap<IntentReporter, ReactNativeHostEffectRuntime>()

const makeReactNativeRefOwnedEffectRuntime = (): {
  readonly runEffect: ReactNativeHostEffectRuntime
  readonly dispose: () => void
} => {
  const interruptors = new Set<(interruptor?: number) => void>()
  let disposed = false
  return {
    runEffect: (effect) => {
      if (disposed) {
        return
      }
      let completed = false
      let interrupt: ((interruptor?: number) => void) | undefined
      interrupt = Effect.runCallback(effect, {
        onExit: () => {
          completed = true
          if (interrupt !== undefined) {
            interruptors.delete(interrupt)
          }
        }
      })
      if (completed) {
        return
      }
      if (disposed) {
        interrupt()
      } else {
        interruptors.add(interrupt)
      }
    },
    dispose: () => {
      if (disposed) {
        return
      }
      disposed = true
      for (const interrupt of interruptors) {
        interrupt()
      }
      interruptors.clear()
    }
  }
}

const withReactNativeHostEffectRuntime = (
  report: IntentReporter,
  runEffect: ReactNativeHostEffectRuntime | undefined
): IntentReporter => {
  if (runEffect === undefined) {
    return report
  }
  const scopedReport: IntentReporter = (ref, runtimeValue) => report(ref, runtimeValue)
  reactNativeIntentReporterRuntimes.set(scopedReport, runEffect)
  return scopedReport
}

// Map the bounded ARIA role contract to the RN accessibilityRole values that
// actually exist on the platform. Roles with no faithful RN equivalent are
// omitted rather than approximated. Keyboard/paste/pointer/drag-drop intents
// from the interaction algebra (issue #24) have no React Native host mapping;
// the RN renderer declares them unsupported by not wiring them (the headless
// renderer still records them in the serialized tree for cross-renderer tests).
const rnAccessibilityRole: Partial<Record<AriaRole, string>> = {
  combobox: "combobox",
  menu: "menu",
  menuitem: "menuitem",
  list: "list",
  tablist: "tablist",
  tab: "tab",
  none: "none",
  presentation: "none"
}

const accessibilityProps = (view: View): Record<string, unknown> => {
  const a11y = view.a11y
  if (a11y === undefined) return {}
  const props: Record<string, unknown> = {}
  if (a11y.role !== undefined && rnAccessibilityRole[a11y.role] !== undefined) {
    props["accessibilityRole"] = rnAccessibilityRole[a11y.role]
  }
  if (a11y.label !== undefined) props["accessibilityLabel"] = a11y.label
  const stateEntries: Record<string, boolean> = {}
  if (a11y.selected !== undefined) stateEntries["selected"] = a11y.selected
  if (a11y.expanded !== undefined) stateEntries["expanded"] = a11y.expanded
  if (a11y.disabled !== undefined) stateEntries["disabled"] = a11y.disabled
  if (Object.keys(stateEntries).length > 0) props["accessibilityState"] = stateEntries
  return props
}

const mobileGestureProps = (
  view: View,
  report: IntentReporter
): Record<string, unknown> => {
  const interactions = "interactions" in view ? view.interactions : undefined
  if (interactions === undefined) return {}
  return {
    ...(interactions.onLongPress === undefined
      ? {}
      : {
          onLongPress: () => runReportedIntent(report, interactions.onLongPress!)
        }),
    ...(interactions.onSwipe === undefined
      ? {}
      : {
          // Commit swipe via accessibility action until gesture-handler is host-injected (#56).
          accessibilityActions: [{ name: "swipeLeft" }, { name: "swipeRight" }],
          onAccessibilityAction: (event: { readonly nativeEvent: { readonly actionName: string } }) => {
            const name = event.nativeEvent.actionName
            const direction = name === "swipeLeft" ? "left" : name === "swipeRight" ? "right" : "up"
            runReportedIntent(report, interactions.onSwipe!, direction)
          }
        })
  }
}

const baseProps = (view: View, style: ReactNativeStyle): Record<string, unknown> => ({
  key: view.key,
  nativeID: nativeId(view),
  ...accessibilityProps(view),
  style
})

const readReactNativeViewport = (dependencies: ReactNativeDependencies): ViewportInput => {
  const dimensions = dependencies.ReactNative.Dimensions
  if (dimensions === undefined) {
    return defaultViewportInput
  }
  const window = dimensions.get("window")
  return {
    width: window.width,
    height: window.height
  }
}

const renderStack = (
  view: StackView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  if (
    resolvedFlatStyle(view, options)?.surface === "glass" &&
    view.children.length > 0 &&
    view.children.every(expoUiLowerableChild)
  ) {
    const expoUi = glassLoweringRuntime(dependencies, options)
    if (expoUi !== undefined) {
      return renderExpoUiGlassContainer(view, expoUi, dependencies, report, options)
    }
  }
  const direction = resolveResponsiveValue(view.direction)
  const gap = view.gap === undefined ? undefined : resolveResponsiveValue(view.gap)
  const padding = view.padding === undefined ? undefined : resolveResponsiveValue(view.padding)
  const style = mergeNativeStyles({
    display: "flex",
    flexDirection: direction,
    ...(gap === undefined ? {} : { gap: spacingValue(options.theme ?? defaultTheme, gap) }),
    ...(view.align === undefined ? {} : { alignItems: flexKeyword(view.align) }),
    ...(view.justify === undefined ? {} : { justifyContent: flexKeyword(view.justify) }),
    ...(padding === undefined ? {} : { padding: spacingValue(options.theme ?? defaultTheme, padding) })
  }, viewStyle(view, options))

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      // A labelled structural View becomes one giant TalkBack/VoiceOver focus
      // target and hides every interactive descendant. Stacks are layout
      // containers, never controls; keep their children independently
      // discoverable even when the catalog carries a region/group label.
      accessible: false,
      importantForAccessibility: "no"
    },
    ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
  )
}

const renderText = (
  view: TextView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const style = mergeNativeStyles(
    typeScaleValue(options.theme ?? defaultTheme, view.variant),
    view.color === undefined ? undefined : { color: colorValue(options.theme ?? defaultTheme, view.color) },
    view.weight === undefined ? undefined : { fontWeight: fontWeightValue(view.weight) },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.Text,
    {
      ...baseProps(view, style),
      accessibilityRole: view.variant === "heading" || view.variant === "title" ? "header" : "text"
    },
    String(view.content)
  )
}

// Button variant lowering (openagents #8597 escalation; vendored from
// upstream effect-native fd1ccc5): RN Text does NOT inherit color and
// Pressable has no default surface, so a Button without explicit theme
// lowering renders a default-black label on whatever the app background is —
// invisible on dark themes. Variants lower to theme tokens: primary = accent
// surface, secondary = surface + border, ghost = accent text on transparent.
// App-level `view.style` still wins via merge order.
const buttonVariantStyle = (view: ButtonView, theme: Theme): ReactNativeStyle => {
  switch (view.variant) {
    case "primary":
      return { backgroundColor: colorValue(theme, "accent") }
    case "secondary":
      return {
        backgroundColor: colorValue(theme, "surface"),
        borderColor: colorValue(theme, "border"),
        borderWidth: 1
      }
    case "ghost":
      return { backgroundColor: "transparent" }
  }
}

const buttonLabelColor = (view: ButtonView, theme: Theme): string =>
  view.variant === "ghost" ? colorValue(theme, "accent") : colorValue(theme, "textPrimary")

const renderButton = (
  view: ButtonView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  if (resolvedFlatStyle(view, options)?.surface === "glass") {
    const expoUi = glassLoweringRuntime(dependencies, options)
    if (expoUi !== undefined) {
      return renderExpoUiButton(view, expoUi, dependencies, report, options)
    }
  }
  const theme = options.theme ?? defaultTheme
  const style = mergeNativeStyles(
    {
      ...buttonVariantStyle(view, theme),
      paddingVertical: spacingValue(theme, "2.5"),
      paddingHorizontal: spacingValue(theme, "4"),
      borderRadius: radiusValue(theme, "md"),
      alignItems: "center",
      justifyContent: "center",
      opacity: view.disabled === true ? 0.5 : 1
    },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      ...baseProps(view, style),
      accessibilityRole: "button",
      accessibilityState: { disabled: view.disabled === true },
      disabled: view.disabled === true,
      onPress: () => {
        if (view.disabled !== true) {
          runReportedIntent(report, view.onPress)
        }
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      {
        style: mergeNativeStyles(
          typeScaleValue(theme, "label"),
          { color: buttonLabelColor(view, theme) }
        )
      },
      view.label
    )
  )
}

const renderLink = (
  view: LinkView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  return createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      ...baseProps(view, viewStyle(view, options)),
      accessibilityRole: "link",
      onPress: () => runReportedIntent(report, makeNavigateIntent(view.destination))
    },
    ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
  )
}

const dismissOverlay = (view: ModalView | SheetView, report: IntentReporter): void => {
  if (view.dismissable) {
    runReportedIntent(report, view.onDismiss)
  }
}

const overlayPanelStyle = (
  options: ReactNativeRenderOptions,
  extra: ReactNativeStyle = {}
): ReactNativeStyle =>
  mergeNativeStyles({
    backgroundColor: colorValue(options.theme ?? defaultTheme, "background"),
    borderColor: colorValue(options.theme ?? defaultTheme, "border"),
    borderWidth: 1,
    padding: spacingValue(options.theme ?? defaultTheme, "4"),
    borderRadius: radiusValue(options.theme ?? defaultTheme, "lg")
  }, extra)

const renderModal = (
  view: ModalView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const open = view.open === true
  return createElement(
    dependencies,
    dependencies.ReactNative.Modal,
    {
      ...baseProps(view, {}),
      animationType: "fade",
      transparent: true,
      visible: open,
      onRequestClose: () => dismissOverlay(view, report),
      accessibilityViewIsModal: open
    },
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        style: {
          flex: 1,
          alignItems: "center",
          justifyContent: "center",
          padding: spacingValue(options.theme ?? defaultTheme, "4"),
          backgroundColor: "rgba(15, 23, 42, 0.32)"
        }
      },
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          style: overlayPanelStyle(options, {
            width: dimensionValue(options.theme ?? defaultTheme, view.size),
            maxWidth: "100%"
          })
        },
        createElement(
          dependencies,
          dependencies.ReactNative.Text,
          {
            style: typeScaleValue(options.theme ?? defaultTheme, "title")
          },
          String(view.title)
        ),
        ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
      )
    )
  )
}

const renderSheet = (
  view: SheetView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const open = view.open === true
  const size = dimensionValue(options.theme ?? defaultTheme, view.detents[0]!)
  const panelStyle = view.edge === "bottom"
    ? overlayPanelStyle(options, {
        width: "100%",
        height: size,
        borderBottomLeftRadius: 0,
        borderBottomRightRadius: 0
      })
    : overlayPanelStyle(options, {
        width: size,
        height: "100%",
        borderTopRightRadius: 0,
        borderBottomRightRadius: 0
      })

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, {
        display: open ? "flex" : "none",
        position: "absolute",
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
        flexDirection: view.edge === "bottom" ? "column" : "row",
        alignItems: view.edge === "bottom" ? "stretch" : "stretch",
        justifyContent: view.edge === "bottom" ? "flex-end" : "flex-end",
        backgroundColor: "rgba(15, 23, 42, 0.32)"
      }),
      accessibilityViewIsModal: open,
      accessibilityElementsHidden: !open,
      importantForAccessibility: open ? "yes" : "no-hide-descendants"
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        style: {
          position: "absolute",
          top: 0,
          right: 0,
          bottom: 0,
          left: 0
        },
        onPress: () => dismissOverlay(view, report)
      }
    ),
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      { style: panelStyle },
      ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
    )
  )
}

const renderImage = (
  view: ImageView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const width = view.width === undefined ? undefined : resolveResponsiveValue(view.width)
  const height = view.height === undefined ? undefined : resolveResponsiveValue(view.height)
  const style = mergeNativeStyles(
    {
      ...(width === undefined ? {} : { width: dimensionValue(options.theme ?? defaultTheme, width) }),
      ...(height === undefined ? {} : { height: dimensionValue(options.theme ?? defaultTheme, height) })
    },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.Image,
    {
      ...baseProps(view, style),
      accessibilityLabel: view.alt,
      alt: view.alt,
      resizeMode: view.fit,
      source: { uri: view.source }
    }
  )
}

const renderTextField = (
  view: TextFieldView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const onChange = view.field === undefined
    ? view.onChange
    : IntentRef("FormFieldChanged", FormFieldValueBinding(view.field))
  return createElement(
    dependencies,
    dependencies.ReactNative.TextInput,
    {
      ...baseProps(view, viewStyle(view, options)),
      accessibilityLabel: view.label,
      autoFocus: view.focused === true,
      multiline: view.multiline === true,
      // v29 (#72): disabled fields accept no input; clear-on-submit rides the
      // controlled `value` prop — RN TextInput always honors app resets.
      editable: view.disabled !== true,
      onChangeText: (value: string) => {
        if (view.disabled === true) return
        if (onChange !== undefined) {
          runReportedIntent(report, onChange, value)
        }
      },
      onBlur: () => {
        if (view.field !== undefined) {
          runReportedIntent(report, IntentRef("FormFieldBlurred", StaticPayload(view.field)))
        }
      },
      onSubmitEditing: (event: { readonly nativeEvent?: { readonly text?: string } }) => {
        if (view.disabled === true) return
        if (view.onSubmit !== undefined) {
          runReportedIntent(report, view.onSubmit, event.nativeEvent?.text ?? view.value)
        }
      },
      placeholder: view.placeholder,
      secureTextEntry: view.secure === true,
      value: view.value
    }
  )
}

const estimatedItemLength = (
  view: ListView | SectionListView,
  options: ReactNativeRenderOptions
): number | undefined => {
  if (view.estimatedItemSize === undefined) {
    return undefined
  }
  const value = dimensionValue(options.theme ?? defaultTheme, view.estimatedItemSize)
  return typeof value === "number" ? value : undefined
}

const nativeCollectionProps = (
  view: ListView | SectionListView,
  report: IntentReporter,
  options: ReactNativeRenderOptions,
  dependencies: ReactNativeDependencies
): Record<string, unknown> => {
  const itemLength = estimatedItemLength(view, options)
  const refreshControl =
    view.onRefresh === undefined
      ? undefined
      : dependencies.ReactNative.RefreshControl === undefined
        ? undefined
        : createElement(dependencies, dependencies.ReactNative.RefreshControl, {
            refreshing: view.refreshing === true,
            tintColor: colorValue(options.theme ?? defaultTheme, "accent"),
            onRefresh: () => runReportedIntent(report, view.onRefresh!)
          })
  // Production-scale virtualization defaults (#57): windowing + end-reach wiring.
  // FlatList always owns the data path (never eager-map all children).
  return {
    windowSize: 10,
    initialNumToRender: 12,
    maxToRenderPerBatch: 10,
    updateCellsBatchingPeriod: 50,
    removeClippedSubviews: true,
    ...(view.onEndReached === undefined
      ? {}
      : {
          onEndReached: () => runReportedIntent(report, view.onEndReached!),
          onEndReachedThreshold: view.endReachedThreshold ?? 0.5
        }),
    ...(view.onRefresh === undefined
      ? {}
      : {
          refreshing: view.refreshing === true,
          onRefresh: () => runReportedIntent(report, view.onRefresh!),
          ...(refreshControl === undefined ? {} : { refreshControl })
        }),
    ...(view.virtualize === true && itemLength !== undefined
      ? {
          getItemLayout: (_data: unknown, index: number) => ({
            length: itemLength,
            offset: itemLength * index,
            index
          })
        }
      : {})
  }
}

const renderList = (
  view: ListView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  return createElement(
    dependencies,
    dependencies.ReactNative.FlatList,
    {
      ...baseProps(view, viewStyle(view, options)),
      data: view.items,
      keyExtractor: (item: View & { readonly key: string }) => item.key,
      renderItem: ({ item }: { readonly item: View }) =>
        renderResolvedReactNativeView(item, dependencies, report, options),
      ...nativeCollectionProps(view, report, options, dependencies)
    }
  )
}

const renderSectionList = (
  view: SectionListView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const sections = view.sections.map((section) => ({
    key: section.key,
    data: section.items,
    header: section.header
  }))

  return createElement(
    dependencies,
    dependencies.ReactNative.SectionList,
    {
      ...baseProps(view, viewStyle(view, options)),
      sections,
      keyExtractor: (item: View & { readonly key: string }) => item.key,
      renderItem: ({ item }: { readonly item: View }) =>
        renderResolvedReactNativeView(item, dependencies, report, options),
      renderSectionHeader: ({ section }: { readonly section: { readonly header: View } }) =>
        renderResolvedReactNativeView(section.header, dependencies, report, options),
      stickySectionHeadersEnabled: view.stickyHeaders === true,
      ...nativeCollectionProps(view, report, options, dependencies)
    }
  )
}

const renderCard = (
  view: CardView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const style = mergeNativeStyles(
    {
      ...(view.padding === undefined ? {} : { padding: spacingValue(options.theme ?? defaultTheme, view.padding) }),
      ...(view.radius === undefined ? {} : { borderRadius: radiusValue(options.theme ?? defaultTheme, view.radius) })
    },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(view, style),
    ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
  )
}

const renderSpacer = (
  view: SpacerView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const style = mergeNativeStyles(
    view.flex === true
      ? { flex: 1 }
      : {
          width: spacingValue(options.theme ?? defaultTheme, view.size),
          height: spacingValue(options.theme ?? defaultTheme, view.size)
        },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      accessibilityElementsHidden: true,
      importantForAccessibility: "no-hide-descendants"
    }
  )
}

// Foreign-host escape hatch on React Native (issue #23/#58/#70). A registered
// host driver is consulted FIRST: it Schema-decodes the props (malformed props
// fail closed to a loud error marker), mounts through the Scope-bound host
// runtime, and maps native events to the Host node's typed `onEvent` intent.
// Without a driver, desktop host kinds (code-editor/terminal/canvas) remain
// loud unsupported markers and mobile kinds voice-input / on-device-model ship
// the minimal structural surface so apps can swap in real native modules under
// the same Host contract.
const renderHost = (
  view: HostView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const driver = options.hostRuntime?.resolve(view.kind) ??
    options.hostDrivers?.find((candidate) => candidate.kind === view.kind)
  if (driver !== undefined) {
    let decoded: unknown
    try {
      decoded = driver.decodeProps(view.props)
    } catch (error) {
      // Fail closed and loud: malformed host props render an error marker that
      // fails the conformance suite, never a silently-empty native mount.
      return createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          ...baseProps(view, viewStyle(view, options)),
          testID: `en-host-error:${view.kind}`,
          accessibilityLabel: `Invalid ${view.kind} host props: ${String(error)}`
        }
      )
    }
    const embedded = options.hostRuntime !== undefined
      ? options.hostRuntime.render(view, driver, decoded, dependencies, report)
      : (() => {
        // No Scope-bound runtime (bare renderReactNativeView call): mount a
        // transient per-emission instance. Unit-test posture only — the
        // surface entrypoints always provide the retained runtime.
        const context: ReactNativeHostContext = {
          dependencies,
          report,
          emit: (payload) => {
            if (view.onEvent !== undefined) {
              runReportedIntent(report, view.onEvent, payload)
            }
          }
        }
        return driver.mount(decoded, context).render(decoded)
      })()
    return createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        ...baseProps(view, viewStyle(view, options)),
        testID: `en-host:${view.kind}`
      },
      embedded
    )
  }
  if (view.kind === "voice-input" || view.kind === "on-device-model") {
    const props = typeof view.props === "object" && view.props !== null && !Array.isArray(view.props)
      ? view.props as Record<string, unknown>
      : {}
    const status =
      view.kind === "voice-input"
        ? props.listening === true
          ? "listening"
          : "idle"
        : typeof props.status === "string"
          ? props.status
          : "idle"
    return createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        ...baseProps(view, mergeNativeStyles({
          padding: spacingValue(theme, "3"),
          borderWidth: 1,
          borderColor: colorValue(theme, "border"),
          backgroundColor: colorValue(theme, "surface")
        }, viewStyle(view, options))),
        testID: `en-host:${view.kind}`,
        accessibilityLabel: `${view.kind} host (${status})`,
        accessibilityRole: "none"
      },
      createElement(
        dependencies,
        dependencies.ReactNative.Text,
        { key: "kind" },
        `${view.kind}: ${status}`
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-host-unsupported:${view.kind}`,
      accessibilityLabel: `Unsupported host kind on React Native: ${view.kind}`
    }
  )
}

// Icon on React Native (issue #31). The closed IconName set is the contract;
// RN renders each glyph from a bounded font-glyph registry (sized from tokens,
// token-driven color). Decorative vs meaningful is honored via accessibility
// props. No raw SVG/markup enters the tree.
const iconGlyphs: Record<IconName, string> = {
  Home: "⌂",
  Agent: "◌",
  ChatCompose: "✎",
  Chats: "☷",
  Code: "‹›",
  Compare: "⇄",
  Folder: "□",
  NotificationBell: "♢",
  Plane: "➤",
  ArrowUp: "↑",
  Settings: "⚙",
  Terminal: ">_",
  Tools: "⌘",
  Plus: "+",
  Play: "▶",
  Pause: "❚❚",
  Stop: "■",
  Reload: "↻",
  Circle: "○",
  Check: "✓",
  X: "✕",
  ChevronUp: "⌃",
  ChevronDown: "⌄",
  ChevronLeft: "‹",
  ChevronRight: "›",
  // Glass-chrome icons (v30, GL-1 openagents#8647) — honest text glyphs for
  // the RN-core path; the @expo/ui SwiftUI lowering renders real SF Symbols.
  Menu: "≡",
  Compose: "✎",
  Mic: "🎤",
  Sparkles: "✦",
  History: "↶",
  Branch: "⑂",
  InfoCircle: "ⓘ"
}

const iconFontSize: Record<IconSize, number> = { sm: 16, md: 20, lg: 24 }

const renderIcon = (
  view: IconView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const style = mergeNativeStyles(
    {
      fontSize: iconFontSize[view.size ?? "md"],
      ...(view.color === undefined ? {} : { color: colorValue(options.theme ?? defaultTheme, view.color) })
    },
    viewStyle(view, options)
  )
  return createElement(
    dependencies,
    dependencies.ReactNative.Text,
    {
      ...baseProps(view, style),
      testID: `en-icon:${view.name}`,
      accessibilityRole: "image",
      ...(view.label === undefined
        ? { accessibilityElementsHidden: true, importantForAccessibility: "no-hide-descendants" }
        : { accessibilityLabel: view.label })
    },
    iconGlyphs[view.name]
  )
}

// Data-display components (issue #39) on React Native.
const toneColorToken: Record<Tone, ColorToken> = {
  neutral: "textMuted",
  info: "info",
  success: "success",
  warn: "warning",
  danger: "danger"
}

const rnTextAlign = (align: "start" | "center" | "end" | undefined): "left" | "center" | "right" =>
  align === "center" ? "center" : align === "end" ? "right" : "left"

const renderDivider = (
  view: DividerView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const orientation = view.orientation ?? "horizontal"
  // Faithful hairline (#53): 1px border-token rule; style contract owns inset.
  const style = mergeNativeStyles(
    orientation === "vertical"
      ? { width: 1, alignSelf: "stretch", backgroundColor: colorValue(theme, "border") }
      : { height: 1, alignSelf: "stretch", backgroundColor: colorValue(theme, "border") },
    viewStyle(view, options)
  )
  return createElement(dependencies, dependencies.ReactNative.View, {
    ...baseProps(view, style),
    testID: `en-divider:${orientation}`,
    accessibilityRole: "none"
  })
}

const renderBadge = (
  view: BadgeView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const tone = view.tone ?? "neutral"
  const style = mergeNativeStyles({ color: colorValue(theme, toneColorToken[tone]) }, viewStyle(view, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.Text,
    { ...baseProps(view, style), testID: `en-badge:${tone}` },
    view.label
  )
}

const renderChip = (
  view: ChipView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const tone = view.tone ?? "neutral"
  const style = mergeNativeStyles({ flexDirection: "row", gap: spacingValue(theme, "1") }, viewStyle(view, options))
  const parts: Array<ReactElementLike> = [
    createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, view.label)
  ]
  if (view.value !== undefined) {
    parts.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Text,
        { key: "value", style: { color: colorValue(theme, toneColorToken[tone]) } },
        view.value
      )
    )
  }
  return createElement(dependencies, dependencies.ReactNative.View, baseProps(view, style), ...parts)
}

const renderMeter = (
  view: MeterView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const tone = view.tone ?? "info"
  const indeterminate = view.indeterminate === true
  const value = view.value ?? 0
  const bar = createElement(dependencies, dependencies.ReactNative.View, {
    key: "bar",
    testID: "en-meter-bar",
    style: {
      height: "100%",
      width: indeterminate ? "100%" : `${Math.round(value * 100)}%`,
      backgroundColor: colorValue(theme, toneColorToken[tone])
    }
  })
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, viewStyle(view, options)),
      accessibilityRole: "progressbar",
      ...(view.label === undefined ? {} : { accessibilityLabel: view.label }),
      ...(indeterminate
        ? { "aria-busy": true }
        : { accessibilityValue: { min: 0, max: 1, now: value } })
    },
    bar
  )
}

const renderStatTile = (
  view: StatTileView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const tone = view.tone ?? "neutral"
  const style = mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(view, style),
    createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, view.label),
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      { key: "value", style: { color: colorValue(theme, toneColorToken[tone]) } },
      view.value
    )
  )
}

const renderTable = (
  view: TableView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const headerCells = view.columns.map((column) =>
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      { key: `col-${column.id}`, style: { flex: 1, textAlign: rnTextAlign(column.align) } },
      column.header
    ))
  const headerRow = createElement(
    dependencies,
    dependencies.ReactNative.View,
    { key: "header", style: { flexDirection: "row" } },
    ...headerCells
  )
  const bodyRows = view.rows.map((row) => {
    const cells = row.cells.map((cell, index) =>
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key: `cell-${index}`, style: { flex: 1, alignItems: rnAlignItems(view.columns[index]?.align) } },
        renderResolvedReactNativeView(cell, dependencies, report, options)
      ))
    const rowProps: Record<string, unknown> = { key: `row-${row.id}`, style: { flexDirection: "row" } }
    if (view.onRowSelect !== undefined) {
      const onRowSelect = view.onRowSelect
      return createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        { ...rowProps, onPress: () => runReportedIntent(report, onRowSelect, row.id) },
        ...cells
      )
    }
    return createElement(dependencies, dependencies.ReactNative.View, rowProps, ...cells)
  })
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(view, viewStyle(view, options)),
    headerRow,
    ...bodyRows
  )
}

const rnAlignItems = (align: "start" | "center" | "end" | undefined): "flex-start" | "center" | "flex-end" =>
  align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start"

// App shell components (issue #27) on React Native. Divider resize (#53) steps
// pane size via pressable +/- on the divider (fires typed onResize); continuous
// drag remains optional when gesture-handler is host-injected. NavRail maps to a
// stacked selectable list; Workbench renders the active pane (keepMounted ok).
const renderSplitPane = (
  view: SplitPaneView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const sizeField = view.orientation === "row" ? "width" : "height"
  const style = mergeNativeStyles(
    { flexDirection: view.orientation, flex: 1 },
    viewStyle(view, options)
  )
  const children: Array<ReactElementLike> = []
  view.panes.forEach((pane, index) => {
    const paneStyle: ReactNativeStyle = pane.collapsed === true
      ? { [sizeField]: 0, overflow: "hidden" }
      : pane.size === undefined
        ? { flex: 1 }
        : { [sizeField]: dimensionValue(theme, pane.size) }
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key: `pane-${pane.id}`, nativeID: `effect-native-pane:${pane.id}`, style: paneStyle },
        renderResolvedReactNativeView(pane.content, dependencies, report, options)
      )
    )
    if (index < view.panes.length - 1) {
      const left = pane
      const currentSize = typeof left.size === "number" ? left.size : 200
      const step = 24
      const min = typeof left.min === "number" ? left.min : 80
      const max = typeof left.max === "number" ? left.max : 480
      const clamp = (n: number) => Math.min(max, Math.max(min, n))
      children.push(
        createElement(
          dependencies,
          dependencies.ReactNative.View,
          {
            key: `divider-${index}`,
            testID: "en-split-divider",
            accessibilityRole: "adjustable",
            accessibilityLabel: `Resize ${left.id}`,
            style: {
              [sizeField]: 12,
              backgroundColor: colorValue(theme, "border"),
              flexDirection: view.orientation === "row" ? "column" : "row",
              alignItems: "center",
              justifyContent: "center",
              gap: 2
            }
          },
          createElement(
            dependencies,
            dependencies.ReactNative.Pressable,
            {
              key: "grow",
              testID: `en-split-resize-grow:${left.id}`,
              ...(view.onResize === undefined
                ? {}
                : {
                    onPress: () =>
                      runReportedIntent(report, view.onResize!, {
                        paneId: left.id,
                        size: clamp(currentSize + step)
                      })
                  })
            },
            createElement(dependencies, dependencies.ReactNative.Text, { key: "g" }, "+")
          ),
          createElement(
            dependencies,
            dependencies.ReactNative.Pressable,
            {
              key: "shrink",
              testID: `en-split-resize-shrink:${left.id}`,
              ...(view.onResize === undefined
                ? {}
                : {
                    onPress: () =>
                      runReportedIntent(report, view.onResize!, {
                        paneId: left.id,
                        size: clamp(currentSize - step)
                      })
                  })
            },
            createElement(dependencies, dependencies.ReactNative.Text, { key: "s" }, "−")
          )
        )
      )
    }
  })
  return createElement(dependencies, dependencies.ReactNative.View, baseProps(view, style), ...children)
}

const renderNavRail = (
  view: NavRailView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const style = mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))
  const sections = view.sections.map((section) => {
    const parts: Array<ReactElementLike> = []
    if (section.label !== undefined) {
      parts.push(
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, section.label)
      )
    }
    for (const item of section.items) {
      const active = item.selected ?? view.activeId === item.id
      const itemChildren: Array<ReactElementLike> = []
      if (item.icon !== undefined) {
        itemChildren.push(
          createElement(dependencies, dependencies.ReactNative.Text, { key: "icon" }, iconGlyphs[item.icon])
        )
      }
      itemChildren.push(
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, item.label)
      )
      if (item.badge !== undefined) {
        itemChildren.push(
          createElement(dependencies, dependencies.ReactNative.Text, { key: "badge" }, item.badge)
        )
      }
      if (item.meta !== undefined) {
        itemChildren.push(
          createElement(dependencies, dependencies.ReactNative.Text, { key: "meta", style: { marginLeft: "auto" } }, item.meta)
        )
      }
      const onSelect = item.onSelect ?? view.onSelect
      parts.push(
        createElement(
          dependencies,
          dependencies.ReactNative.Pressable,
          {
            key: `item-${item.id}`,
            testID: `en-nav-item:${item.id}`,
            accessibilityRole: view.role === "tree" ? "button" : "menuitem",
            accessibilityLabel: item.accessibilityLabel ?? item.label,
            accessibilityState: { selected: active, disabled: item.disabled === true, ...(item.expanded === undefined ? {} : { expanded: item.expanded }) },
            disabled: item.disabled === true,
            style: { flexDirection: "row", gap: spacingValue(theme, "2"), paddingLeft: spacingValue(theme, "2") + (item.depth ?? 0) * 12 },
            ...(item.disabled === true || onSelect === undefined
              ? {}
              : { onPress: () => runReportedIntent(report, onSelect, item.id) })
          },
          ...itemChildren
        )
      )
    }
    return createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: `section-${section.id}`, style: { flexDirection: section.layout ?? "column" } },
      ...parts
    )
  })
  return createElement(dependencies, dependencies.ReactNative.View, baseProps(view, style), ...sections)
}

const renderWorkbench = (
  view: WorkbenchView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const style = mergeNativeStyles({ flex: 1 }, viewStyle(view, options))
  const keepMounted = view.keepMounted === true
  const panes = keepMounted ? view.panes : view.panes.filter((pane) => pane.id === view.activePaneId)
  const children = panes.map((pane) => {
    const active = pane.id === view.activePaneId
    return createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        key: `pane-${pane.id}`,
        nativeID: `effect-native-pane:${pane.id}`,
        style: { flex: 1, display: active ? "flex" : "none" }
      },
      renderResolvedReactNativeView(pane.content, dependencies, report, options)
    )
  })
  return createElement(dependencies, dependencies.ReactNative.View, baseProps(view, style), ...children)
}

// Anchored overlay family (issue #28/#53) on React Native. Open surfaces use
// RN Modal (back-button dismiss) with placement encoded in testID/a11y. Menus
// are pressable rows with typed onSelect; ContextMenu includes pointer origin.
// Tooltip maps content to accessibilityHint + optional label bubble when open.
const renderMenuRows = (
  items: ReadonlyArray<MenuItem>,
  depth: number,
  dependencies: ReactNativeDependencies,
  theme: Theme,
  onSelect: IntentRef,
  onDismiss: IntentRef | undefined,
  report: IntentReporter
): ReadonlyArray<ReactElementLike> =>
  items.flatMap((item) => {
    // #71-class bug: RN Text does not inherit color — menu rows must theme
    // their glyph/label/keybinding or they render default-black on the dark
    // theme surface panel.
    const rowColor = colorValue(theme, item.danger === true ? "danger" : "textPrimary")
    const parts: Array<ReactElementLike> = []
    if (item.icon !== undefined) {
      parts.push(
        createElement(dependencies, dependencies.ReactNative.Text, { key: "icon", style: { color: rowColor } }, iconGlyphs[item.icon])
      )
    }
    parts.push(
      createElement(dependencies, dependencies.ReactNative.Text, { key: "label", style: { color: rowColor } }, item.label)
    )
    if (item.keybinding !== undefined) {
      parts.push(
        createElement(
          dependencies,
          dependencies.ReactNative.Text,
          { key: "kbd", style: { color: colorValue(theme, "textMuted") } },
          item.keybinding
        )
      )
    }
    const row = createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: `item-${item.id}`,
        testID: `en-menu-item:${item.id}`,
        accessibilityRole: "menuitem",
        accessibilityState: { disabled: item.disabled === true },
        disabled: item.disabled === true,
        style: {
          flexDirection: "row",
          gap: spacingValue(theme, "2"),
          paddingVertical: spacingValue(theme, "2"),
          paddingLeft: spacingValue(theme, "2") * (depth + 1),
          opacity: item.disabled === true ? 0.5 : 1
        },
        ...(item.disabled === true
          ? {}
          : {
              onPress: () => {
                runReportedIntent(report, onSelect, item.id)
                if (onDismiss !== undefined) runReportedIntent(report, onDismiss)
              }
            })
      },
      ...parts
    )
    if (item.items === undefined || item.items.length === 0) return [row]
    return [row, ...renderMenuRows(item.items, depth + 1, dependencies, theme, onSelect, onDismiss, report)]
  })

const renderPopover = (
  view: PopoverView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const open = view.open === true
  const theme = options.theme ?? defaultTheme
  if (!open) {
    return createElement(dependencies, dependencies.ReactNative.View, {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-popover:${view.placement.side}:${view.placement.align}`,
      accessibilityRole: "none"
    })
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.Modal,
    {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-popover:${view.placement.side}:${view.placement.align}`,
      transparent: true,
      visible: true,
      accessibilityViewIsModal: true,
      accessibilityLabel: `Popover ${view.placement.side} ${view.placement.align}`,
      onRequestClose: () => {
        if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: "backdrop",
        testID: "en-popover-backdrop",
        onPress: () => {
          if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
        },
        style: {
          flex: 1,
          justifyContent:
            view.placement.side === "top"
              ? "flex-start"
              : view.placement.side === "bottom"
                ? "flex-end"
                : "center",
          alignItems:
            view.placement.align === "start"
              ? "flex-start"
              : view.placement.align === "end"
                ? "flex-end"
                : "center",
          backgroundColor: "rgba(0,0,0,0.35)",
          padding: spacingValue(theme, "4")
        }
      },
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          key: "panel",
          testID: "en-popover-panel",
          style: { backgroundColor: colorValue(theme, "surface"), padding: spacingValue(theme, "3") }
        },
        ...view.children.map((child) =>
          renderResolvedReactNativeView(child, dependencies, report, options)
        )
      )
    )
  )
}

const renderDropdownMenu = (
  view: DropdownMenuView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const open = view.open === true
  if (!open) {
    return createElement(dependencies, dependencies.ReactNative.View, {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-dropdown-menu:${view.placement.side}:${view.placement.align}`,
      accessibilityRole: "menu"
    })
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.Modal,
    {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-dropdown-menu:${view.placement.side}:${view.placement.align}`,
      transparent: true,
      visible: true,
      accessibilityViewIsModal: true,
      accessibilityRole: "menu",
      onRequestClose: () => {
        if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: "backdrop",
        testID: "en-dropdown-backdrop",
        onPress: () => {
          if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
        },
        style: {
          flex: 1,
          justifyContent: "center",
          backgroundColor: "rgba(0,0,0,0.35)",
          padding: spacingValue(theme, "4")
        }
      },
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          key: "menu",
          testID: "en-dropdown-panel",
          style: {
            backgroundColor: colorValue(theme, "surface"),
            flexDirection: "column",
            padding: spacingValue(theme, "2")
          }
        },
        ...renderMenuRows(view.items, 0, dependencies, theme, view.onSelect, view.onDismiss, report)
      )
    )
  )
}

const renderContextMenu = (
  view: ContextMenuView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const open = view.open === true
  if (!open) {
    return createElement(dependencies, dependencies.ReactNative.View, {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-context-menu:${view.x}:${view.y}`,
      accessibilityRole: "menu"
    })
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.Modal,
    {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-context-menu:${view.x}:${view.y}`,
      transparent: true,
      visible: true,
      accessibilityViewIsModal: true,
      accessibilityRole: "menu",
      accessibilityLabel: `Context menu at ${view.x},${view.y}`,
      onRequestClose: () => {
        if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: "backdrop",
        testID: "en-context-backdrop",
        onPress: () => {
          if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
        },
        style: { flex: 1, backgroundColor: "rgba(0,0,0,0.35)" }
      },
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          key: "menu",
          testID: "en-context-panel",
          style: {
            position: "absolute",
            left: view.x,
            top: view.y,
            backgroundColor: colorValue(theme, "surface"),
            flexDirection: "column",
            padding: spacingValue(theme, "2")
          }
        },
        ...renderMenuRows(view.items, 0, dependencies, theme, view.onSelect, view.onDismiss, report)
      )
    )
  )
}

const renderTooltip = (
  view: TooltipView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const target = renderResolvedReactNativeView(view.children[0]!, dependencies, report, options)
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))),
      testID: "en-tooltip",
      accessibilityHint: view.content,
      accessibilityLabel: view.content
    },
    target,
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      {
        key: "tip",
        testID: "en-tooltip-content",
        style: { color: colorValue(theme, "textMuted"), fontSize: 12 }
      },
      view.content
    )
  )
}

// Command palette + Combobox (issue #29/#53) on React Native. Filtering stays
// app-supplied. FlatList listbox + selected option a11y; highlight via
// onPressIn → onHighlight. CommandPalette mounts in RN Modal when open.
const renderComboboxOption = (
  option: ComboboxOption,
  view: ComboboxView,
  dependencies: ReactNativeDependencies,
  theme: Theme,
  report: IntentReporter
): ReactElementLike => {
  const parts: Array<ReactElementLike> = []
  if (option.icon !== undefined) {
    parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "icon" }, iconGlyphs[option.icon]))
  }
  parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, option.label))
  if (option.subtitle !== undefined) {
    parts.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Text,
        { key: "subtitle", style: { color: colorValue(theme, "textMuted") } },
        option.subtitle
      )
    )
  }
  if (option.keybinding !== undefined) {
    parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "kbd" }, option.keybinding))
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      key: `option-${option.id}`,
      testID: `en-combobox-option:${option.id}`,
      accessibilityRole: "button",
      accessibilityState: { selected: view.highlightedId === option.id, disabled: option.disabled === true },
      disabled: option.disabled === true,
      style: { flexDirection: "row", gap: spacingValue(theme, "2") },
      ...(option.disabled === true
        ? {}
        : {
            onPressIn: () => {
              if (view.onHighlight !== undefined) runReportedIntent(report, view.onHighlight, option.id)
            },
            onPress: () => runReportedIntent(report, view.onSelect, option.id)
          })
    },
    ...parts
  )
}

const renderCombobox = (
  view: ComboboxView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const input = createElement(dependencies, dependencies.ReactNative.TextInput, {
    key: "control",
    testID: "en-combobox-input",
    accessibilityRole: "search",
    accessibilityLabel: view.placeholder ?? "Search",
    placeholder: view.placeholder,
    value: view.query,
    ...(view.onQueryChange === undefined
      ? {}
      : { onChangeText: (value: string) => runReportedIntent(report, view.onQueryChange!, value) }),
    onSubmitEditing: () => {
      if (view.highlightedId !== undefined) {
        runReportedIntent(report, view.onSelect, view.highlightedId)
      }
    }
  })
  const listbox =
    view.options.length === 0
      ? createElement(
          dependencies,
          dependencies.ReactNative.Text,
          { key: "empty", testID: "en-combobox-empty", accessibilityRole: "text" },
          view.loading === true ? "Loading…" : (view.emptyLabel ?? "No results")
        )
      : createElement(dependencies, dependencies.ReactNative.FlatList, {
          key: "listbox",
          testID: "en-combobox-listbox",
          accessibilityRole: "list",
          data: view.options,
          keyExtractor: (option: ComboboxOption) => option.id,
          keyboardShouldPersistTaps: "handled",
          initialNumToRender: 12,
          windowSize: 8,
          renderItem: ({ item: option }: { readonly item: ComboboxOption }) =>
            renderComboboxOption(option, view, dependencies, theme, report)
        })
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))),
      accessibilityRole: "none",
      testID: "en-combobox"
    },
    input,
    listbox
  )
}

const renderCommandPalette = (
  view: CommandPaletteView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const open = view.open === true
  const theme = options.theme ?? defaultTheme
  if (!open) {
    return createElement(dependencies, dependencies.ReactNative.View, {
      ...baseProps(view, {}),
      testID: "en-command-palette",
      accessibilityRole: "none"
    })
  }
  const children: Array<ReactElementLike> = []
  if (view.title !== undefined) {
    children.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "title" }, view.title))
  }
  children.push(renderCombobox(view.combobox, dependencies, report, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.Modal,
    {
      ...baseProps(view, {}),
      testID: "en-command-palette",
      transparent: true,
      visible: true,
      accessibilityViewIsModal: true,
      accessibilityRole: "none",
      onRequestClose: () => {
        if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: "backdrop",
        testID: "en-command-palette-backdrop",
        onPress: () => {
          if (view.onDismiss !== undefined) runReportedIntent(report, view.onDismiss)
        },
        style: {
          flex: 1,
          justifyContent: "flex-start",
          backgroundColor: "rgba(0,0,0,0.45)",
          padding: spacingValue(theme, "6")
        }
      },
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          key: "panel",
          testID: "en-command-palette-panel",
          style: {
            backgroundColor: colorValue(theme, "surface"),
            flexDirection: "column",
            padding: spacingValue(theme, "3"),
            gap: spacingValue(theme, "2")
          }
        },
        ...children
      )
    )
  )
}

// Tabs (issue #30/#53) — segmented tab bar + active panel; swipe-between via
// accessibility actions prev/next (touch selection remains primary).
const renderTabs = (
  view: TabsView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const orientation = view.orientation ?? "horizontal"
  const enabledIds = view.tabs.filter((tab) => tab.disabled !== true).map((tab) => tab.id)
  const move = (direction: 1 | -1) => {
    if (enabledIds.length === 0) return
    const index = enabledIds.indexOf(view.selectedId)
    const next = enabledIds[(index + direction + enabledIds.length) % enabledIds.length]!
    runReportedIntent(report, view.onSelect, next)
  }
  const tabBar = createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      key: "tablist",
      accessibilityRole: "tablist",
      accessibilityActions: [{ name: "increment" }, { name: "decrement" }],
      onAccessibilityAction: (event: { readonly nativeEvent: { readonly actionName: string } }) => {
        if (event.nativeEvent.actionName === "increment") move(1)
        if (event.nativeEvent.actionName === "decrement") move(-1)
      },
      style: { flexDirection: orientation === "vertical" ? "column" : "row", gap: spacingValue(theme, "1") }
    },
    ...view.tabs.map((tab) => {
      const selected = view.selectedId === tab.id
      const parts: Array<ReactElementLike> = []
      if (tab.icon !== undefined) {
        parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "icon" }, iconGlyphs[tab.icon]))
      }
      parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, tab.label))
      if (tab.badge !== undefined) {
        parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "badge" }, tab.badge))
      }
      return createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        {
          key: `tab-${tab.id}`,
          testID: `en-tab:${tab.id}`,
          accessibilityRole: "tab",
          accessibilityState: { selected, disabled: tab.disabled === true },
          disabled: tab.disabled === true,
          style: { flexDirection: "row", gap: spacingValue(theme, "1") },
          ...(tab.disabled === true ? {} : { onPress: () => runReportedIntent(report, view.onSelect, tab.id) })
        },
        ...parts
      )
    })
  )
  const panels = (view.keepMounted === true
    ? view.panels
    : view.panels.filter((panel) => panel.id === view.selectedId)
  ).map((panel) => {
    const active = panel.id === view.selectedId
    return createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: `panel-${panel.id}`, testID: `en-tabpanel:${panel.id}`, accessibilityRole: "tabpanel", style: { display: active ? "flex" : "none" } },
      renderResolvedReactNativeView(panel.content, dependencies, report, options)
    )
  })
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(view, mergeNativeStyles({ flexDirection: orientation === "vertical" ? "row" : "column" }, viewStyle(view, options))),
    tabBar,
    ...panels
  )
}

// Rich composer (issue #32) on React Native — a multiline TextInput bound to
// the same typed document (flattened to plaintext via composerPlainText; inline
// mention chips render as a typed chip strip above the flattened TextInput (#53 parity).
// Enter submit-vs-newline and history nav map to onSubmitEditing / typed key
// commands; the autocomplete combobox renders below.
const renderComposer = (
  view: ComposerView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  if (resolvedFlatStyle(view, options)?.surface === "glass") {
    const expoUi = glassLoweringRuntime(dependencies, options)
    if (
      expoUi?.TextField !== undefined &&
      expoUi.useNativeState !== undefined &&
      dependencies.React.useEffect !== undefined
    ) {
      return renderExpoUiComposer(view, expoUi, dependencies, report, options)
    }
  }
  const input = createElement(dependencies, dependencies.ReactNative.TextInput, {
    key: "control",
    testID: "en-composer-input",
    accessibilityLabel: view.placeholder,
    multiline: true,
    returnKeyType: "send",
    blurOnSubmit: true,
    submitBehavior: "blurAndSubmit",
    placeholder: view.placeholder,
    placeholderTextColor: colorValue(theme, "textMuted"),
    value: composerPlainText(view.doc),
    // v29 (#72): disabled composers accept no input; submitting keeps typing
    // live but suppresses onSubmit dispatch (follow-up drafting); clear-on-
    // submit rides the controlled value — RN always honors app resets.
    editable: view.disabled !== true,
    accessibilityState: {
      disabled: view.disabled === true,
      busy: view.submitting === true
    },
    style: {
      flex: 1,
      minHeight: 44,
      color: colorValue(theme, "textPrimary")
    },
    ...(view.onChange === undefined ? {} : {
      onChangeText: (value: string) => {
        if (view.disabled === true) return
        runReportedIntent(report, view.onChange!, value)
      }
    }),
    onSubmitEditing: (event: { readonly nativeEvent?: { readonly text?: string } }) => {
      if (view.disabled === true) return
      if (view.onKeyCommand !== undefined) runReportedIntent(report, view.onKeyCommand, "submit")
      if (view.submitting === true) return
      if (view.onSubmit !== undefined) runReportedIntent(report, view.onSubmit, event.nativeEvent?.text ?? composerPlainText(view.doc))
    }
  })
  const submitDisabled = view.disabled === true || view.submitting === true ||
    view.onSubmit === undefined
  const submit = createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      key: "submit",
      testID: "en-composer-submit",
      accessibilityRole: "button",
      accessibilityLabel: view.onSubmit === undefined
        ? "Send unavailable"
        : view.submitting === true ? "Message is sending" : "Send message",
      accessibilityState: { disabled: submitDisabled, busy: view.submitting === true },
      disabled: submitDisabled,
      style: {
        width: 44,
        height: 44,
        alignItems: "center",
        justifyContent: "center"
      },
      ...(submitDisabled || view.onSubmit === undefined ? {} : {
        onPress: () => runReportedIntent(report, view.onSubmit!, composerPlainText(view.doc))
      })
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      { style: { color: colorValue(theme, submitDisabled ? "textMuted" : "accent") } },
      view.submitting === true ? "…" : "↑"
    )
  )
  const children: Array<ReactElementLike> = [
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: "input-row", style: { flexDirection: "row", alignItems: "center" } },
      input,
      submit
    )
  ]
  const mentionChips = view.doc.filter((run): run is { readonly kind: "mention"; readonly id: string; readonly label: string } => run.kind === "mention")
  if (mentionChips.length > 0) {
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key: "mentions", testID: "en-composer-mentions", style: { flexDirection: "row", flexWrap: "wrap", gap: spacingValue(theme, "1") } },
        ...mentionChips.map((chip) =>
          createElement(
            dependencies,
            dependencies.ReactNative.Text,
            { key: `mention-${chip.id}`, testID: `en-composer-mention:${chip.id}`, style: { color: colorValue(theme, "accent") } },
            chip.label
          )
        )
      )
    )
  }
  if (view.attachments !== undefined && view.attachments.length > 0) {
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key: "attachments", testID: "en-composer-attachments", style: { flexDirection: "row", gap: spacingValue(theme, "1") } },
        ...view.attachments.map((attachment) =>
          createElement(
            dependencies,
            dependencies.ReactNative.Text,
            { key: `attachment-${attachment.id}`, testID: `en-composer-attachment:${attachment.id}` },
            attachment.name
          ))
      )
    )
  }
  if (view.autocomplete !== undefined) {
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key: "autocomplete", testID: `en-composer-autocomplete:${view.autocomplete.trigger}` },
        renderCombobox(view.autocomplete.combobox, dependencies, report, options)
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), testID: `en-composer:${view.mode}` },
    ...children
  )
}

// Settings form controls (issue #38) on React Native. Each emits a typed
// onChange (or a #12 FormFieldChanged intent when `field` is bound), mapped to
// native switch/picker/checkbox/radio/slider/number equivalents.
const controlChangeIntent = (view: {
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
}): IntentRef | undefined =>
  view.field !== undefined ? IntentRef("FormFieldChanged", FormFieldValueBinding(view.field)) : view.onChange

const renderToggle = (
  view: ToggleView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const onChange = controlChangeIntent(view)
  return createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      ...baseProps(view, viewStyle(view, options)),
      testID: "en-toggle",
      accessibilityRole: "switch",
      accessibilityState: { checked: view.value, disabled: view.disabled === true },
      disabled: view.disabled === true,
      ...(onChange === undefined || view.disabled === true
        ? {}
        : { onPress: () => runReportedIntent(report, onChange, !view.value) })
    },
    createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, view.label ?? (view.value ? "On" : "Off"))
  )
}

const renderSelect = (
  view: SelectView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const onChange = controlChangeIntent(view)
  // No native <select> in RN; render selectable rows (picker-style).
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), testID: "en-select", accessibilityLabel: view.label },
    ...view.options.map((option) =>
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        {
          key: `option-${option.value}`,
          testID: `en-select-option:${option.value}`,
          accessibilityRole: "menuitem",
          accessibilityState: { selected: view.value === option.value, disabled: view.disabled === true || option.disabled === true },
          disabled: view.disabled === true || option.disabled === true,
          ...(onChange === undefined || view.disabled === true || option.disabled === true
            ? {}
            : { onPress: () => runReportedIntent(report, onChange, option.value) })
        },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, option.label)
      ))
  )
}

const renderCheckbox = (
  view: CheckboxView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const onChange = controlChangeIntent(view)
  return createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "row" }, viewStyle(view, options))),
      testID: "en-checkbox",
      accessibilityRole: "checkbox",
      accessibilityState: { checked: view.checked, disabled: view.disabled === true },
      disabled: view.disabled === true,
      ...(onChange === undefined || view.disabled === true
        ? {}
        : { onPress: () => runReportedIntent(report, onChange, !view.checked) })
    },
    createElement(dependencies, dependencies.ReactNative.Text, { key: "box" }, view.checked ? "☑" : "☐"),
    ...(view.label === undefined ? [] : [createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, view.label)])
  )
}

const renderRadioGroup = (
  view: RadioGroupView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const onChange = controlChangeIntent(view)
  const orientation = view.orientation ?? "vertical"
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: orientation === "horizontal" ? "row" : "column" }, viewStyle(view, options))), testID: "en-radio-group", accessibilityRole: "radiogroup", accessibilityLabel: view.label },
    ...view.options.map((option) =>
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        {
          key: `radio-${option.value}`,
          testID: `en-radio:${option.value}`,
          accessibilityRole: "radio",
          accessibilityState: { selected: view.value === option.value, disabled: view.disabled === true || option.disabled === true },
          disabled: view.disabled === true || option.disabled === true,
          ...(onChange === undefined || view.disabled === true || option.disabled === true
            ? {}
            : { onPress: () => runReportedIntent(report, onChange, option.value) })
        },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "dot" }, view.value === option.value ? "◉" : "○"),
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, option.label)
      ))
  )
}

const renderSlider = (
  view: SliderView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  // Faithful subset (#53): step via +/- pressables + adjustable a11y (drag still optional via host).
  const theme = options.theme ?? defaultTheme
  const onChange = controlChangeIntent(view)
  const step = view.step ?? 1
  const clamp = (n: number) => Math.min(view.max, Math.max(view.min, n))
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "row", alignItems: "center", gap: spacingValue(theme, "2") }, viewStyle(view, options))),
      testID: "en-slider",
      accessibilityRole: "adjustable",
      accessibilityLabel: view.label,
      accessibilityValue: { min: view.min, max: view.max, now: view.value }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: "dec",
        testID: "en-slider-dec",
        disabled: view.disabled === true,
        ...(onChange === undefined || view.disabled === true
          ? {}
          : { onPress: () => runReportedIntent(report, onChange, clamp(view.value - step)) })
      },
      createElement(dependencies, dependencies.ReactNative.Text, { key: "dec-label" }, "−")
    ),
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      { key: "value", testID: "en-slider-value" },
      String(view.value)
    ),
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key: "inc",
        testID: "en-slider-inc",
        disabled: view.disabled === true,
        ...(onChange === undefined || view.disabled === true
          ? {}
          : { onPress: () => runReportedIntent(report, onChange, clamp(view.value + step)) })
      },
      createElement(dependencies, dependencies.ReactNative.Text, { key: "inc-label" }, "+")
    )
  )
}

const renderNumberField = (
  view: NumberFieldView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const onChange = controlChangeIntent(view)
  return createElement(dependencies, dependencies.ReactNative.TextInput, {
    ...baseProps(view, viewStyle(view, options)),
    testID: "en-number-field",
    accessibilityLabel: view.label,
    keyboardType: "numeric",
    editable: view.disabled !== true,
    placeholder: view.placeholder,
    value: String(view.value),
    ...(onChange === undefined
      ? {}
      : {
          onChangeText: (text: string) => {
            const parsed = Number(text)
            runReportedIntent(report, onChange, text === "" || Number.isNaN(parsed) ? null : parsed)
          }
        })
  })
}

const renderFieldRow = (
  view: FieldRowView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const children: Array<ReactElementLike> = [
    createElement(dependencies, dependencies.ReactNative.Text, { key: "label", testID: "en-field-row-label" }, view.label)
  ]
  if (view.description !== undefined) {
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Text,
        { key: "description", style: { color: colorValue(theme, "textMuted") } },
        view.description
      )
    )
  }
  children.push(renderResolvedReactNativeView(view.control, dependencies, report, options))
  if (view.error !== undefined) {
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Text,
        { key: "error", testID: "en-field-row-error", accessibilityRole: "alert", style: { color: colorValue(theme, "danger") } },
        view.error
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(view, mergeNativeStyles({ flexDirection: "column", gap: spacingValue(theme, "1") }, viewStyle(view, options))),
    ...children
  )
}

// Feedback surfaces (issue #40) on React Native. Toasts/banners carry
// accessibilityLiveRegion so TalkBack/VoiceOver announce them; auto-dismiss
// timing is left to the app/runtime (declared unsupported at render time).
// RecoveryOverlay renders as a blocking modal-styled surface when open.
const rnLiveRegion = (tone: Tone): string => (tone === "danger" ? "assertive" : "polite")

const renderNotificationCard = (
  notification: NotificationModel,
  onDismiss: IntentRef,
  dependencies: ReactNativeDependencies,
  theme: Theme,
  report: IntentReporter
): ReactElementLike => {
  const parts: Array<ReactElementLike> = [
    createElement(dependencies, dependencies.ReactNative.Text, { key: "title" }, notification.title)
  ]
  if (notification.detail !== undefined) {
    parts.push(
      createElement(dependencies, dependencies.ReactNative.Text, { key: "detail", style: { color: colorValue(theme, "textMuted") } }, notification.detail)
    )
  }
  if (notification.action !== undefined && notification.actionLabel !== undefined) {
    const action = notification.action
    parts.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        { key: "action", testID: `en-toast-action:${notification.id}`, onPress: () => runReportedIntent(report, action, notification.id) },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, notification.actionLabel)
      )
    )
  }
  parts.push(
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      { key: "dismiss", testID: `en-toast-dismiss:${notification.id}`, accessibilityLabel: "Dismiss", onPress: () => runReportedIntent(report, onDismiss, notification.id) },
      createElement(dependencies, dependencies.ReactNative.Text, { key: "x" }, "×")
    )
  )
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      key: `notification-${notification.id}`,
      testID: `en-notification:${notification.id}`,
      accessibilityRole: notification.tone === "danger" ? "alert" : "text",
      accessibilityLiveRegion: rnLiveRegion(notification.tone),
      style: { borderLeftWidth: 3, borderLeftColor: colorValue(theme, toneColorToken[notification.tone]) }
    },
    ...parts
  )
}

const scheduleToastAutoDismiss = (
  notification: { readonly id: string; readonly autoDismissMillis?: number },
  onDismiss: IntentRef,
  report: IntentReporter
): void => {
  if (notification.autoDismissMillis === undefined || notification.autoDismissMillis <= 0) return
  // Renderer-scheduled auto-dismiss (#53): fires typed onDismiss with the toast id.
  setTimeout(() => {
    runReportedIntent(report, onDismiss, notification.id)
  }, notification.autoDismissMillis)
}

const renderToast = (
  view: ToastView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  scheduleToastAutoDismiss(view.notification, view.onDismiss, report)
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, viewStyle(view, options)), testID: "en-toast" },
    renderNotificationCard(view.notification, view.onDismiss, dependencies, theme, report)
  )
}

const renderToastRegion = (
  view: ToastRegionView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  for (const notification of view.notifications) {
    scheduleToastAutoDismiss(notification, view.onDismiss, report)
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), testID: `en-toast-region:${view.placement ?? "bottom-end"}`, accessibilityRole: "none" },
    ...view.notifications.map((notification) => renderNotificationCard(notification, view.onDismiss, dependencies, theme, report))
  )
}

const renderStatusBanner = (
  view: StatusBannerView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const parts: Array<ReactElementLike> = [
    createElement(dependencies, dependencies.ReactNative.Text, { key: "message" }, view.message)
  ]
  if (view.onRetry !== undefined) {
    const onRetry = view.onRetry
    parts.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        { key: "retry", testID: "en-status-banner-retry", onPress: () => runReportedIntent(report, onRetry) },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, "Retry")
      )
    )
  }
  if (view.onDismiss !== undefined) {
    const onDismiss = view.onDismiss
    parts.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        { key: "dismiss", testID: "en-status-banner-dismiss", accessibilityLabel: "Dismiss", onPress: () => runReportedIntent(report, onDismiss) },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "x" }, "×")
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "row", borderLeftWidth: 3, borderLeftColor: colorValue(theme, toneColorToken[view.tone]) }, viewStyle(view, options))),
      testID: `en-status-banner:${view.tone}`,
      accessibilityRole: view.tone === "danger" ? "alert" : "text",
      accessibilityLiveRegion: rnLiveRegion(view.tone)
    },
    ...parts
  )
}

const renderRecoveryOverlay = (
  view: RecoveryOverlayView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const open = view.open === true
  const children: Array<ReactElementLike> = [
    createElement(dependencies, dependencies.ReactNative.Text, { key: "title", accessibilityRole: "header" }, view.title)
  ]
  if (view.status !== undefined) {
    children.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "status", accessibilityLiveRegion: "polite" }, view.status))
  }
  if (view.message !== undefined) {
    children.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "message" }, view.message))
  }
  for (const action of view.actions) {
    const intent = action.action
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        { key: `action-${action.id}`, testID: `en-recovery-action:${action.id}`, onPress: () => runReportedIntent(report, intent, action.id) },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, action.label)
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ display: open ? "flex" : "none", flexDirection: "column" }, {})),
      testID: "en-recovery-overlay",
      accessibilityViewIsModal: true,
      accessibilityRole: "none"
    },
    ...(open ? children : [])
  )
}

// Streaming transcript / markdown (issue #35) on React Native. The same
// pre-parsed typed model maps to nested Text/View; no parser. Transcript renders
// role-tagged bubbles with typed status; auto-pin is an app/runtime concern on
// RN (the model is append-optimized upstream).
let markdownKeyCounter = 0
const renderMarkdownInline = (
  inline: MarkdownInline,
  dependencies: ReactNativeDependencies
): ReactElementLike => {
  const key = `md-${markdownKeyCounter++}`
  switch (inline.kind) {
    case "text":
      return createElement(dependencies, dependencies.ReactNative.Text, { key }, inline.text)
    case "code":
      return createElement(dependencies, dependencies.ReactNative.Text, { key, style: { fontFamily: "monospace" } }, inline.text)
    case "strong":
      return createElement(dependencies, dependencies.ReactNative.Text, { key, style: { fontWeight: "700" } }, ...inline.children.map((child) => renderMarkdownInline(child, dependencies)))
    case "emphasis":
      return createElement(dependencies, dependencies.ReactNative.Text, { key, style: { fontStyle: "italic" } }, ...inline.children.map((child) => renderMarkdownInline(child, dependencies)))
    case "link":
      return createElement(dependencies, dependencies.ReactNative.Text, { key, accessibilityRole: "link", style: { textDecorationLine: "underline" } }, ...inline.children.map((child) => renderMarkdownInline(child, dependencies)))
  }
}

const renderMarkdownBlock = (
  block: MarkdownBlock,
  dependencies: ReactNativeDependencies
): ReactElementLike => {
  const key = `mdb-${markdownKeyCounter++}`
  switch (block.kind) {
    case "heading":
      return createElement(dependencies, dependencies.ReactNative.Text, { key, accessibilityRole: "header", style: { fontWeight: "700" } }, ...block.children.map((child) => renderMarkdownInline(child, dependencies)))
    case "paragraph":
      return createElement(dependencies, dependencies.ReactNative.Text, { key }, ...block.children.map((child) => renderMarkdownInline(child, dependencies)))
    case "list":
      return createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key },
        ...block.items.map((item, index) =>
          createElement(
            dependencies,
            dependencies.ReactNative.View,
            { key: `li-${index}`, style: { flexDirection: "row" } },
            createElement(dependencies, dependencies.ReactNative.Text, { key: "bullet" }, block.ordered ? `${index + 1}. ` : "• "),
            createElement(dependencies, dependencies.ReactNative.View, { key: "content" }, ...item.map((child) => renderMarkdownBlock(child, dependencies)))
          ))
      )
    case "blockquote":
      return createElement(dependencies, dependencies.ReactNative.View, { key, style: { borderLeftWidth: 2, paddingLeft: 8 } }, ...block.children.map((child) => renderMarkdownBlock(child, dependencies)))
  }
}

const renderMarkdown = (
  view: MarkdownView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike =>
  createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), testID: "en-markdown" },
    ...view.blocks.map((block) => renderMarkdownBlock(block, dependencies))
  )

const renderTranscript = (
  view: TranscriptView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  // Production-scale transcript (#57): FlatList-backed with pin-to-end and
  // maintainVisibleContentPosition so streaming append stays O(new).
  const theme = options.theme ?? defaultTheme
  return createElement(
    dependencies,
    dependencies.ReactNative.FlatList,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))),
      testID: "en-transcript",
      accessibilityLiveRegion: "polite",
      data: view.messages,
      keyExtractor: (message: { readonly key: string }) => message.key,
      windowSize: 12,
      initialNumToRender: 16,
      maxToRenderPerBatch: 8,
      removeClippedSubviews: true,
      ...(view.pinToEnd === true
        ? {
            maintainVisibleContentPosition: { minIndexForVisible: 0 },
            onContentSizeChange: () => {
              // Host list ref scroll-to-end is adapter-owned; mark pin intent for apps.
              if (view.onPinnedChange !== undefined) {
                runReportedIntent(report, view.onPinnedChange, true)
              }
            }
          }
        : {}),
      renderItem: ({ item: message }: {
        readonly item: {
          readonly key: string
          readonly role: string
          readonly status?: string
          readonly senderLabel?: string
          readonly timestamp?: string
          readonly body: ReadonlyArray<View>
        }
      }) => {
        // Role-differentiated chrome (v29, #72): meta row (sender/timestamp)
        // separated from the body; user rows end-aligned bounded bubbles.
        const children: Array<ReactElementLike> = []
        if (message.senderLabel !== undefined || message.timestamp !== undefined) {
          const metaChildren: Array<ReactElementLike> = []
          if (message.senderLabel !== undefined) {
            metaChildren.push(
              createElement(dependencies, dependencies.ReactNative.Text, {
                key: "sender",
                testID: `en-message-sender:${message.key}`,
                style: {
                  fontSize: 11,
                  fontWeight: "600",
                  letterSpacing: 0.8,
                  textTransform: "uppercase",
                  color: message.role === "user" ? colorValue(theme, "accent") : colorValue(theme, "textMuted")
                }
              }, message.senderLabel)
            )
          }
          if (message.timestamp !== undefined) {
            metaChildren.push(
              createElement(dependencies, dependencies.ReactNative.Text, {
                key: "timestamp",
                testID: `en-message-timestamp:${message.key}`,
                style: { fontSize: 11, color: colorValue(theme, "textMuted") }
              }, message.timestamp)
            )
          }
          children.push(
            createElement(
              dependencies,
              dependencies.ReactNative.View,
              {
                key: "meta",
                testID: `en-message-meta:${message.key}`,
                style: { flexDirection: "row", alignItems: "baseline", gap: spacingValue(theme, "2") }
              },
              ...metaChildren
            )
          )
        }
        children.push(
          createElement(
            dependencies,
            dependencies.ReactNative.View,
            {
              key: "body",
              testID: `en-message-body:${message.key}`,
              style: message.role === "user"
                ? {
                    backgroundColor: colorValue(theme, "surfaceRaised"),
                    borderColor: colorValue(theme, "border"),
                    borderWidth: 1,
                    borderRadius: 8,
                    paddingVertical: spacingValue(theme, "2"),
                    paddingHorizontal: spacingValue(theme, "3")
                  }
                : {}
            },
            ...message.body.map((child) =>
              renderResolvedReactNativeView(child, dependencies, report, options)
            )
          )
        )
        const messageElement = createElement(
          dependencies,
          dependencies.ReactNative.View,
          {
            testID: `en-message:${message.key}`,
            nativeID: `effect-native-message:${message.role}`,
            style: {
              gap: spacingValue(theme, "1"),
              maxWidth: "82%",
              minWidth: 0,
              flexShrink: 1
            },
            ...(message.status === undefined
              ? {}
              : {
                  accessibilityState: {
                    busy: message.status === "streaming" || message.status === "thinking"
                  }
                })
          },
          ...children
        )
        return createElement(
          dependencies,
          dependencies.ReactNative.View,
          {
            key: `message-${message.key}`,
            testID: `en-message-row:${message.key}`,
            style: {
              width: "100%",
              minWidth: 0,
              flexDirection: "row",
              justifyContent: message.role === "user" ? "flex-end" : "flex-start"
            }
          },
          messageElement
        )
      }
    }
  )
}

// CodeBlock + unified diff (issue #36) on React Native. Pre-tokenized lines and
// pre-parsed diff rows map to colored Text runs; no highlighter/parser. The
// review affordances render as a supported pressable subset.
const codeTokenColor: Record<CodeTokenKind, ColorToken> = {
  plain: "textPrimary",
  keyword: "syntaxKeyword",
  string: "syntaxString",
  comment: "syntaxComment",
  function: "syntaxFunction",
  number: "syntaxNumber",
  operator: "syntaxOperator"
}

const renderCodeTokens = (
  tokens: ReadonlyArray<CodeToken>,
  dependencies: ReactNativeDependencies,
  theme: Theme
): ReadonlyArray<ReactElementLike> =>
  tokens.map((token, index) =>
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      { key: `tok-${index}`, style: { color: colorValue(theme, codeTokenColor[token.kind]), fontFamily: "monospace" } },
      token.text
    ))

const renderCodeBlock = (
  view: CodeBlockView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const startLine = view.startLine ?? 1
  const children: Array<ReactElementLike> = []
  if (view.onCopy !== undefined) {
    const onCopy = view.onCopy
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        { key: "copy", testID: "en-code-copy", accessibilityLabel: "Copy code", onPress: () => runReportedIntent(report, onCopy, codeBlockPlainText(view.lines)) },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, "Copy")
      )
    )
  }
  view.lines.forEach((line, index) => {
    const parts: Array<ReactElementLike> = []
    if (view.showLineNumbers === true) {
      parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "gutter", style: { color: colorValue(theme, "textMuted") } }, `${startLine + index} `))
    }
    parts.push(...renderCodeTokens(line.tokens, dependencies, theme))
    children.push(createElement(dependencies, dependencies.ReactNative.Text, { key: `line-${index}`, testID: `en-code-line:${index}` }, ...parts))
  })
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ backgroundColor: colorValue(theme, "codeBackground") }, viewStyle(view, options))), testID: "en-code-block" },
    ...children
  )
}

const renderDiffView = (
  view: DiffViewView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const children: Array<ReactElementLike> = []
  for (const hunk of view.hunks) {
    children.push(createElement(dependencies, dependencies.ReactNative.Text, { key: `hunk-${hunk.header}`, style: { color: colorValue(theme, "textMuted") } }, hunk.header))
    for (const row of hunk.rows) {
      const bg = row.kind === "add" ? colorValue(theme, "diffAdd") : row.kind === "remove" ? colorValue(theme, "diffRemove") : undefined
      const marker = row.kind === "add" ? "+" : row.kind === "remove" ? "-" : " "
      const rowParts: Array<ReactElementLike> = [
        createElement(dependencies, dependencies.ReactNative.Text, { key: "marker" }, `${marker} `),
        ...renderCodeTokens(row.tokens, dependencies, theme)
      ]
      if (row.id !== undefined && view.onLineVerdict !== undefined) {
        const onLineVerdict = view.onLineVerdict
        const rowId = row.id
        rowParts.push(
          createElement(
            dependencies,
            dependencies.ReactNative.Pressable,
            { key: "approve", testID: `en-diff-verdict:${rowId}:approved`, onPress: () => runReportedIntent(report, onLineVerdict, { rowId, verdict: "approved" }) },
            createElement(dependencies, dependencies.ReactNative.Text, { key: "t" }, "✓")
          )
        )
      }
      children.push(
        createElement(
          dependencies,
          dependencies.ReactNative.View,
          { key: `row-${row.id ?? `${row.oldLine}:${row.newLine}`}`, testID: row.id === undefined ? undefined : `en-diff-row:${row.id}`, style: { flexDirection: "row", ...(bg === undefined ? {} : { backgroundColor: bg }) } },
          ...rowParts
        )
      )
    }
  }
  if (view.actions !== undefined && view.onSourceControlAction !== undefined) {
    const onAction = view.onSourceControlAction
    children.push(
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        { key: "actions", style: { flexDirection: "row", gap: spacingValue(theme, "2") } },
        ...view.actions.map((action) =>
          createElement(
            dependencies,
            dependencies.ReactNative.Pressable,
            { key: `action-${action.id}`, testID: `en-diff-action:${action.id}`, onPress: () => runReportedIntent(report, onAction, action.id) },
            createElement(dependencies, dependencies.ReactNative.Text, { key: "l" }, action.label)
          ))
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ backgroundColor: colorValue(theme, "codeBackground") }, viewStyle(view, options))), testID: `en-diff:${view.layout ?? "unified"}` },
    ...children
  )
}

// GraphFigure + Timeline (issue #37/#53) on React Native. Nodes + edges +
// camera step controls (pan/zoom intents). Continuous gesture pan is optional
// when a canvas/Skia host is registered; Timeline stays a status-tagged list.
const renderGraphFigure = (
  view: GraphFigureView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const statusColor = (status: GraphStatus | undefined) => colorValue(theme, graphStatusColorToken[status ?? "idle"])
  const camera = view.camera ?? { x: 0, y: 0, zoom: 1 }
  const cameraControls =
    view.onCameraChange === undefined
      ? []
      : [
          createElement(
            dependencies,
            dependencies.ReactNative.View,
            {
              key: "camera",
              testID: "en-graph-camera",
              style: { flexDirection: "row", gap: spacingValue(theme, "2") }
            },
            ...(["pan-left", "pan-right", "zoom-in", "zoom-out"] as const).map((action) =>
              createElement(
                dependencies,
                dependencies.ReactNative.Pressable,
                {
                  key: action,
                  testID: `en-graph-camera:${action}`,
                  onPress: () => {
                    const next =
                      action === "pan-left"
                        ? { ...camera, x: camera.x - 20 }
                        : action === "pan-right"
                          ? { ...camera, x: camera.x + 20 }
                          : action === "zoom-in"
                            ? { ...camera, zoom: Math.min(4, camera.zoom + 0.1) }
                            : { ...camera, zoom: Math.max(0.25, camera.zoom - 0.1) }
                    runReportedIntent(report, view.onCameraChange!, next)
                  }
                },
                createElement(dependencies, dependencies.ReactNative.Text, { key: "l" }, action)
              )
            )
          )
        ]
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))),
      testID: "en-graph-figure",
      accessibilityLabel: view.a11y?.label
    },
    ...cameraControls,
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: "nodes", testID: "en-graph-nodes", style: { flexDirection: "column", gap: spacingValue(theme, "1") } },
      ...view.nodes.map((node) => {
        const dot = createElement(dependencies, dependencies.ReactNative.View, { key: "dot", style: { width: 8, height: 8, borderRadius: 999, backgroundColor: statusColor(node.status) } })
        const label = createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, node.label)
        const props: Record<string, unknown> = {
          key: `node-${node.id}`,
          testID: `en-graph-node:${node.id}`,
          style: { flexDirection: "row", gap: spacingValue(theme, "2") }
        }
        if (view.onNodeSelect !== undefined) {
          const onNodeSelect = view.onNodeSelect
          return createElement(dependencies, dependencies.ReactNative.Pressable, { ...props, onPress: () => runReportedIntent(report, onNodeSelect, node.id) }, dot, label)
        }
        return createElement(dependencies, dependencies.ReactNative.View, props, dot, label)
      })
    ),
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: "edges", testID: "en-graph-edges", style: { flexDirection: "column", gap: spacingValue(theme, "1") } },
      ...view.edges.map((edge) =>
        createElement(
          dependencies,
          dependencies.ReactNative.Text,
          {
            key: `edge-${edge.id}`,
            testID: `en-graph-edge:${edge.id}`,
            style: { color: colorValue(theme, "textMuted") }
          },
          `${edge.from} → ${edge.to}`
        )
      )
    )
  )
}

const renderTimeline = (
  view: TimelineView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const statusColor = (status: GraphStatus | undefined) => colorValue(theme, graphStatusColorToken[status ?? "idle"])
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column", gap: spacingValue(theme, "2") }, viewStyle(view, options))), testID: "en-timeline" },
    ...view.events.map((graphEvent) => {
      const parts: Array<ReactElementLike> = [
        graphEvent.icon === undefined
          ? createElement(dependencies, dependencies.ReactNative.View, { key: "dot", style: { width: 8, height: 8, borderRadius: 999, backgroundColor: statusColor(graphEvent.status) } })
          : createElement(dependencies, dependencies.ReactNative.Text, { key: "icon", accessibilityElementsHidden: true }, iconGlyphs[graphEvent.icon]),
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, graphEvent.label)
      ]
      if (graphEvent.time !== undefined) parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "time", style: { color: colorValue(theme, "textMuted") } }, graphEvent.time))
      if (graphEvent.detail !== undefined) parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "detail", style: { color: colorValue(theme, "textMuted"), flexShrink: 1 } }, graphEvent.detail))
      const selected = view.selectedId === graphEvent.id
      const props: Record<string, unknown> = { key: graphEvent.key ?? `event-${graphEvent.id}`, testID: `en-timeline-event:${graphEvent.id}`, accessibilityLabel: graphEvent.accessibilityLabel ?? graphEvent.label, ...(graphEvent.variant === undefined ? {} : { accessibilityHint: graphEvent.variant }), accessibilityState: { selected }, style: { flexDirection: "row", gap: spacingValue(theme, "2") } }
      const onSelect = graphEvent.onSelect ?? view.onEventSelect
      if (onSelect !== undefined) {
        return createElement(dependencies, dependencies.ReactNative.Pressable, { ...props, onPress: () => runReportedIntent(report, onSelect, graphEvent.id) }, ...parts)
      }
      return createElement(dependencies, dependencies.ReactNative.View, props, ...parts)
    })
  )
}


// Marketing catalog (#46–#51) — structural RN subset
const renderMarketingShell = (
  view: View,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const box = (testID: string, children: ReadonlyArray<ReactElementLike>) =>
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        ...baseProps(view, mergeNativeStyles({ flexDirection: "column", gap: spacingValue(theme, "2") }, viewStyle(view as never, options))),
        testID
      },
      ...children
    )
  const text = (value: string, key: string) =>
    createElement(dependencies, dependencies.ReactNative.Text, { key }, value)
  const press = (label: string, intent: IntentRef, key: string) =>
    createElement(
      dependencies,
      dependencies.ReactNative.Pressable,
      {
        key,
        testID: `en-mkt-press-${key}`,
        onPress: () => runReportedIntent(report, intent)
      },
      text(label, `${key}-label`)
    )

  switch (view._tag) {
    case "Section":
    case "Glow":
    case "MockupFrame":
      return box(
        `en-${view._tag}`,
        view.children.map((child, index) =>
          renderResolvedReactNativeView(child, dependencies, report, options)
        ) as ReactElementLike[]
      )
    case "Hero": {
      const kids: Array<ReactElementLike> = [
        text(typeof view.headline === "string" ? view.headline : "", "headline")
      ]
      if (typeof view.subhead === "string") kids.push(text(view.subhead, "subhead"))
      for (const child of view.actions) kids.push(renderResolvedReactNativeView(child, dependencies, report, options))
      if (view.media !== undefined) kids.push(renderResolvedReactNativeView(view.media, dependencies, report, options))
      return box("en-Hero", kids)
    }
    case "AnnouncementBadge": {
      const label =
        view.actionLabel === undefined ? view.label : `${view.label} · ${view.actionLabel}`
      if (view.onPress === undefined) {
        return box("en-AnnouncementBadge", [text(view.label, "label")])
      }
      return createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        {
          ...baseProps(view, mergeNativeStyles(
            { flexDirection: "row", alignItems: "center", gap: spacingValue(theme, "2") },
            viewStyle(view as never, options)
          )),
          testID: "en-AnnouncementBadge",
          onPress: () => runReportedIntent(report, view.onPress!)
        },
        text(label, "label")
      )
    }
    case "CtaSection": {
      const kids: Array<ReactElementLike> = [
        text(typeof view.headline === "string" ? view.headline : "", "headline")
      ]
      if (typeof view.body === "string") kids.push(text(view.body, "body"))
      for (const child of view.actions) kids.push(renderResolvedReactNativeView(child, dependencies, report, options))
      return box("en-CtaSection", kids)
    }
    case "Footer": {
      const kids: Array<ReactElementLike> = []
      if (view.brand !== undefined) kids.push(renderResolvedReactNativeView(view.brand, dependencies, report, options))
      for (const column of view.columns) {
        for (const link of column.links) kids.push(renderResolvedReactNativeView(link, dependencies, report, options))
      }
      if (view.legal !== undefined) kids.push(renderResolvedReactNativeView(view.legal, dependencies, report, options))
      return box("en-Footer", kids)
    }
    case "NavBar": {
      const kids: Array<ReactElementLike> = [
        renderResolvedReactNativeView(view.brand, dependencies, report, options)
      ]
      for (const link of view.links) kids.push(press(link.label, link.onPress, link.id))
      for (const child of view.actions ?? []) kids.push(renderResolvedReactNativeView(child, dependencies, report, options))
      return box("en-NavBar", kids)
    }
    case "Accordion": {
      const kids: Array<ReactElementLike> = []
      for (const item of view.items) {
        kids.push(press(item.header, view.onToggle, item.id))
        if (view.expandedIds.includes(item.id)) {
          for (const child of item.content) kids.push(renderResolvedReactNativeView(child, dependencies, report, options))
        }
      }
      return box("en-Accordion", kids)
    }
    case "PricingColumn": {
      const kids: Array<ReactElementLike> = [
        text(view.name, "name"),
        text(view.period === undefined ? view.price : `${view.price} / ${view.period}`, "price")
      ]
      for (const feature of view.features) {
        kids.push(text(`${feature.included ? "yes" : "no"}: ${feature.label}`, feature.id))
      }
      kids.push(press(view.ctaLabel, view.onCta, "cta"))
      return box("en-PricingColumn", kids)
    }
    case "PricingTable":
      return box(
        "en-PricingTable",
        view.columns.map((column) =>
          renderResolvedReactNativeView(column, dependencies, report, options)
        ) as ReactElementLike[]
      )
    case "LogoRow":
      return box(
        "en-LogoRow",
        view.logos.map((logo) =>
          createElement(dependencies, dependencies.ReactNative.Image, {
            key: logo.id,
            source: { uri: logo.source },
            accessibilityLabel: logo.alt,
            style: { width: 72, height: 28 }
          })
        )
      )
    case "StatsBand":
      return box(
        "en-StatsBand",
        view.stats.map((stat) =>
          createElement(
            dependencies,
            dependencies.ReactNative.View,
            { key: stat.id },
            text(typeof stat.value === "string" ? stat.value : "", `${stat.id}-v`),
            text(stat.label, `${stat.id}-l`)
          )
        )
      )
    default:
      return box(`en-${view._tag}`, [])
  }
}

const renderResolvedReactNativeView = (
  view: View,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions = {}
): ReactElementLike => {
  switch (view._tag) {
    case "Stack":
      return renderStack(view, dependencies, report, options)
    case "Text":
      return renderText(view, dependencies, options)
    case "Button":
      return renderButton(view, dependencies, report, options)
    case "Link":
      return renderLink(view, dependencies, report, options)
    case "Modal":
      return renderModal(view, dependencies, report, options)
    case "Sheet":
      return renderSheet(view, dependencies, report, options)
    case "Image":
      return renderImage(view, dependencies, options)
    case "TextField":
      return renderTextField(view, dependencies, report, options)
    case "List":
      return renderList(view, dependencies, report, options)
    case "SectionList":
      return renderSectionList(view, dependencies, report, options)
    case "Card":
      return renderCard(view, dependencies, report, options)
    case "Spacer":
      return renderSpacer(view, dependencies, options)
    case "Host":
      return renderHost(view, dependencies, report, options)
    case "Icon":
      return renderIcon(view, dependencies, options)
    case "Divider":
      return renderDivider(view, dependencies, options)
    case "Badge":
      return renderBadge(view, dependencies, options)
    case "Chip":
      return renderChip(view, dependencies, options)
    case "Meter":
      return renderMeter(view, dependencies, options)
    case "StatTile":
      return renderStatTile(view, dependencies, options)
    case "Table":
      return renderTable(view, dependencies, report, options)
    case "SplitPane":
      return renderSplitPane(view, dependencies, report, options)
    case "NavRail":
      return renderNavRail(view, dependencies, report, options)
    case "Workbench":
      return renderWorkbench(view, dependencies, report, options)
    case "Popover":
      return renderPopover(view, dependencies, report, options)
    case "DropdownMenu":
      return renderDropdownMenu(view, dependencies, report, options)
    case "ContextMenu":
      return renderContextMenu(view, dependencies, report, options)
    case "Tooltip":
      return renderTooltip(view, dependencies, report, options)
    case "Combobox":
      return renderCombobox(view, dependencies, report, options)
    case "CommandPalette":
      return renderCommandPalette(view, dependencies, report, options)
    case "Tabs":
      return renderTabs(view, dependencies, report, options)
    case "Composer":
      return renderComposer(view, dependencies, report, options)
    case "Toggle":
      return renderToggle(view, dependencies, report, options)
    case "Select":
      return renderSelect(view, dependencies, report, options)
    case "Checkbox":
      return renderCheckbox(view, dependencies, report, options)
    case "RadioGroup":
      return renderRadioGroup(view, dependencies, report, options)
    case "Slider":
      return renderSlider(view, dependencies, report, options)
    case "NumberField":
      return renderNumberField(view, dependencies, report, options)
    case "FieldRow":
      return renderFieldRow(view, dependencies, report, options)
    case "Toast":
      return renderToast(view, dependencies, report, options)
    case "ToastRegion":
      return renderToastRegion(view, dependencies, report, options)
    case "StatusBanner":
      return renderStatusBanner(view, dependencies, report, options)
    case "RecoveryOverlay":
      return renderRecoveryOverlay(view, dependencies, report, options)
    case "Markdown":
      return renderMarkdown(view, dependencies, options)
    case "Transcript":
      return renderTranscript(view, dependencies, report, options)
    case "CodeBlock":
      return renderCodeBlock(view, dependencies, report, options)
    case "DiffView":
      return renderDiffView(view, dependencies, report, options)
    case "GraphFigure":
      return renderGraphFigure(view, dependencies, report, options)
    case "Timeline":
      return renderTimeline(view, dependencies, report, options)
    case "Section":
    case "Hero":
    case "AnnouncementBadge":
    case "CtaSection":
    case "Footer":
    case "NavBar":
    case "Accordion":
    case "PricingColumn":
    case "PricingTable":
    case "LogoRow":
    case "StatsBand":
    case "Glow":
    case "MockupFrame":
      return renderMarketingShell(view, dependencies, report, options)
    case "Pager":
      return renderPager(view, dependencies, report, options)
    case "SwipeableListItem":
      return renderSwipeableListItem(view, dependencies, report, options)
    case "BackgroundGradient":
    case "Wallpaper":
    case "Spotlight":
    case "Frame":
    case "BlurredPopup":
      return renderMobileSurfaceShell(view, dependencies, report, options)
    case "IconButton":
      return renderIconButton(view, dependencies, report, options)
    case "Toolbar":
      return renderToolbar(view, dependencies, report, options)
  }
}

// ── Glass set (GL-1, openagents#8647) ────────────────────────────────────────
//
// `surface: "glass"` is a SEMANTIC contract. Three honest lowerings:
//   1. iOS 26+ with @expo/ui present → real SwiftUI Liquid Glass through the
//      render-rn-INTERNAL @expo/ui lowering below (app code never imports
//      @expo/ui — the hybrid decision in openagents
//      docs/fable/2026-07-09-swiftui-expo-ui-and-the-effect-native-stdlib.md).
//   2. Everything else (Android, iOS < 26, missing native module, tests) →
//      the documented RN-core material approximation (glassSurfaceStyle).
//   3. GL-4 replaces the @expo/ui lowering component-by-component with owned
//      lowerings under the SAME catalog contract (convert-and-delete).

// Internal structural view of the "@expo/ui/swift-ui" (+ "/modifiers") runtime.
// Loaded by require() inside this renderer only; tests inject a fake through
// `ReactNativeRenderOptions.expoUi`.
export interface ExpoUiSwiftUiRuntime {
  readonly Host: unknown
  readonly HStack: unknown
  readonly VStack: unknown
  readonly Button: unknown
  readonly Image: unknown
  readonly Text: unknown
  readonly Spacer: unknown
  readonly TextField?: unknown
  readonly useNativeState?: <Value>(initialValue: Value) => {
    readonly get: () => Value
    readonly set: (value: Value) => void
  }
  readonly modifiers: {
    readonly glassEffect: (params?: {
      readonly glass?: {
        readonly variant: "regular" | "clear" | "identity"
        readonly interactive?: boolean
        readonly tint?: string
      }
      readonly shape?: "circle" | "capsule" | "rectangle" | "ellipse" | "roundedRectangle" | "containerRelativeShape"
      readonly cornerRadius?: number
    }) => unknown
    readonly foregroundStyle: (style: string) => unknown
    readonly frame: (params: Record<string, number | string>) => unknown
    readonly padding?: (params?: Record<string, number>) => unknown
    readonly disabled?: (disabled?: boolean) => unknown
    // Hit-testing shape (SwiftUI contentShape): without it only the visible
    // label responds to taps — a flexed button's free space would be dead.
    readonly contentShape?: (shape: unknown) => unknown
    readonly shapes?: { readonly rectangle: (params?: Record<string, unknown>) => unknown }
  }
}

let cachedExpoUiRuntime: ExpoUiSwiftUiRuntime | null | undefined
const loadExpoUiRuntime = (): ExpoUiSwiftUiRuntime | undefined => {
  if (cachedExpoUiRuntime !== undefined) {
    return cachedExpoUiRuntime ?? undefined
  }
  if (typeof require !== "function") {
    cachedExpoUiRuntime = null
    return undefined
  }
  try {
    const swiftUi = require("@expo/ui/swift-ui") as Record<string, unknown>
    const modifiers = require("@expo/ui/swift-ui/modifiers") as ExpoUiSwiftUiRuntime["modifiers"]
    if (
      swiftUi.Host === undefined || swiftUi.Button === undefined || swiftUi.Image === undefined ||
      swiftUi.HStack === undefined || swiftUi.VStack === undefined || swiftUi.Text === undefined ||
      swiftUi.Spacer === undefined || typeof modifiers.glassEffect !== "function"
    ) {
      cachedExpoUiRuntime = null
      return undefined
    }
    cachedExpoUiRuntime = {
      Host: swiftUi.Host,
      HStack: swiftUi.HStack,
      VStack: swiftUi.VStack,
      Button: swiftUi.Button,
      Image: swiftUi.Image,
      Text: swiftUi.Text,
      Spacer: swiftUi.Spacer,
      ...(swiftUi.TextField === undefined ? {} : { TextField: swiftUi.TextField }),
      ...(typeof swiftUi.useNativeState !== "function"
        ? {}
        : { useNativeState: swiftUi.useNativeState as NonNullable<ExpoUiSwiftUiRuntime["useNativeState"]> }),
      modifiers
    } as ExpoUiSwiftUiRuntime
    return cachedExpoUiRuntime ?? undefined
  } catch {
    // @expo/ui absent (web/tests/Expo Go without the module) — honest material
    // fallback, never a crash.
    cachedExpoUiRuntime = null
    return undefined
  }
}

const iosMajorVersion = (dependencies: ReactNativeDependencies): number | undefined => {
  const platform = dependencies.ReactNative.Platform
  if (platform?.OS !== "ios") {
    return undefined
  }
  const version = platform.Version
  const major = typeof version === "number"
    ? Math.trunc(version)
    : typeof version === "string"
      ? Number.parseInt(version, 10)
      : Number.NaN
  return Number.isFinite(major) ? major : undefined
}

// The @expo/ui lowering activates ONLY where the material is real: iOS 26+
// (SwiftUI .glassEffect / Liquid Glass). Below 26 the @expo/ui modifiers
// degrade to no-ops (transparent chrome), which would be DISHONEST — the RN
// material approximation stays the fallback there.
const glassLoweringRuntime = (
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ExpoUiSwiftUiRuntime | undefined => {
  const major = iosMajorVersion(dependencies)
  if (major === undefined || major < 26) {
    return undefined
  }
  return options.expoUi ?? loadExpoUiRuntime()
}

// SF Symbol names for the closed IconName set — a render-rn-internal asset
// detail of the SwiftUI lowering (parallel to iconGlyphs / the DOM SVG
// registry). No raw symbol strings enter the app-facing contract.
const sfSymbolForIcon: Record<IconName, string> = {
  Plus: "plus",
  Play: "play.fill",
  Pause: "pause.fill",
  Stop: "stop.fill",
  Reload: "arrow.clockwise",
  Circle: "circle",
  Check: "checkmark",
  X: "xmark",
  ChevronUp: "chevron.up",
  ChevronDown: "chevron.down",
  ChevronLeft: "chevron.left",
  ChevronRight: "chevron.right",
  Menu: "line.3.horizontal",
  Compose: "square.and.pencil",
  Mic: "mic",
  Sparkles: "sparkles",
  // Monorepo-vendored icon extensions (desktop/mobile app set; see the core
  // iconNames divergence note in VENDORING.md).
  Home: "house",
  Agent: "circle.dashed",
  ChatCompose: "square.and.pencil",
  Chats: "bubble.left.and.bubble.right",
  Code: "chevron.left.forwardslash.chevron.right",
  Compare: "arrow.left.arrow.right",
  Folder: "folder",
  NotificationBell: "bell",
  Plane: "paperplane.fill",
  ArrowUp: "arrow.up",
  Settings: "gearshape",
  Terminal: "apple.terminal",
  Tools: "wrench.and.screwdriver",
  History: "clock.arrow.circlepath",
  Branch: "arrow.triangle.branch",
  InfoCircle: "info.circle"
}

// Resolved flat style (responsive variants applied, tokens NOT yet lowered) —
// used to detect the semantic `surface: "glass"` key before lowering.
const resolvedFlatStyle = (
  view: View,
  options: ReactNativeRenderOptions
): FlatStyle | undefined => {
  if (!("style" in view) || view.style === undefined) {
    return undefined
  }
  const viewport = options.viewport === undefined
    ? undefined
    : makeViewport(options.viewport, options.theme ?? defaultTheme)
  return resolveStyle(view.style as FlatStyle, {
    platform: options.platform ?? "ios",
    ...(viewport === undefined ? {} : { breakpoint: viewport.breakpoint })
  })
}

// App style lowered WITHOUT the glass surface keys: on the @expo/ui path the
// material is native, so the RN container must not also paint the translucent
// approximation behind it.
const viewStyleWithoutSurface = (view: View, options: ReactNativeRenderOptions): ReactNativeStyle => {
  const flat = resolvedFlatStyle(view, options)
  if (flat === undefined) {
    return {}
  }
  const rest: Record<string, unknown> = { ...flat }
  delete rest.surface
  return lowerStyle(rest as FlatStyle, options)
}

const expoUiLowerableChild = (child: View): boolean =>
  child._tag === "IconButton" || child._tag === "Button" || child._tag === "Text" ||
  child._tag === "Spacer" || child._tag === "Icon"

// Lower one bounded catalog leaf into its SwiftUI (@expo/ui) equivalent.
// Events stay typed: every press dispatches the SAME IntentRef through the
// SAME reporter as the RN path.
const renderExpoUiLeaf = (
  child: View,
  expoUi: ExpoUiSwiftUiRuntime,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  switch (child._tag) {
    case "IconButton":
      return createElement(
        dependencies,
        expoUi.Button,
        {
          key: child.key,
          onPress: () => {
            if (child.disabled !== true) {
              runReportedIntent(report, child.onPress)
            }
          },
          ...(child.disabled === true && expoUi.modifiers.disabled !== undefined
            ? { modifiers: [expoUi.modifiers.disabled(true)] }
            : {})
        },
        createElement(dependencies, expoUi.Image, {
          systemName: sfSymbolForIcon[child.icon],
          size: 17,
          color: colorValue(theme, "textPrimary")
        })
      )
    case "Button": {
      const flat = resolvedFlatStyle(child, options)
      return createElement(
        dependencies,
        expoUi.Button,
        {
          key: child.key,
          onPress: () => {
            if (child.disabled !== true) {
              runReportedIntent(report, child.onPress)
            }
          },
          modifiers: [
            expoUi.modifiers.foregroundStyle(
              flat?.color !== undefined
                ? colorValue(theme, flat.color as ColorToken)
                : buttonLabelColor(child, theme)
            ),
            ...(child.disabled === true && expoUi.modifiers.disabled !== undefined
              ? [expoUi.modifiers.disabled(true)]
              : [])
          ]
        },
        createElement(
          dependencies,
          expoUi.Text,
          {
            key: "label",
            // style.flex: the LABEL expands across the container's free
            // space and the whole run hit-tests. The frame must live INSIDE
            // the Button label — a frame on the Button itself does not
            // extend its tappable area (SwiftUI buttons hug their label,
            // leaving dead zones the island's full-capsule dispatcher never
            // had); contentShape makes the empty run tappable.
            ...(flat?.flex !== undefined
              ? {
                modifiers: [
                  expoUi.modifiers.frame({ maxWidth: 100000, alignment: "leading" }),
                  ...(expoUi.modifiers.contentShape !== undefined && expoUi.modifiers.shapes !== undefined
                    ? [expoUi.modifiers.contentShape(expoUi.modifiers.shapes.rectangle())]
                    : [])
                ]
              }
              : {})
          },
          child.label
        )
      )
    }
    case "Text":
      return createElement(
        dependencies,
        expoUi.Text,
        {
          key: child.key,
          modifiers: [
            expoUi.modifiers.foregroundStyle(
              colorValue(theme, child.color ?? "textPrimary")
            )
          ]
        },
        String(child.content)
      )
    case "Icon":
      return createElement(dependencies, expoUi.Image, {
        key: child.key,
        systemName: sfSymbolForIcon[child.name],
        size: iconFontSize[child.size ?? "md"],
        color: colorValue(theme, child.color ?? "textPrimary")
      })
    case "Spacer":
      return createElement(dependencies, expoUi.Spacer, { key: child.key })
    default:
      // Guarded by expoUiLowerableChild; loud if the guard drifts.
      throw new Error(`not an @expo/ui-lowerable child: ${child._tag}`)
  }
}

// Standalone glass IconButton → SwiftUI Button + SF Symbol in a 44pt Liquid
// Glass circle (the ChatGPT-chrome shape), hosted inside the RN-styled
// container the shell positions.
const renderExpoUiIconButton = (
  view: IconButtonView,
  expoUi: ExpoUiSwiftUiRuntime,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const style = mergeNativeStyles(
    {
      width: 44,
      height: 44,
      opacity: view.disabled === true ? 0.5 : 1
    },
    viewStyleWithoutSurface(view, options)
  )
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      testID: `en-icon-button:${view.icon}`,
      accessibilityRole: "button",
      accessibilityLabel: view.accessibilityLabel,
      accessibilityState: { disabled: view.disabled === true }
    },
    createElement(
      dependencies,
      expoUi.Host,
      { key: "host", style: { flex: 1 } },
      createElement(
        dependencies,
        expoUi.Button,
        {
          key: "button",
          onPress: () => {
            if (view.disabled !== true) {
              runReportedIntent(report, view.onPress)
            }
          },
          modifiers: [
            expoUi.modifiers.frame({ width: 44, height: 44 }),
            expoUi.modifiers.glassEffect({
              glass: { variant: "regular", interactive: true },
              shape: "circle"
            }),
            ...(view.disabled === true && expoUi.modifiers.disabled !== undefined
              ? [expoUi.modifiers.disabled(true)]
              : [])
          ]
        },
        createElement(dependencies, expoUi.Image, {
          systemName: sfSymbolForIcon[view.icon],
          size: 17,
          color: colorValue(theme, "textPrimary")
        })
      )
    )
  )
}

// Glass Button (style.surface === "glass") → SwiftUI Button label in a Liquid
// Glass capsule — the ChatGPT-chrome pill.
const renderExpoUiButton = (
  view: ButtonView,
  expoUi: ExpoUiSwiftUiRuntime,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const flat = resolvedFlatStyle(view, options)
  const style = mergeNativeStyles(
    { opacity: view.disabled === true ? 0.5 : 1 },
    viewStyleWithoutSurface(view, options)
  )
  // SwiftUI owns the capsule's intrinsic size (matchContents reports it back
  // into the RN tree); an app-provided height lowers to a SwiftUI frame.
  const heightValue = typeof style.height === "number" ? style.height : undefined
  const labelColor = flat?.color !== undefined
    ? colorValue(theme, flat.color as ColorToken)
    : buttonLabelColor(view, theme)
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      accessibilityRole: "button",
      accessibilityState: { disabled: view.disabled === true }
    },
    createElement(
      dependencies,
      expoUi.Host,
      { key: "host", matchContents: true },
      createElement(
        dependencies,
        expoUi.Button,
        {
          key: "button",
          onPress: () => {
            if (view.disabled !== true) {
              runReportedIntent(report, view.onPress)
            }
          },
          modifiers: [
            expoUi.modifiers.foregroundStyle(labelColor),
            ...(expoUi.modifiers.padding === undefined
              ? []
              : [expoUi.modifiers.padding({ horizontal: spacingValue(theme, "4") })]),
            ...(heightValue === undefined ? [] : [expoUi.modifiers.frame({ height: heightValue })]),
            expoUi.modifiers.glassEffect({
              glass: { variant: "regular", interactive: true },
              shape: "capsule"
            }),
            ...(view.disabled === true && expoUi.modifiers.disabled !== undefined
              ? [expoUi.modifiers.disabled(true)]
              : [])
          ]
        },
        createElement(dependencies, expoUi.Text, { key: "label" }, view.label)
      )
    )
  )
}

// Glass container (Toolbar, or Stack with style.surface === "glass") whose
// children are ALL bounded lowerable leaves → one SwiftUI subtree: an
// HStack/VStack of native controls sharing a single Liquid Glass shape (the
// floating-composer / option-sheet pattern). A single non-lowerable child
// falls the WHOLE container back to the honest RN path — never a half-native
// hybrid.
const renderExpoUiGlassContainer = (
  view: ToolbarView | StackView,
  expoUi: ExpoUiSwiftUiRuntime,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const isToolbar = view._tag === "Toolbar"
  const direction = isToolbar ? "row" : resolveResponsiveValue((view as StackView).direction)
  const gap = isToolbar
    ? spacingValue(theme, "2")
    : (view as StackView).gap === undefined
      ? undefined
      : spacingValue(theme, resolveResponsiveValue((view as StackView).gap!))
  const style = viewStyleWithoutSurface(view, options)
  const loweredRadius = style.borderRadius
  const shape: { readonly shape: "capsule" | "roundedRectangle"; readonly cornerRadius?: number } = isToolbar
    ? { shape: "capsule" }
    : typeof loweredRadius === "number"
      ? { shape: "roundedRectangle", cornerRadius: loweredRadius }
      : { shape: "roundedRectangle", cornerRadius: 24 }
  const stackType = direction === "row" ? expoUi.HStack : expoUi.VStack
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      ...(isToolbar ? { testID: `en-toolbar:${(view as ToolbarView).placement ?? "bottom-floating"}` } : {})
    },
    createElement(
      dependencies,
      expoUi.Host,
      // Without an app-provided height the RN wrapper has no intrinsic size;
      // matchContents reports the SwiftUI layout back so the container stays
      // hit-testable (glassEffect overdraw LOOKS right even at zero height —
      // taps do not).
      style.height !== undefined
        ? { key: "host", style: { flex: 1 } }
        : { key: "host", matchContents: true },
      createElement(
        dependencies,
        stackType,
        {
          key: "stack",
          ...(gap === undefined ? {} : { spacing: gap }),
          modifiers: [
            ...(expoUi.modifiers.padding === undefined
              ? []
              : [expoUi.modifiers.padding({ horizontal: spacingValue(theme, "3"), vertical: spacingValue(theme, "2") })]),
            // Fill the host width so the shared glass shape spans the bar
            // (SwiftUI stacks otherwise hug their content, centered).
            expoUi.modifiers.frame(direction === "row" ? { maxWidth: 100000 } : { maxWidth: 100000, maxHeight: 100000 }),
            expoUi.modifiers.glassEffect({
              glass: { variant: "regular", interactive: true },
              ...shape
            })
          ]
        },
        ...view.children.map((child) => renderExpoUiLeaf(child, expoUi, dependencies, report, options))
      )
    )
  )
}

// A glass Composer remains one typed catalog node. On iOS 26+ the renderer,
// not the app, owns @expo/ui's observable TextField state and explicit send
// button; controlled app resets synchronize through the Scope-bound React
// lifecycle. Android, older iOS, and missing-module hosts use the RN fallback
// in renderComposer with the same intents and accessibility labels.
interface ExpoUiNativeComposerProps {
  readonly view: ComposerView
  readonly expoUi: ExpoUiSwiftUiRuntime & Required<Pick<ExpoUiSwiftUiRuntime, "TextField" | "useNativeState">>
  readonly dependencies: ReactNativeDependencies
  readonly report: IntentReporter
  readonly theme: Theme
  readonly useEffect: NonNullable<ReactRuntime["useEffect"]>
  readonly useState: NonNullable<ReactRuntime["useState"]>
}

interface ExpoUiTextFieldRef {
  readonly clear: () => Promise<void>
}

interface ExpoUiPendingComposerClear {
  current: string | null
}

interface ExpoUiNativeComposerEdit {
  current: string | null
}

// Keep this component at module scope. Defining it inside
// `renderExpoUiComposer` creates a new React component type for every emitted
// Effect Native view. A controlled draft emits after every character, so that
// old shape remounted the SwiftUI TextField and dismissed the iOS keyboard.
const ExpoUiNativeComposer = (props: ExpoUiNativeComposerProps): ReactElementLike => {
  const { view, expoUi, dependencies, report, theme, useEffect, useState } = props
  const controlledValue = composerPlainText(view.doc)
  const textState = expoUi.useNativeState(controlledValue)
  // @expo/ui ObservableState writes from JS are scheduled asynchronously on
  // the UI thread. Keep an immediate, stable JS-side guard as well as a native
  // TextField ref: React state alone leaves the already-committed
  // `onTextChange` closure able to replay the submitted value before the next
  // render, and ObservableState.set("") alone does not synchronously clear the
  // SwiftUI control.
  const [pendingClear] = useState<ExpoUiPendingComposerClear>(() => ({ current: null }))
  // A native edit is already committed inside SwiftUI before onTextChange is
  // delivered. The app then emits the same value back as its controlled draft.
  // Writing that echo through ObservableState.set rebuilds the native field on
  // @expo/ui and drops first responder (the keyboard disappears after every
  // character). Remember the last native edit so its matching controlled
  // emission is acknowledged without writing it back into the focused field.
  const [nativeEdit] = useState<ExpoUiNativeComposerEdit>(() => ({ current: null }))
  const [textFieldRef] = useState<{ current: ExpoUiTextFieldRef | null }>(() => ({ current: null }))
  const clearNativeText = (): void => {
    textState.set("")
    const clear = textFieldRef.current?.clear()
    if (clear !== undefined) {
      // A disappearing native host can reject an in-flight imperative clear.
      // The ObservableState write remains the source-of-truth fallback.
      void clear.catch(() => undefined)
    }
  }
  useEffect(() => {
    const submittedValue = pendingClear.current
    if (submittedValue !== null) {
      if (controlledValue === submittedValue || controlledValue === "") {
        clearNativeText()
        return
      }
      // A distinct controlled value is a real external replacement, not the
      // stale submitted echo. Resume ordinary controlled synchronization.
      pendingClear.current = null
    }
    if (nativeEdit.current === controlledValue) {
      nativeEdit.current = null
      return
    }
    // This is an external controlled replacement rather than the echo of the
    // last focused native edit. It owns the field from here onward.
    nativeEdit.current = null
    if (textState.get() !== controlledValue) {
      textState.set(controlledValue)
    }
  }, [controlledValue, nativeEdit, pendingClear, textFieldRef, textState])
  const submitDisabled = view.disabled === true || view.submitting === true ||
    view.onSubmit === undefined
  const submit = (): void => {
    if (submitDisabled || view.onSubmit === undefined) return
    const value = textState.get()
    runReportedIntent(report, view.onSubmit, value)
    if (view.clearOnSubmit === true) {
      pendingClear.current = value
      clearNativeText()
    }
  }
  return createElement(
    dependencies,
    expoUi.HStack,
    {
      spacing: spacingValue(theme, "2"),
      modifiers: [
        expoUi.modifiers.frame({ minHeight: 44, maxWidth: 100000 }),
        expoUi.modifiers.glassEffect({
          glass: { variant: "regular", interactive: true },
          shape: "capsule"
        })
      ]
    },
    createElement(dependencies, expoUi.TextField, {
      key: "control",
      ref: textFieldRef,
      text: textState,
      placeholder: view.placeholder,
      axis: "vertical",
      accessibilityLabel: view.placeholder,
      onTextChange: (value: string) => {
        const submittedValue = pendingClear.current
        // Native delivery is asynchronous. Ignore both the clear event and a
        // late echo of the just-submitted value; either one would otherwise
        // repopulate the controlled app draft after the field was cleared.
        if (submittedValue !== null && value === submittedValue) {
          // The stale event means SwiftUI is visibly holding the old value;
          // reassert the native clear in the same callback, without waiting
          // for another React/app emission.
          clearNativeText()
          return
        }
        if (submittedValue !== null && value === "") return
        if (view.disabled === true || view.onChange === undefined) return
        if (submittedValue !== null) pendingClear.current = null
        nativeEdit.current = value
        runReportedIntent(report, view.onChange, value)
      },
      modifiers: [expoUi.modifiers.frame({ minHeight: 44, maxWidth: 100000 })]
    }),
    createElement(
      dependencies,
      expoUi.Button,
      {
        key: "submit",
        accessibilityLabel: view.onSubmit === undefined
          ? "Send unavailable"
          : view.submitting === true ? "Message is sending" : "Send message",
        onPress: submit,
        modifiers: [
          expoUi.modifiers.frame({ width: 44, height: 44 }),
          ...(submitDisabled && expoUi.modifiers.disabled !== undefined
            ? [expoUi.modifiers.disabled(true)]
            : [])
        ]
      },
      createElement(dependencies, expoUi.Image, {
        systemName: view.submitting === true ? "ellipsis" : "arrow.up",
        size: 17,
        color: colorValue(theme, submitDisabled ? "textMuted" : "accent")
      })
    )
  )
}

const renderExpoUiComposer = (
  view: ComposerView,
  expoUi: ExpoUiSwiftUiRuntime,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const useNativeState = expoUi.useNativeState
  const TextField = expoUi.TextField
  const useEffect = dependencies.React.useEffect
  const useState = dependencies.React.useState
  if (useNativeState === undefined || TextField === undefined || useEffect === undefined || useState === undefined) {
    throw new Error("@expo/ui composer lowering requires TextField, useNativeState, React.useEffect, and React.useState")
  }
  const theme = options.theme ?? defaultTheme
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, viewStyleWithoutSurface(view, options)),
      testID: `en-composer:${view.mode}`,
      accessibilityState: { disabled: view.disabled === true, busy: view.submitting === true }
    },
    createElement(
      dependencies,
      expoUi.Host,
      { key: "host", style: { flex: 1 } },
      createElement(dependencies, ExpoUiNativeComposer, {
        key: "composer",
        view,
        expoUi: { ...expoUi, TextField, useNativeState },
        dependencies,
        report,
        theme,
        useEffect,
        useState
      })
    )
  )
}

const renderIconButton = (
  view: IconButtonView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  if (view.surface === "glass") {
    const expoUi = glassLoweringRuntime(dependencies, options)
    if (expoUi !== undefined) {
      return renderExpoUiIconButton(view, expoUi, dependencies, report, options)
    }
  }
  const theme = options.theme ?? defaultTheme
  const style = mergeNativeStyles(
    {
      width: 44,
      height: 44,
      borderRadius: 9999,
      alignItems: "center",
      justifyContent: "center",
      opacity: view.disabled === true ? 0.5 : 1,
      // surface "glass" -> translucent theme surface + hairline border (the
      // honest RN-core material approximation; see glassSurfaceStyle);
      // otherwise the plain theme surface.
      ...(view.surface === "glass"
        ? glassSurfaceStyle(theme)
        : { backgroundColor: colorValue(theme, "surface") })
    },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      ...baseProps(view, style),
      testID: `en-icon-button:${view.icon}`,
      accessibilityRole: "button",
      accessibilityLabel: view.accessibilityLabel,
      accessibilityState: { disabled: view.disabled === true },
      disabled: view.disabled === true,
      onPress: () => {
        if (view.disabled !== true) {
          runReportedIntent(report, view.onPress)
        }
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      {
        // RN Text does not inherit color (#71-class bug): the glyph must be
        // explicitly themed or it renders default-black on dark surfaces.
        style: {
          fontSize: iconFontSize.md,
          color: colorValue(theme, "textPrimary")
        }
      },
      iconGlyphs[view.icon]
    )
  )
}

const renderToolbar = (
  view: ToolbarView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  if (view.surface === "glass" && view.children.every(expoUiLowerableChild)) {
    const expoUi = glassLoweringRuntime(dependencies, options)
    if (expoUi !== undefined) {
      return renderExpoUiGlassContainer(view, expoUi, dependencies, report, options)
    }
  }
  const theme = options.theme ?? defaultTheme
  const style = mergeNativeStyles(
    {
      flexDirection: "row",
      alignItems: "center",
      gap: spacingValue(theme, "2"),
      paddingVertical: spacingValue(theme, "2"),
      paddingHorizontal: spacingValue(theme, "3"),
      borderRadius: 9999,
      borderColor: colorValue(theme, "border"),
      borderWidth: 1,
      backgroundColor: view.surface === "glass"
        ? translucentColor(colorValue(theme, "surface"), 0.72)
        : colorValue(theme, "surfaceRaised")
    },
    viewStyle(view, options)
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      testID: `en-toolbar:${view.placement ?? "bottom-floating"}`
    },
    ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
  )
}

const renderMobileSurfaceShell = (
  view: BackgroundGradientView | WallpaperView | SpotlightView | FrameView | BlurredPopupView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  if (view._tag === "BlurredPopup") {
    if (!view.open) {
      return createElement(dependencies, dependencies.ReactNative.View, {
        ...baseProps(view, viewStyle(view as never, options)),
        testID: "en-BlurredPopup-closed"
      })
    }
    return createElement(
      dependencies,
      dependencies.ReactNative.Modal,
      {
        ...baseProps(view, viewStyle(view as never, options)),
        testID: "en-BlurredPopup",
        transparent: true,
        visible: true,
        onRequestClose: () => runReportedIntent(report, view.onDismiss)
      },
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        {
          key: "backdrop",
          testID: "en-BlurredPopup-backdrop",
          onPress: () => runReportedIntent(report, view.onDismiss),
          style: { flex: 1, justifyContent: "center", backgroundColor: "rgba(0,0,0,0.45)" }
        },
        createElement(
          dependencies,
          dependencies.ReactNative.View,
          { key: "panel", testID: "en-BlurredPopup-panel", style: { margin: spacingValue(theme, "4"), padding: spacingValue(theme, "3"), backgroundColor: colorValue(theme, "surface") } },
          ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
        )
      )
    )
  }
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(
      view,
      mergeNativeStyles(
        {
          flexDirection: "column",
          ...(view._tag === "Frame"
            ? { borderWidth: 1, borderColor: colorValue(theme, "accent"), padding: spacingValue(theme, "3") }
            : {}),
          ...(view._tag === "Spotlight"
            ? { shadowColor: colorValue(theme, "accent"), shadowOpacity: 0.45, shadowRadius: 16 }
            : {}),
          ...(view._tag === "BackgroundGradient" || view._tag === "Wallpaper"
            ? { backgroundColor: colorValue(theme, "surface") }
            : {})
        },
        viewStyle(view as never, options)
      )
    ),
    ...view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
  )
}


const renderSwipeableListItem = (
  view: SwipeableListItemView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const actionButtons = (
    side: "leading" | "trailing",
    items: ReadonlyArray<{
      readonly id: string
      readonly label: string
      readonly destructive?: boolean
    }>
  ) =>
    items.map((action) =>
      createElement(
        dependencies,
        dependencies.ReactNative.Pressable,
        {
          key: `${side}-${action.id}`,
          testID: `en-swipe-action:${action.id}`,
          onPress: () => runReportedIntent(report, view.onAction, action.id),
          style: {
            paddingHorizontal: spacingValue(theme, "2"),
            justifyContent: "center",
            backgroundColor: action.destructive === true
              ? colorValue(theme, "danger")
              : colorValue(theme, "surface")
          }
        },
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, action.label)
      )
    )

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(
      view,
      mergeNativeStyles(
        { flexDirection: "row", alignItems: "stretch", gap: spacingValue(theme, "1") },
        viewStyle(view as never, options)
      )
    ),
    ...actionButtons("leading", view.leadingActions ?? []),
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: "body", testID: "en-swipe-body", style: { flex: 1 } },
      renderResolvedReactNativeView(view.child, dependencies, report, options)
    ),
    ...actionButtons("trailing", view.trailingActions ?? [])
  )
}

const renderPager = (
  view: PagerView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const stepIds = view.steps.map((step) => step.id)
  const activeIndex = Math.max(0, stepIds.indexOf(view.activeStepId))
  const canBack = view.canGoBack !== false && activeIndex > 0
  const canAdvance = view.canAdvance !== false && activeIndex < stepIds.length - 1
  const isLast = activeIndex >= stepIds.length - 1
  const progress = view.progress ?? "dots"

  const progressKids: Array<ReactElementLike> = []
  if (progress === "dots") {
    for (const [index, step] of view.steps.entries()) {
      progressKids.push(
        createElement(
          dependencies,
          dependencies.ReactNative.Pressable,
          {
            key: `dot-${step.id}`,
            testID: `en-pager-dot:${step.id}`,
            onPress: () => runReportedIntent(report, view.onStepChange, step.id),
            style: {
              width: 8,
              height: 8,
              borderRadius: 999,
              backgroundColor: index === activeIndex
                ? colorValue(theme, "accent")
                : colorValue(theme, "border")
            }
          }
        )
      )
    }
  }

  const panels = (view.keepMounted === true
    ? view.panels
    : view.panels.filter((panel) => panel.id === view.activeStepId)
  ).map((panel) =>
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        key: `panel-${panel.id}`,
        testID: `en-pager-panel:${panel.id}`,
        style: { display: panel.id === view.activeStepId ? "flex" : "none" }
      },
      renderResolvedReactNativeView(panel.content, dependencies, report, options)
    )
  )

  const back = createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      key: "back",
      testID: "en-pager-back",
      disabled: !canBack,
      ...(canBack
        ? {
            onPress: () => {
              const prev = stepIds[activeIndex - 1]!
              if (view.onBack !== undefined) runReportedIntent(report, view.onBack, prev)
              runReportedIntent(report, view.onStepChange, prev)
            }
          }
        : {}),
      style: { opacity: canBack ? 1 : 0.4 }
    },
    createElement(dependencies, dependencies.ReactNative.Text, { key: "back-label" }, "Back")
  )

  const next = createElement(
    dependencies,
    dependencies.ReactNative.Pressable,
    {
      key: "next",
      testID: "en-pager-next",
      onPress: () => {
        if (isLast) {
          if (view.onComplete !== undefined) {
            runReportedIntent(report, view.onComplete, view.activeStepId)
          }
          return
        }
        if (!canAdvance) return
        const nxt = stepIds[activeIndex + 1]!
        if (view.onAdvance !== undefined) runReportedIntent(report, view.onAdvance, nxt)
        runReportedIntent(report, view.onStepChange, nxt)
      }
    },
    createElement(
      dependencies,
      dependencies.ReactNative.Text,
      { key: "next-label" },
      isLast ? "Done" : "Continue"
    )
  )

  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    baseProps(
      view,
      mergeNativeStyles(
        { flexDirection: "column", gap: spacingValue(theme, "3") },
        viewStyle(view as never, options)
      )
    ),
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        key: "progress",
        testID: "en-pager-progress",
        style: { flexDirection: "row", gap: spacingValue(theme, "2"), justifyContent: "center" }
      },
      ...progressKids
    ),
    ...panels,
    createElement(
      dependencies,
      dependencies.ReactNative.View,
      {
        key: "nav",
        testID: "en-pager-nav",
        style: { flexDirection: "row", justifyContent: "space-between", gap: spacingValue(theme, "2") }
      },
      back,
      next
    )
  )
}

export const renderReactNativeView = (
  view: View,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderRuntimeOptions = {}
): ReactElementLike => {
  const viewport = options.viewport === undefined
    ? undefined
    : makeViewport(options.viewport, options.theme ?? defaultTheme)
  const resolved = resolveView(view, {
    ...(viewport === undefined ? {} : { viewport }),
    platform: options.platform ?? "ios"
  })
  const ownedRuntime = options.runEffect === undefined
    ? makeReactNativeRefOwnedEffectRuntime()
    : undefined
  const element = renderResolvedReactNativeView(
    resolved,
    dependencies,
    withReactNativeHostEffectRuntime(report, options.runEffect ?? ownedRuntime?.runEffect),
    options
  )
  if (ownedRuntime === undefined) {
    return element
  }
  const existingRef = element.props.ref
  return dependencies.React.createElement(element.type, {
    ...element.props,
    key: element.key,
    ref: (instance: unknown) => {
      if (typeof existingRef === "function") {
        existingRef(instance)
      }
      if (instance === null) {
        ownedRuntime.dispose()
      }
    }
  })
}

const normalizeChildren = (children: unknown): ReadonlyArray<ReactNodeLike> => {
  if (children === undefined || children === null) {
    return []
  }
  return Array.isArray(children) ? children as ReadonlyArray<ReactNodeLike> : [children as ReactNodeLike]
}

const textContent = (node: ReactNodeLike): string => {
  if (node === undefined || node === null || typeof node === "boolean") {
    return ""
  }
  if (typeof node === "string" || typeof node === "number") {
    return String(node)
  }
  return normalizeChildren(node.props.children).map(textContent).join("")
}

export const reactNativeStructure = (node: ReactNodeLike): ReactNativeStructure | undefined => {
  if (node === undefined || node === null || typeof node !== "object" || !("props" in node)) {
    return undefined
  }

  const metadata = parseNativeId(node.props.nativeID)
  if (metadata === undefined) {
    for (const child of normalizeChildren(node.props.children)) {
      const found = reactNativeStructure(child)
      if (found !== undefined) {
        return found
      }
    }
    return undefined
  }

  const children =
    metadata.tag === "List"
      ? ((node.props.data as ReadonlyArray<View> | undefined) ?? [])
          .map((item) => {
            const renderItem = node.props.renderItem as ((input: { readonly item: View }) => ReactNodeLike) | undefined
            return renderItem === undefined ? undefined : reactNativeStructure(renderItem({ item }))
          })
          .filter((child): child is ReactNativeStructure => child !== undefined)
      : metadata.tag === "SectionList"
        ? ((node.props.sections as ReadonlyArray<{
            readonly header: View
            readonly data: ReadonlyArray<View>
          }> | undefined) ?? []).flatMap((section) => {
            const renderItem = node.props.renderItem as
              | ((input: { readonly item: View }) => ReactNodeLike)
              | undefined
            const renderSectionHeader = node.props.renderSectionHeader as
              | ((input: { readonly section: { readonly header: View } }) => ReactNodeLike)
              | undefined
            const header = renderSectionHeader === undefined
              ? undefined
              : reactNativeStructure(renderSectionHeader({ section }))
            const items = section.data
              .map((item) => renderItem === undefined ? undefined : reactNativeStructure(renderItem({ item })))
              .filter((child): child is ReactNativeStructure => child !== undefined)
            return header === undefined ? items : [header, ...items]
          })
        : normalizeChildren(node.props.children)
            .map((child) => reactNativeStructure(child))
            .filter((child): child is ReactNativeStructure => child !== undefined)

  return {
    tag: metadata.tag,
    ...(metadata.key === undefined ? {} : { key: metadata.key }),
    ...(metadata.tag === "Text" || metadata.tag === "Button" ? { text: textContent(node) } : {}),
    ...(children.length === 0 ? {} : { children })
  }
}

export const viewStructure = (view: View): ReactNativeStructure => {
  switch (view._tag) {
    case "Stack":
      return {
        tag: "Stack",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    case "Text":
      return {
        tag: "Text",
        ...(view.key === undefined ? {} : { key: view.key }),
        text: String(view.content)
      }
    case "Button":
      return {
        tag: "Button",
        ...(view.key === undefined ? {} : { key: view.key }),
        text: view.label
      }
    case "Link":
      return {
        tag: "Link",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    case "Modal":
      return {
        tag: "Modal",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    case "Sheet":
      return {
        tag: "Sheet",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    case "List":
      return {
        tag: "List",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.items.map(viewStructure)
      }
    case "SectionList":
      return {
        tag: "SectionList",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.sections.flatMap((section) => [
          viewStructure(section.header),
          ...section.items.map(viewStructure)
        ])
      }
    case "Card":
      return {
        tag: "Card",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    case "SplitPane":
      return {
        tag: "SplitPane",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.panes.map((pane) => viewStructure(pane.content))
      }
    case "Workbench":
      return {
        tag: "Workbench",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: (view.keepMounted === true
          ? view.panes
          : view.panes.filter((pane) => pane.id === view.activePaneId)
        ).map((pane) => viewStructure(pane.content))
      }
    case "Tooltip":
      return {
        tag: "Tooltip",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    case "CommandPalette":
      return {
        tag: "CommandPalette",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: [viewStructure(view.combobox)]
      }
    case "FieldRow":
      return {
        tag: "FieldRow",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: [viewStructure(view.control)]
      }
    case "Transcript":
      return {
        tag: "Transcript",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.messages.flatMap((message) => message.body.map(viewStructure))
      }
    case "Tabs":
      return {
        tag: "Tabs",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: (view.keepMounted === true
          ? view.panels
          : view.panels.filter((panel) => panel.id === view.selectedId)
        ).map((panel) => viewStructure(panel.content))
      }
    case "Pager":
      return {
        tag: "Pager",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: (view.keepMounted === true
          ? view.panels
          : view.panels.filter((panel) => panel.id === view.activeStepId)
        ).map((panel) => viewStructure(panel.content))
      }
    case "SwipeableListItem":
      return {
        tag: "SwipeableListItem",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: [viewStructure(view.child)]
      }
    case "Toolbar":
      return {
        tag: "Toolbar",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.children.map(viewStructure)
      }
    default:
      return {
        tag: view._tag,
        ...(view.key === undefined ? {} : { key: view.key })
      }
  }
}

export const createEffectNativeSurface = (
  dependencies: ReactNativeDependencies
): ((props: EffectNativeSurfaceProps) => ReactNodeLike) => {
  const { React } = dependencies
  const useEffect = React.useEffect
  const useState = React.useState
  if (useEffect === undefined || useState === undefined) {
    throw new Error("EffectNativeSurface requires React useEffect and useState")
  }

  return function EffectNativeSurfaceWithDependencies(props: EffectNativeSurfaceProps): ReactNodeLike {
    const [view, setView] = useState<View | undefined>(() => props.initialView)
    const [runtimeSlot] = useState<{ runEffect: ReactNativeHostEffectRuntime | undefined }>(() => ({
      runEffect: undefined
    }))
    // One Scope-equivalent host runtime per surface component instance:
    // driver instances mount on first appearance, update per emission, and
    // unmount on React unmount (the component's lifecycle IS the surface
    // scope on this entrypoint).
    const [hostRuntime] = useState<ReactNativeHostRuntime | undefined>(() =>
      props.hostDrivers === undefined || props.hostDrivers.length === 0
        ? undefined
        : makeReactNativeHostRuntime(props.hostDrivers)
    )

    useEffect(() => {
      let ownedRunEffect: ReactNativeHostEffectRuntime | undefined
      const interrupt = Effect.runCallback(
        Effect.scoped(
          Effect.gen(function*() {
            const runFiber = yield* FiberSet.makeRuntime<never, void, never>()
            const runEffect: ReactNativeHostEffectRuntime = (effect) => {
              runFiber(effect)
            }
            yield* Effect.sync(() => {
              ownedRunEffect = runEffect
              runtimeSlot.runEffect = runEffect
            })
            yield* props.viewStream.pipe(
              Stream.runForEach((nextView) =>
                Effect.sync(() => {
                  setView(nextView)
                })
              ),
              Effect.ignoreCause
            )
            return yield* Effect.never
          })
        ).pipe(
          Effect.ignoreCause,
          Effect.ensuring(Effect.sync(() => {
            if (runtimeSlot.runEffect === ownedRunEffect) {
              runtimeSlot.runEffect = undefined
            }
          }))
        )
      )

      return () => {
        if (runtimeSlot.runEffect === ownedRunEffect) {
          runtimeSlot.runEffect = undefined
        }
        interrupt()
      }
    }, [props.viewStream])

    useEffect(() => () => {
      hostRuntime?.dispose()
    }, [hostRuntime])

    const renderOptions: ReactNativeRenderRuntimeOptions = {
      ...(props.theme === undefined ? {} : { theme: props.theme }),
      ...(props.platform === undefined ? {} : { platform: props.platform }),
      ...(props.viewport === undefined ? {} : { viewport: props.viewport }),
      ...(props.hostDrivers === undefined ? {} : { hostDrivers: props.hostDrivers }),
      ...(hostRuntime === undefined ? {} : { hostRuntime }),
      ...(props.expoUi === undefined ? {} : { expoUi: props.expoUi }),
      runEffect: (effect) => runtimeSlot.runEffect?.(effect)
    }

    if (view === undefined) {
      return null
    }
    const element = renderReactNativeView(view, dependencies, props.report, renderOptions)
    // Instances whose Host node left the tree this pass unmount now.
    hostRuntime?.sweep()
    return element
  }
}

export const EffectNativeSurface = (props: EffectNativeSurfaceProps): ReactNodeLike => {
  const dependencies = loadPeerDependencies()
  return dependencies.React.createElement(
    createEffectNativeSurface(dependencies),
    props as unknown as Record<string, unknown>
  )
}

export const makeReactNativeRenderer = (
  options: ReactNativeRendererOptions = {}
): RendererAdapter<ReactNativeContainer | undefined, ReactNativeMountedSurface> => ({
  mount: (container, viewStream, report) =>
    Effect.gen(function*() {
      const parentScope = yield* Scope.Scope
      const surfaceScope = yield* Scope.fork(parentScope)

      return yield* Scope.provide(surfaceScope)(Effect.gen(function*() {
        const dependencies = options.dependencies ?? loadPeerDependencies()
        // Host-driver instances are Scope-owned (issue #70): mounted on first
        // Host appearance, swept when the node leaves the tree, and all
        // unmounted when the surface scope closes.
        const hostRuntime = options.hostDrivers === undefined || options.hostDrivers.length === 0
          ? undefined
          : makeReactNativeHostRuntime(options.hostDrivers)
        if (hostRuntime !== undefined) {
          yield* Effect.addFinalizer(() =>
            Effect.sync(() => {
              hostRuntime.dispose()
            })
          )
        }
        const renderOptions: ReactNativeRenderOptions = hostRuntime === undefined
          ? options
          : { ...options, hostRuntime }
        const viewport = yield* makeViewportService(
          options.viewport ?? readReactNativeViewport(dependencies),
          options.theme === undefined ? {} : { theme: options.theme }
        )
        const runFiber = yield* FiberSet.makeRuntime<never, void, never>()
        const runEffect: ReactNativeHostEffectRuntime = (effect) => {
          runFiber(effect)
        }
        const scopedReport = withReactNativeHostEffectRuntime(report, runEffect)
        const current = yield* Ref.make<View | undefined>(undefined)
        const currentElement = yield* Ref.make<ReactNodeLike | undefined>(undefined)
        const ready = yield* Deferred.make<void>()
        const dimensions = dependencies.ReactNative.Dimensions
        const resolvedViewStream = viewStream.pipe(
          Stream.zipLatestWith(viewport.stream, (view, currentViewport) =>
            resolveView(view, {
              viewport: currentViewport,
              platform: options.platform ?? "ios"
            })
          )
        )

        yield* Effect.addFinalizer(() =>
          Effect.sync(() => {
            container?.render?.(undefined)
          })
        )
        if (dimensions?.addEventListener !== undefined) {
          const updateViewport = (event: { readonly window?: ReactNativeDimensionMetrics }) => {
            const metrics = event.window ?? dimensions.get("window")
            runEffect(
              viewport.set({
                width: metrics.width,
                height: metrics.height
              }).pipe(Effect.ignoreCause)
            )
          }
          const subscription = dimensions.addEventListener("change", updateViewport)
          yield* Effect.addFinalizer(() =>
            Effect.sync(() => {
              if (typeof subscription === "function") {
                subscription()
              } else {
                subscription.remove()
              }
            })
          )
        }

        yield* resolvedViewStream.pipe(
          Stream.runForEach((view) =>
            Effect.gen(function*() {
              const element = renderResolvedReactNativeView(view, dependencies, scopedReport, renderOptions)
              hostRuntime?.sweep()
              yield* Ref.set(current, view)
              yield* Ref.set(currentElement, element)
              yield* Effect.sync(() => {
                container?.render?.(element)
              })
              yield* Deferred.succeed(ready, undefined)
            })
          ),
          Effect.forkScoped
        )
        yield* Deferred.await(ready)

        return {
          unmount: Scope.close(surfaceScope, Exit.void),
          current: Ref.get(current),
          currentElement: Ref.get(currentElement),
          serialize: Ref.get(currentElement).pipe(Effect.map((element) => reactNativeStructure(element))),
          currentViewport: viewport.current,
          setViewport: viewport.set
        }
      }))
    })
})
