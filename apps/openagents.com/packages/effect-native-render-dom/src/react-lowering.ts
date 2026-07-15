import { Effect } from "effect"
import {
  resolveButtonAppearance,
  resolveResponsiveValue,
  resolveStyle,
  type A11y,
  type FlatStyle,
  type IntentError,
  type IntentReporter,
  type IntentRef,
  type JsonPayload,
  type View
} from "@effect-native/core"
import { createElement, type CSSProperties, type ReactElement } from "react"
import { defaultTheme, type Theme } from "@effect-native/tokens"
import { resolveKhalaStaticDecoration } from "./khala-static.js"

export class UnsupportedReactViewError extends Error {
  readonly viewTag: View["_tag"]
  constructor(viewTag: View["_tag"]) {
    super(`The React DOM backend does not yet support Effect Native ${viewTag}.`)
    this.name = "UnsupportedReactViewError"
    this.viewTag = viewTag
  }
}

export interface ReactLoweringContext {
  readonly report: IntentReporter
  readonly theme?: Theme
}

const cssToken = (kind: string, token: string): string => `var(--en-${kind}-${token})`
const dimension = (value: string | number): string =>
  typeof value === "number" ? `${value}px` : cssToken("dimension", value)
const flexKeyword = (value: string): string => {
  if (value === "start") return "flex-start"
  if (value === "end") return "flex-end"
  if (value === "between") return "space-between"
  if (value === "around") return "space-around"
  return value
}

const styleProperties = (style: FlatStyle | undefined): CSSProperties | undefined => {
  if (style === undefined) return undefined
  const result: Record<string, string | number> = {}
  for (const [key, value] of Object.entries(style)) {
    if (value === undefined) continue
    if (["margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "gap"].includes(key)) {
      result[key] = cssToken("spacing", String(value))
    } else if (["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight"].includes(key)) {
      result[key] = dimension(value as string | number)
    } else if (key === "backgroundColor" || key === "borderColor" || key === "color") {
      result[key] = cssToken("color", String(value))
    } else if (key === "borderRadius") {
      result[key] = cssToken("radius", String(value))
    } else if (key === "surface" && value === "glass") {
      result.backgroundColor = cssToken("color", "glassSurface")
      result.backdropFilter = "blur(18px) saturate(140%)"
    } else if (key === "typeScale") {
      result.fontSize = cssToken("type", `${String(value)}-fontSize`)
      result.lineHeight = cssToken("type", `${String(value)}-lineHeight`)
      result.fontWeight = cssToken("type", `${String(value)}-fontWeight`)
    } else if (key === "fontWeight") {
      result.fontWeight = value === "regular" ? 400 : value === "medium" ? 500 : value === "semibold" ? 600 : value === "bold" ? 700 : String(value)
    } else {
      result[key] = value as string | number
    }
  }
  return result as CSSProperties
}

const mergeStyle = (...styles: ReadonlyArray<CSSProperties | undefined>): CSSProperties =>
  Object.assign({}, ...styles.filter((style): style is CSSProperties => style !== undefined))

const a11yProperties = (key: string | undefined, a11y: A11y | undefined): Record<string, unknown> => ({
  ...(key === undefined ? {} : { id: `en-${key}` }),
  ...(a11y?.role === undefined ? {} : { role: a11y.role }),
  ...(a11y?.label === undefined ? {} : { "aria-label": a11y.label }),
  ...(a11y?.activeDescendant === undefined ? {} : { "aria-activedescendant": `en-${a11y.activeDescendant}` }),
  ...(a11y?.selected === undefined ? {} : { "aria-selected": a11y.selected }),
  ...(a11y?.expanded === undefined ? {} : { "aria-expanded": a11y.expanded }),
  ...(a11y?.disabled === undefined ? {} : { "aria-disabled": a11y.disabled }),
  ...(a11y?.hidden !== true ? {} : { "aria-hidden": true }),
  ...(a11y?.tabIndex === undefined ? {} : { tabIndex: a11y.tabIndex })
})

const reportIntent = (report: IntentReporter, ref: IntentRef, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(ref, payload) as Effect.Effect<void, IntentError>).catch(() => {})
}

const baseProperties = (view: View, context: ReactLoweringContext): Record<string, unknown> => ({
  key: view.key,
  "data-en-tag": view._tag,
  ...(view.key === undefined ? {} : { "data-en-key": view.key }),
  ...a11yProperties(view.key, view.a11y),
  ...(view.interactions?.onFocus === undefined ? {} : { onFocus: () => reportIntent(context.report, view.interactions!.onFocus!) }),
  ...(view.interactions?.onBlur === undefined ? {} : { onBlur: () => reportIntent(context.report, view.interactions!.onBlur!) }),
  ...(view.interactions?.onPointerEnter === undefined ? {} : { onPointerEnter: () => reportIntent(context.report, view.interactions!.onPointerEnter!) }),
  ...(view.interactions?.onPointerLeave === undefined ? {} : { onPointerLeave: () => reportIntent(context.report, view.interactions!.onPointerLeave!) })
})

