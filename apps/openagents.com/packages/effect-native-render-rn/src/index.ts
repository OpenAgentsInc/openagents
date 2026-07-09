import { Deferred, Effect, Exit, Fiber, Ref, Scope, Stream } from "effect"
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
  void Effect.runPromise(report(ref, runtimeValue) as Effect.Effect<void, IntentError>).catch(() => {
    // Intent failures are recorded by the registry; host event handlers stay total.
  })
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
    baseProps(view, style),
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

const renderButton = (
  view: ButtonView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const style = mergeNativeStyles(
    { opacity: view.disabled === true ? 0.5 : 1 },
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
        style: typeScaleValue(options.theme ?? defaultTheme, "label")
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
      onChangeText: (value: string) => {
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
  options: ReactNativeRenderOptions
): Record<string, unknown> => {
  const itemLength = estimatedItemLength(view, options)
  return {
    ...(view.onEndReached === undefined
      ? {}
      : {
          onEndReached: () => runReportedIntent(report, view.onEndReached!),
          onEndReachedThreshold: view.endReachedThreshold ?? 0.5
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
      ...nativeCollectionProps(view, report, options)
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
      ...nativeCollectionProps(view, report, options)
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

// Foreign-host escape hatch on React Native (issue #23). The React Native
// renderer ships no host drivers: Monaco, xterm, and canvas are DOM/webview
// widgets with no faithful RN host mapping. Rather than silently no-op, every
// Host kind renders a loud unsupported marker (testID + accessibilityLabel) so
// the conformance suite fails visibly for any host kind on this renderer.
const renderHost = (
  view: HostView,
  dependencies: ReactNativeDependencies,
  options: ReactNativeRenderOptions
): ReactElementLike =>
  createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, viewStyle(view, options)),
      testID: `en-host-unsupported:${view.kind}`,
      accessibilityLabel: `Unsupported host kind on React Native: ${view.kind}`
    }
  )

// Icon on React Native (issue #31). The closed IconName set is the contract;
// RN renders each glyph from a bounded font-glyph registry (sized from tokens,
// token-driven color). Decorative vs meaningful is honored via accessibility
// props. No raw SVG/markup enters the tree.
const iconGlyphs: Record<IconName, string> = {
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
  ChevronRight: "›"
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
  const style = mergeNativeStyles(
    orientation === "vertical"
      ? { width: 1, alignSelf: "stretch", backgroundColor: colorValue(theme, "border") }
      : { height: 1, backgroundColor: colorValue(theme, "border") },
    viewStyle(view, options)
  )
  return createElement(dependencies, dependencies.ReactNative.View, {
    ...baseProps(view, style),
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

// App shell components (issue #27) on React Native. Divider drag-to-resize has
// no faithful RN host mapping and is declared unsupported (dividers render as
// static separators, sizes are honored). NavRail maps to a stacked selectable
// list; Workbench renders the active pane (hidden-but-mounted when keepMounted).
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
      children.push(
        createElement(dependencies, dependencies.ReactNative.View, {
          key: `divider-${index}`,
          testID: "en-split-divider",
          accessibilityRole: "none",
          style: {
            [sizeField]: 1,
            backgroundColor: colorValue(theme, "border")
          }
        })
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
      const active = view.activeId === item.id
      const itemChildren: Array<ReactElementLike> = []
      if (item.icon !== undefined) {
        itemChildren.push(
          createElement(dependencies, dependencies.ReactNative.Text, { key: "icon" }, iconGlyphs[item.icon])
        )
      }
      itemChildren.push(
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, item.label)
      )
      parts.push(
        createElement(
          dependencies,
          dependencies.ReactNative.Pressable,
          {
            key: `item-${item.id}`,
            testID: `en-nav-item:${item.id}`,
            accessibilityRole: "menuitem",
            accessibilityState: { selected: active, disabled: item.disabled === true },
            disabled: item.disabled === true,
            style: { flexDirection: "row", gap: spacingValue(theme, "2") },
            ...(item.disabled === true
              ? {}
              : { onPress: () => runReportedIntent(report, view.onSelect, item.id) })
          },
          ...itemChildren
        )
      )
    }
    return createElement(
      dependencies,
      dependencies.ReactNative.View,
      { key: `section-${section.id}` },
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

// Anchored overlay family (issue #28) on React Native. Placement/positioning
// and collision have no faithful RN mapping and are declared unsupported
// (recorded via testID). Menus render as pressable rows with a typed onSelect;
// Tooltip maps to an accessibilityHint on its single target (no hover surface).
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
    const parts: Array<ReactElementLike> = []
    if (item.icon !== undefined) {
      parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "icon" }, iconGlyphs[item.icon]))
    }
    parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, item.label))
    if (item.keybinding !== undefined) {
      parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "kbd" }, item.keybinding))
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
          paddingLeft: spacingValue(theme, "2") * (depth + 1),
          ...(item.danger === true ? { } : {})
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
  const style = mergeNativeStyles({ display: open ? "flex" : "none" }, viewStyle(view, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, style),
      testID: `en-popover:${view.placement.side}:${view.placement.align}`,
      accessibilityRole: "none"
    },
    ...(open ? view.children.map((child) => renderResolvedReactNativeView(child, dependencies, report, options)) : [])
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
  const style = mergeNativeStyles({ display: open ? "flex" : "none", flexDirection: "column" }, viewStyle(view, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, style), testID: `en-dropdown-menu:${view.placement.side}:${view.placement.align}`, accessibilityRole: "menu" },
    ...(open ? renderMenuRows(view.items, 0, dependencies, theme, view.onSelect, view.onDismiss, report) : [])
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
  const style = mergeNativeStyles({ display: open ? "flex" : "none", flexDirection: "column" }, viewStyle(view, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, style), testID: `en-context-menu:${view.x}:${view.y}`, accessibilityRole: "menu" },
    ...(open ? renderMenuRows(view.items, 0, dependencies, theme, view.onSelect, view.onDismiss, report) : [])
  )
}

