import { Deferred, Effect, Exit, Layer, Scope, Stream } from "effect"
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
  NavigationHandler,
  type NavigationDestination,
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
  makeNavigateIntent,
  makeViewportService,
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

export const packageName = "@effect-native/render-dom" as const

export interface DomRendererOptions {
  readonly document?: Document
  readonly theme?: Theme
  readonly viewport?: ViewportInput
}

export interface DomMountedSurface extends MountedSurface {
  readonly root: HTMLElement
  readonly stylesheet: HTMLStyleElement
  readonly serialize: Effect.Effect<DomStructure | undefined>
  readonly stylesheetText: Effect.Effect<string>
  readonly setTheme: (theme: Theme) => Effect.Effect<void>
  readonly currentViewport: Effect.Effect<Viewport>
  readonly setViewport: (input: ViewportInput) => Effect.Effect<void>
}

export interface DomStructure {
  readonly tag: View["_tag"]
  readonly key?: string
  readonly text?: string
  readonly children?: ReadonlyArray<DomStructure>
}

export interface DomNavigationHandlerOptions {
  readonly document?: Document
}

const navigationDocument = (options: DomNavigationHandlerOptions = {}): Document =>
  options.document ?? globalThis.document

export const makeDomNavigationHandler = (
  options: DomNavigationHandlerOptions = {}
): NavigationHandler => ({
  navigate: (destination: NavigationDestination) =>
    Effect.sync(() => {
      const document = navigationDocument(options)
      const window = document.defaultView ?? globalThis.window
      switch (destination.kind) {
        case "url": {
          if (destination.target === "blank") {
            window.open(destination.href, "_blank", "noopener,noreferrer")
          } else {
            window.location.assign(destination.href)
          }
          return
        }
        case "path": {
          if (destination.replace === true) {
            window.history.replaceState(null, "", destination.path)
          } else {
            window.history.pushState(null, "", destination.path)
          }
          return
        }
        case "anchor": {
          window.location.hash = destination.id
          const target = document.getElementById(destination.id)
          if (target !== null && typeof target.scrollIntoView === "function") {
            target.scrollIntoView()
          }
          return
        }
      }
    })
})

export const makeDomNavigationHandlerLayer = (
  options: DomNavigationHandlerOptions = {}
) => Layer.succeed(NavigationHandler, makeDomNavigationHandler(options))

type EventCleanup = () => void

const kebab = (key: string): string => key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)

const cssEscape = (value: string): string => {
  if (globalThis.CSS?.escape !== undefined) {
    return globalThis.CSS.escape(value)
  }
  return value.replace(/[^a-zA-Z0-9_-]/g, "_")
}

const px = (value: number): string => `${value}px`

const spacingValue = (token: SpacingToken): string => `var(--en-spacing-${cssEscape(token)})`
const colorValue = (token: ColorToken): string => `var(--en-color-${cssEscape(token)})`
const radiusValue = (token: RadiusToken): string => `var(--en-radius-${cssEscape(token)})`
const dimensionValue = (value: Dimension): string =>
  typeof value === "number" ? px(value) : `var(--en-dimension-${cssEscape(value as DimensionToken)})`
const typeScaleValue = (token: TypeScaleToken, field: "fontSize" | "lineHeight" | "fontWeight"): string =>
  `var(--en-type-${cssEscape(token)}-${field})`