const lower = (view: View, context: ReactLoweringContext): ReactElement => {
  const style = "style" in view && view.style !== undefined
    ? styleProperties(resolveStyle(view.style as never, { platform: "web" }) as FlatStyle)
    : undefined
  const base = baseProperties(view, context)
  switch (view._tag) {
    case "Stack": {
      const gap = view.gap === undefined ? undefined : resolveResponsiveValue(view.gap)
      const padding = view.padding === undefined ? undefined : resolveResponsiveValue(view.padding)
      return createElement("div", {
        ...base,
        style: mergeStyle({
          display: "flex",
          flexDirection: resolveResponsiveValue(view.direction),
          gap: gap === undefined ? undefined : cssToken("spacing", gap),
          padding: padding === undefined ? undefined : cssToken("spacing", padding),
          alignItems: view.align === undefined ? undefined : flexKeyword(view.align),
          justifyContent: view.justify === undefined ? undefined : flexKeyword(view.justify)
        }, style)
      }, ...view.children.map((child) => lower(child, context)))
    }
    case "Text":
      return createElement(view.variant === "heading" || view.variant === "title" ? "p" : "span", {
        ...base,
        "data-en-variant": view.variant,
        style: mergeStyle({
          color: view.color === undefined ? undefined : cssToken("color", view.color),
          fontWeight: view.weight === undefined ? undefined : view.weight === "regular" ? 400 : view.weight === "medium" ? 500 : view.weight === "semibold" ? 600 : 700
        }, style)
      }, String(view.content))
    case "Button": {
      const appearance = resolveButtonAppearance(view)
      const disabled = view.disabled === true || view.loading === true
      return createElement("button", {
        ...base,
        type: "button",
        disabled,
        "data-en-component": "button",
        "data-en-tone": appearance.tone,
        "data-en-variant": appearance.variant,
        "data-en-size": appearance.size,
        "data-en-disabled": String(disabled),
        "data-en-pill": String(view.pill === true),
        "data-en-block": String(view.block === true),
        "data-en-loading": String(view.loading === true),
        "data-en-selected": String(view.selected === true),
        ...(view.loading === true ? { "aria-busy": true } : {}),
        ...(view.selected === undefined ? {} : { "aria-pressed": view.selected }),
        onClick: disabled ? undefined : () => reportIntent(context.report, view.onPress),
        style
      }, view.label)
    }
    case "Card":
      return createElement("section", {
        ...base,
        style: mergeStyle({
          padding: view.padding === undefined ? undefined : cssToken("spacing", view.padding),
          borderRadius: view.radius === undefined ? undefined : cssToken("radius", view.radius)
        }, style)
      }, ...view.children.map((child) => lower(child, context)))
    case "Spacer":
      return createElement("div", {
        ...base,
        "aria-hidden": true,
        style: mergeStyle(view.flex === true
          ? { flex: "1 1 0" }
          : { width: cssToken("spacing", view.size), height: cssToken("spacing", view.size) }, style)
      })
    case "Divider": {
      const orientation = view.orientation ?? "horizontal"
      return createElement("div", {
        ...base,
        role: "separator",
        "aria-orientation": orientation,
        style: mergeStyle(orientation === "vertical"
          ? { width: 1, alignSelf: "stretch", borderLeft: `1px solid ${cssToken("color", "border")}` }
          : { height: 1, borderTop: `1px solid ${cssToken("color", "border")}` }, style)
      })
    }
    case "Frame": {
      const frameStyle = mergeStyle({
        position: "relative",
        overflow: "visible",
        isolation: "isolate",
        padding: cssToken("spacing", "3"),
        border: view.khala === undefined ? `1px solid ${cssToken("color", "accent")}` : undefined,
        borderRadius: view.variant === "rounded" || view.variant === "arcade" ? cssToken("radius", "lg") : undefined
      }, style)
      if (view.khala === undefined) {
        return createElement(
          "div",
          { ...base, "data-en-frame": view.variant ?? "square", style: frameStyle },
          ...view.children.map((child) => lower(child, context))
        )
      }

      const decoration = resolveKhalaStaticDecoration(view.khala, context.theme ?? defaultTheme)
      return createElement(
        "div",
        {
          ...base,
          "data-en-frame": view.variant ?? "square",
          "data-en-khala": view.khala.motif,
          "data-en-khala-id": decoration.id,
          "data-en-khala-collapse": decoration.geometry.collapse,
          style: frameStyle
        },
        createElement(
          "svg",
          {
            key: decoration.id,
            id: decoration.id,
            "data-en-khala-decoration": "true",
            "data-en-khala-decorative-nodes": String(decoration.paths.length + 1),
            "aria-hidden": true,
            focusable: false,
            viewBox: `0 0 ${view.khala.width} ${view.khala.height}`,
            preserveAspectRatio: "none",
            style: {
              position: "absolute",
              inset: 0,
              width: "100%",
              height: "100%",
              overflow: "visible",
              pointerEvents: "none",
              zIndex: 0
            }
          },
          ...decoration.paths.map((group) => createElement("path", {
            key: group.id,
            id: group.id,
            "data-en-khala-role": group.role,
            d: group.data,
            fill: "none",
            stroke: view.khala?.forcedColors === true ? "CanvasText" : cssToken("color", group.color),
            strokeWidth: group.width,
            vectorEffect: "non-scaling-stroke"
          }))
        ),
        createElement(
          "div",
          {
            key: `${decoration.id}-content`,
            "data-en-khala-content": "true",
            style: { position: "relative", zIndex: 1 }
          },
          ...view.children.map((child) => lower(child, context))
        )
      )
    }
    default:
      throw new UnsupportedReactViewError(view._tag)
  }
}

export const renderReactDomView = (view: View, context: ReactLoweringContext): ReactElement =>
  lower(view, context)