const renderTooltip = (
  view: TooltipView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const target = renderResolvedReactNativeView(view.children[0]!, dependencies, report, options)
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, viewStyle(view, options)), testID: "en-tooltip", accessibilityHint: view.content },
    target
  )
}

// Command palette + Combobox (issue #29) on React Native. Filtering stays
// app-supplied. TextInput + pressable options map faithfully; roving
// aria-activedescendant has no RN equivalent (highlight is reflected via
// accessibilityState.selected). CommandPalette renders its combobox inside a
// modal-styled container when open.
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
      accessibilityRole: "menuitem",
      accessibilityState: { selected: view.highlightedId === option.id, disabled: option.disabled === true },
      disabled: option.disabled === true,
      style: { flexDirection: "row", gap: spacingValue(theme, "2") },
      ...(option.disabled === true ? {} : { onPress: () => runReportedIntent(report, view.onSelect, option.id) })
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
    placeholder: view.placeholder,
    value: view.query,
    ...(view.onQueryChange === undefined
      ? {}
      : { onChangeText: (value: string) => runReportedIntent(report, view.onQueryChange!, value) })
  })
  const listChildren: Array<ReactElementLike> = []
  if (view.options.length === 0) {
    listChildren.push(
      createElement(
        dependencies,
        dependencies.ReactNative.Text,
        { key: "empty", testID: "en-combobox-empty", accessibilityRole: "text" },
        view.loading === true ? "" : (view.emptyLabel ?? "No results")
      )
    )
  } else {
    let currentGroup: string | undefined
    let started = false
    for (const option of view.options) {
      if (!started || option.group !== currentGroup) {
        currentGroup = option.group
        started = true
        if (option.group !== undefined) {
          listChildren.push(
            createElement(
              dependencies,
              dependencies.ReactNative.Text,
              { key: `group-${option.group}`, testID: `en-combobox-group:${option.group}` },
              option.group
            )
          )
        }
      }
      listChildren.push(renderComboboxOption(option, view, dependencies, theme, report))
    }
  }
  const listbox = createElement(
    dependencies,
    dependencies.ReactNative.View,
    { key: "listbox", accessibilityRole: "list", ...(view.loading === true ? { "aria-busy": true } : {}) },
    ...listChildren
  )
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), accessibilityRole: "none" },
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
  const children: Array<ReactElementLike> = []
  if (view.title !== undefined) {
    children.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "title" }, view.title))
  }
  if (open) children.push(renderCombobox(view.combobox, dependencies, report, options))
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      ...baseProps(view, mergeNativeStyles({ display: open ? "flex" : "none", flexDirection: "column" }, {})),
      testID: "en-command-palette",
      accessibilityViewIsModal: true,
      accessibilityRole: "none"
    },
    ...children
  )
}

// Tabs (issue #30) on React Native — a segmented tab bar of pressable tabs
// plus the active panel. Roving-tabindex/arrow-key nav has no RN mapping (touch
// selection only); vertical orientation is honored via layout direction.
const renderTabs = (
  view: TabsView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const orientation = view.orientation ?? "horizontal"
  const tabBar = createElement(
    dependencies,
    dependencies.ReactNative.View,
    {
      key: "tablist",
      accessibilityRole: "tablist",
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
// mention chips are declared unsupported and render as their label text).
// Enter submit-vs-newline and history nav map to onSubmitEditing / typed key
// commands; the autocomplete combobox renders below.
const renderComposer = (
  view: ComposerView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const input = createElement(dependencies, dependencies.ReactNative.TextInput, {
    key: "control",
    testID: "en-composer-input",
    accessibilityLabel: view.placeholder,
    multiline: true,
    placeholder: view.placeholder,
    value: composerPlainText(view.doc),
    ...(view.onChange === undefined ? {} : { onChangeText: (value: string) => runReportedIntent(report, view.onChange!, value) }),
    onSubmitEditing: (event: { readonly nativeEvent?: { readonly text?: string } }) => {
      if (view.onKeyCommand !== undefined) runReportedIntent(report, view.onKeyCommand, "submit")
      if (view.onSubmit !== undefined) runReportedIntent(report, view.onSubmit, event.nativeEvent?.text ?? composerPlainText(view.doc))
    }
  })
  const children: Array<ReactElementLike> = [input]
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
  options: ReactNativeRenderOptions
): ReactElementLike =>
  // No RN core Slider; expose the value/range as an accessible adjustable and
  // reflect the current value. Drag-to-change is declared unsupported (a
  // community Slider lib is an app-level swap).
  createElement(dependencies, dependencies.ReactNative.View, {
    ...baseProps(view, viewStyle(view, options)),
    testID: "en-slider",
    accessibilityRole: "adjustable",
    accessibilityLabel: view.label,
    accessibilityValue: { min: view.min, max: view.max, now: view.value }
  })

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

const renderToast = (
  view: ToastView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
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
): ReactElementLike =>
  createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), testID: "en-transcript", accessibilityLiveRegion: "polite" },
    ...view.messages.map((message) =>
      createElement(
        dependencies,
        dependencies.ReactNative.View,
        {
          key: `message-${message.key}`,
          testID: `en-message:${message.key}`,
          nativeID: `effect-native-message:${message.role}`,
          ...(message.status === undefined ? {} : { accessibilityState: { busy: message.status === "streaming" || message.status === "thinking" } })
        },
        ...message.body.map((child) => renderResolvedReactNativeView(child, dependencies, report, options))
      ))
  )

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

