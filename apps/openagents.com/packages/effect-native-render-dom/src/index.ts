import { Deferred, Effect, Exit, Scope, Stream } from "effect"
import {
  type ButtonView,
  type CardView,
  type ColorToken,
  type Dimension,
  type FlatStyle,
  type ImageView,
  type IntentError,
  type IntentRef,
  type IntentReporter,
  type JsonPayload,
  type ListView,
  type MountedSurface,
  type RendererAdapter,
  type SpacerView,
  type StackView,
  type TextFieldView,
  type TextView,
  type View,
  defaultTheme,
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
}

export interface DomMountedSurface extends MountedSurface {
  readonly root: HTMLElement
  readonly stylesheet: HTMLStyleElement
  readonly serialize: Effect.Effect<DomStructure | undefined>
  readonly stylesheetText: Effect.Effect<string>
  readonly setTheme: (theme: Theme) => Effect.Effect<void>
}

export interface DomStructure {
  readonly tag: View["_tag"]
  readonly key?: string
  readonly text?: string
  readonly children?: ReadonlyArray<DomStructure>
}

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

  constructor(container: Element, document: Document, theme: Theme) {
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
    this.root.remove()
    this.styles.dispose()
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
  state.resetListeners(element)
  element.style.display = "flex"
  element.style.flexDirection = view.direction
  element.style.gap = view.gap === undefined ? "" : `var(--en-spacing-${cssEscape(view.gap)})`
  element.style.alignItems = view.align === undefined ? "" : flexKeyword(view.align)
  element.style.justifyContent = view.justify === undefined ? "" : flexKeyword(view.justify)
  element.style.padding = view.padding === undefined ? "" : `var(--en-spacing-${cssEscape(view.padding)})`
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

const renderImage = (view: ImageView, state: DomRendererState): HTMLElement => {
  const element = state.keyedElement(view, "img") as HTMLImageElement
  state.resetListeners(element)
  element.src = view.source
  element.alt = view.alt
  element.style.objectFit = view.fit ?? ""
  if (typeof view.width === "number") {
    element.width = view.width
  }
  if (typeof view.height === "number") {
    element.height = view.height
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
  if (view.onChange !== undefined) {
    state.addListener(field, "input", () => runReportedIntent(report, view.onChange!, fieldValue(field)))
  }
  if (view.onSubmit !== undefined) {
    state.addListener(field, "keydown", (event) => {
      if ((event as KeyboardEvent).key === "Enter") {
        runReportedIntent(report, view.onSubmit!, fieldValue(field))
      }
    })
  }
  element.appendChild(field)
  if (fieldWasActive) {
    field.focus()
  }
  applyBaseStyle(element, view, state)
  return element
}

const renderList = (view: ListView, state: DomRendererState, report: IntentReporter): HTMLElement => {
  const element = state.keyedElement(view, "ul")
  state.resetListeners(element)
  const items = view.items.map((item) => {
    const listItem = element.ownerDocument.createElement("li")
    listItem.setAttribute("data-en-role", "item")
    listItem.appendChild(renderView(item, state, report))
    return listItem
  })
  element.replaceChildren(...items)
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
    case "Image":
      return renderImage(view, state)
    case "TextField":
      return renderTextField(view, state, report)
    case "List":
      return renderList(view, state, report)
    case "Card":
      return renderCard(view, state, report)
    case "Spacer":
      return renderSpacer(view, state)
  }
}

const commitView = (view: View, state: DomRendererState, report: IntentReporter): void => {
  const activeBefore = state.root.ownerDocument.activeElement as HTMLElement | null
  state.styles.beginRender()
  const element = renderView(view, state, report)
  state.root.replaceChildren(element)
  if (
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
  const childElements = Array.from(element.children)
    .filter((child) => child.getAttribute("data-en-role") !== "label")
  const children = childElements
    .map((child) => serializeElement(child))
    .filter((child): child is DomStructure => child !== undefined)

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
    case "List":
      return {
        tag: "List",
        ...(view.key === undefined ? {} : { key: view.key }),
        children: view.items.map(viewStructure)
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
        const state = new DomRendererState(container, document, options.theme ?? defaultTheme)
        const ready = yield* Deferred.make<void>()

        yield* Effect.addFinalizer(() =>
          Effect.sync(() => {
            state.dispose()
          })
        )

        yield* viewStream.pipe(
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
          setTheme: (theme: Theme) => Effect.sync(() => state.styles.setTheme(theme))
        }
      }))
    })
})