const fontWeightValue = (weight: string): string => {
  switch (weight) {
    case "regular":
      return "400"
    case "medium":
      return "500"
    case "semibold":
      return "600"
    case "bold":
      return "700"
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

const focusableSelector = [
  "button:not([disabled])",
  "a[href]",
  "input:not([disabled])",
  "textarea:not([disabled])",
  "select:not([disabled])",
  '[tabindex]:not([tabindex="-1"])'
].join(",")

const focusableElements = (root: HTMLElement): ReadonlyArray<HTMLElement> =>
  Array.from(root.querySelectorAll(focusableSelector)) as ReadonlyArray<HTMLElement>

const readDomViewport = (document: Document): ViewportInput => {
  const window = document.defaultView
  return {
    width: window?.innerWidth ?? defaultViewportInput.width,
    height: window?.innerHeight ?? defaultViewportInput.height
  }
}

const styleDeclarations = (key: string, value: unknown): ReadonlyArray<readonly [string, string]> => {
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
      return [[kebab(key), spacingValue(value as SpacingToken)]]
    case "width":
    case "height":
    case "minWidth":
    case "minHeight":
    case "maxWidth":
    case "maxHeight":
      return [[kebab(key), dimensionValue(value as Dimension)]]
    case "flex":
    case "opacity":
    case "borderWidth":
      return [[kebab(key), String(value)]]
    case "alignSelf":
      return [["align-self", flexKeyword(String(value))]]
    case "backgroundColor":
    case "borderColor":
    case "color":
      return [[kebab(key), colorValue(value as ColorToken)]]
    case "borderRadius":
      return [["border-radius", radiusValue(value as RadiusToken)]]
    case "fontWeight":
      return [["font-weight", fontWeightValue(String(value))]]
    case "textAlign":
      return [["text-align", String(value)]]
    case "typeScale":
      return [
        ["font-size", typeScaleValue(value as TypeScaleToken, "fontSize")],
        ["line-height", typeScaleValue(value as TypeScaleToken, "lineHeight")],
        ["font-weight", typeScaleValue(value as TypeScaleToken, "fontWeight")]
      ]
    default:
      return []
  }
}

class AtomicStyleSheet {
  readonly element: HTMLStyleElement
  #theme: Theme
  #rules = new Map<string, string>()
  #used = new Set<string>()
  #nextId = 0

  constructor(document: Document, theme: Theme) {
    this.#theme = theme
    this.element = document.createElement("style")
    this.element.setAttribute("data-effect-native", "dom")
    document.head.appendChild(this.element)
    this.flush()
  }

  setTheme(theme: Theme): void {
    this.#theme = theme
    this.flush()
  }

  beginRender(): void {
    this.#used = new Set()
  }

  classFor(key: string, value: unknown): string {
    const declarationKey = `${key}:${JSON.stringify(value)}`
    let className = this.#rules.get(declarationKey)
    if (className === undefined) {
      className = `en-${this.#nextId.toString(36)}`
      this.#nextId += 1
      this.#rules.set(declarationKey, className)
    }
    this.#used.add(declarationKey)
    return className
  }

  apply(element: HTMLElement, style: FlatStyle | undefined): void {
    const declarations = new Map<string, string>()
    if (style !== undefined) {
      for (const [key, value] of Object.entries(style)) {
        for (const [property, cssValue] of styleDeclarations(key, value)) {
          declarations.set(property, cssValue)
        }
      }
    }

    const classes = Array.from(declarations.entries()).map(([property, value]) =>
      this.classFor(property, value)
    )
    const existing = Array.from(element.classList).filter((className) => !className.startsWith("en-"))
    element.className = [...existing, ...classes].join(" ")
  }

  flush(): void {
    const themeRules = [
      ":root{",
      ...Object.entries(this.#theme.spacing).map(([key, value]) => `--en-spacing-${cssEscape(key)}:${px(value)};`),
      ...Object.entries(this.#theme.color).map(([key, value]) => `--en-color-${cssEscape(key)}:${value};`),
      ...Object.entries(this.#theme.radius).map(([key, value]) => `--en-radius-${cssEscape(key)}:${px(value)};`),
      ...Object.entries(this.#theme.dimension).map(([key, value]) =>
        `--en-dimension-${cssEscape(key)}:${typeof value === "number" ? px(value) : value};`
      ),
      ...Object.entries(this.#theme.typeScale).flatMap(([key, value]) => [
        `--en-type-${cssEscape(key)}-fontSize:${px(value.fontSize)};`,
        `--en-type-${cssEscape(key)}-lineHeight:${px(value.lineHeight)};`,
        `--en-type-${cssEscape(key)}-fontWeight:${value.fontWeight};`
      ]),
      "}"
    ].join("")
    const atomicRules = Array.from(this.#rules.entries())
      .filter(([key]) => this.#used.has(key))
      .map(([key, className]) => {
        const separator = key.indexOf(":")
        const property = key.slice(0, separator)
        const value = JSON.parse(key.slice(separator + 1)) as string
        return `.${className}{${property}:${value};}`
      })
      .join("")
    this.element.textContent = `${themeRules}${atomicRules}`
  }

  dispose(): void {
    this.element.remove()
    this.#rules.clear()
    this.#used.clear()
  }
}

class DomRendererState {
  readonly root: HTMLElement
  readonly styles: AtomicStyleSheet
  readonly keyed = new Map<string, HTMLElement>()
  readonly listeners = new WeakMap<EventTarget, Array<EventCleanup>>()
  readonly allListeners = new Set<EventCleanup>()
  theme: Theme
  focusRequest: HTMLElement | undefined
  overlayOpen = false
  overlayRestoreFocus: HTMLElement | undefined
  overlayBodyOverflow: string | undefined
  readonly endReachedSignatures = new Map<string, string>()

  constructor(container: Element, document: Document, theme: Theme) {
    this.theme = theme
    this.root = document.createElement("div")
    this.root.setAttribute("data-effect-native-surface", "dom")
    container.appendChild(this.root)
    this.styles = new AtomicStyleSheet(document, theme)
  }

  dispose(): void {
    for (const cleanup of this.allListeners) {
      cleanup()
    }
    this.allListeners.clear()
    this.keyed.clear()
    if (this.overlayOpen) {
      this.root.ownerDocument.body.style.overflow = this.overlayBodyOverflow ?? ""
    }
    this.root.remove()
    this.styles.dispose()
  }

  requestFocus(element: HTMLElement): void {
    this.focusRequest = element
  }

  clearFocusRequest(): void {
    this.focusRequest = undefined
  }

  setTheme(theme: Theme): void {
    this.theme = theme
    this.styles.setTheme(theme)
  }

  consumeFocusRequest(): HTMLElement | undefined {
    const element = this.focusRequest
    this.focusRequest = undefined
    return element
  }

  syncOverlayLifecycle(hasOpenOverlay: boolean, activeBefore: HTMLElement | null): HTMLElement | undefined {
    const document = this.root.ownerDocument
    if (hasOpenOverlay && !this.overlayOpen) {
      this.overlayOpen = true
      this.overlayRestoreFocus = activeBefore !== null && activeBefore !== document.body ? activeBefore : undefined
      this.overlayBodyOverflow = document.body.style.overflow
      document.body.style.overflow = "hidden"
      const overlay = this.root.querySelector('[data-en-overlay-open="true"]') as HTMLElement | null
      return overlay === null ? undefined : focusableElements(overlay)[0] ?? overlay
    }

    if (!hasOpenOverlay && this.overlayOpen) {
      this.overlayOpen = false
      document.body.style.overflow = this.overlayBodyOverflow ?? ""
      this.overlayBodyOverflow = undefined
      const restore = this.overlayRestoreFocus
      this.overlayRestoreFocus = undefined
      return restore
    }

    return undefined
  }

  resetListeners(target: EventTarget): void {
    const existing = this.listeners.get(target)
    if (existing !== undefined) {
      for (const cleanup of existing) {
        cleanup()
        this.allListeners.delete(cleanup)
      }
    }
    this.listeners.set(target, [])
  }

  addListener<K extends keyof HTMLElementEventMap>(
    target: HTMLElement,
    type: K,
    listener: (event: HTMLElementEventMap[K]) => void
  ): void {
    target.addEventListener(type, listener as EventListener)
    const cleanup = () => target.removeEventListener(type, listener as EventListener)
    this.listeners.get(target)?.push(cleanup)
    this.allListeners.add(cleanup)
  }

  keyedElement(view: View, tagName: string): HTMLElement {
    const key = view.key === undefined ? undefined : `${view._tag}:${view.key}`
    if (key !== undefined) {
      const existing = this.keyed.get(key)
      if (existing !== undefined && existing.localName === tagName) {
        existing.setAttribute("data-en-tag", view._tag)
        existing.setAttribute("data-en-key", view.key!)
        return existing
      }
    }

    const element = this.root.ownerDocument.createElement(tagName)
    element.setAttribute("data-en-tag", view._tag)
    if (view.key !== undefined) {
      element.setAttribute("data-en-key", view.key)
      this.keyed.set(`${view._tag}:${view.key}`, element)
    }
    return element
  }
}

const runReportedIntent = (
  report: IntentReporter,
  ref: IntentRef,
  runtimeValue: JsonPayload = null
): void => {
  void Effect.runPromise(report(ref, runtimeValue) as Effect.Effect<void, IntentError>).catch(() => {
    // Intent failures are recorded by the registry; DOM event handlers stay total.
  })
}

const applyBaseStyle = (element: HTMLElement, view: View, state: DomRendererState): void => {
  if ("style" in view && view.style !== undefined) {
    state.styles.apply(element, resolveStyle(view.style, { platform: "web" }))
  } else {
    state.styles.apply(element, undefined)
  }
}

const renderChildren = (
  element: HTMLElement,
  children: ReadonlyArray<View>,
  state: DomRendererState,
  report: IntentReporter
): void => {
  const rendered = children.map((child) => renderView(child, state, report))
  element.replaceChildren(...rendered)
}

const renderStack = (view: StackView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "div")
  const direction = resolveResponsiveValue(view.direction)
  const gap = view.gap === undefined ? undefined : resolveResponsiveValue(view.gap)
  const padding = view.padding === undefined ? undefined : resolveResponsiveValue(view.padding)
  state.resetListeners(element)
  element.style.display = "flex"
  element.style.flexDirection = direction
  element.style.gap = gap === undefined ? "" : `var(--en-spacing-${cssEscape(gap)})`
  element.style.alignItems = view.align === undefined ? "" : flexKeyword(view.align)
  element.style.justifyContent = view.justify === undefined ? "" : flexKeyword(view.justify)
  element.style.padding = padding === undefined ? "" : `var(--en-spacing-${cssEscape(padding)})`
  applyBaseStyle(element, view, state)
  renderChildren(element, view.children, state, report)
  return element
}

const renderText = (view: TextView, state: DomRendererState): HTMLElement => {
  const tagName = view.variant === "heading" || view.variant === "title" ? "p" : "span"
  const element = state.keyedElement(view, tagName)
  state.resetListeners(element)
  element.textContent = String(view.content)
  element.setAttribute("data-en-variant", view.variant)
  element.style.color = view.color === undefined ? "" : colorValue(view.color)
  element.style.fontWeight = view.weight === undefined ? "" : fontWeightValue(view.weight)
  applyBaseStyle(element, view, state)
  return element
}

const renderButton = (view: ButtonView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "button") as HTMLButtonElement
  state.resetListeners(element)
  element.type = "button"
  element.textContent = view.label
  element.disabled = view.disabled === true
  element.setAttribute("data-en-variant", view.variant)
  state.addListener(element, "click", () => runReportedIntent(report, view.onPress))
  applyBaseStyle(element, view, state)
  return element
}

const destinationHref = (destination: NavigationDestination): string => {
  switch (destination.kind) {
    case "url":
      return destination.href
    case "path":
      return destination.path
    case "anchor":
      return `#${destination.id}`
  }
}

const renderLink = (view: LinkView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "a") as HTMLAnchorElement
  state.resetListeners(element)
  element.href = destinationHref(view.destination)
  if (view.destination.kind === "url" && view.destination.target === "blank") {
    element.target = "_blank"
    element.rel = "noopener noreferrer"
  } else {
    element.removeAttribute("target")
    element.removeAttribute("rel")
  }
  state.addListener(element, "click", (event) => {
    event.preventDefault()
    runReportedIntent(report, makeNavigateIntent(view.destination))
  })
  applyBaseStyle(element, view, state)
  renderChildren(element, view.children, state, report)
  return element
}

const dismissOverlay = (view: ModalView | SheetView, report: IntentReporter): void => {
  if (view.dismissable) {
    runReportedIntent(report, view.onDismiss)
  }
}

const trapOverlayFocus = (root: HTMLElement, event: KeyboardEvent): void => {
  if (event.key !== "Tab") {
    return
  }
  const focusables = focusableElements(root)
  if (focusables.length === 0) {
    event.preventDefault()
    root.focus()
    return
  }

  const first = focusables[0]!
  const last = focusables[focusables.length - 1]!
  const active = root.ownerDocument.activeElement
  if (event.shiftKey && active === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && active === last) {
    event.preventDefault()
    first.focus()
  }
}

const renderModal = (view: ModalView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "dialog") as HTMLDialogElement
  const open = view.open === true
  const titleId = `en-modal-title-${cssEscape(view.key ?? "modal")}`
  state.resetListeners(element)
  element.setAttribute("role", "dialog")
  element.setAttribute("aria-modal", "true")
  element.setAttribute("aria-labelledby", titleId)
  element.setAttribute("data-en-overlay", "modal")
  element.setAttribute("data-en-overlay-open", open ? "true" : "false")
  element.tabIndex = -1
  element.hidden = !open
  if (open) {
    element.setAttribute("open", "")
  } else {
    element.removeAttribute("open")
  }
  element.style.position = "fixed"
  element.style.inset = "0"
  element.style.display = open ? "flex" : "none"
  element.style.alignItems = "center"
  element.style.justifyContent = "center"
  element.style.width = "100%"
  element.style.height = "100%"
  element.style.maxWidth = "none"
  element.style.maxHeight = "none"
  element.style.padding = "var(--en-spacing-4)"
  element.style.backgroundColor = "rgba(15, 23, 42, 0.32)"
  element.style.border = "0"

  const panel = element.ownerDocument.createElement("section")
  panel.setAttribute("data-en-role", "panel")
  panel.style.width = dimensionValue(view.size)
  panel.style.maxWidth = "100%"
  panel.style.background = "var(--en-color-background)"
  panel.style.border = "1px solid var(--en-color-border)"
  panel.style.borderRadius = "var(--en-radius-lg)"
  panel.style.padding = "var(--en-spacing-4)"

  const title = element.ownerDocument.createElement("h2")
  title.id = titleId
  title.textContent = String(view.title)
  title.style.margin = "0 0 var(--en-spacing-3) 0"
  panel.appendChild(title)
  for (const child of view.children) {
    panel.appendChild(renderView(child, state, report))
  }
  element.replaceChildren(panel)

  state.addListener(element, "click", (event) => {
    if (event.target === element) {
      dismissOverlay(view, report)
    }
  })
  state.addListener(element, "keydown", (event) => {
    if ((event as KeyboardEvent).key === "Escape") {
      dismissOverlay(view, report)
      return
    }
    trapOverlayFocus(element, event as KeyboardEvent)
  })
  state.addListener(element, "cancel", (event) => {
    event.preventDefault()
    dismissOverlay(view, report)
  })
  return element
}

const renderSheet = (view: SheetView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "aside")
  const open = view.open === true
  state.resetListeners(element)
  element.setAttribute("role", "dialog")
  element.setAttribute("aria-modal", "true")
  element.setAttribute("data-en-overlay", "sheet")
  element.setAttribute("data-en-overlay-open", open ? "true" : "false")
  element.setAttribute("data-en-edge", view.edge)
  element.setAttribute("data-en-detents", view.detents.join(" "))
  element.tabIndex = -1
  element.hidden = !open
  element.style.position = "fixed"
  element.style.inset = "0"
  element.style.display = open ? "flex" : "none"
  element.style.alignItems = view.edge === "bottom" ? "flex-end" : "stretch"
  element.style.justifyContent = view.edge === "bottom" ? "center" : "flex-end"
  element.style.backgroundColor = "rgba(15, 23, 42, 0.32)"

  const backdrop = element.ownerDocument.createElement("div")
  backdrop.setAttribute("data-en-role", "backdrop")
  backdrop.style.position = "absolute"
  backdrop.style.inset = "0"
  state.resetListeners(backdrop)
  state.addListener(backdrop, "click", () => dismissOverlay(view, report))

  const panel = element.ownerDocument.createElement("section")
  panel.setAttribute("data-en-role", "panel")
  panel.style.position = "relative"
  panel.style.background = "var(--en-color-background)"
  panel.style.border = "1px solid var(--en-color-border)"
  panel.style.padding = "var(--en-spacing-4)"
  panel.style.width = view.edge === "bottom" ? "100%" : dimensionValue(view.detents[0]!)
  panel.style.height = view.edge === "bottom" ? dimensionValue(view.detents[0]!) : "100%"
  panel.style.borderRadius = view.edge === "bottom"
    ? "var(--en-radius-lg) var(--en-radius-lg) 0 0"
    : "var(--en-radius-lg) 0 0 var(--en-radius-lg)"
  for (const child of view.children) {
    panel.appendChild(renderView(child, state, report))
  }
  element.replaceChildren(backdrop, panel)

  state.addListener(element, "keydown", (event) => {
    if ((event as KeyboardEvent).key === "Escape") {
      dismissOverlay(view, report)
      return
    }
    trapOverlayFocus(element, event as KeyboardEvent)
  })
  state.addListener(element, "cancel", (event) => {
    event.preventDefault()
    dismissOverlay(view, report)
  })
  return element
}