// GraphFigure + Timeline (issue #37) on React Native — a read-only subset. RN
// has no core SVG/canvas, so the graph renders as a selectable node list with
// status colors (edges + pan/zoom declared unsupported); Timeline maps to a
// list of status-tagged rows.
const renderGraphFigure = (
  view: GraphFigureView,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions
): ReactElementLike => {
  const theme = options.theme ?? defaultTheme
  const statusColor = (status: GraphStatus | undefined) => colorValue(theme, graphStatusColorToken[status ?? "idle"])
  return createElement(
    dependencies,
    dependencies.ReactNative.View,
    { ...baseProps(view, mergeNativeStyles({ flexDirection: "column" }, viewStyle(view, options))), testID: "en-graph-figure", accessibilityLabel: view.a11y?.label },
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
        createElement(dependencies, dependencies.ReactNative.View, { key: "dot", style: { width: 8, height: 8, borderRadius: 999, backgroundColor: statusColor(graphEvent.status) } }),
        createElement(dependencies, dependencies.ReactNative.Text, { key: "label" }, graphEvent.label)
      ]
      if (graphEvent.time !== undefined) parts.push(createElement(dependencies, dependencies.ReactNative.Text, { key: "time", style: { color: colorValue(theme, "textMuted") } }, graphEvent.time))
      const props: Record<string, unknown> = { key: `event-${graphEvent.id}`, testID: `en-timeline-event:${graphEvent.id}`, style: { flexDirection: "row", gap: spacingValue(theme, "2") } }
      if (view.onEventSelect !== undefined) {
        const onEventSelect = view.onEventSelect
        return createElement(dependencies, dependencies.ReactNative.Pressable, { ...props, onPress: () => runReportedIntent(report, onEventSelect, graphEvent.id) }, ...parts)
      }
      return createElement(dependencies, dependencies.ReactNative.View, props, ...parts)
    })
  )
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
      return renderHost(view, dependencies, options)
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
      return renderSlider(view, dependencies, options)
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
  }
}

export const renderReactNativeView = (
  view: View,
  dependencies: ReactNativeDependencies,
  report: IntentReporter,
  options: ReactNativeRenderOptions = {}
): ReactElementLike => {
  const viewport = options.viewport === undefined
    ? undefined
    : makeViewport(options.viewport, options.theme ?? defaultTheme)
  const resolved = resolveView(view, {
    ...(viewport === undefined ? {} : { viewport }),
    platform: options.platform ?? "ios"
  })
  return renderResolvedReactNativeView(resolved, dependencies, report, options)
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

    useEffect(() => {
      const fiber = Effect.runFork(
        props.viewStream.pipe(
          Stream.runForEach((nextView) =>
            Effect.sync(() => {
              setView(nextView)
            })
          )
        )
      )

      return () => {
        void Effect.runPromise(Fiber.interrupt(fiber)).catch(() => {
          // React unmount cleanup must stay total.
        })
      }
    }, [props.viewStream])

    const renderOptions: ReactNativeRenderOptions = {
      ...(props.theme === undefined ? {} : { theme: props.theme }),
      ...(props.platform === undefined ? {} : { platform: props.platform }),
      ...(props.viewport === undefined ? {} : { viewport: props.viewport })
    }

    return view === undefined
      ? null
      : renderReactNativeView(view, dependencies, props.report, renderOptions)
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
        const viewport = yield* makeViewportService(
          options.viewport ?? readReactNativeViewport(dependencies),
          options.theme === undefined ? {} : { theme: options.theme }
        )
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
            void Effect.runPromise(viewport.set({
              width: metrics.width,
              height: metrics.height
            })).catch(() => {
              // Host dimension callbacks must stay total.
            })
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
              const element = renderResolvedReactNativeView(view, dependencies, report, options)
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
