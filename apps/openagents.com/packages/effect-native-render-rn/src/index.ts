import { Deferred, Effect, Exit, Fiber, Ref, Scope, Stream } from "effect"
import {
  type ButtonView,
  type CardView,
  type ColorToken,
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
  type PlatformVariant,
  type RendererAdapter,
  type SectionListView,
  type SheetView,
  type SpacerView,
  type StackView,
  type TextFieldView,
  type TextView,
  type View,
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

const baseProps = (view: View, style: ReactNativeStyle): Record<string, unknown> => ({
  key: view.key,
  nativeID: nativeId(view),
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