const renderImage = (view: ImageView, state: DomRendererState): HTMLElement => {
  const element = state.keyedElement(view, "img") as HTMLImageElement
  const width = view.width === undefined ? undefined : resolveResponsiveValue(view.width)
  const height = view.height === undefined ? undefined : resolveResponsiveValue(view.height)
  state.resetListeners(element)
  element.src = view.source
  element.alt = view.alt
  element.style.objectFit = view.fit ?? ""
  element.style.width = width === undefined ? "" : dimensionValue(width)
  element.style.height = height === undefined ? "" : dimensionValue(height)
  if (typeof width === "number") {
    element.width = width
  } else {
    element.removeAttribute("width")
  }
  if (typeof height === "number") {
    element.height = height
  } else {
    element.removeAttribute("height")
  }
  applyBaseStyle(element, view, state)
  return element
}

const fieldValue = (element: HTMLInputElement | HTMLTextAreaElement): JsonPayload => element.value

const renderTextField = (view: TextFieldView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "label") as HTMLLabelElement
  state.resetListeners(element)
  const fieldTag = view.multiline === true ? "textarea" : "input"
  const document = element.ownerDocument
  const existingField = Array.from(element.children).find((child) =>
    child.localName === fieldTag
  ) as HTMLInputElement | HTMLTextAreaElement | undefined
  const fieldWasActive = existingField !== undefined && document.activeElement === existingField
  element.replaceChildren()
  element.style.display = "grid"
  element.style.gap = "var(--en-spacing-1)"
  const id = `en-field-${cssEscape(view.key ?? view.label ?? "field")}`
  if (view.label !== undefined) {
    const caption = document.createElement("span")
    caption.setAttribute("data-en-role", "label")
    caption.textContent = view.label
    element.appendChild(caption)
  }

  const field = existingField ?? document.createElement(fieldTag)
  state.resetListeners(field)
  field.id = id
  field.setAttribute("data-en-role", "control")
  field.setAttribute("placeholder", view.placeholder ?? "")
  if (field.localName === "input") {
    field.setAttribute("type", view.secure === true ? "password" : "text")
  }
  if (!fieldWasActive) {
    field.value = view.value
  }
  field.style.boxSizing = "border-box"
  field.style.width = "100%"
  field.style.minWidth = "0"
  field.style.font = "inherit"
  field.style.color = "inherit"
  field.style.background = "transparent"
  field.style.border = "0"
  field.style.outline = "0"
  if (field.localName === "textarea") {
    field.style.resize = "vertical"
  }
  const onChange = view.field === undefined
    ? view.onChange
    : IntentRef("FormFieldChanged", FormFieldValueBinding(view.field))
  if (onChange !== undefined) {
    state.addListener(field, "input", () => runReportedIntent(report, onChange, fieldValue(field)))
  }
  if (view.field !== undefined) {
    state.addListener(field, "blur", () =>
      runReportedIntent(report, IntentRef("FormFieldBlurred", StaticPayload(view.field!)))
    )
  }
  if (view.onSubmit !== undefined) {
    state.addListener(field, "keydown", (event) => {
      if ((event as KeyboardEvent).key === "Enter") {
        runReportedIntent(report, view.onSubmit!, fieldValue(field))
      }
    })
  }
  element.appendChild(field)
  if (fieldWasActive || view.focused === true) {
    state.requestFocus(field)
  }
  applyBaseStyle(element, view, state)
  return element
}

