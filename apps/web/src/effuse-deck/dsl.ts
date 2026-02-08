import { html } from "@openagentsinc/effuse"
import type { TemplateResult } from "@openagentsinc/effuse"

export type TokenRef = { readonly $token: string }
export type RefNode = { readonly $ref: string }

export type DeckNode = {
  readonly type: string
  readonly id?: string
  readonly props?: Readonly<Record<string, unknown>>
  readonly children?: ReadonlyArray<DeckNodeChild>
}

export type DeckNodeChild = DeckNode | RefNode | string

export type DeckLayout = DeckNode

export type DeckSlide = {
  readonly id: string
  readonly title?: string
  readonly layout?: string
  readonly background?: unknown
  readonly notes?: unknown
  readonly regions?: Readonly<Record<string, ReadonlyArray<DeckNodeChild>>>
  readonly content?: ReadonlyArray<DeckNodeChild>
}

export type DeckTheme = {
  readonly tokens?: Readonly<Record<string, unknown>>
  readonly defaults?: Readonly<Record<string, unknown>>
}

export type DeckDocument = {
  readonly dsl: "effuse.slide-deck"
  readonly version: string
  readonly meta?: Readonly<Record<string, unknown>>
  readonly theme?: { readonly tokens?: Readonly<Record<string, unknown>>; readonly defaults?: Readonly<Record<string, unknown>> }
  readonly assets?: Readonly<Record<string, unknown>>
  readonly layouts?: Readonly<Record<string, DeckLayout>>
  readonly deck: {
    readonly aspectRatio?: string
    readonly size?: { readonly width: number; readonly height: number }
    readonly background?: unknown
    readonly defaultSlideLayout?: string
    readonly controls?: Readonly<Record<string, unknown>>
    readonly slides: ReadonlyArray<DeckSlide>
  }
}

export type DeckParseResult =
  | { readonly _tag: "Ok"; readonly doc: DeckDocument }
  | { readonly _tag: "Error"; readonly message: string }

const isRecord = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value)

const expectString = (obj: Record<string, unknown>, key: string): string | null => {
  const v = obj[key]
  return typeof v === "string" ? v : null
}

const safeJson = (value: unknown): string => {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

export const parseDeckDocument = (json: unknown): DeckParseResult => {
  if (!isRecord(json)) {
    return { _tag: "Error", message: "Deck JSON must be an object." }
  }

  const dsl = expectString(json, "dsl")
  if (dsl !== "effuse.slide-deck") {
    return { _tag: "Error", message: `Invalid dsl: expected "effuse.slide-deck", got ${safeJson(dsl)}` }
  }

  const version = expectString(json, "version")
  if (!version) {
    return { _tag: "Error", message: "Missing required field: version" }
  }

  const deckRaw = json["deck"]
  if (!isRecord(deckRaw)) {
    return { _tag: "Error", message: "Missing required field: deck" }
  }

  const slidesRaw = deckRaw["slides"]
  if (!Array.isArray(slidesRaw) || slidesRaw.length === 0) {
    return { _tag: "Error", message: "deck.slides must be a non-empty array." }
  }

  const slides: DeckSlide[] = []
  const seen = new Set<string>()
  for (const [i, s] of slidesRaw.entries()) {
    if (!isRecord(s)) {
      return { _tag: "Error", message: `deck.slides[${i}] must be an object.` }
    }
    const id = expectString(s, "id")
    if (!id) {
      return { _tag: "Error", message: `deck.slides[${i}] missing required field: id` }
    }
    if (seen.has(id)) {
      return { _tag: "Error", message: `Duplicate slide id: ${id}` }
    }
    seen.add(id)
    slides.push(s as any)
  }

  const doc: DeckDocument = {
    ...(json as any),
    dsl: "effuse.slide-deck",
    version,
    deck: {
      ...(deckRaw as any),
      slides,
    },
  }

  // Validate layout references (strict).
  const layouts = (isRecord(json["layouts"]) ? (json["layouts"] as any) : null) as Record<string, unknown> | null
  const defaultLayout =
    typeof deckRaw["defaultSlideLayout"] === "string" ? (deckRaw["defaultSlideLayout"] as string) : null

  const hasLayout = (name: string) => (layouts ? Object.prototype.hasOwnProperty.call(layouts, name) : false)
  if (defaultLayout && !hasLayout(defaultLayout)) {
    return { _tag: "Error", message: `deck.defaultSlideLayout references missing layout: ${defaultLayout}` }
  }
  for (const slide of slides) {
    if (slide.layout && !hasLayout(slide.layout)) {
      return { _tag: "Error", message: `slide.layout references missing layout: ${slide.layout} (slideId=${slide.id})` }
    }
  }

  return { _tag: "Ok", doc }
}

export const isTokenRef = (value: unknown): value is TokenRef =>
  isRecord(value) && typeof (value as any).$token === "string"

export const isRefNode = (value: unknown): value is RefNode =>
  isRecord(value) && typeof (value as any).$ref === "string"

export const resolveToken = (theme: DeckTheme | undefined, tokenKey: string): unknown => {
  const tokens = theme?.tokens
  if (tokens && Object.prototype.hasOwnProperty.call(tokens, tokenKey)) {
    return (tokens as any)[tokenKey]
  }
  return undefined
}

export const resolveTokenValue = (theme: DeckTheme | undefined, value: unknown): unknown => {
  if (isTokenRef(value)) {
    const resolved = resolveToken(theme, value.$token)
    return resolved !== undefined ? resolved : value
  }
  return value
}

export const normalizeChildToNode = (child: DeckNodeChild): DeckNode | string => {
  if (typeof child === "string") return child
  if (isRefNode(child)) {
    // `$ref` resolution is applied later; keep as text placeholder if unresolved.
    return `[ref:${child.$ref}]`
  }
  return child
}

export const renderUnknownNode = (type: string): TemplateResult =>
  html`<div class="text-xs text-red-400">[unknown node: ${type}]</div>`