const defaultVirtualViewportSize = 400
const virtualOverscan = 3

const dimensionPixels = (value: Dimension, theme: Theme): number => {
  if (typeof value === "number") {
    return value
  }
  const resolved = theme.dimension[value as DimensionToken]
  return typeof resolved === "number" ? resolved : 44
}

const collectionIdentity = (view: ListView | SectionListView): string =>
  `${view._tag}:${view.key ?? "unkeyed"}`

const collectionThreshold = (view: ListView | SectionListView): number =>
  view.endReachedThreshold ?? 0.5

const viewportExtent = (element: HTMLElement): number => {
  const inlineHeight = Number.parseFloat(element.style.height)
  return element.clientHeight > 0
    ? element.clientHeight
    : Number.isFinite(inlineHeight) && inlineHeight > 0
      ? inlineHeight
      : defaultVirtualViewportSize
}

const maybeReportEndReached = (
  view: ListView | SectionListView,
  element: HTMLElement,
  rowCount: number,
  itemSize: number,
  report: IntentReporter,
  state: DomRendererState
): void => {
  if (view.onEndReached === undefined || rowCount === 0) {
    return
  }

  const viewport = viewportExtent(element)
  const total = rowCount * itemSize
  const remaining = total - (element.scrollTop + viewport)
  const withinThreshold = remaining <= viewport * collectionThreshold(view)
  const identity = collectionIdentity(view)
  const signature = `${rowCount}:${itemSize}:${view.onEndReached.name}`

  if (withinThreshold && state.endReachedSignatures.get(identity) !== signature) {
    state.endReachedSignatures.set(identity, signature)
    runReportedIntent(report, view.onEndReached)
  } else if (!withinThreshold) {
    state.endReachedSignatures.delete(identity)
  }
}

const virtualWindow = (
  element: HTMLElement,
  rowCount: number,
  itemSize: number
): { readonly start: number; readonly end: number } => {
  const viewport = viewportExtent(element)
  const start = Math.max(0, Math.floor(element.scrollTop / itemSize) - virtualOverscan)
  const visible = Math.ceil(viewport / itemSize) + virtualOverscan * 2
  return {
    start,
    end: Math.min(rowCount, start + visible)
  }
}

const virtualSpacer = (document: Document, height: number, tagName: "li" | "div"): HTMLElement => {
  const spacer = document.createElement(tagName)
  spacer.setAttribute("data-en-role", "virtual-spacer")
  spacer.setAttribute("aria-hidden", "true")
  spacer.style.height = px(height)
  if (tagName === "li") {
    spacer.style.listStyle = "none"
  }
  return spacer
}

const renderListItem = (
  element: HTMLElement,
  item: View,
  state: DomRendererState,
  report: IntentReporter
): HTMLElement => {
  const listItem = element.ownerDocument.createElement("li")
  listItem.setAttribute("data-en-role", "item")
  listItem.style.listStyle = "none"
  listItem.appendChild(renderView(item, state, report))
  return listItem
}

const prepareVirtualCollection = (
  element: HTMLElement,
  view: ListView | SectionListView,
  state: DomRendererState,
  rowCount: number,
  itemSize: number,
  report: IntentReporter,
  renderRows: () => void
): void => {
  element.setAttribute("data-en-virtualized", "true")
  element.style.overflowY = "auto"
  element.style.contain = "content"
  if (view.style === undefined || !("height" in view.style) && !("maxHeight" in view.style)) {
    element.style.height = px(Math.min(defaultVirtualViewportSize, Math.max(itemSize, rowCount * itemSize)))
  }
  renderRows()
  maybeReportEndReached(view, element, rowCount, itemSize, report, state)
  state.addListener(element, "scroll", () => {
    renderRows()
    maybeReportEndReached(view, element, rowCount, itemSize, report, state)
  })
}

const resetCollectionStyle = (element: HTMLElement): void => {
  element.removeAttribute("data-en-virtualized")
  element.style.overflowY = ""
  element.style.contain = ""
  element.style.height = ""
}

const renderList = (view: ListView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "ul")
  state.resetListeners(element)
  const itemSize = Math.max(
    1,
    view.estimatedItemSize === undefined
      ? 44
      : dimensionPixels(view.estimatedItemSize, state.theme)
  )

  if (view.virtualize === true) {
    prepareVirtualCollection(
      element,
      view,
      state,
      view.items.length,
      itemSize,
      report,
      () => {
        const { start, end } = virtualWindow(element, view.items.length, itemSize)
        const rows = view.items.slice(start, end).map((item) => renderListItem(element, item, state, report))
        element.replaceChildren(
          virtualSpacer(element.ownerDocument, start * itemSize, "li"),
          ...rows,
          virtualSpacer(element.ownerDocument, (view.items.length - end) * itemSize, "li")
        )
      }
    )
  } else {
    resetCollectionStyle(element)
    element.replaceChildren(...view.items.map((item) => renderListItem(element, item, state, report)))
  }

  applyBaseStyle(element, view, state)
  return element
}

type SectionRow =
  | {
      readonly kind: "header"
      readonly sectionKey: string
      readonly view: View
    }
  | {
      readonly kind: "item"
      readonly sectionKey: string
      readonly view: View
    }

const sectionRows = (view: SectionListView): ReadonlyArray<SectionRow> =>
  view.sections.flatMap((section) => [
    {
      kind: "header" as const,
      sectionKey: section.key,
      view: section.header
    },
    ...section.items.map((item) => ({
      kind: "item" as const,
      sectionKey: section.key,
      view: item
    }))
  ])

const renderSectionRow = (
  element: HTMLElement,
  row: SectionRow,
  stickyHeaders: boolean,
  state: DomRendererState,
  report: IntentReporter
): HTMLElement => {
  const rowElement = element.ownerDocument.createElement("div")
  rowElement.setAttribute("data-en-role", row.kind === "header" ? "section-header" : "item")
  rowElement.setAttribute("data-en-section-key", row.sectionKey)
  if (row.kind === "header" && stickyHeaders) {
    rowElement.style.position = "sticky"
    rowElement.style.top = "0"
    rowElement.style.zIndex = "1"
    rowElement.style.background = "var(--en-color-background)"
  }
  rowElement.appendChild(renderView(row.view, state, report))
  return rowElement
}

const renderSectionList = (
  view: SectionListView,
  state: DomRendererState,
  report: IntentReporter
): HTMLElement => {
  const element = state.keyedElement(view, "section")
  state.resetListeners(element)
  const rows = sectionRows(view)
  const stickyHeaders = view.stickyHeaders === true
  const itemSize = Math.max(
    1,
    view.estimatedItemSize === undefined
      ? 44
      : dimensionPixels(view.estimatedItemSize, state.theme)
  )

  if (view.virtualize === true) {
    prepareVirtualCollection(
      element,
      view,
      state,
      rows.length,
      itemSize,
      report,
      () => {
        const { start, end } = virtualWindow(element, rows.length, itemSize)
        const renderedRows = rows
          .slice(start, end)
          .map((row) => renderSectionRow(element, row, stickyHeaders, state, report))
        element.replaceChildren(
          virtualSpacer(element.ownerDocument, start * itemSize, "div"),
          ...renderedRows,
          virtualSpacer(element.ownerDocument, (rows.length - end) * itemSize, "div")
        )
      }
    )
  } else {
    resetCollectionStyle(element)
    element.replaceChildren(
      ...rows.map((row) => renderSectionRow(element, row, stickyHeaders, state, report))
    )
  }

  applyBaseStyle(element, view, state)
  return element
}

const renderCard = (view: CardView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "section")
  state.resetListeners(element)
  element.style.padding = view.padding === undefined ? "" : `var(--en-spacing-${cssEscape(view.padding)})`
  element.style.borderRadius = view.radius === undefined ? "" : radiusValue(view.radius)
  applyBaseStyle(element, view, state)
  renderChildren(element, view.children, state, report)
  return element
}

const renderSpacer = (view: SpacerView, state: DomRendererState): HTMLElement => {
  const element = state.keyedElement(view, "div")
  state.resetListeners(element)
  element.setAttribute("aria-hidden", "true")
  if (view.flex === true) {
    element.style.flex = "1 1 0"
    element.style.width = ""
    element.style.height = ""
  } else {
    element.style.flex = ""
    element.style.width = `var(--en-spacing-${cssEscape(view.size)})`
    element.style.height = `var(--en-spacing-${cssEscape(view.size)})`
  }
  applyBaseStyle(element, view, state)
  return element
}

const renderView = (view: View, state: DomRendererState, report: IntentReporter): HTMLElement => {
  switch (view._tag) {
    case "Stack":
      return renderStack(view, state, report)
    case "Text":
      return renderText(view, state)
    case "Button":
      return renderButton(view, state, report)
    case "Link":
      return renderLink(view, state, report)
    case "Modal":
      return renderModal(view, state, report)
    case "Sheet":
      return renderSheet(view, state, report)
    case "Image":
      return renderImage(view, state)
    case "TextField":
      return renderTextField(view, state, report)
    case "List":
      return renderList(view, state, report)
    case "SectionList":
      return renderSectionList(view, state, report)
    case "Card":
      return renderCard(view, state, report)
    case "Spacer":
      return renderSpacer(view, state)
  }
}

const commitView = (view: View, state: DomRendererState, report: IntentReporter): void => {
  const activeBefore = state.root.ownerDocument.activeElement as HTMLElement | null
  state.clearFocusRequest()
  state.styles.beginRender()
  const element = renderView(view, state, report)
  state.root.replaceChildren(element)
  const focusRequest = state.consumeFocusRequest()
  const overlayFocus = state.syncOverlayLifecycle(
    state.root.querySelector('[data-en-overlay-open="true"]') !== null,
    activeBefore
  )
  if (focusRequest !== undefined && state.root.contains(focusRequest)) {
    focusRequest.focus()
  } else if (
    overlayFocus !== undefined &&
    overlayFocus.ownerDocument.body.contains(overlayFocus) &&
    typeof overlayFocus.focus === "function"
  ) {
    overlayFocus.focus()
  } else if (
    activeBefore !== null &&
    activeBefore !== state.root.ownerDocument.body &&
    state.root.contains(activeBefore) &&
    typeof activeBefore.focus === "function"
  ) {
    activeBefore.focus()
  }
  state.styles.flush()
}

const serializeElement = (element: Element): DomStructure | undefined => {
  const tag = element.getAttribute("data-en-tag") as View["_tag"] | null
  if (tag === null) {
    for (const child of Array.from(element.children)) {
      const found = serializeElement(child)
      if (found !== undefined) {
        return found
      }
    }
    return undefined
  }

  const key = element.getAttribute("data-en-key") ?? undefined
  const serializeChildren = (root: Element): ReadonlyArray<DomStructure> =>
    Array.from(root.children)
      .filter((child) =>
        child.getAttribute("data-en-role") !== "label" &&
        child.getAttribute("data-en-role") !== "virtual-spacer"
      )
      .flatMap((child) => {
        if (child.getAttribute("data-en-tag") !== null) {
          const serialized = serializeElement(child)
          return serialized === undefined ? [] : [serialized]
        }
        return serializeChildren(child)
      })
  const children = serializeChildren(element)

  return {
    tag,
    ...(key === undefined ? {} : { key }),
    ...(tag === "Text" || tag === "Button" ? { text: element.textContent ?? "" } : {}),
    ...(children.length === 0 ? {} : { children })
  }
}

export const serializeDomStructure = (container: Element): DomStructure | undefined => serializeElement(container)

export const viewStructure = (view: View): DomStructure => {
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

export const makeDomRenderer = (options: DomRendererOptions = {}): RendererAdapter<Element, DomMountedSurface> => ({
  mount: (container, viewStream, report) =>
    Effect.gen(function*() {
      const parentScope = yield* Scope.Scope
      const surfaceScope = yield* Scope.fork(parentScope)

      return yield* Scope.provide(surfaceScope)(Effect.gen(function*() {
        const document = options.document ?? container.ownerDocument ?? globalThis.document
        const theme = options.theme ?? defaultTheme
        const viewport = yield* makeViewportService(options.viewport ?? readDomViewport(document), { theme })
        const state = new DomRendererState(container, document, theme)
        const ready = yield* Deferred.make<void>()
        const window = document.defaultView
        const resolvedViewStream = viewStream.pipe(
          Stream.zipLatestWith(viewport.stream, (view, currentViewport) =>
            resolveView(view, { viewport: currentViewport, platform: "web" })
          )
        )

        yield* Effect.addFinalizer(() =>
          Effect.sync(() => {
            state.dispose()
          })
        )
        if (window !== null) {
          const updateViewport = () => {
            void Effect.runPromise(viewport.set(readDomViewport(document))).catch(() => {
              // Host resize callbacks must stay total.
            })
          }
          window.addEventListener("resize", updateViewport)
          yield* Effect.addFinalizer(() =>
            Effect.sync(() => {
              window.removeEventListener("resize", updateViewport)
            })
          )
        }

        yield* resolvedViewStream.pipe(
          Stream.runForEach((view) =>
            Effect.gen(function*() {
              yield* Effect.sync(() => {
                commitView(view, state, report)
              })
              yield* Deferred.succeed(ready, undefined)
            })
          ),
          Effect.forkScoped
        )
        yield* Deferred.await(ready)

        return {
          root: state.root,
          stylesheet: state.styles.element,
          unmount: Scope.close(surfaceScope, Exit.void),
          serialize: Effect.sync(() => serializeDomStructure(state.root)),
          stylesheetText: Effect.sync(() => state.styles.element.textContent ?? ""),
          setTheme: (theme: Theme) => Effect.sync(() => state.setTheme(theme)),
          currentViewport: viewport.current,
          setViewport: viewport.set
        }
      }))
    })
})
